/**
 * 语音输入 Hook — 管理麦克风录音和语音转文字
 * Speech input hook — manages microphone recording and speech-to-text
 *
 * Adapted from AionUi useSpeechInput pattern, simplified for Tauri/Web.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';

export type SpeechInputAvailability = 'record' | 'file' | 'unsupported';
export type SpeechInputStatus = 'idle' | 'recording' | 'transcribing' | 'error';
export type SpeechInputErrorCode =
  | 'aborted'
  | 'audio-capture'
  | 'empty-transcript'
  | 'file-too-large'
  | 'network'
  | 'not-configured'
  | 'permission-denied'
  | 'recording-unsupported'
  | 'transcription-failed'
  | 'unknown';

const RECORDING_MIME_TYPES = ['audio/webm;codecs=opus', 'audio/webm', 'audio/mp4', 'audio/ogg;codecs=opus'];
const MAX_AUDIO_FILE_SIZE = 25 * 1024 * 1024; // 25MB

export function getSpeechInputAvailability(): SpeechInputAvailability {
  if (typeof window === 'undefined') return 'unsupported';

  const hasMediaDevices = typeof navigator !== 'undefined' && Boolean(navigator.mediaDevices?.getUserMedia);
  const hasMediaRecorder = typeof MediaRecorder !== 'undefined';
  const isSecureContext = window.isSecureContext;
  const isLocalhost = ['localhost', '127.0.0.1', '::1'].includes(window.location.hostname);

  if (hasMediaDevices && hasMediaRecorder && (isSecureContext || isLocalhost)) {
    return 'record';
  }

  if (typeof document !== 'undefined') {
    return 'file';
  }

  return 'unsupported';
}

export function pickRecordingMimeType(): string {
  if (typeof MediaRecorder === 'undefined' || typeof MediaRecorder.isTypeSupported !== 'function') {
    return '';
  }
  for (const mimeType of RECORDING_MIME_TYPES) {
    if (MediaRecorder.isTypeSupported(mimeType)) {
      return mimeType;
    }
  }
  return '';
}

interface UseSpeechInputOptions {
  locale?: string;
  onTranscript: (transcript: string) => void;
}

/**
 * 语音输入 Hook，支持麦克风录音和文件上传两种模式。
 * Speech input hook supporting both microphone recording and file upload modes.
 *
 * @example
 * ```tsx
 * const { status, startRecording, stopRecording, error } = useSpeechInput({
 *   onTranscript: (text) => setInput(text),
 * });
 * ```
 */
export function useSpeechInput(options: UseSpeechInputOptions) {
  const { onTranscript } = options;
  const [status, setStatus] = useState<SpeechInputStatus>('idle');
  const [errorCode, setErrorCode] = useState<SpeechInputErrorCode | null>(null);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [audioLevel, setAudioLevel] = useState(0);

  const mediaRecorderRef = useRef<MediaRecorder | null>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const chunksRef = useRef<Blob[]>([]);
  const analyserRef = useRef<AnalyserNode | null>(null);
  const audioContextRef = useRef<AudioContext | null>(null);
  const animFrameRef = useRef<number | null>(null);
  const onTranscriptRef = useRef(onTranscript);
  onTranscriptRef.current = onTranscript;

  const availability = useMemo(() => getSpeechInputAvailability(), []);

  const cleanup = useCallback(() => {
    if (animFrameRef.current) {
      cancelAnimationFrame(animFrameRef.current);
      animFrameRef.current = null;
    }
    if (mediaRecorderRef.current && mediaRecorderRef.current.state !== 'inactive') {
      try {
        mediaRecorderRef.current.stop();
      } catch {
        // ignore
      }
    }
    mediaRecorderRef.current = null;
    if (streamRef.current) {
      streamRef.current.getTracks().forEach((track) => track.stop());
      streamRef.current = null;
    }
    if (audioContextRef.current) {
      void audioContextRef.current.close().catch(() => {});
      audioContextRef.current = null;
    }
    chunksRef.current = [];
    analyserRef.current = null;
    setAudioLevel(0);
  }, []);

  const processAudioBlob = useCallback(
    async (blob: Blob) => {
      setStatus('transcribing');
      try {
        // Convert blob to Uint8Array for Tauri backend.
        const arrayBuffer = await blob.arrayBuffer();
        const audioData = new Uint8Array(arrayBuffer);

        // Derive the audio format from the MIME type for the Whisper API.
        const mimeType = blob.type;
        const format = mimeType.includes('webm') ? 'webm'
          : mimeType.includes('mp4') ? 'mp4'
          : mimeType.includes('ogg') ? 'ogg'
          : 'wav';

        // Dynamically import to avoid errors in non-Tauri environments.
        const { transcribeAudio } = await import('../../lib/tauri');
        const transcript = await transcribeAudio(audioData, format);

        if (transcript && transcript.trim()) {
          onTranscriptRef.current(transcript);
          setStatus('idle');
          setErrorCode(null);
          setErrorMessage(null);
        } else {
          setStatus('error');
          setErrorCode('empty-transcript');
          setErrorMessage('No transcript produced');
        }
      } catch (error) {
        setStatus('error');
        setErrorCode('transcription-failed');
        setErrorMessage(error instanceof Error ? error.message : 'Transcription failed');
      }
    },
    [],
  );

  const startRecording = useCallback(async () => {
    if (availability !== 'record') {
      setErrorCode('recording-unsupported');
      setErrorMessage('Recording is not supported in this environment');
      return;
    }

    try {
      cleanup();
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      streamRef.current = stream;

      const mimeType = pickRecordingMimeType();
      const recorder = mimeType ? new MediaRecorder(stream, { mimeType }) : new MediaRecorder(stream);
      mediaRecorderRef.current = recorder;
      chunksRef.current = [];

      recorder.ondataavailable = (event) => {
        if (event.data.size > 0) {
          chunksRef.current.push(event.data);
        }
      };

      recorder.onstop = () => {
        const blob = new Blob(chunksRef.current, { type: mimeType || 'audio/webm' });
        void processAudioBlob(blob);
      };

      recorder.onerror = () => {
        setStatus('error');
        setErrorCode('audio-capture');
        setErrorMessage('Audio capture error');
        cleanup();
      };

      // 设置音频分析器
      const audioContext = new AudioContext();
      audioContextRef.current = audioContext;
      const source = audioContext.createMediaStreamSource(stream);
      const analyser = audioContext.createAnalyser();
      analyser.fftSize = 256;
      source.connect(analyser);
      analyserRef.current = analyser;

      const dataArray = new Uint8Array(analyser.frequencyBinCount);
      const updateLevel = () => {
        analyser.getByteFrequencyData(dataArray);
        const avg = dataArray.reduce((sum, val) => sum + val, 0) / dataArray.length;
        setAudioLevel(avg / 255);
        animFrameRef.current = requestAnimationFrame(updateLevel);
      };
      animFrameRef.current = requestAnimationFrame(updateLevel);

      recorder.start(100);
      setStatus('recording');
      setErrorCode(null);
      setErrorMessage(null);
    } catch (error) {
      setStatus('error');
      if (error instanceof DOMException && error.name === 'NotAllowedError') {
        setErrorCode('permission-denied');
        setErrorMessage('Microphone permission denied');
      } else {
        setErrorCode('audio-capture');
        setErrorMessage(error instanceof Error ? error.message : 'Failed to start recording');
      }
      cleanup();
    }
  }, [availability, cleanup, processAudioBlob]);

  const stopRecording = useCallback(() => {
    if (mediaRecorderRef.current && mediaRecorderRef.current.state === 'recording') {
      mediaRecorderRef.current.stop();
    }
    if (animFrameRef.current) {
      cancelAnimationFrame(animFrameRef.current);
      animFrameRef.current = null;
    }
    if (streamRef.current) {
      streamRef.current.getTracks().forEach((track) => track.stop());
      streamRef.current = null;
    }
  }, []);

  const handleFileUpload = useCallback(
    async (file: File) => {
      if (file.size > MAX_AUDIO_FILE_SIZE) {
        setStatus('error');
        setErrorCode('file-too-large');
        setErrorMessage('Audio file is too large (max 25MB)');
        return;
      }
      const blob = new Blob([file], { type: file.type });
      await processAudioBlob(blob);
    },
    [processAudioBlob],
  );

  useEffect(() => {
    return () => {
      cleanup();
    };
  }, [cleanup]);

  return {
    availability,
    status,
    errorCode,
    errorMessage,
    audioLevel,
    startRecording,
    stopRecording,
    handleFileUpload,
  };
}

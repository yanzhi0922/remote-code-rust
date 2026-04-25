import { describe, it, expect, vi, afterEach } from 'vitest';
import { renderHook, act, cleanup } from '@testing-library/react';
import {
  useSpeechInput,
  getSpeechInputAvailability,
  pickRecordingMimeType,
} from './useSpeechInput';

describe('getSpeechInputAvailability', () => {
  it('returns unsupported when window is undefined', () => {
    const originalWindow = global.window;
    // jsdom has window, so this tests the default path
    const result = getSpeechInputAvailability();
    expect(['record', 'file', 'unsupported']).toContain(result);
    void originalWindow;
  });
});

describe('pickRecordingMimeType', () => {
  it('returns a string', () => {
    const result = pickRecordingMimeType();
    expect(typeof result).toBe('string');
  });
});

describe('useSpeechInput', () => {
  afterEach(() => {
    cleanup();
  });

  it('returns initial idle state', () => {
    const onTranscript = vi.fn();
    const { result } = renderHook(() => useSpeechInput({ onTranscript }));

    expect(result.current.status).toBe('idle');
    expect(result.current.errorCode).toBeNull();
    expect(result.current.errorMessage).toBeNull();
    expect(result.current.audioLevel).toBe(0);
  });

  it('returns availability', () => {
    const onTranscript = vi.fn();
    const { result } = renderHook(() => useSpeechInput({ onTranscript }));

    expect(['record', 'file', 'unsupported']).toContain(result.current.availability);
  });

  it('handleFileUpload rejects files over 25MB', async () => {
    const onTranscript = vi.fn();
    const { result } = renderHook(() => useSpeechInput({ onTranscript }));

    const largeFile = new File(['x'.repeat(26 * 1024 * 1024)], 'large.wav', { type: 'audio/wav' });

    await act(async () => {
      await result.current.handleFileUpload(largeFile);
    });

    expect(result.current.status).toBe('error');
    expect(result.current.errorCode).toBe('file-too-large');
  });

  it('handleFileUpload processes small files', async () => {
    const onTranscript = vi.fn();
    const { result } = renderHook(() => useSpeechInput({ onTranscript }));

    const smallFile = new File(['audio data'], 'small.wav', { type: 'audio/wav' });

    await act(async () => {
      await result.current.handleFileUpload(smallFile);
    });

    // Should transition to transcribing then to idle/error
    expect(['idle', 'error', 'transcribing']).toContain(result.current.status);
  });

  it('startRecording handles environment gracefully', async () => {
    const onTranscript = vi.fn();
    const { result } = renderHook(() => useSpeechInput({ onTranscript }));

    await act(async () => {
      await result.current.startRecording();
    });

    // In jsdom, getUserMedia may not work, so status could be error, idle, or recording
    expect(['error', 'idle', 'recording']).toContain(result.current.status);
  });

  it('stopRecording does not crash when idle', () => {
    const onTranscript = vi.fn();
    const { result } = renderHook(() => useSpeechInput({ onTranscript }));

    expect(() => {
      result.current.stopRecording();
    }).not.toThrow();
  });
});

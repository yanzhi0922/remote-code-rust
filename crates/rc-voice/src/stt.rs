//! Speech-to-Text (STT) trait and implementations.
//!
//! Provides the [`SpeechToText`] trait for speech recognition, a
//! [`MockStt`] implementation for testing, and a [`WhisperStt`]
//! implementation that calls the OpenAI Whisper API.

use std::sync::Mutex;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tracing;

use crate::types::{TranscriptResult, VoiceConfig, VoiceState};

// ---------------------------------------------------------------------------
// VoiceStreamConfig
// ---------------------------------------------------------------------------

/// Configuration for streaming audio input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceStreamConfig {
    /// Audio sample rate in Hz.
    pub sample_rate: u32,
    /// Number of audio channels.
    pub channels: u16,
    /// Bits per sample.
    pub bits_per_sample: u16,
    /// Whether to return interim (partial) results.
    pub interim_results: bool,
    /// Language hint for recognition.
    pub language: String,
}

impl Default for VoiceStreamConfig {
    fn default() -> Self {
        Self {
            sample_rate: 16000,
            channels: 1,
            bits_per_sample: 16,
            interim_results: true,
            language: "en-US".to_string(),
        }
    }
}

impl VoiceStreamConfig {
    /// Create a new stream config.
    #[must_use]
    pub fn new(sample_rate: u32) -> Self {
        Self {
            sample_rate,
            ..Self::default()
        }
    }

    /// Set the language.
    #[must_use]
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = language.into();
        self
    }

    /// Disable interim results.
    #[must_use]
    pub fn final_only(mut self) -> Self {
        self.interim_results = false;
        self
    }
}

// ---------------------------------------------------------------------------
// SpeechToText trait
// ---------------------------------------------------------------------------

/// Trait for speech-to-text implementations.
pub trait SpeechToText: Send + Sync {
    /// Start listening for speech input.
    fn start_listening(&mut self) -> Result<()>;

    /// Stop listening for speech input.
    fn stop_listening(&mut self) -> Result<()>;

    /// Get the latest transcript.
    fn get_transcript(&self) -> Result<Option<TranscriptResult>>;

    /// Get the current voice state.
    fn state(&self) -> VoiceState;

    /// Update the voice configuration.
    fn set_config(&mut self, config: VoiceConfig);
}

// ---------------------------------------------------------------------------
// MockStt
// ---------------------------------------------------------------------------

/// Mock STT implementation for testing.
#[derive(Debug)]
pub struct MockStt {
    state: VoiceState,
    config: VoiceConfig,
    transcript: Option<TranscriptResult>,
    /// History of state transitions.
    state_history: Vec<VoiceState>,
}

impl MockStt {
    /// Create a new mock STT.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: VoiceState::Idle,
            config: VoiceConfig::default(),
            transcript: None,
            state_history: Vec::new(),
        }
    }

    /// Inject a transcript result (for testing).
    pub fn inject_transcript(&mut self, result: TranscriptResult) {
        self.transcript = Some(result);
    }

    /// Get the state transition history.
    #[must_use]
    pub fn state_history(&self) -> &[VoiceState] {
        &self.state_history
    }
}

impl Default for MockStt {
    fn default() -> Self {
        Self::new()
    }
}

impl SpeechToText for MockStt {
    fn start_listening(&mut self) -> Result<()> {
        self.state_history.push(self.state);
        self.state = VoiceState::Listening;
        Ok(())
    }

    fn stop_listening(&mut self) -> Result<()> {
        self.state_history.push(self.state);
        self.state = VoiceState::Idle;
        Ok(())
    }

    fn get_transcript(&self) -> Result<Option<TranscriptResult>> {
        Ok(self.transcript.clone())
    }

    fn state(&self) -> VoiceState {
        self.state
    }

    fn set_config(&mut self, config: VoiceConfig) {
        self.config = config;
    }
}

// ---------------------------------------------------------------------------
// WhisperStt — OpenAI Whisper API implementation
// ---------------------------------------------------------------------------

/// STT backend using the OpenAI Whisper API.
///
/// This is a one-shot transcription backend (not streaming). Audio data is
/// sent to the Whisper API via `multipart/form-data` and the transcribed text
/// is returned.
pub struct WhisperStt {
    api_key: String,
    model: String,
    state: Mutex<VoiceState>,
    language: Option<String>,
}

impl WhisperStt {
    /// Create a new Whisper STT client with the given API key.
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            model: "whisper-1".to_string(),
            state: Mutex::new(VoiceState::Idle),
            language: None,
        }
    }

    /// Set the language hint for transcription (e.g. `"en"`, `"zh"`).
    #[must_use]
    pub fn with_language(mut self, lang: String) -> Self {
        self.language = Some(lang);
        self
    }

    /// Override the model name (default: `"whisper-1"`).
    #[must_use]
    pub fn with_model(mut self, model: String) -> Self {
        self.model = model;
        self
    }

    /// Transcribe audio data using the OpenAI Whisper API.
    ///
    /// `audio_data` — raw audio bytes.
    /// `format` — file extension / container format (e.g. `"webm"`, `"wav"`,
    ///   `"mp4"`, `"ogg"`).
    pub async fn transcribe(&self, audio_data: &[u8], format: &str) -> Result<TranscriptResult> {
        let client = reqwest::Client::new();

        // Build the file part with an appropriate MIME type.
        let mime = match format {
            "webm" => "audio/webm",
            "mp4" | "m4a" => "audio/mp4",
            "ogg" => "audio/ogg",
            "flac" => "audio/flac",
            _ => "audio/wav",
        };

        let file_part = reqwest::multipart::Part::bytes(audio_data.to_vec())
            .file_name(format!("audio.{format}"))
            .mime_str(mime)
            .unwrap_or_else(|_| {
                // Fallback: omit MIME type if the string is invalid.
                reqwest::multipart::Part::bytes(audio_data.to_vec())
                    .file_name(format!("audio.{format}"))
            });

        let mut form = reqwest::multipart::Form::new()
            .text("model", self.model.clone())
            .part("file", file_part);

        if let Some(ref lang) = self.language {
            form = form.text("language", lang.clone());
        }

        // Set state to Processing while the request is in flight.
        {
            let mut st = self.state.lock().expect("WhisperStt state lock");
            *st = VoiceState::Processing;
        }

        let response = client
            .post("https://api.openai.com/v1/audio/transcriptions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await;

        // Reset state to Idle.
        {
            let mut st = self.state.lock().expect("WhisperStt state lock");
            *st = VoiceState::Idle;
        }

        let response = response?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            tracing::error!("Whisper API error {status}: {body}");
            anyhow::bail!("Whisper API error {status}: {body}");
        }

        let result: serde_json::Value = response.json().await?;
        let text = result["text"].as_str().unwrap_or("").trim().to_string();

        Ok(TranscriptResult::final_result(text))
    }
}

impl SpeechToText for WhisperStt {
    fn start_listening(&mut self) -> Result<()> {
        let mut st = self.state.lock().expect("WhisperStt state lock");
        *st = VoiceState::Listening;
        Ok(())
    }

    fn stop_listening(&mut self) -> Result<()> {
        let mut st = self.state.lock().expect("WhisperStt state lock");
        *st = VoiceState::Idle;
        Ok(())
    }

    fn get_transcript(&self) -> Result<Option<TranscriptResult>> {
        // Whisper is one-shot, not streaming — use `transcribe()` instead.
        Ok(None)
    }

    fn state(&self) -> VoiceState {
        *self.state.lock().expect("WhisperStt state lock")
    }

    fn set_config(&mut self, _config: VoiceConfig) {
        // Configuration is handled through the builder methods.
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- VoiceStreamConfig ----------------------------------------------------

    #[test]
    fn stream_config_default() {
        let cfg = VoiceStreamConfig::default();
        assert_eq!(cfg.sample_rate, 16000);
        assert_eq!(cfg.channels, 1);
        assert_eq!(cfg.bits_per_sample, 16);
        assert!(cfg.interim_results);
    }

    #[test]
    fn stream_config_new() {
        let cfg = VoiceStreamConfig::new(48000);
        assert_eq!(cfg.sample_rate, 48000);
    }

    #[test]
    fn stream_config_with_language() {
        let cfg = VoiceStreamConfig::new(16000).with_language("zh-CN");
        assert_eq!(cfg.language, "zh-CN");
    }

    #[test]
    fn stream_config_final_only() {
        let cfg = VoiceStreamConfig::new(16000).final_only();
        assert!(!cfg.interim_results);
    }

    #[test]
    fn stream_config_serialization() {
        let cfg = VoiceStreamConfig::default();
        let json = serde_json::to_string(&cfg).expect("serialize");
        let back: VoiceStreamConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(cfg.sample_rate, back.sample_rate);
    }

    // -- MockStt --------------------------------------------------------------

    #[test]
    fn mock_stt_new() {
        let stt = MockStt::new();
        assert_eq!(stt.state(), VoiceState::Idle);
    }

    #[test]
    fn mock_stt_default() {
        let stt = MockStt::default();
        assert_eq!(stt.state(), VoiceState::Idle);
    }

    #[test]
    fn mock_stt_start_listening() {
        let mut stt = MockStt::new();
        stt.start_listening().expect("start");
        assert_eq!(stt.state(), VoiceState::Listening);
    }

    #[test]
    fn mock_stt_stop_listening() {
        let mut stt = MockStt::new();
        stt.start_listening().expect("start");
        stt.stop_listening().expect("stop");
        assert_eq!(stt.state(), VoiceState::Idle);
    }

    #[test]
    fn mock_stt_state_history() {
        let mut stt = MockStt::new();
        stt.start_listening().expect("start");
        stt.stop_listening().expect("stop");
        assert_eq!(
            stt.state_history(),
            &[VoiceState::Idle, VoiceState::Listening]
        );
    }

    #[test]
    fn mock_stt_get_transcript_empty() {
        let stt = MockStt::new();
        let result = stt.get_transcript().expect("get");
        assert!(result.is_none());
    }

    #[test]
    fn mock_stt_inject_transcript() {
        let mut stt = MockStt::new();
        stt.inject_transcript(TranscriptResult::final_result("hello"));
        let result = stt.get_transcript().expect("get");
        assert!(result.is_some());
        assert_eq!(result.expect("result").text, "hello");
    }

    #[test]
    fn mock_stt_set_config() {
        let mut stt = MockStt::new();
        let cfg = VoiceConfig::new("zh-CN").with_model("whisper");
        stt.set_config(cfg);
    }

    // -- SpeechToText trait (via MockStt) -------------------------------------

    #[test]
    fn trait_object() {
        let mut stt: Box<dyn SpeechToText> = Box::new(MockStt::new());
        stt.start_listening().expect("start");
        assert_eq!(stt.state(), VoiceState::Listening);
        stt.stop_listening().expect("stop");
        assert_eq!(stt.state(), VoiceState::Idle);
    }

    #[test]
    fn trait_set_config() {
        let mut stt: Box<dyn SpeechToText> = Box::new(MockStt::new());
        stt.set_config(VoiceConfig::new("ja-JP"));
    }

    #[test]
    fn trait_get_transcript() {
        let stt: Box<dyn SpeechToText> = Box::new(MockStt::new());
        let result = stt.get_transcript().expect("get");
        assert!(result.is_none());
    }
}

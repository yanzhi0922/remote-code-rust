//! Voice types: configuration, state, events, and transcript results.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// VoiceConfig
// ---------------------------------------------------------------------------

/// Configuration for voice services.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceConfig {
    /// Language code (e.g. "en-US", "zh-CN").
    pub language: String,
    /// Model name for the voice service.
    pub model: String,
    /// Audio sample rate in Hz.
    pub sample_rate: u32,
}

impl Default for VoiceConfig {
    fn default() -> Self {
        Self {
            language: "en-US".to_string(),
            model: "default".to_string(),
            sample_rate: 16000,
        }
    }
}

impl VoiceConfig {
    /// Create a new voice config.
    #[must_use]
    pub fn new(language: impl Into<String>) -> Self {
        Self {
            language: language.into(),
            ..Self::default()
        }
    }

    /// Set the model.
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Set the sample rate.
    #[must_use]
    pub fn with_sample_rate(mut self, rate: u32) -> Self {
        self.sample_rate = rate;
        self
    }
}

// ---------------------------------------------------------------------------
// VoiceState
// ---------------------------------------------------------------------------

/// Current state of the voice service.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum VoiceState {
    /// No voice activity.
    #[default]
    Idle,
    /// Microphone is active and listening.
    Listening,
    /// Audio is being processed (e.g. transcription).
    Processing,
    /// Audio is being played back (TTS).
    Speaking,
}

impl std::fmt::Display for VoiceState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Idle => write!(f, "idle"),
            Self::Listening => write!(f, "listening"),
            Self::Processing => write!(f, "processing"),
            Self::Speaking => write!(f, "speaking"),
        }
    }
}


// ---------------------------------------------------------------------------
// VoiceEvent
// ---------------------------------------------------------------------------

/// Events emitted by the voice service.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VoiceEvent {
    /// Speech input has started.
    SpeechStart,
    /// Speech input has ended.
    SpeechEnd,
    /// A transcript has been produced.
    Transcript {
        /// The transcribed text.
        text: String,
        /// Confidence score (0.0–1.0).
        confidence: f64,
        /// Whether this is a final transcript (vs. partial).
        is_final: bool,
    },
    /// An error occurred.
    Error {
        /// Error message.
        message: String,
    },
}

impl VoiceEvent {
    /// Create a speech-start event.
    #[must_use]
    pub fn speech_start() -> Self {
        Self::SpeechStart
    }

    /// Create a speech-end event.
    #[must_use]
    pub fn speech_end() -> Self {
        Self::SpeechEnd
    }

    /// Create a transcript event.
    #[must_use]
    pub fn transcript(text: impl Into<String>, confidence: f64, is_final: bool) -> Self {
        Self::Transcript {
            text: text.into(),
            confidence,
            is_final,
        }
    }

    /// Create an error event.
    #[must_use]
    pub fn error(message: impl Into<String>) -> Self {
        Self::Error {
            message: message.into(),
        }
    }

    /// Whether this is a transcript event.
    #[must_use]
    pub fn is_transcript(&self) -> bool {
        matches!(self, Self::Transcript { .. })
    }

    /// Whether this is an error event.
    #[must_use]
    pub fn is_error(&self) -> bool {
        matches!(self, Self::Error { .. })
    }
}

// ---------------------------------------------------------------------------
// TranscriptResult
// ---------------------------------------------------------------------------

/// Result of a speech-to-text transcription.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptResult {
    /// The transcribed text.
    pub text: String,
    /// Confidence score (0.0–1.0).
    pub confidence: f64,
    /// Whether this is a final result (vs. partial/interim).
    pub is_final: bool,
}

impl TranscriptResult {
    /// Create a new transcript result.
    #[must_use]
    pub fn new(text: impl Into<String>, confidence: f64, is_final: bool) -> Self {
        Self {
            text: text.into(),
            confidence,
            is_final,
        }
    }

    /// Create a final transcript with full confidence.
    #[must_use]
    pub fn final_result(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            confidence: 1.0,
            is_final: true,
        }
    }

    /// Create a partial (interim) transcript.
    #[must_use]
    pub fn partial(text: impl Into<String>, confidence: f64) -> Self {
        Self {
            text: text.into(),
            confidence,
            is_final: false,
        }
    }

    /// Whether this is a high-confidence result (>0.9).
    #[must_use]
    pub fn is_high_confidence(&self) -> bool {
        self.confidence > 0.9
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- VoiceConfig ----------------------------------------------------------

    #[test]
    fn voice_config_default() {
        let cfg = VoiceConfig::default();
        assert_eq!(cfg.language, "en-US");
        assert_eq!(cfg.model, "default");
        assert_eq!(cfg.sample_rate, 16000);
    }

    #[test]
    fn voice_config_new() {
        let cfg = VoiceConfig::new("zh-CN");
        assert_eq!(cfg.language, "zh-CN");
    }

    #[test]
    fn voice_config_builder() {
        let cfg = VoiceConfig::new("en-US")
            .with_model("whisper-large")
            .with_sample_rate(48000);
        assert_eq!(cfg.model, "whisper-large");
        assert_eq!(cfg.sample_rate, 48000);
    }

    #[test]
    fn voice_config_serialization() {
        let cfg = VoiceConfig::new("ja-JP");
        let json = serde_json::to_string(&cfg).expect("serialize");
        let back: VoiceConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(cfg.language, back.language);
    }

    // -- VoiceState -----------------------------------------------------------

    #[test]
    fn voice_state_default() {
        assert_eq!(VoiceState::default(), VoiceState::Idle);
    }

    #[test]
    fn voice_state_display() {
        assert_eq!(VoiceState::Idle.to_string(), "idle");
        assert_eq!(VoiceState::Listening.to_string(), "listening");
        assert_eq!(VoiceState::Processing.to_string(), "processing");
        assert_eq!(VoiceState::Speaking.to_string(), "speaking");
    }

    #[test]
    fn voice_state_serialization() {
        let state = VoiceState::Listening;
        let json = serde_json::to_string(&state).expect("serialize");
        let back: VoiceState = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(state, back);
    }

    // -- VoiceEvent -----------------------------------------------------------

    #[test]
    fn voice_event_speech_start() {
        let e = VoiceEvent::speech_start();
        assert!(!e.is_transcript());
        assert!(!e.is_error());
    }

    #[test]
    fn voice_event_speech_end() {
        let e = VoiceEvent::speech_end();
        assert!(!e.is_transcript());
    }

    #[test]
    fn voice_event_transcript() {
        let e = VoiceEvent::transcript("hello world", 0.95, true);
        assert!(e.is_transcript());
        assert!(!e.is_error());
    }

    #[test]
    fn voice_event_error() {
        let e = VoiceEvent::error("mic not found");
        assert!(e.is_error());
        assert!(!e.is_transcript());
    }

    #[test]
    fn voice_event_serialization_roundtrip() {
        let events = vec![
            VoiceEvent::speech_start(),
            VoiceEvent::speech_end(),
            VoiceEvent::transcript("test", 0.8, false),
            VoiceEvent::error("err"),
        ];
        for event in &events {
            let json = serde_json::to_string(event).expect("serialize");
            let back: VoiceEvent = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(json, serde_json::to_string(&back).expect("serialize"));
        }
    }

    // -- TranscriptResult -----------------------------------------------------

    #[test]
    fn transcript_result_new() {
        let r = TranscriptResult::new("hello", 0.9, true);
        assert_eq!(r.text, "hello");
        assert!((r.confidence - 0.9).abs() < f64::EPSILON);
        assert!(r.is_final);
    }

    #[test]
    fn transcript_result_final() {
        let r = TranscriptResult::final_result("world");
        assert!(r.is_final);
        assert!(r.is_high_confidence());
    }

    #[test]
    fn transcript_result_partial() {
        let r = TranscriptResult::partial("hel", 0.5);
        assert!(!r.is_final);
        assert!(!r.is_high_confidence());
    }

    #[test]
    fn transcript_result_high_confidence() {
        let r = TranscriptResult::new("test", 0.95, true);
        assert!(r.is_high_confidence());
    }

    #[test]
    fn transcript_result_low_confidence() {
        let r = TranscriptResult::new("test", 0.5, true);
        assert!(!r.is_high_confidence());
    }

    #[test]
    fn transcript_result_serialization() {
        let r = TranscriptResult::final_result("hello");
        let json = serde_json::to_string(&r).expect("serialize");
        let back: TranscriptResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(r.text, back.text);
        assert_eq!(r.is_final, back.is_final);
    }
}

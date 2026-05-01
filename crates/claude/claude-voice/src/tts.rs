//! Text-to-Speech (TTS) trait and mock implementation.
//!
//! Provides the [`TextToSpeech`] trait for speech synthesis and a
//! [`MockTts`] implementation for testing.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::types::VoiceState;

// ---------------------------------------------------------------------------
// VoiceProfile
// ---------------------------------------------------------------------------

/// Configuration for a specific TTS voice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoiceProfile {
    /// Voice identifier (e.g. "alloy", "nova", "echo").
    pub voice_id: String,
    /// Speaking rate (1.0 = normal).
    pub speed: f64,
    /// Pitch adjustment in semitones (0 = normal).
    pub pitch: f64,
    /// Volume level (0.0–1.0).
    pub volume: f64,
}

impl Default for VoiceProfile {
    fn default() -> Self {
        Self {
            voice_id: "default".to_string(),
            speed: 1.0,
            pitch: 0.0,
            volume: 1.0,
        }
    }
}

impl VoiceProfile {
    /// Create a new voice profile.
    #[must_use]
    pub fn new(voice_id: impl Into<String>) -> Self {
        Self {
            voice_id: voice_id.into(),
            ..Self::default()
        }
    }

    /// Set the speed.
    #[must_use]
    pub fn with_speed(mut self, speed: f64) -> Self {
        self.speed = speed;
        self
    }

    /// Set the pitch.
    #[must_use]
    pub fn with_pitch(mut self, pitch: f64) -> Self {
        self.pitch = pitch;
        self
    }

    /// Set the volume.
    #[must_use]
    pub fn with_volume(mut self, volume: f64) -> Self {
        self.volume = volume;
        self
    }

    /// Clamp all values to valid ranges.
    #[must_use]
    pub fn normalized(&self) -> Self {
        Self {
            voice_id: self.voice_id.clone(),
            speed: self.speed.clamp(0.25, 4.0),
            pitch: self.pitch.clamp(-12.0, 12.0),
            volume: self.volume.clamp(0.0, 1.0),
        }
    }
}

// ---------------------------------------------------------------------------
// TextToSpeech trait
// ---------------------------------------------------------------------------

/// Trait for text-to-speech implementations.
pub trait TextToSpeech: Send + Sync {
    /// Speak the given text.
    fn speak(&mut self, text: &str) -> Result<()>;

    /// Stop current speech.
    fn stop(&mut self) -> Result<()>;

    /// Set the voice profile.
    fn set_voice(&mut self, profile: VoiceProfile);

    /// Get the current state.
    fn state(&self) -> VoiceState;
}

// ---------------------------------------------------------------------------
// MockTts
// ---------------------------------------------------------------------------

/// Mock TTS implementation for testing.
#[derive(Debug)]
pub struct MockTts {
    state: VoiceState,
    profile: VoiceProfile,
    /// History of spoken texts.
    spoken: Vec<String>,
}

impl MockTts {
    /// Create a new mock TTS.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: VoiceState::Idle,
            profile: VoiceProfile::default(),
            spoken: Vec::new(),
        }
    }

    /// Get the history of spoken texts.
    #[must_use]
    pub fn spoken_history(&self) -> &[String] {
        &self.spoken
    }

    /// Get the current voice profile.
    #[must_use]
    pub fn profile(&self) -> &VoiceProfile {
        &self.profile
    }
}

impl Default for MockTts {
    fn default() -> Self {
        Self::new()
    }
}

impl TextToSpeech for MockTts {
    fn speak(&mut self, text: &str) -> Result<()> {
        self.spoken.push(text.to_string());
        self.state = VoiceState::Speaking;
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.state = VoiceState::Idle;
        Ok(())
    }

    fn set_voice(&mut self, profile: VoiceProfile) {
        self.profile = profile;
    }

    fn state(&self) -> VoiceState {
        self.state
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- VoiceProfile ---------------------------------------------------------

    #[test]
    fn voice_profile_default() {
        let p = VoiceProfile::default();
        assert_eq!(p.voice_id, "default");
        assert!((p.speed - 1.0).abs() < f64::EPSILON);
        assert!((p.pitch).abs() < f64::EPSILON);
        assert!((p.volume - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn voice_profile_new() {
        let p = VoiceProfile::new("alloy");
        assert_eq!(p.voice_id, "alloy");
    }

    #[test]
    fn voice_profile_builder() {
        let p = VoiceProfile::new("nova")
            .with_speed(1.5)
            .with_pitch(2.0)
            .with_volume(0.8);
        assert!((p.speed - 1.5).abs() < f64::EPSILON);
        assert!((p.pitch - 2.0).abs() < f64::EPSILON);
        assert!((p.volume - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn voice_profile_normalized_clamps() {
        let p = VoiceProfile::new("test")
            .with_speed(10.0)
            .with_pitch(-20.0)
            .with_volume(2.0);
        let n = p.normalized();
        assert!((n.speed - 4.0).abs() < f64::EPSILON);
        assert!((n.pitch - (-12.0)).abs() < f64::EPSILON);
        assert!((n.volume - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn voice_profile_normalized_keeps_valid() {
        let p = VoiceProfile::new("test")
            .with_speed(1.0)
            .with_pitch(0.0)
            .with_volume(0.5);
        let n = p.normalized();
        assert!((n.speed - 1.0).abs() < f64::EPSILON);
        assert!((n.pitch).abs() < f64::EPSILON);
        assert!((n.volume - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn voice_profile_serialization() {
        let p = VoiceProfile::new("echo").with_speed(0.5);
        let json = serde_json::to_string(&p).expect("serialize");
        let back: VoiceProfile = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(p.voice_id, back.voice_id);
    }

    // -- MockTts --------------------------------------------------------------

    #[test]
    fn mock_tts_new() {
        let tts = MockTts::new();
        assert_eq!(tts.state(), VoiceState::Idle);
        assert!(tts.spoken_history().is_empty());
    }

    #[test]
    fn mock_tts_default() {
        let tts = MockTts::default();
        assert_eq!(tts.state(), VoiceState::Idle);
    }

    #[test]
    fn mock_tts_speak() {
        let mut tts = MockTts::new();
        tts.speak("hello").expect("speak");
        assert_eq!(tts.state(), VoiceState::Speaking);
        assert_eq!(tts.spoken_history(), &["hello"]);
    }

    #[test]
    fn mock_tts_speak_multiple() {
        let mut tts = MockTts::new();
        tts.speak("hello").expect("speak");
        tts.speak("world").expect("speak");
        assert_eq!(tts.spoken_history(), &["hello", "world"]);
    }

    #[test]
    fn mock_tts_stop() {
        let mut tts = MockTts::new();
        tts.speak("hello").expect("speak");
        tts.stop().expect("stop");
        assert_eq!(tts.state(), VoiceState::Idle);
    }

    #[test]
    fn mock_tts_set_voice() {
        let mut tts = MockTts::new();
        let profile = VoiceProfile::new("nova").with_speed(1.2);
        tts.set_voice(profile);
        assert_eq!(tts.profile().voice_id, "nova");
    }

    // -- TextToSpeech trait (via MockTts) -------------------------------------

    #[test]
    fn trait_object_speak() {
        let mut tts: Box<dyn TextToSpeech> = Box::new(MockTts::new());
        tts.speak("test").expect("speak");
        assert_eq!(tts.state(), VoiceState::Speaking);
    }

    #[test]
    fn trait_object_stop() {
        let mut tts: Box<dyn TextToSpeech> = Box::new(MockTts::new());
        tts.speak("test").expect("speak");
        tts.stop().expect("stop");
        assert_eq!(tts.state(), VoiceState::Idle);
    }

    #[test]
    fn trait_object_set_voice() {
        let mut tts: Box<dyn TextToSpeech> = Box::new(MockTts::new());
        tts.set_voice(VoiceProfile::new("alloy"));
    }
}

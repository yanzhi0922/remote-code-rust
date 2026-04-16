//! `rc-voice` — Voice services for speech-to-text and text-to-speech.
//!
//! This crate provides a skeleton for voice interaction in remote-code,
//! including speech recognition (STT) and speech synthesis (TTS) traits
//! with mock implementations for testing.
//!
//! # Overview
//!
//! - **[`types`]** — Voice configuration, state, and event types
//! - **[`stt`]** — Speech-to-text trait and mock implementation
//! - **[`tts`]** — Text-to-speech trait and mock implementation
//!
//! # Example
//!
//! ```rust,ignore
//! use rc_voice::stt::MockStt;
//! use rc_voice::types::VoiceConfig;
//!
//! let mut stt = MockStt::new();
//! stt.start_listening().expect("start");
//! ```

pub mod stt;
pub mod tts;
pub mod types;

pub use types::{VoiceConfig, VoiceEvent, VoiceState};

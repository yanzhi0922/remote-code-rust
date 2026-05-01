//! # Roo Provider: Poe
//!
//! Poe provider for Roo Code Rust.
//! Uses the OpenAI-compatible chat completions API via Poe platform.
//!
//! Features:
//! - OpenAI-compatible API format
//! - Access to GPT-4o and Claude 3.5 Sonnet via subscription
//! - Reasoning budget/effort support
//! - Image support

mod handler;
mod models;
mod types;

pub use handler::PoeHandler;
pub use models::{default_model_id, models};
pub use types::PoeConfig;

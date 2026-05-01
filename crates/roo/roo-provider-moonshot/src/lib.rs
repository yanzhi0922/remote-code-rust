//! # Roo Provider: Moonshot / Kimi AI
//!
//! Moonshot AI (Kimi) provider for Roo Code Rust.
//! Uses the OpenAI-compatible chat completions API.

mod handler;
mod models;
mod types;

pub use handler::MoonshotHandler;
pub use models::{default_model_id, models};
pub use types::MoonshotConfig;

//! # Roo Provider: DeepSeek
//!
//! DeepSeek API provider for Roo Code Rust.
//! Uses the OpenAI-compatible chat completions API with extended thinking support.

mod handler;
mod models;
mod types;

pub use handler::DeepSeekHandler;
pub use models::{default_model_id, models};
pub use types::DeepSeekConfig;

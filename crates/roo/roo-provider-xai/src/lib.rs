//! # Roo Provider: xAI
//!
//! xAI/Grok API provider for Roo Code Rust.
//! Uses the OpenAI-compatible chat completions API.

mod handler;
mod models;
mod types;

pub use handler::XaiHandler;
pub use models::{default_model_id, models};
pub use types::XaiConfig;

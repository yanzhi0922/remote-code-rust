//! # Roo Provider: OpenAI
//!
//! OpenAI API provider for Roo Code Rust.
//! Uses the Chat Completions API with SSE streaming.
//! Supports function calling, tool_choice, and reasoning_effort.

mod handler;
mod models;
mod types;

pub use handler::OpenAiHandler;
pub use models::{default_model_id, models};
pub use types::OpenAiConfig;

//! # Roo Provider: Qwen / 通义千问
//!
//! Qwen Code provider for Roo Code Rust.
//! Uses the OpenAI-compatible chat completions API via DashScope.

mod handler;
mod models;
mod types;

pub use handler::QwenHandler;
pub use models::{default_model_id, models};
pub use types::QwenConfig;

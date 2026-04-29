//! # Roo Provider: LiteLLM
//!
//! LiteLLM proxy provider for Roo Code Rust.
//! Uses the OpenAI-compatible chat completions API via LiteLLM proxy server.
//!
//! Features:
//! - OpenAI-compatible API format
//! - Prompt caching support
//! - GPT-5 and Gemini model detection
//! - Dynamic model fetching from LiteLLM server

mod handler;
mod models;
mod types;

pub use handler::LiteLlmHandler;
pub use models::{default_model_id, models};
pub use types::LiteLlmConfig;

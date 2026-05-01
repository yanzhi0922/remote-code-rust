//! # Roo Provider: Ollama
//!
//! Ollama local API provider for Roo Code Rust.
//! Uses the OpenAI-compatible chat completions API provided by Ollama.

mod handler;
mod models;
mod types;

pub use handler::OllamaHandler;
pub use models::{default_model_id, models};
pub use types::OllamaConfig;

//! # Roo Provider: ZAI (Zhipu/GLM)
//!
//! ZAI provider for Roo Code Rust.
//! Uses the OpenAI-compatible chat completions API.

mod handler;
mod models;
mod types;

pub use handler::ZaiHandler;
pub use models::{default_model_id, models};
pub use types::ZaiConfig;

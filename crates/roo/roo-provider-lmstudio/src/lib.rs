//! # Roo Provider: LM Studio
//!
//! LM Studio local API provider for Roo Code Rust.
//! Uses the OpenAI-compatible chat completions API provided by LM Studio.
//! Supports `<think/>` tag processing for reasoning content classification.

mod handler;
mod models;
mod types;

pub use handler::LmStudioHandler;
pub use models::{default_model_id, default_model_info};
pub use types::LmStudioConfig;

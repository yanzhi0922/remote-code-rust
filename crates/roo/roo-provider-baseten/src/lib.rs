//! # Roo Provider: Baseten
//!
//! Baseten provider for Roo Code Rust.
//! Uses the OpenAI-compatible chat completions API.

mod handler;
mod models;
mod types;

pub use handler::BasetenHandler;
pub use models::{default_model_id, models};
pub use types::BasetenConfig;

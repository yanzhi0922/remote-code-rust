//! # Roo Provider: Roo Code Cloud
//!
//! Roo Code Cloud provider for Roo Code Rust.
//! Uses the OpenAI-compatible chat completions API via Roo's infrastructure.
//!
//! Features:
//! - OpenAI-compatible API format
//! - Session token authentication
//! - Dynamic model loading
//! - Reasoning details support
//! - Image generation support
//! - Prompt caching support

mod handler;
mod models;
mod types;

pub use handler::RooHandler;
pub use models::{default_model_id, models};
pub use types::RooConfig;

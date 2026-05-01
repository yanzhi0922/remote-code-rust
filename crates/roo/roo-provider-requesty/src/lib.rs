//! # Roo Provider: Requesty
//!
//! Requesty router provider for Roo Code Rust.
//! Uses the OpenAI-compatible chat completions API via Requesty.
//!
//! Features:
//! - OpenAI-compatible API format
//! - Trace ID tracking for observability
//! - Mode identification
//! - Custom usage metrics with caching token support

mod handler;
mod models;
mod types;

pub use handler::RequestyHandler;
pub use models::{default_model_id, models};
pub use types::RequestyConfig;

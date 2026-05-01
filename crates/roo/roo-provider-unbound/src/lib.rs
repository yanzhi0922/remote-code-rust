//! # Roo Provider: Unbound
//!
//! Unbound provider for Roo Code Rust.
//! Uses the OpenAI-compatible chat completions API via Unbound.
//!
//! Features:
//! - OpenAI-compatible API format
//! - Custom metadata headers (X-Unbound-Metadata)
//! - Cache token tracking (cache_creation_input_tokens, cache_read_input_tokens)

mod handler;
mod models;
mod types;

pub use handler::UnboundHandler;
pub use models::{default_model_id, models};
pub use types::UnboundConfig;

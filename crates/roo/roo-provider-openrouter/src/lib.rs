//! # Roo Provider: OpenRouter
//!
//! OpenRouter API provider for Roo Code Rust.
//! Uses the OpenAI-compatible chat completions API via OpenRouter's gateway.

mod endpoint_cache;
mod handler;
mod models;
mod types;

pub use endpoint_cache::ModelEndpointCache;
pub use handler::OpenRouterHandler;
pub use models::{default_model_id, models};
pub use types::OpenRouterConfig;

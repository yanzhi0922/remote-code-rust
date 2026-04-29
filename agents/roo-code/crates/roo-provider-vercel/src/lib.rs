//! # Roo Provider: Vercel AI Gateway
//!
//! Vercel AI Gateway provider for Roo Code Rust.
//! Uses the OpenAI-compatible chat completions API via Vercel AI Gateway.
//!
//! Features:
//! - OpenAI-compatible API format
//! - Default temperature of 0.5
//! - Prompt caching support
//! - Dynamic model fetching from Vercel AI Gateway
//! - Access to models from Anthropic, OpenAI, Google, etc.

mod handler;
mod models;
mod types;

pub use handler::VercelHandler;
pub use models::{default_model_id, models};
pub use types::VercelConfig;

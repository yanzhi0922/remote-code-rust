//! # Roo Provider: MiniMax
//!
//! MiniMax AI provider for Roo Code Rust.
//! Uses the Anthropic Messages API protocol via MiniMax's endpoint.

mod handler;
mod models;
mod types;

pub use handler::MiniMaxHandler;
pub use models::{default_model_id, models};
pub use types::MiniMaxConfig;

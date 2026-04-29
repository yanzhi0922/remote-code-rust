//! # Roo Provider: Anthropic
//!
//! Anthropic API provider for Roo Code Rust.
//! Uses the native Anthropic Messages API with SSE streaming.
//! Supports extended thinking, prompt caching, and tool use.
//!
//! Also includes the Anthropic Vertex AI handler, which uses the same
//! Anthropic Messages API format through Vertex AI's `streamRawPredict`
//! endpoint with OAuth2 bearer token authentication.

mod handler;
mod models;
mod types;

pub use handler::AnthropicHandler;
pub use handler::AnthropicVertexHandler;
pub use models::{default_model_id, models};
pub use types::AnthropicConfig;
pub use types::AnthropicVertexConfig;
pub use types::anthropic_vertex_models;
pub use types::anthropic_vertex_default_model_id;

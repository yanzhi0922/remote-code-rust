//! # Roo Provider: Google Gemini & Vertex AI
//!
//! Google Gemini and Vertex AI API provider for Roo Code Rust.
//! Uses the Gemini generateContent API with SSE streaming.
//! Supports grounding, safety settings, and thinking mode.
//!
//! ## Modules
//!
//! - [`GoogleHandler`] — Standard Google Gemini API handler
//! - [`VertexHandler`] — Vertex AI handler (same Gemini format, different auth/URL)

mod handler;
mod models;
mod types;

pub use handler::{GoogleHandler, VertexHandler};
pub use models::{
    default_model_id, models, vertex_default_model_id, vertex_models,
    VERTEX_DEFAULT_MODEL_ID,
};
pub use types::{GoogleConfig, VertexConfig};

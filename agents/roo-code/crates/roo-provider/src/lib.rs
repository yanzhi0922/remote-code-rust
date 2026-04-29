//! # Roo Provider
//!
//! API provider abstraction layer for Roo Code Rust.
//!
//! This crate defines the core `Provider` trait and provides:
//! - `BaseProvider` — common functionality for all providers
//! - `OpenAiCompatibleProvider` — base for OpenAI-compatible APIs (SSE streaming)
//! - Transform layer for converting between Anthropic, OpenAI, and Gemini formats
//! - `cost` — API cost calculation utilities
//! - `metrics` — API request metrics aggregation
//! - `fetcher` — Model fetching and caching
//!
//! Individual provider implementations live in their own crates
//! (e.g., `roo-provider-anthropic`, `roo-provider-openai`).

pub mod base_provider;
pub mod cost;
pub mod error;
pub mod fetcher;
pub mod handler;
pub mod metrics;
pub mod openai_compatible;
pub mod single_completion;
pub mod transform;
pub mod vertex_auth;

// Re-export key types
pub use error::{ProviderError, Result};
pub use handler::{ApiStream, CreateMessageMetadata, Provider, build_api_handler, register_provider, ProviderFactoryFn};
pub use base_provider::{
    BaseProvider,
    convert_tools_for_openai,
    convert_tool_schema_for_openai,
};
pub use openai_compatible::{
    OpenAiCompatibleConfig,
    OpenAiCompatibleProvider,
    process_usage_metrics,
};

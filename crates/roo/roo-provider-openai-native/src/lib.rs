//! # Roo Provider: OpenAI Native + Codex
//!
//! OpenAI Native and Codex API providers for Roo Code Rust.
//!
//! Both providers use the **OpenAI Responses API** (`POST /v1/responses`)
//! and share the same streaming/event-processing logic.
//!
//! ## Providers
//!
//! - **OpenAI Native** ([`OpenAiNativeHandler`]) — standard API key auth,
//!   routed to `https://api.openai.com/v1/responses`
//! - **OpenAI Codex** ([`OpenAiCodexHandler`]) — OAuth Bearer token auth,
//!   routed to `https://chatgpt.com/backend-api/codex/responses`
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────┐    ┌──────────────────┐
//! │ OpenAiNative    │    │ OpenAiCodex      │
//! │ Handler         │    │ Handler          │
//! └────────┬────────┘    └────────┬─────────┘
//!          │                      │
//!          └──────┬───────────────┘
//!                 │
//!         ┌───────▼────────┐
//!         │ responses_api  │  ← shared logic
//!         │ (build, parse) │
//!         └────────────────┘
//! ```

mod codex_handler;
mod handler;
pub mod models;
pub mod responses_api;
pub mod types;

pub use codex_handler::OpenAiCodexHandler;
pub use handler::OpenAiNativeHandler;
pub use types::{OpenAiCodexConfig, OpenAiNativeConfig, ResponsesApiRequestBody};

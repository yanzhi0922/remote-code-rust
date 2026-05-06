//! # Roo Provider: FakeAI
//!
//! Test-double provider for Roo Code Rust.
//!
//! `FakeAiHandler` implements the [`Provider`] trait but never contacts a real
//! API.  Instead it returns canned responses from its configuration, making it
//! ideal for unit tests, integration tests, and CI pipelines.
//!
//! # Response patterns
//!
//! - **Text** — return a fixed string.
//! - **Streaming** — return text split into chunks with optional inter-chunk
//!   delay.
//! - **ToolCall** — return a synthetic tool call.
//! - **Error** — inject a streaming error.
//! - **Sequence** — cycle through a list of patterns on successive calls.

mod handler;
mod types;

pub use handler::FakeAiHandler;
pub use types::{FakeAiConfig, ResponsePattern};
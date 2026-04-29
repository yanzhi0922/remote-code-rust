//! # Roo Provider: VS Code Language Model API
//!
//! VS Code Language Model API provider for Roo Code Rust.
//! Uses the VS Code Language Model API to interact with models
//! registered in VS Code (e.g., GitHub Copilot models).
//!
//! **Note**: This provider requires the VS Code extension host runtime
//! for actual model inference. In standalone mode, it provides structural
//! types and returns runtime errors for model calls.
//!
//! Features:
//! - VS Code Language Model API integration
//! - Dynamic model discovery via `vscode.lm.selectChatModels()`
//! - Tool call support
//! - Token counting via VS Code API
//! - No API key required (uses VS Code authentication)

mod handler;
mod models;
mod types;

pub use handler::VscodeLmHandler;
pub use models::{default_model_id, models};
pub use types::{VscodeLmConfig, VscodeLmMessageRole, VscodeLmResponsePart, VscodeLmToolCall};

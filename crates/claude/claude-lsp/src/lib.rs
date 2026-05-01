//! `rc-lsp` — Language Server Protocol client and service management.
//!
//! This crate provides a lightweight LSP client for communicating with
//! language servers, along with a registry for managing multiple server
//! instances across different languages.
//!
//! # Overview
//!
//! - **[`types`]** — Core LSP types (Position, Range, Diagnostic, etc.)
//! - **[`client`]** — LSP client for sending requests and notifications
//! - **[`registry`]** — Multi-server management and lifecycle
//! - **[`diagnostics`]** — Diagnostic collection and querying
//!
//! # Example
//!
//! ```rust,ignore
//! use claude_lsp::client::LspClient;
//! use claude_lsp::types::Position;
//!
//! let client = LspClient::new("file:///project");
//! client.initialize("remote-code", "1.0.0");
//! ```

pub mod client;
pub mod diagnostics;
pub mod registry;
pub mod types;

// Re-export commonly used types
pub use types::{
    CompletionItem, Diagnostic, DiagnosticSeverity, DocumentSymbol, Hover, Location, LspMessage,
    LspResponse, Position, Range,
};

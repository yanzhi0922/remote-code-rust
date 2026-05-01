//! # roo-terminal
//!
//! Terminal integration for Roo Code Rust.
//!
//! This crate provides a cross-platform terminal abstraction for executing
//! shell commands, managing terminal processes, and tracking their lifecycle.
//!
//! ## Architecture
//!
//! - **[`types`]** — Core type definitions: [`TerminalId`], [`TerminalState`],
//!   [`CommandResult`], [`ShellExecutionDetails`], [`TerminalCallbacks`].
//! - **[`process`]** — Terminal process management with state machine:
//!   [`TerminalProcess`], [`ProcessState`].
//! - **[`terminal`]** — Terminal abstraction trait [`RooTerminal`] and
//!   default implementation [`DefaultTerminal`] using `tokio::process::Command`.
//! - **[`registry`]** — [`TerminalRegistry`] for managing multiple terminal instances.
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use roo_terminal::registry::TerminalRegistry;
//! use roo_terminal::types::NoopCallbacks;
//! use roo_terminal::RooTerminal;
//!
//! #[tokio::main]
//! async fn main() {
//!     let registry = TerminalRegistry::new();
//!     let id = registry.create_terminal(".").await;
//!
//!     let terminal = registry.get_terminal(id).await.expect("terminal should exist");
//!     let guard = terminal.lock().await;
//!     let result = guard.run_command("echo hello", &NoopCallbacks).await;
//!     println!("{:?}", result);
//! }
//! ```

pub mod output_interceptor;
pub mod process;
pub mod registry;
pub mod shell_integration;
pub mod shell_utils;
pub mod terminal;
pub mod types;

// Re-export the most commonly used types at the crate root.
pub use process::{ProcessState, SharedTerminalProcess, TerminalProcess};
pub use registry::TerminalRegistry;
pub use shell_integration::{
    ShellIntegrationManager, ShellType, ExecaTerminalConfig, ExecaProcessResult,
    merge_promise,
};
pub use terminal::{DefaultTerminal, RooTerminal, TerminalError, get_env};
pub use types::{
    CommandResult, NoopCallbacks, ShellExecutionDetails, TerminalCallbacks, TerminalId, TerminalState,
};

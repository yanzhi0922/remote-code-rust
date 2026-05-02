//! # Roo Context
//!
//! Context management for Roo Code Rust.
//!
//! Combines intelligent condensation of prior messages when approaching configured
//! thresholds with sliding window truncation as a fallback when necessary.
//!
//! Behavior and exports are preserved exactly from the previous sliding-window implementation.

pub mod management;
pub mod tiktoken;
pub mod token;
pub mod truncation;
pub mod error_handling;

pub use management::{
    cleanup_after_truncation, get_effective_api_history, manage_context, will_manage_context,
    ContextManagementOptions, ContextManagementResult, WillManageContextOptions,
};
pub use token::estimate_token_count;
pub use truncation::{truncate_conversation, TruncationResult};
pub use error_handling::check_context_window_exceeded_error;

/// Default percentage of the context window to use as a buffer when deciding
/// when to truncate.
///
/// Source: `src/core/context-management/index.ts` — `TOKEN_BUFFER_PERCENTAGE`
pub const TOKEN_BUFFER_PERCENTAGE: f64 = 0.1;

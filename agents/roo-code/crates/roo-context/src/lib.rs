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

pub use management::{manage_context, will_manage_context, ContextManagementOptions, ContextManagementResult, WillManageContextOptions};
pub use token::estimate_token_count;
pub use truncation::{truncate_conversation, TruncationResult};

/// Default percentage of the context window to use as a buffer when deciding
/// when to truncate.
///
/// Source: `src/core/context-management/index.ts` — `TOKEN_BUFFER_PERCENTAGE`
pub const TOKEN_BUFFER_PERCENTAGE: f64 = 0.1;

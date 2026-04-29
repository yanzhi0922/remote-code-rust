//! # Roo Condense
//!
//! Conversation condensation for Roo Code Rust.
//!
//! Provides tools for summarizing conversations to reduce context window usage:
//! - Convert tool blocks to text representations
//! - Transform messages for condensing
//! - Summarize conversations using LLM calls
//! - Manage conversation history after condensation

pub mod cleanup;
pub mod convert;
pub mod history;
pub mod summarize;
pub mod transform;

pub use cleanup::cleanup_after_truncation;
pub use convert::{convert_tool_blocks_to_text, extract_command_blocks, tool_result_to_text, tool_use_to_text};
pub use history::{get_effective_api_history, get_messages_since_last_summary};
pub use summarize::{summarize_conversation, SummarizeConversationOptions, SummarizeResponse};
pub use transform::{inject_synthetic_tool_results, transform_messages_for_condensing};

/// Minimum condense threshold percentage.
///
/// Source: `src/core/condense/index.ts` — `MIN_CONDENSE_THRESHOLD`
pub const MIN_CONDENSE_THRESHOLD: f64 = 5.0;

/// Maximum condense threshold percentage.
///
/// Source: `src/core/condense/index.ts` — `MAX_CONDENSE_THRESHOLD`
pub const MAX_CONDENSE_THRESHOLD: f64 = 100.0;

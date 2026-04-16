//! `rc-compact` — Compact engine for conversation context management.
//!
//! This crate provides a comprehensive set of compaction strategies for
//! managing conversation context windows in LLM-powered applications.
//!
//! # Overview
//!
//! When a conversation grows too large for the model's context window, a
//! *compaction* strategy reduces the token footprint while preserving the
//! most important context.  This crate implements six strategies:
//!
//! - **Full** — summarise the entire conversation, keep the recent tail.
//! - **Partial** — compact one side of a pivot point, keep the other.
//! - **Auto** — automatically trigger when token usage exceeds a threshold.
//! - **Micro** — clear old tool results to reclaim tokens (no LLM call).
//! - **Snip** — trim oversized tool outputs (no LLM call).
//! - **Reactive** — respond to API prompt-too-long errors.
//! - **Session Memory** — preserve key facts, compress the rest.
//!
//! # Architecture
//!
//! The engine does **not** depend on any specific LLM provider.  Instead,
//! callers supply a [`SummaryProvider`] implementation that knows how to call
//! the model and return summary text.
//!
//! ```text
//!                ┌──────────────────┐
//!                │  CompactStrategy │  (trait)
//!                └──────┬───────────┘
//!          ┌────────────┼────────────┐
//!          ▼            ▼            ▼
//!     FullCompact  AutoCompact  MicroCompact  …
//!          │            │
//!          ▼            ▼
//!     compact_conversation()  (engine)
//!          │
//!          ▼
//!     SummaryProvider::generate_summary()  (callback)
//! ```
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use rc_compact::{
//!     FullCompactStrategy, CompactOptions, CompactStrategy,
//!     FnSummaryProvider,
//! };
//! use rc_core::Message;
//!
//! async fn example(messages: &[Message]) {
//!     let provider = FnSummaryProvider::new(|msgs, sys, user| {
//!         Box::pin(async move {
//!             // Call your LLM here…
//!             Ok("Summary of the conversation".into())
//!         })
//!     });
//!
//!     let strategy = FullCompactStrategy;
//!     let options = CompactOptions::default();
//!     let result = strategy.compact(messages, &options, &provider, None).await;
//! }
//! ```

pub mod attachment;
pub mod auto;
pub mod compact_warning;
pub mod context_collapse;
pub mod engine;
pub mod grouping;
pub mod micro;
pub mod post_compact;
pub mod prompt;
pub mod reactive;
pub mod session_memory;
pub mod snip;
pub mod strategy;

// ---------------------------------------------------------------------------
// Re-exports: core types
// ---------------------------------------------------------------------------

pub use strategy::{
    CompactOptions, CompactProgressEvent, CompactStrategy, CompactStrategyType, CompactionResult,
    FnSummaryProvider, PreservedSegment, SummaryProvider,
};

// ---------------------------------------------------------------------------
// Re-exports: engine (full & partial compact)
// ---------------------------------------------------------------------------

pub use engine::{
    build_post_compact_messages, compact_conversation, create_compact_boundary_message,
    merge_hook_instructions, partial_compact_conversation, FullCompactStrategy,
    PartialCompactStrategy,
};

// ---------------------------------------------------------------------------
// Re-exports: auto compact
// ---------------------------------------------------------------------------

pub use auto::{
    auto_compact, should_auto_compact, AutoCompactStrategy, AutoCompactTrackingState,
    TokenWarningState, AUTOCOMPACT_BUFFER_TOKENS, ERROR_THRESHOLD_BUFFER_TOKENS,
    MANUAL_COMPACT_BUFFER_TOKENS, MAX_CONSECUTIVE_AUTOCOMPACT_FAILURES,
    WARNING_THRESHOLD_BUFFER_TOKENS,
};

// ---------------------------------------------------------------------------
// Re-exports: micro compact
// ---------------------------------------------------------------------------

pub use micro::{
    estimate_messages_tokens, micro_compact, MicroCompactConfig, MicroCompactStrategy,
    TIME_BASED_MC_CLEARED_MESSAGE,
};

// ---------------------------------------------------------------------------
// Re-exports: snip compact
// ---------------------------------------------------------------------------

pub use snip::{
    is_snip_boundary_message, snip_compact, SnipCompactConfig, SnipCompactStrategy,
    DEFAULT_SNIP_THRESHOLD_TOKENS, SNIPPED_CONTENT_MARKER,
};

// ---------------------------------------------------------------------------
// Re-exports: reactive compact
// ---------------------------------------------------------------------------

pub use reactive::{
    reactive_compact, ReactiveCompactConfig, ReactiveCompactStrategy,
    MAX_REACTIVE_COMPACT_RETRIES,
};

// ---------------------------------------------------------------------------
// Re-exports: session memory compact
// ---------------------------------------------------------------------------

pub use session_memory::{
    has_text_blocks, session_memory_compact, SessionMemoryCompactConfig,
    SessionMemoryCompactStrategy, DEFAULT_SM_COMPACT_MAX_TOKENS,
    DEFAULT_SM_COMPACT_MIN_TEXT_BLOCK_MESSAGES, DEFAULT_SM_COMPACT_MIN_TOKENS,
};

// ---------------------------------------------------------------------------
// Re-exports: prompt
// ---------------------------------------------------------------------------

pub use prompt::{
    build_compact_prompt, build_compact_user_summary_message, build_partial_compact_prompt,
    format_compact_summary, rough_token_count, PartialCompactDirection, COMPACT_SYSTEM_PROMPT,
};

// ---------------------------------------------------------------------------
// Re-exports: attachment
// ---------------------------------------------------------------------------

pub use attachment::{
    create_file_attachment_message, create_plan_attachment_if_needed,
    create_post_compact_file_attachments, create_skill_attachment_if_needed, FileState,
    FileStateCache, InvokedSkill, InvokedSkillRegistry, POST_COMPACT_MAX_FILES_TO_RESTORE,
    POST_COMPACT_MAX_TOKENS_PER_FILE, POST_COMPACT_SKILLS_TOKEN_BUDGET,
    POST_COMPACT_TOKEN_BUDGET,
};

// ---------------------------------------------------------------------------
// Re-exports: context collapse
// ---------------------------------------------------------------------------

pub use context_collapse::{
    CollapseOperation, CollapsePersistence, CollapseResult, ContextCollapseConfig,
    ContextCollapseEngine, Ratio64,
};

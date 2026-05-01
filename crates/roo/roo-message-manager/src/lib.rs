//! # roo-message-manager
//!
//! Centralized message management for conversation rewind operations in
//! Roo Code Rust.
//!
//! This crate handles all chat-history rewind scenarios (delete, edit,
//! checkpoint restore, etc.) and ensures that the API conversation history
//! is kept consistent, including:
//!
//! - Removing orphaned Summary messages when their `condense_context` event
//!   is removed.
//! - Removing orphaned truncation markers when their `sliding_window_truncation`
//!   event is removed.
//! - Cleaning up orphaned `condenseParent` / `truncationParent` tags.
//! - Detecting orphaned command-output artifacts.
//!
//! ## Architecture
//!
//! The crate is organised into four modules:
//!
//! | Module | Responsibility |
//! |--------|---------------|
//! | [`types`] | Core data types (`ClineMessage`, `ApiMessage`, `RewindOptions`, …) |
//! | [`manager`] | [`MessageManager`] — the main entry point for rewind operations |
//! | [`cleanup`] | Post-truncation cleanup of orphaned parent references |
//! | [`artifact`] | Orphaned artifact detection |
//!
//! ## TS Source Mapping
//!
//! | TS Source | Rust Module |
//! |-----------|-------------|
//! | `core/message-manager/index.ts` | [`manager`] |
//! | `core/condense/index.ts` — `cleanupAfterTruncation` | [`cleanup`] |
//! | `core/message-manager/index.ts` — `cleanupOrphanedArtifacts` | [`artifact`] |
//!
//! ## Usage
//!
//! ```rust,ignore
//! use roo_message_manager::{MessageManager, RewindOptions};
//!
//! let mgr = MessageManager::new();
//! let result = mgr.rewind_to_timestamp(
//!     &cline_messages,
//!     &api_history,
//!     target_ts,
//!     RewindOptions::default(),
//! )?;
//!
//! // Persist the updated lists
//! save_cline_messages(&result.cline_messages);
//! save_api_history(&result.api_history);
//! ```

pub mod artifact;
pub mod cleanup;
pub mod manager;
pub mod types;

// Re-export the primary public API at the crate root.
pub use artifact::{compute_valid_ids, find_orphaned_artifacts};
pub use cleanup::{
    cleanup_after_truncation, find_orphaned_condense_ids, find_orphaned_truncation_ids,
};
pub use manager::{MessageManager, MessageManagerError, RewindResult};
pub use types::{
    ApiMessage, ClineMessage, ContextCondenseInfo, ContextEventIds, ContextTruncationInfo,
    RewindOptions,
};

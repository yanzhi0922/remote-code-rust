//! # Roo Task Persistence
//!
//! Task persistence layer for the Roo Code Rust project.
//!
//! This crate provides:
//! - **Types**: [`TaskMetadata`], [`HistoryItem`], [`PersistenceTaskStatus`], [`TokenUsage`]
//! - **Message I/O**: [`read_task_messages`], [`save_task_messages`]
//! - **Metadata computation**: [`compute_task_metadata`], [`compute_history_item`]
//! - **Filesystem abstraction**: [`TaskFileSystem`] trait, [`OsFileSystem`] implementation
//! - **History management**: [`list_history`], [`search_history`], [`delete_task`], [`get_history_item`]

use thiserror::Error;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during task persistence operations.
#[derive(Debug, Error)]
pub enum TaskPersistenceError {
    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// A serialization/deserialization error occurred.
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    /// A task was not found.
    #[error("Task not found: {0}")]
    NotFound(String),

    /// An invalid operation was attempted.
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
}

// ---------------------------------------------------------------------------
// Module declarations
// ---------------------------------------------------------------------------

pub mod types;
pub mod messages;
pub mod api_messages;
pub mod metadata;
pub mod storage;
pub mod history;
pub mod task_history_store;
pub mod consolidate_token_usage;
pub mod safe_json_parse;

// ---------------------------------------------------------------------------
// Re-exports
// ---------------------------------------------------------------------------

pub use types::{
    DirEntry, HistoryItem, PersistenceTaskStatus, TaskMetadata, TaskMetadataOptions,
    TaskStorageInfo,
};

pub use messages::{read_task_messages, save_task_messages};

pub use api_messages::{read_api_messages, save_api_messages};

pub use metadata::{compute_history_item, compute_task_metadata};

pub use storage::{OsFileSystem, TaskFileSystem, api_messages_path, ensure_task_dir, messages_path, metadata_path, task_dir};

pub use history::{delete_task, get_history_item, list_history, search_history};

pub use task_history_store::{TaskHistoryStore, TaskHistoryStoreOptions};

pub use consolidate_token_usage::{
    consolidate_token_usage, has_token_usage_changed, TokenUsage, ClineMessageRef,
    ParsedApiReqStartedText,
};

pub use safe_json_parse::{safe_json_parse, safe_json_parse_or};

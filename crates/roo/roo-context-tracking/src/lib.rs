//! File context tracking for Roo Code Rust.
//!
//! This crate tracks file operations that may result in stale context.
//! If a user modifies a file outside of Roo, the context may become stale
//! and need to be updated. The tracker informs Roo that a change has occurred
//! and tells Roo to reload the file before making changes to it.
//!
//! # Architecture
//!
//! - [`types`] — Core data types (`RecordSource`, `RecordState`, `FileMetadataEntry`, `TaskMetadata`)
//! - [`store`] — IO abstraction via the [`MetadataStore`] trait, with filesystem and in-memory implementations
//! - [`tracker`] — [`FileContextTracker`] containing all business logic
//!
//! # Example
//!
//! ```rust
//! use roo_context_tracking::{
//!     FileContextTracker, InMemoryMetadataStore, RecordSource,
//! };
//!
//! let store = InMemoryMetadataStore::new();
//! let mut tracker = FileContextTracker::new("task-123", store);
//!
//! // Track a file read by Roo
//! tracker.add_file_to_context_mut("src/main.rs", RecordSource::ReadTool).unwrap();
//!
//! // Get files read by Roo
//! let files = tracker.get_files_read_by_roo(None).unwrap();
//! assert!(files.contains(&"src/main.rs".to_string()));
//! ```

mod store;
mod tracker;
mod types;

// Re-export public API
pub use store::{FileMetadataStore, InMemoryMetadataStore, MetadataStore, StoreError};
pub use tracker::FileContextTracker;
pub use types::{FileMetadataEntry, RecordSource, RecordState, TaskMetadata};

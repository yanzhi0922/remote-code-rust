//! Conversation-level version control with file snapshots, undo, and restore.
//!
//! Inspired by ZCode's checkpoint system: every user message creates a checkpoint
//! that captures the state of all workspace files. Users can review changes, undo
//! the last interaction, or restore to any historical checkpoint.
//!
//! # Architecture
//!
//! ```text
//! User sends message
//!     │
//!     ▼
//! create_pre_snapshot()  ← scan workspace, record file hashes
//!     │
//!     ▼
//! Agent executes (tools run, files change)
//!     │
//!     ▼
//! finalize_snapshot()    ← rescan, compute diffs, store checkpoint
//!     │
//!     ▼
//! GUI receives checkpoint_created event
//! ```
//!
//! # Storage
//!
//! Checkpoints are stored in SQLite alongside session data. File content is stored
//! as compressed blobs to minimize disk usage.

pub mod diff;
pub mod restore;
pub mod snapshot;
pub mod storage;
pub mod types;

pub use diff::CheckpointDiffer;
pub use restore::RestoreEngine;
pub use snapshot::SnapshotScanner;
pub use storage::CheckpointStore;
pub use types::*;

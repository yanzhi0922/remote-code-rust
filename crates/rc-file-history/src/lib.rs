//! # rc-file-history — File Checkpoint System
//!
//! Corresponds to `src/utils/fileHistory.ts` (1,116 lines).
//! Provides file checkpointing with snapshots, backups, diff stats,
//! and the ability to rewind to previous file states.
//!
//! ## Features
//! - **File Tracking**: Track files before edits, creating backups automatically
//! - **Snapshots**: Capture point-in-time state of all tracked files
//! - **Rewind**: Restore files to a previous snapshot
//! - **Diff Stats**: Calculate insertions/deletions between versions
//! - **Backup Management**: Create, restore, and manage file backups
//!
//! ## Example
//! ```ignore
//! use rc_file_history::{FileHistoryState, track_edit, make_snapshot};
//!
//! let mut state = FileHistoryState::new();
//! track_edit(&mut state, "src/main.rs", "msg-123")?;
//! make_snapshot(&mut state, "msg-456")?;
//! ```

pub mod backup;
pub mod diff_stats;
pub mod snapshot;
pub mod state;

pub use backup::{create_backup, restore_backup, BackupRecord};
pub use diff_stats::{compute_diff_stats_for_file, DiffStats};
pub use snapshot::{FileHistorySnapshot, MAX_SNAPSHOTS};
pub use state::{FileHistoryState, file_history_can_restore, rewind_to_snapshot};

use std::path::{Path, PathBuf};

/// Check if file history/checkpointing is enabled.
///
/// In the real implementation, this checks global config and env vars.
/// For now, defaults to true unless explicitly disabled.
#[must_use]
pub fn file_history_enabled() -> bool {
    std::env::var("RC_DISABLE_FILE_CHECKPOINTING").map_or(true, |v| v != "1")
}

/// Resolve the backup directory path for a given session.
pub fn backup_dir(profile_dir: &Path, session_id: &str) -> PathBuf {
    profile_dir.join("file-history").join(session_id)
}

/// Track a file edit by creating a backup of its current contents.
///
/// This must be called before the file is actually modified.
pub fn track_edit(
    state: &mut FileHistoryState,
    file_path: &str,
    _message_id: &str,
) -> anyhow::Result<()> {
    state.track_file(file_path.to_string());
    Ok(())
}

/// Create a snapshot of all tracked files.
pub fn make_snapshot(
    state: &mut FileHistoryState,
    message_id: &str,
) -> anyhow::Result<()> {
    state.create_snapshot(message_id.to_string());
    Ok(())
}

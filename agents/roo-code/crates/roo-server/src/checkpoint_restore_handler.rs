//! Checkpoint restore handling for delete and edit operations.
//!
//! Derived from `src/core/webview/checkpointRestoreHandler.ts`.
//!
//! Handles checkpoint restoration for both delete and edit operations,
//! consolidating common logic while handling operation-specific behavior.

use serde::{Deserialize, Serialize};
use tracing::{debug, error, info, warn};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Configuration for a checkpoint restore operation.
///
/// Source: `src/core/webview/checkpointRestoreHandler.ts` — `CheckpointRestoreConfig`
#[derive(Debug, Clone)]
pub struct CheckpointRestoreConfig {
    pub task_id: String,
    pub message_ts: u64,
    pub message_index: usize,
    pub checkpoint: CheckpointInfo,
    pub operation: CheckpointOperation,
    pub edit_data: Option<EditData>,
}

/// Checkpoint information.
#[derive(Debug, Clone)]
pub struct CheckpointInfo {
    pub hash: String,
}

/// The type of operation triggering the checkpoint restore.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckpointOperation {
    Delete,
    Edit,
}

/// Edit-specific data for checkpoint restore.
#[derive(Debug, Clone)]
pub struct EditData {
    pub edited_content: String,
    pub images: Option<Vec<String>>,
    pub api_conversation_history_index: isize,
}

/// Result of a checkpoint restore operation.
#[derive(Debug, Clone)]
pub struct CheckpointRestoreResult {
    pub success: bool,
    pub error: Option<String>,
}

/// Pending edit operation data.
#[derive(Debug, Clone)]
pub struct PendingEditOperation {
    pub message_ts: u64,
    pub edited_content: String,
    pub images: Option<Vec<String>>,
    pub message_index: usize,
    pub api_conversation_history_index: isize,
}

// ---------------------------------------------------------------------------
// Checkpoint restore handler
// ---------------------------------------------------------------------------

/// Handles checkpoint restoration for both delete and edit operations.
///
/// Source: `src/core/webview/checkpointRestoreHandler.ts` — `handleCheckpointRestoreOperation`
///
/// This consolidates the common logic while handling operation-specific behavior:
/// - For delete: aborts the task, waits for abort completion, performs restore,
///   saves messages, and reinitializes
/// - For edit: sets up pending edit data, performs restore, and lets
///   reinitialization process the pending edit
///
/// # Arguments
/// * `config` - Configuration for the restore operation
/// * `callbacks` - Callbacks for performing restore operations
///
/// # Returns
/// A `CheckpointRestoreResult` indicating success or failure.
pub async fn handle_checkpoint_restore_operation<F, G, H>(
    config: CheckpointRestoreConfig,
    callbacks: &CheckpointRestoreCallbacks<F, G, H>,
) -> CheckpointRestoreResult
where
    F: Fn(&str, &str) -> Result<(), String>,
    G: Fn() -> Result<(), String>,
    H: Fn() -> Result<(), String>,
{
    let CheckpointRestoreConfig {
        task_id,
        message_ts,
        checkpoint,
        operation,
        edit_data,
        ..
    } = &config;

    // For delete operations, abort the task first
    if *operation == CheckpointOperation::Delete {
        match (callbacks.abort_task)() {
            Ok(()) => {
                debug!("Task aborted for delete checkpoint restore");
            }
            Err(e) => {
                warn!("Failed to abort task: {}", e);
            }
        }
    }

    // For edit operations, set up pending edit data
    if *operation == CheckpointOperation::Edit {
        if let Some(edit) = edit_data {
            let operation_id = format!("task-{task_id}");
            (callbacks.set_pending_edit)(
                &operation_id,
                &PendingEditOperation {
                    message_ts: *message_ts,
                    edited_content: edit.edited_content.clone(),
                    images: edit.images.clone(),
                    message_index: config.message_index,
                    api_conversation_history_index: edit.api_conversation_history_index,
                },
            );
        }
    }

    // Perform the checkpoint restoration
    let ts_str = message_ts.to_string();
    match (callbacks.checkpoint_restore)(&ts_str, &checkpoint.hash) {
        Ok(()) => {
            info!("Checkpoint restored successfully for {} operation", 
                match operation {
                    CheckpointOperation::Delete => "delete",
                    CheckpointOperation::Edit => "edit",
                });
        }
        Err(e) => {
            error!("Failed to restore checkpoint: {}", e);
            return CheckpointRestoreResult {
                success: false,
                error: Some(format!("Checkpoint restore failed: {e}")),
            };
        }
    }

    // For delete operations, save messages and reinitialize
    if *operation == CheckpointOperation::Delete {
        match (callbacks.save_and_reinit)() {
            Ok(()) => {
                debug!("Messages saved and task reinitialized");
            }
            Err(e) => {
                error!("Failed to save and reinitialize: {}", e);
                return CheckpointRestoreResult {
                    success: false,
                    error: Some(format!("Save and reinit failed: {e}")),
                };
            }
        }
    }

    CheckpointRestoreResult {
        success: true,
        error: None,
    }
}

/// Callbacks for checkpoint restore operations.
pub struct CheckpointRestoreCallbacks<F, G, H>
where
    F: Fn(&str, &str) -> Result<(), String>,
    G: Fn() -> Result<(), String>,
    H: Fn() -> Result<(), String>,
{
    /// Abort the current task.
    pub abort_task: G,
    /// Set pending edit operation data.
    pub set_pending_edit: FnSetPendingEdit,
    /// Perform the checkpoint restore.
    pub checkpoint_restore: F,
    /// Save messages and reinitialize the task.
    pub save_and_reinit: H,
}

/// Type alias for the set_pending_edit callback.
type FnSetPendingEdit = Box<dyn Fn(&str, &PendingEditOperation)>;

// ---------------------------------------------------------------------------
// Wait for initialization
// ---------------------------------------------------------------------------

/// Waits for a task to be initialized after checkpoint restore.
///
/// Source: `src/core/webview/checkpointRestoreHandler.ts` — `waitForClineInitialization`
///
/// # Arguments
/// * `check_fn` - Function that returns true when initialization is complete
/// * `timeout_ms` - Maximum time to wait in milliseconds
///
/// # Returns
/// `true` if initialization completed within the timeout, `false` otherwise.
pub async fn wait_for_initialization(
    check_fn: impl Fn() -> bool,
    timeout_ms: u64,
) -> bool {
    let start = std::time::Instant::now();
    let timeout = std::time::Duration::from_millis(timeout_ms);

    while start.elapsed() < timeout {
        if check_fn() {
            return true;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    warn!("Timed out waiting for initialization after {}ms", timeout_ms);
    false
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_operation_serialization() {
        let delete = CheckpointOperation::Delete;
        let json = serde_json::to_string(&delete).unwrap();
        assert_eq!(json, "\"delete\"");

        let edit = CheckpointOperation::Edit;
        let json = serde_json::to_string(&edit).unwrap();
        assert_eq!(json, "\"edit\"");
    }

    #[test]
    fn test_checkpoint_restore_config_construction() {
        let config = CheckpointRestoreConfig {
            task_id: "task-123".to_string(),
            message_ts: 1234567890,
            message_index: 5,
            checkpoint: CheckpointInfo {
                hash: "abc123".to_string(),
            },
            operation: CheckpointOperation::Delete,
            edit_data: None,
        };
        assert_eq!(config.task_id, "task-123");
        assert_eq!(config.checkpoint.hash, "abc123");
    }

    #[test]
    fn test_edit_data_construction() {
        let edit = EditData {
            edited_content: "New content".to_string(),
            images: Some(vec!["img1.png".to_string()]),
            api_conversation_history_index: 3,
        };
        assert_eq!(edit.edited_content, "New content");
        assert_eq!(edit.images.as_ref().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_wait_for_initialization_immediate() {
        let result = wait_for_initialization(|| true, 1000).await;
        assert!(result);
    }

    #[tokio::test]
    async fn test_wait_for_initialization_timeout() {
        let result = wait_for_initialization(|| false, 100).await;
        assert!(!result);
    }

    #[test]
    fn test_pending_edit_operation() {
        let pending = PendingEditOperation {
            message_ts: 12345,
            edited_content: "edited".to_string(),
            images: None,
            message_index: 2,
            api_conversation_history_index: 1,
        };
        assert_eq!(pending.message_ts, 12345);
    }
}

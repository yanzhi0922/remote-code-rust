//! Task metadata computation.
//!
//! Computes [`TaskMetadata`] (and [`HistoryItem`]) from raw task data including
//! token statistics, timestamps, and directory sizes.

use serde::Deserialize;

use roo_types::message::{ClineMessage, MessageType};

use crate::messages;
use crate::storage::TaskFileSystem;
use crate::types::{HistoryItem, TaskMetadata, TaskMetadataOptions};
use crate::TaskPersistenceError;

// ---------------------------------------------------------------------------
// compute_task_metadata
// ---------------------------------------------------------------------------

/// Compute full task metadata from the given options.
///
/// This reads messages from disk (if available), aggregates token usage,
/// determines the task description from the first user message, and
/// calculates the task directory size.
pub fn compute_task_metadata(
    fs: &dyn TaskFileSystem,
    opts: &TaskMetadataOptions,
) -> Result<TaskMetadata, TaskPersistenceError> {
    // Read messages from disk or use provided messages
    let messages = if opts.messages.is_empty() {
        let msg_path = opts.global_storage_path.join("tasks").join(&opts.task_id).join("messages.json");
        messages::read_task_messages(fs, &msg_path).unwrap_or_default()
    } else {
        opts.messages.clone()
    };

    // Compute token usage from messages
    let (tokens_in, tokens_out, cache_writes, cache_reads, total_cost) =
        aggregate_token_usage(&messages);

    // Extract task description from the first user text message
    let task_description = extract_task_description(&messages);

    // Compute directory size
    let task_dir = opts.global_storage_path.join("tasks").join(&opts.task_id);
    let size = fs.dir_size(&task_dir).unwrap_or(0);

    // Use current timestamp if no messages
    let timestamp = messages
        .first()
        .map(|m| m.ts as u64)
        .unwrap_or_else(|| {
            chrono::Utc::now().timestamp_millis() as u64
        });

    Ok(TaskMetadata {
        task_id: opts.task_id.clone(),
        root_task_id: opts.root_task_id.clone(),
        parent_task_id: opts.parent_task_id.clone(),
        task_number: opts.task_number,
        timestamp,
        task_description,
        tokens_in,
        tokens_out,
        cache_writes,
        cache_reads,
        total_cost,
        size,
        workspace: opts.workspace.clone(),
        mode: opts.mode.clone(),
        api_config_name: opts.api_config_name.clone(),
        status: opts.initial_status,
    })
}

// ---------------------------------------------------------------------------
// compute_history_item
// ---------------------------------------------------------------------------

/// Compute a history item from task metadata options.
///
/// Convenience wrapper that computes metadata and converts to [`HistoryItem`].
pub fn compute_history_item(
    fs: &dyn TaskFileSystem,
    opts: &TaskMetadataOptions,
) -> Result<HistoryItem, TaskPersistenceError> {
    let metadata = compute_task_metadata(fs, opts)?;
    Ok(HistoryItem::from(metadata))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Aggregate token usage from a list of messages.
///
/// Looks at `ApiReqStarted` messages (which carry token info in their text)
/// and sums up token counts. Returns `(tokens_in, tokens_out, cache_writes, cache_reads, total_cost)`.
fn aggregate_token_usage(messages: &[ClineMessage]) -> (u64, u64, u64, u64, f64) {
    let mut tokens_in: u64 = 0;
    let mut tokens_out: u64 = 0;
    let mut cache_writes: u64 = 0;
    let mut cache_reads: u64 = 0;
    let mut total_cost: f64 = 0.0;

    for msg in messages {
        // API request finished messages carry token usage data
        if msg.r#type == MessageType::Say {
            if let Some(text) = &msg.text {
                if let Ok(usage) = serde_json::from_str::<TokenUsageData>(text) {
                    tokens_in += usage.total_tokens_in.unwrap_or(0);
                    tokens_out += usage.total_tokens_out.unwrap_or(0);
                    cache_writes += usage.total_cache_writes.unwrap_or(0);
                    cache_reads += usage.total_cache_reads.unwrap_or(0);
                    total_cost += usage.total_cost.unwrap_or(0.0);
                }
            }
        }
    }

    (tokens_in, tokens_out, cache_writes, cache_reads, total_cost)
}

/// Internal structure for parsing token usage from message text.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TokenUsageData {
    #[serde(default)]
    total_tokens_in: Option<u64>,
    #[serde(default)]
    total_tokens_out: Option<u64>,
    #[serde(default)]
    total_cache_writes: Option<u64>,
    #[serde(default)]
    total_cache_reads: Option<u64>,
    #[serde(default)]
    total_cost: Option<f64>,
}

/// Extract the task description from the first user text message.
fn extract_task_description(messages: &[ClineMessage]) -> String {
    for msg in messages {
        if msg.r#type == MessageType::Ask {
            if let Some(text) = &msg.text {
                if !text.is_empty() {
                    // Truncate to a reasonable length for display
                    return truncate_description(text, 200);
                }
            }
        }
    }
    // Fallback: use first say text
    for msg in messages {
        if msg.r#type == MessageType::Say {
            if let Some(text) = &msg.text {
                if !text.is_empty() {
                    return truncate_description(text, 200);
                }
            }
        }
    }
    String::new()
}

/// Truncate a description string, adding ellipsis if needed.
fn truncate_description(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len).collect();
        format!("{truncated}...")
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::OsFileSystem;
    use crate::PersistenceTaskStatus;

    use std::path::PathBuf;

    fn make_opts() -> TaskMetadataOptions {
        TaskMetadataOptions {
            task_id: "test-task-1".to_string(),
            root_task_id: None,
            parent_task_id: None,
            task_number: 1,
            messages: Vec::new(),
            global_storage_path: PathBuf::from("/tmp/nonexistent"),
            workspace: "/tmp/workspace".to_string(),
            mode: Some("code".to_string()),
            api_config_name: None,
            initial_status: PersistenceTaskStatus::Active,
        }
    }

    #[test]
    fn test_compute_metadata_no_messages() {
        let fs = OsFileSystem;
        let opts = make_opts();
        let meta = compute_task_metadata(&fs, &opts).unwrap();

        assert_eq!(meta.task_id, "test-task-1");
        assert_eq!(meta.tokens_in, 0);
        assert_eq!(meta.tokens_out, 0);
        assert_eq!(meta.total_cost, 0.0);
        assert_eq!(meta.task_description, "");
        assert_eq!(meta.workspace, "/tmp/workspace");
    }

    #[test]
    fn test_compute_metadata_with_messages() {
        let fs = OsFileSystem;
        let mut opts = make_opts();
        opts.messages = vec![
            ClineMessage {
                ts: 1700000000.0,
                r#type: MessageType::Ask,
                ask: Some(roo_types::message::ClineAsk::Followup),
                say: None,
                text: Some("Build me a todo app".to_string()),
                images: None,
                partial: None,
                reasoning: None,
                conversation_history_index: None,
                checkpoint: None,
                progress_status: None,
                context_condense: None,
                context_truncation: None,
                is_protected: None,
                api_protocol: None,
                is_answered: None,
            },
        ];

        let meta = compute_task_metadata(&fs, &opts).unwrap();
        assert_eq!(meta.task_description, "Build me a todo app");
        assert_eq!(meta.timestamp, 1700000000);
    }

    #[test]
    fn test_compute_metadata_with_token_usage() {
        let fs = OsFileSystem;
        let mut opts = make_opts();
        opts.messages = vec![
            ClineMessage {
                ts: 1700000000.0,
                r#type: MessageType::Ask,
                ask: Some(roo_types::message::ClineAsk::Followup),
                say: None,
                text: Some("Hello".to_string()),
                images: None,
                partial: None,
                reasoning: None,
                conversation_history_index: None,
                checkpoint: None,
                progress_status: None,
                context_condense: None,
                context_truncation: None,
                is_protected: None,
                api_protocol: None,
                is_answered: None,
            },
            ClineMessage {
                ts: 1700000001.0,
                r#type: MessageType::Say,
                ask: None,
                say: Some(roo_types::message::ClineSay::ApiReqFinished),
                text: Some(r#"{"totalTokensIn":100,"totalTokensOut":50,"totalCacheWrites":10,"totalCacheReads":5,"totalCost":0.05}"#.to_string()),
                images: None,
                partial: None,
                reasoning: None,
                conversation_history_index: None,
                checkpoint: None,
                progress_status: None,
                context_condense: None,
                context_truncation: None,
                is_protected: None,
                api_protocol: None,
                is_answered: None,
            },
        ];

        let meta = compute_task_metadata(&fs, &opts).unwrap();
        assert_eq!(meta.tokens_in, 100);
        assert_eq!(meta.tokens_out, 50);
        assert_eq!(meta.cache_writes, 10);
        assert_eq!(meta.cache_reads, 5);
        assert!((meta.total_cost - 0.05).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compute_history_item() {
        let fs = OsFileSystem;
        let mut opts = make_opts();
        opts.messages = vec![ClineMessage {
            ts: 1700000000.0,
            r#type: MessageType::Ask,
            ask: Some(roo_types::message::ClineAsk::Followup),
            say: None,
            text: Some("Test task".to_string()),
            images: None,
            partial: None,
            reasoning: None,
            conversation_history_index: None,
            checkpoint: None,
            progress_status: None,
            context_condense: None,
            context_truncation: None,
            is_protected: None,
            api_protocol: None,
            is_answered: None,
        }];

        let item = compute_history_item(&fs, &opts).unwrap();
        assert_eq!(item.id, "test-task-1");
        assert_eq!(item.task, "Test task");
        assert_eq!(item.number, 1);
    }

    #[test]
    fn test_truncate_description_short() {
        assert_eq!(truncate_description("Hello", 200), "Hello");
    }

    #[test]
    fn test_truncate_description_long() {
        let long: String = "a".repeat(300);
        let result = truncate_description(&long, 200);
        assert!(result.ends_with("..."));
        assert_eq!(result.len(), 203); // 200 chars + "..."
    }

    #[test]
    fn test_extract_task_description_empty_messages() {
        let desc = extract_task_description(&[]);
        assert!(desc.is_empty());
    }

    #[test]
    fn test_extract_task_description_fallback_to_say() {
        let messages = vec![ClineMessage {
            ts: 1700000000.0,
            r#type: MessageType::Say,
            ask: None,
            say: Some(roo_types::message::ClineSay::Text),
            text: Some("Fallback text".to_string()),
            images: None,
            partial: None,
            reasoning: None,
            conversation_history_index: None,
            checkpoint: None,
            progress_status: None,
            context_condense: None,
            context_truncation: None,
            is_protected: None,
            api_protocol: None,
            is_answered: None,
        }];
        let desc = extract_task_description(&messages);
        assert_eq!(desc, "Fallback text");
    }

    #[test]
    fn test_compute_metadata_with_disk_messages() {
        let dir = tempfile::tempdir().unwrap();
        let fs = OsFileSystem;

        // Create task directory and messages file
        let task_dir = dir.path().join("tasks").join("disk-task-1");
        fs.create_dir_all(&task_dir).unwrap();

        let msg = ClineMessage {
            ts: 1700000000.0,
            r#type: MessageType::Ask,
            ask: Some(roo_types::message::ClineAsk::Followup),
            say: None,
            text: Some("Disk task".to_string()),
            images: None,
            partial: None,
            reasoning: None,
            conversation_history_index: None,
            checkpoint: None,
            progress_status: None,
            context_condense: None,
            context_truncation: None,
            is_protected: None,
            api_protocol: None,
            is_answered: None,
        };
        let msg_path = task_dir.join("messages.json");
        messages::save_task_messages(&fs, &msg_path, &[msg]).unwrap();

        let opts = TaskMetadataOptions {
            task_id: "disk-task-1".to_string(),
            root_task_id: None,
            parent_task_id: None,
            task_number: 1,
            messages: Vec::new(), // empty, should read from disk
            global_storage_path: dir.path().to_path_buf(),
            workspace: "/tmp/ws".to_string(),
            mode: None,
            api_config_name: None,
            initial_status: PersistenceTaskStatus::Active,
        };

        let meta = compute_task_metadata(&fs, &opts).unwrap();
        assert_eq!(meta.task_description, "Disk task");
    }
}

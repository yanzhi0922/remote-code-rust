//! Task persistence type definitions.
//!
//! Defines TaskMetadata, TaskMetadataOptions, TokenUsage, TaskStorageInfo,
//! HistoryItem, and related types for task persistence.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use roo_types::message::ClineMessage;
use roo_types::task::TaskStatus;

// ---------------------------------------------------------------------------
// PersistenceTaskStatus
// ---------------------------------------------------------------------------

/// Task status used in the persistence layer.
///
/// This mirrors [`roo_types::task::TaskStatus`] but adds a `Delegated` variant
/// and uses lowercase serialization to match the TypeScript source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PersistenceTaskStatus {
    Active,
    Completed,
    Aborted,
    Delegated,
}

impl Default for PersistenceTaskStatus {
    fn default() -> Self {
        Self::Active
    }
}

impl std::fmt::Display for PersistenceTaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Completed => write!(f, "completed"),
            Self::Aborted => write!(f, "aborted"),
            Self::Delegated => write!(f, "delegated"),
        }
    }
}

impl From<TaskStatus> for PersistenceTaskStatus {
    fn from(status: TaskStatus) -> Self {
        match status {
            TaskStatus::Running | TaskStatus::Paused | TaskStatus::Idle => Self::Active,
            TaskStatus::Completed => Self::Completed,
            TaskStatus::Aborted => Self::Aborted,
        }
    }
}

// ---------------------------------------------------------------------------
// TaskMetadata
// ---------------------------------------------------------------------------

/// Metadata for a persisted task.
///
/// Source: `.research/Roo-Code/src/core/task-persistence/taskMetadata.ts`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskMetadata {
    pub task_id: String,
    pub root_task_id: Option<String>,
    pub parent_task_id: Option<String>,
    pub task_number: usize,
    pub timestamp: u64,
    pub task_description: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cache_writes: u64,
    pub cache_reads: u64,
    pub total_cost: f64,
    pub size: u64,
    pub workspace: String,
    pub mode: Option<String>,
    pub api_config_name: Option<String>,
    pub status: PersistenceTaskStatus,
}

// ---------------------------------------------------------------------------
// TaskMetadataOptions
// ---------------------------------------------------------------------------

/// Options for constructing task metadata.
///
/// Source: `.research/Roo-Code/src/core/task-persistence/taskMetadata.ts` — `TaskMetadataOptions`
#[derive(Debug, Clone)]
pub struct TaskMetadataOptions {
    pub task_id: String,
    pub root_task_id: Option<String>,
    pub parent_task_id: Option<String>,
    pub task_number: usize,
    pub messages: Vec<ClineMessage>,
    pub global_storage_path: PathBuf,
    pub workspace: String,
    pub mode: Option<String>,
    pub api_config_name: Option<String>,
    pub initial_status: PersistenceTaskStatus,
}

// ---------------------------------------------------------------------------
// HistoryItem
// ---------------------------------------------------------------------------

/// A single entry in the task history list.
///
/// Source: `.research/Roo-Code/src/core/task-persistence/taskMetadata.ts` — return type of `taskMetadata()`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub id: String,
    pub number: usize,
    pub timestamp: u64,
    pub task: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cache_writes: u64,
    pub cache_reads: u64,
    pub total_cost: f64,
    pub size: u64,
    pub workspace: String,
    pub mode: Option<String>,
    pub api_config_name: Option<String>,
    pub status: PersistenceTaskStatus,
    pub root_task_id: Option<String>,
    pub parent_task_id: Option<String>,
}

impl From<TaskMetadata> for HistoryItem {
    fn from(meta: TaskMetadata) -> Self {
        Self {
            id: meta.task_id,
            number: meta.task_number,
            timestamp: meta.timestamp,
            task: meta.task_description,
            tokens_in: meta.tokens_in,
            tokens_out: meta.tokens_out,
            cache_writes: meta.cache_writes,
            cache_reads: meta.cache_reads,
            total_cost: meta.total_cost,
            size: meta.size,
            workspace: meta.workspace,
            mode: meta.mode,
            api_config_name: meta.api_config_name,
            status: meta.status,
            root_task_id: meta.root_task_id,
            parent_task_id: meta.parent_task_id,
        }
    }
}

// ---------------------------------------------------------------------------
// TaskStorageInfo
// ---------------------------------------------------------------------------

/// Information about a task's on-disk storage location.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStorageInfo {
    pub task_id: String,
    pub base_path: PathBuf,
    pub messages_file: PathBuf,
    pub meta_file: PathBuf,
}

impl TaskStorageInfo {
    /// Create a new storage info for a task under the given base directory.
    pub fn new(base_dir: &std::path::Path, task_id: &str) -> Self {
        let task_dir = base_dir.join("tasks").join(task_id);
        Self {
            task_id: task_id.to_string(),
            messages_file: task_dir.join("messages.json"),
            meta_file: task_dir.join("meta.json"),
            base_path: task_dir,
        }
    }
}

// ---------------------------------------------------------------------------
// DirEntry
// ---------------------------------------------------------------------------

/// Simplified directory entry for use with the filesystem trait.
#[derive(Debug, Clone)]
pub struct DirEntry {
    pub path: PathBuf,
    pub file_name: String,
    pub is_dir: bool,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use roo_types::message::TokenUsage;

    #[test]
    fn test_persistence_task_status_default() {
        assert_eq!(PersistenceTaskStatus::default(), PersistenceTaskStatus::Active);
    }

    #[test]
    fn test_persistence_task_status_display() {
        assert_eq!(format!("{}", PersistenceTaskStatus::Active), "active");
        assert_eq!(format!("{}", PersistenceTaskStatus::Completed), "completed");
        assert_eq!(format!("{}", PersistenceTaskStatus::Aborted), "aborted");
        assert_eq!(format!("{}", PersistenceTaskStatus::Delegated), "delegated");
    }

    #[test]
    fn test_persistence_task_status_from_task_status() {
        assert_eq!(
            PersistenceTaskStatus::from(TaskStatus::Running),
            PersistenceTaskStatus::Active
        );
        assert_eq!(
            PersistenceTaskStatus::from(TaskStatus::Idle),
            PersistenceTaskStatus::Active
        );
        assert_eq!(
            PersistenceTaskStatus::from(TaskStatus::Paused),
            PersistenceTaskStatus::Active
        );
        assert_eq!(
            PersistenceTaskStatus::from(TaskStatus::Completed),
            PersistenceTaskStatus::Completed
        );
        assert_eq!(
            PersistenceTaskStatus::from(TaskStatus::Aborted),
            PersistenceTaskStatus::Aborted
        );
    }

    #[test]
    fn test_persistence_task_status_serde_roundtrip() {
        let status = PersistenceTaskStatus::Completed;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"completed\"");
        let deserialized: PersistenceTaskStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, status);
    }

    #[test]
    fn test_persistence_task_status_all_variants_serde() {
        let variants = [
            PersistenceTaskStatus::Active,
            PersistenceTaskStatus::Completed,
            PersistenceTaskStatus::Aborted,
            PersistenceTaskStatus::Delegated,
        ];
        for v in &variants {
            let json = serde_json::to_string(v).unwrap();
            let back: PersistenceTaskStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(&back, v);
        }
    }

    #[test]
    fn test_task_metadata_serialization() {
        let meta = TaskMetadata {
            task_id: "task-123".to_string(),
            root_task_id: None,
            parent_task_id: None,
            task_number: 1,
            timestamp: 1700000000,
            task_description: "Test task".to_string(),
            tokens_in: 100,
            tokens_out: 50,
            cache_writes: 10,
            cache_reads: 5,
            total_cost: 0.05,
            size: 2048,
            workspace: "/tmp/workspace".to_string(),
            mode: Some("code".to_string()),
            api_config_name: None,
            status: PersistenceTaskStatus::Active,
        };
        let json = serde_json::to_string(&meta).unwrap();
        let back: TaskMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(back.task_id, "task-123");
        assert_eq!(back.tokens_in, 100);
        assert_eq!(back.total_cost, 0.05);
        assert_eq!(back.mode, Some("code".to_string()));
    }

    #[test]
    fn test_history_item_from_metadata() {
        let meta = TaskMetadata {
            task_id: "task-456".to_string(),
            root_task_id: Some("root-1".to_string()),
            parent_task_id: None,
            task_number: 3,
            timestamp: 1700000001,
            task_description: "Build feature".to_string(),
            tokens_in: 200,
            tokens_out: 100,
            cache_writes: 20,
            cache_reads: 10,
            total_cost: 0.10,
            size: 4096,
            workspace: "/tmp/ws".to_string(),
            mode: Some("architect".to_string()),
            api_config_name: Some("gpt4".to_string()),
            status: PersistenceTaskStatus::Completed,
        };
        let item: HistoryItem = meta.into();
        assert_eq!(item.id, "task-456");
        assert_eq!(item.number, 3);
        assert_eq!(item.task, "Build feature");
        assert_eq!(item.root_task_id, Some("root-1".to_string()));
        assert_eq!(item.status, PersistenceTaskStatus::Completed);
    }

    #[test]
    fn test_history_item_serialization() {
        let item = HistoryItem {
            id: "task-789".to_string(),
            number: 5,
            timestamp: 1700000002,
            task: "Debug issue".to_string(),
            tokens_in: 300,
            tokens_out: 150,
            cache_writes: 30,
            cache_reads: 15,
            total_cost: 0.15,
            size: 8192,
            workspace: "/home/user/project".to_string(),
            mode: None,
            api_config_name: None,
            status: PersistenceTaskStatus::Aborted,
            root_task_id: None,
            parent_task_id: None,
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        // Verify camelCase serialization (field "id" stays as "id")
        assert!(parsed.get("id").is_some());
        assert!(parsed.get("tokensIn").is_some());
        assert!(parsed.get("totalCost").is_some());
        assert!(parsed.get("cacheWrites").is_some());
        assert!(parsed.get("rootTaskId").is_some());
    }

    #[test]
    fn test_task_storage_info_new() {
        let base = std::path::Path::new("/data/storage");
        let info = TaskStorageInfo::new(base, "abc-123");
        assert_eq!(info.task_id, "abc-123");
        assert_eq!(info.base_path, std::path::PathBuf::from("/data/storage/tasks/abc-123"));
        assert_eq!(info.messages_file, std::path::PathBuf::from("/data/storage/tasks/abc-123/messages.json"));
        assert_eq!(info.meta_file, std::path::PathBuf::from("/data/storage/tasks/abc-123/meta.json"));
    }

    #[test]
    fn test_token_usage_default() {
        let usage = TokenUsage::default();
        assert_eq!(usage.total_tokens_in, 0);
        assert_eq!(usage.total_tokens_out, 0);
        assert_eq!(usage.total_cost, 0.0);
        assert_eq!(usage.context_tokens, 0);
        assert!(usage.total_cache_writes.is_none());
        assert!(usage.total_cache_reads.is_none());
    }

    #[test]
    fn test_token_usage_accumulation() {
        let mut usage = TokenUsage {
            total_tokens_in: 100,
            total_tokens_out: 50,
            total_cache_writes: Some(10),
            total_cache_reads: Some(5),
            total_cost: 0.05,
            context_tokens: 200,
        };
        // Simulate accumulation
        usage.total_tokens_in += 200;
        usage.total_tokens_out += 100;
        usage.total_cost += 0.10;
        assert_eq!(usage.total_tokens_in, 300);
        assert_eq!(usage.total_tokens_out, 150);
        assert!((usage.total_cost - 0.15).abs() < f64::EPSILON);
    }

    #[test]
    fn test_token_usage_serialization() {
        let usage = TokenUsage {
            total_tokens_in: 1000,
            total_tokens_out: 500,
            total_cache_writes: Some(100),
            total_cache_reads: Some(50),
            total_cost: 1.5,
            context_tokens: 2000,
        };
        let json = serde_json::to_string(&usage).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["totalTokensIn"], 1000);
        assert_eq!(parsed["totalTokensOut"], 500);
        assert_eq!(parsed["totalCost"], 1.5);
        assert_eq!(parsed["contextTokens"], 2000);
    }
}

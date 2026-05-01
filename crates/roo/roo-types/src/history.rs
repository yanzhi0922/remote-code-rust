//! History type definitions.
//!
//! Derived from `packages/types/src/history.ts`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// HistoryItem
// ---------------------------------------------------------------------------

/// A summary entry for a task in the history.
///
/// Source: `packages/types/src/history.ts` — `historyItemSchema`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryItem {
    pub id: String,
    pub root_task_id: Option<String>,
    pub parent_task_id: Option<String>,
    pub number: u64,
    pub ts: f64,
    pub task: String,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub cache_writes: Option<u64>,
    pub cache_reads: Option<u64>,
    pub total_cost: f64,
    pub size: Option<u64>,
    pub workspace: Option<String>,
    pub mode: Option<String>,
    pub api_config_name: Option<String>,
    pub status: Option<HistoryItemStatus>,
    pub delegated_to_id: Option<String>,
    pub child_ids: Option<Vec<String>>,
    pub awaiting_child_id: Option<String>,
    pub completed_by_child_id: Option<String>,
    pub completion_result_summary: Option<String>,
}

/// Status of a history item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HistoryItemStatus {
    Active,
    Completed,
    Delegated,
}

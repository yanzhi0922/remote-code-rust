//! Todo type definitions.
//!
//! Derived from `packages/types/src/todo.ts`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// TodoStatus
// ---------------------------------------------------------------------------

/// Status of a todo item.
///
/// Source: `packages/types/src/todo.ts` — `todoStatusSchema`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TodoStatus {
    Pending,
    InProgress,
    Completed,
}

// ---------------------------------------------------------------------------
// TodoItem
// ---------------------------------------------------------------------------

/// A single todo item.
///
/// Source: `packages/types/src/todo.ts` — `todoItemSchema`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoItem {
    pub id: String,
    pub content: String,
    pub status: TodoStatus,
}

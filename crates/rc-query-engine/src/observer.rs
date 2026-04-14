use anyhow::Result;
use async_trait::async_trait;
use rc_core::{Message, SessionId, ToolCall, ToolResult};
use rc_engine_events::Usage;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Checkpoint categories surfaced by the compat engine.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QueryCheckpointKind {
    ResumeBoundary,
    ToolBatch,
}

/// Durable checkpoint marker that host adapters can translate into session state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueryCheckpoint {
    pub kind: QueryCheckpointKind,
    pub session_id: SessionId,
    pub turn: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assistant_message_id: Option<Uuid>,
    #[serde(default)]
    pub tool_use_ids: Vec<String>,
    #[serde(default)]
    pub message_count: usize,
}

impl QueryCheckpoint {
    #[must_use]
    pub fn new(
        kind: QueryCheckpointKind,
        session_id: SessionId,
        turn: u32,
        assistant_message_id: Option<Uuid>,
        tool_use_ids: Vec<String>,
        message_count: usize,
    ) -> Self {
        Self {
            kind,
            session_id,
            turn,
            assistant_message_id,
            tool_use_ids,
            message_count,
        }
    }
}

/// Budget status exposed to host observers before each provider round.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct QueryBudgetState {
    pub turn: u32,
    pub total_tokens: u64,
    pub max_turns: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_total_tokens: Option<u64>,
}

/// Context-window snapshot exposed to compat observers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QueryContextBudgetState {
    pub estimated_tokens: u64,
    pub max_input_tokens: u64,
    pub threshold_tokens: u64,
    pub usage_ratio: f64,
    pub needs_compaction: bool,
}

/// Local observer event surface for host-side compat adapters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QueryObserverEvent {
    QueryStarted {
        session_id: SessionId,
        existing_messages: usize,
        new_messages: usize,
    },
    MessagesAppended {
        session_id: SessionId,
        appended: Vec<Message>,
        total_messages: usize,
    },
    BudgetEvaluated {
        budget: QueryBudgetState,
    },
    BudgetExceeded {
        budget: QueryBudgetState,
        reason: String,
    },
    ContextBudgetEvaluated {
        turn: u32,
        context: QueryContextBudgetState,
        message_count: usize,
    },
    ContextCompactionApplied {
        turn: u32,
        before_messages: usize,
        after_messages: usize,
        max_input_tokens: u64,
        threshold_tokens: u64,
        usage_ratio_before: f64,
        usage_ratio_after: f64,
        estimated_tokens_before: u64,
        estimated_tokens_after: u64,
    },
    AssistantMessageCommitted {
        message: Message,
        stop_reason: String,
        turn: u32,
        usage: Usage,
    },
    ToolCallStarted {
        tool_call: ToolCall,
        turn: u32,
        batch_size: usize,
        batch_index: usize,
    },
    ToolResultCommitted {
        tool_call: ToolCall,
        result: ToolResult,
        turn: u32,
        total_messages: usize,
    },
    CheckpointCreated {
        checkpoint: QueryCheckpoint,
    },
    CheckpointCleared {
        checkpoint: QueryCheckpoint,
    },
    QueryFinished {
        stop_reason: String,
        turns: u32,
        final_text: Option<String>,
        usage: Usage,
    },
    QueryFailed {
        error: String,
        turns: u32,
        consecutive_failures: usize,
        usage: Usage,
    },
}

impl QueryObserverEvent {
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::QueryStarted { .. } => "query_started",
            Self::MessagesAppended { .. } => "messages_appended",
            Self::BudgetEvaluated { .. } => "budget_evaluated",
            Self::BudgetExceeded { .. } => "budget_exceeded",
            Self::ContextBudgetEvaluated { .. } => "context_budget_evaluated",
            Self::ContextCompactionApplied { .. } => "context_compaction_applied",
            Self::AssistantMessageCommitted { .. } => "assistant_message_committed",
            Self::ToolCallStarted { .. } => "tool_call_started",
            Self::ToolResultCommitted { .. } => "tool_result_committed",
            Self::CheckpointCreated { .. } => "checkpoint_created",
            Self::CheckpointCleared { .. } => "checkpoint_cleared",
            Self::QueryFinished { .. } => "query_finished",
            Self::QueryFailed { .. } => "query_failed",
        }
    }
}

/// Observer seam for compat adapters that need richer lifecycle hooks than EventStream provides.
#[async_trait]
pub trait QueryObserver: Send + Sync {
    async fn on_event(&self, event: QueryObserverEvent) -> Result<()>;
}

/// Default observer used when hosts do not need local lifecycle callbacks.
#[derive(Debug, Default)]
pub struct NoopQueryObserver;

#[async_trait]
impl QueryObserver for NoopQueryObserver {
    async fn on_event(&self, _event: QueryObserverEvent) -> Result<()> {
        Ok(())
    }
}

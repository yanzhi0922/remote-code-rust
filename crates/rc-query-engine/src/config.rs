use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use rc_core::{
    AgentId, FileHistoryState, Message, PermissionMode, SessionId, ToolPermissionContext,
    ToolResult,
};
use rc_provider::ConversationBackend;
use rc_provider::context::ContextWindowManager;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::observer::{NoopQueryObserver, QueryObserver};

pub type PostCompactTransform =
    dyn Fn(Vec<rc_core::ConversationEntry>) -> Vec<rc_core::ConversationEntry> + Send + Sync;

/// Query effort hint aligned with Claude Code's runtime knobs.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EffortLevel {
    Low,
    #[default]
    Medium,
    High,
}

/// Source that initiated a query.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuerySource {
    #[default]
    User,
    Compact,
    SessionMemory,
    Agent,
}

/// Provider invocation mode for a compat query run.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProviderInvocationMode {
    #[default]
    Buffered,
    Streaming,
}

/// Thinking/extended reasoning controls.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThinkingConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub adaptive: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_tokens: Option<u32>,
}

/// Optional task budget limits injected per query.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TaskBudget {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_turns: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_total_tokens: Option<u64>,
}

/// Host-side context passed into a query run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessUserInputContext {
    pub session_id: SessionId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<AgentId>,
    #[serde(default)]
    pub permission_mode: PermissionMode,
    #[serde(default)]
    pub tool_permission_context: ToolPermissionContext,
    #[serde(default)]
    pub file_history: FileHistoryState,
    #[serde(default)]
    pub thinking_config: ThinkingConfig,
    #[serde(default)]
    pub effort: EffortLevel,
    #[serde(default)]
    pub fast_mode: bool,
    #[serde(default)]
    pub query_source: QuerySource,
    pub model: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_budget: Option<TaskBudget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mcp_instructions: Option<String>,
    #[serde(default)]
    pub discovered_skills: HashSet<String>,
}

impl ProcessUserInputContext {
    #[must_use]
    pub fn new(
        session_id: SessionId,
        permission_mode: PermissionMode,
        model: impl Into<String>,
    ) -> Self {
        Self {
            session_id,
            agent_id: None,
            permission_mode,
            tool_permission_context: ToolPermissionContext::default(),
            file_history: FileHistoryState::default(),
            thinking_config: ThinkingConfig::default(),
            effort: EffortLevel::default(),
            fast_mode: false,
            query_source: QuerySource::default(),
            model: model.into(),
            task_budget: None,
            memory_content: None,
            mcp_instructions: None,
            discovered_skills: HashSet::new(),
        }
    }
}

/// Host-provided tool execution seam for the compat query engine.
#[derive(Debug, Clone)]
pub struct ToolRunResult {
    pub result: ToolResult,
    pub pre_messages: Vec<Message>,
    pub post_messages: Vec<Message>,
    pub permission_denial: Option<Value>,
}

impl From<ToolResult> for ToolRunResult {
    fn from(result: ToolResult) -> Self {
        Self {
            result,
            pre_messages: Vec::new(),
            post_messages: Vec::new(),
            permission_denial: None,
        }
    }
}

/// Host-provided tool execution seam for the compat query engine.
#[async_trait]
pub trait ToolRunner: Send + Sync {
    async fn run_tool(
        &self,
        tool_call: &rc_core::ToolCall,
        context: &ProcessUserInputContext,
    ) -> Result<ToolRunResult>;
}

/// Immutable configuration for the compat query engine.
pub struct QueryEngineConfig {
    pub session_id: SessionId,
    pub model: String,
    pub backend: Arc<dyn ConversationBackend>,
    pub tool_runner: Arc<dyn ToolRunner>,
    pub observer: Arc<dyn QueryObserver>,
    pub event_stream: rc_engine_events::EventStream,
    pub provider_invocation_mode: ProviderInvocationMode,
    pub max_turns: u32,
    pub context_manager: ContextWindowManager,
    pub failure_threshold: usize,
    /// Maximum number of parallel tool executions.
    pub max_parallel_tools: usize,
    /// Optional JSON Schema for structured output enforcement.
    pub structured_output_schema: Option<Value>,
    /// Maximum retries for stop hooks.
    pub stop_hook_max_retries: usize,
    /// Optional fallback model for runtime model switching.
    pub fallback_model: Option<String>,
    /// Whether to enable tool result summarization.
    pub enable_tool_summarization: bool,
    /// Maximum tool result length before summarization.
    pub tool_result_max_length: usize,
    /// Maximum chain nesting depth for sub-queries.
    pub max_chain_depth: u32,
    pub post_compact_transform: Option<Arc<PostCompactTransform>>,
    #[allow(dead_code)]
    pub metadata: Value,
}

impl QueryEngineConfig {
    #[must_use]
    pub fn new(
        session_id: SessionId,
        model: impl Into<String>,
        backend: Arc<dyn ConversationBackend>,
        tool_runner: Arc<dyn ToolRunner>,
        event_stream: rc_engine_events::EventStream,
    ) -> Self {
        let model = model.into();
        Self {
            session_id,
            context_manager: ContextWindowManager::for_model(&model),
            model,
            backend,
            tool_runner,
            observer: Arc::new(NoopQueryObserver),
            event_stream,
            provider_invocation_mode: ProviderInvocationMode::Buffered,
            max_turns: 8,
            failure_threshold: 3,
            max_parallel_tools: 4,
            structured_output_schema: None,
            stop_hook_max_retries: 3,
            fallback_model: None,
            enable_tool_summarization: true,
            tool_result_max_length: 10_000,
            max_chain_depth: 4,
            post_compact_transform: None,
            metadata: Value::Null,
        }
    }

    #[must_use]
    pub fn with_observer(mut self, observer: Arc<dyn QueryObserver>) -> Self {
        self.observer = observer;
        self
    }

    #[must_use]
    pub fn with_provider_invocation_mode(mut self, mode: ProviderInvocationMode) -> Self {
        self.provider_invocation_mode = mode;
        self
    }

    #[must_use]
    pub fn with_fallback_model(mut self, model: impl Into<String>) -> Self {
        self.fallback_model = Some(model.into());
        self
    }

    #[must_use]
    pub fn with_structured_output_schema(mut self, schema: Value) -> Self {
        self.structured_output_schema = Some(schema);
        self
    }

    #[must_use]
    pub fn with_max_parallel_tools(mut self, max: usize) -> Self {
        self.max_parallel_tools = max;
        self
    }

    #[must_use]
    pub fn with_post_compact_transform(mut self, transform: Arc<PostCompactTransform>) -> Self {
        self.post_compact_transform = Some(transform);
        self
    }
}

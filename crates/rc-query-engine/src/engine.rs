use std::time::Instant;

use rc_core::{
    AssistantContentBlock, AssistantMessage, ConversationEntry, Message, MessageBase,
    MessageOrigin, SessionId, SystemMessage, SystemMessageSubtype, ToolCall, ToolUseSummaryMessage,
    UsageAccumulator,
};
use rc_engine_events::EngineEvent;
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::config::{ProcessUserInputContext, QueryEngineConfig};
use crate::query_loop::run_query_loop;
use crate::token_budget::BudgetTracker;

/// Runtime error returned by the compat query engine.
#[derive(Debug, Error)]
pub enum EngineError {
    #[error(transparent)]
    Other(#[from] anyhow::Error),
    #[error("query stopped: {0}")]
    Stopped(String),
}

/// Mutable state carried across query turns.
#[derive(Debug, Clone)]
pub struct EngineState {
    pub turn: u32,
    pub messages: Vec<Message>,
    pub usage: UsageAccumulator,
    pub budget_tracker: BudgetTracker,
    pub stop_reason: Option<String>,
    pub consecutive_failures: usize,
    pub permission_denials: Vec<Value>,
}

impl EngineState {
    #[must_use]
    pub fn new(messages: Vec<Message>, budget_tracker: BudgetTracker) -> Self {
        Self {
            turn: 0,
            messages,
            usage: UsageAccumulator::default(),
            budget_tracker,
            stop_reason: None,
            consecutive_failures: 0,
            permission_denials: Vec::new(),
        }
    }

    /// Convert the current v2 message state into the legacy provider transcript format.
    #[must_use]
    pub fn legacy_conversation(&self) -> Vec<ConversationEntry> {
        self.messages
            .iter()
            .filter_map(Message::as_conversation_entry)
            .collect()
    }

    pub(crate) fn replace_from_legacy(&mut self, conversation: &[ConversationEntry]) {
        self.messages = conversation.iter().cloned().map(Message::from).collect();
    }
}

/// Final result of a compat query engine run.
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub state: EngineState,
    pub final_text: Option<String>,
    pub stop_reason: String,
    pub turns: u32,
    pub permission_denials: Vec<Value>,
}

/// Minimal query engine that owns state/config and delegates loop execution.
pub struct QueryEngine {
    config: QueryEngineConfig,
    state: EngineState,
}

impl QueryEngine {
    #[must_use]
    pub fn new(config: QueryEngineConfig, existing_messages: Vec<Message>) -> Self {
        let budget_tracker = BudgetTracker::new(config.max_turns, None);
        Self {
            config,
            state: EngineState::new(existing_messages, budget_tracker),
        }
    }

    #[must_use]
    pub fn state(&self) -> &EngineState {
        &self.state
    }

    /// Submit new user input and execute the compat query loop to completion.
    pub async fn submit_message(
        &mut self,
        user_input: Vec<Message>,
        context: ProcessUserInputContext,
    ) -> Result<QueryResult, EngineError> {
        let started = Instant::now();
        self.config.event_stream.emit(EngineEvent::QueryStarted {
            session_id: event_session_id(&context.session_id),
        });
        let result = run_query_loop(&self.config, &mut self.state, user_input, &context).await;
        match &result {
            Ok(_) => self.config.event_stream.emit(EngineEvent::QueryCompleted {
                session_id: event_session_id(&context.session_id),
                duration_ms: started.elapsed().as_millis() as u64,
            }),
            Err(_) => self.config.event_stream.emit(EngineEvent::QueryAborted {
                session_id: event_session_id(&context.session_id),
            }),
        }
        result
    }
}

fn event_session_id(session_id: &SessionId) -> Uuid {
    session_id.try_as_uuid().unwrap_or_else(|_| Uuid::nil())
}

pub(crate) fn assistant_message_from_response(response: &rc_core::ProviderResponse) -> Message {
    let mut blocks = Vec::new();
    if !response.text.trim().is_empty() {
        blocks.push(AssistantContentBlock::Text {
            text: response.text.clone(),
        });
    }
    if let Some(thinking) = response.thinking.clone()
        && !thinking.trim().is_empty()
    {
        blocks.push(AssistantContentBlock::Thinking {
            text: thinking,
            signature: None,
        });
    }
    for tool_call in &response.tool_calls {
        blocks.push(AssistantContentBlock::ToolUse {
            id: tool_call.id.clone(),
            name: tool_call.name.clone(),
            input: tool_call.input.clone(),
        });
    }

    Message::Assistant(AssistantMessage {
        base: MessageBase::with_origin(MessageOrigin::Provider),
        text: response.text.clone(),
        blocks,
        tool_calls: response.tool_calls.clone(),
    })
}

pub(crate) fn tool_result_message(tool_call: &ToolCall, result: &rc_core::ToolResult) -> Message {
    Message::ToolUseSummary(ToolUseSummaryMessage {
        base: MessageBase::with_origin(MessageOrigin::Tool),
        tool_call_id: tool_call.id.clone(),
        tool_name: tool_call.name.clone(),
        summary: result.content.clone(),
        is_error: result.is_error,
    })
}

pub(crate) fn budget_stop_message(reason: impl Into<String>) -> Message {
    Message::System(SystemMessage {
        base: MessageBase::with_origin(MessageOrigin::System),
        subtype: SystemMessageSubtype::Informational,
        text: reason.into(),
        error: None,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use anyhow::{Result, anyhow};
    use async_trait::async_trait;
    use rc_core::{
        ConversationEntry, PermissionMode, ProviderResponse, SessionId, SubAgentCompletion,
        ToolCall, ToolResult, UsageSummary,
    };
    use rc_provider::{ConversationBackend, StreamingCallbacks};

    use super::QueryEngine;
    use crate::config::{ProcessUserInputContext, QueryEngineConfig, ToolRunner};

    struct DummyCompletion;

    #[async_trait]
    impl SubAgentCompletion for DummyCompletion {
        async fn complete(
            &self,
            _conversation: &[ConversationEntry],
        ) -> anyhow::Result<ProviderResponse> {
            Ok(ProviderResponse::default())
        }
    }

    struct MockBackend {
        responses: Mutex<VecDeque<ProviderResponse>>,
    }

    #[async_trait]
    impl ConversationBackend for MockBackend {
        async fn complete(&self, _conversation: &[ConversationEntry]) -> Result<ProviderResponse> {
            self.responses
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .pop_front()
                .ok_or_else(|| anyhow!("no more responses"))
        }

        async fn complete_streaming(
            &self,
            conversation: &[ConversationEntry],
            _callbacks: Option<StreamingCallbacks>,
        ) -> Result<ProviderResponse> {
            self.complete(conversation).await
        }

        fn sub_agent_completion(&self) -> Arc<dyn SubAgentCompletion> {
            Arc::new(DummyCompletion)
        }
    }

    struct MockToolRunner;

    #[async_trait]
    impl ToolRunner for MockToolRunner {
        async fn run_tool(
            &self,
            tool_call: &ToolCall,
            _context: &ProcessUserInputContext,
        ) -> Result<ToolResult> {
            Ok(ToolResult {
                content: format!("tool:{} ok", tool_call.name),
                is_error: false,
            })
        }
    }

    #[tokio::test]
    async fn query_engine_completes_basic_tool_round_trip() {
        let session_id = SessionId::new();
        let backend = Arc::new(MockBackend {
            responses: Mutex::new(VecDeque::from([
                ProviderResponse {
                    text: String::new(),
                    history_text: None,
                    thinking: None,
                    content_blocks: Vec::new(),
                    tool_calls: vec![ToolCall {
                        id: "tool-1".to_owned(),
                        name: "bash_command".to_owned(),
                        input: serde_json::json!({"command": "echo hi"}),
                    }],
                    usage: UsageSummary {
                        input_tokens: 10,
                        output_tokens: 5,
                        cache_read_input_tokens: 0,
                        cache_creation_input_tokens: 0,
                    },
                    stop_reason: "tool_use".to_owned(),
                },
                ProviderResponse {
                    text: "done".to_owned(),
                    history_text: None,
                    thinking: None,
                    content_blocks: Vec::new(),
                    tool_calls: Vec::new(),
                    usage: UsageSummary {
                        input_tokens: 3,
                        output_tokens: 7,
                        cache_read_input_tokens: 0,
                        cache_creation_input_tokens: 0,
                    },
                    stop_reason: "end_turn".to_owned(),
                },
            ])),
        });
        let config = QueryEngineConfig::new(
            session_id.clone(),
            "mock-model",
            backend,
            Arc::new(MockToolRunner),
            rc_engine_events::EventStream::new(64),
        );
        let mut engine = QueryEngine::new(
            config,
            vec![rc_core::Message::from(ConversationEntry::system("sys"))],
        );
        let context =
            ProcessUserInputContext::new(session_id, PermissionMode::Default, "mock-model");
        let result = engine
            .submit_message(
                vec![rc_core::Message::from(ConversationEntry::user("hello"))],
                context,
            )
            .await
            .expect("query engine should succeed");

        assert_eq!(result.final_text.as_deref(), Some("done"));
        assert_eq!(result.stop_reason, "end_turn");
        assert_eq!(result.turns, 2);
        assert_eq!(result.state.usage.input_tokens, 13);
        assert_eq!(result.state.usage.output_tokens, 12);
        assert!(
            result
                .state
                .messages
                .iter()
                .filter_map(rc_core::Message::as_conversation_entry)
                .any(|entry| entry.role == rc_core::ConversationRole::Tool)
        );
    }
}

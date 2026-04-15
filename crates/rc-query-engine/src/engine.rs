use std::time::Instant;

use rc_core::{
    AssistantContentBlock, AssistantMessage, ConversationEntry, Message, MessageBase,
    MessageOrigin, SessionId, SystemMessage, SystemMessageSubtype, ToolCall, ToolUseSummaryMessage,
    UsageAccumulator,
};
use rc_engine_events::{EngineEvent, Usage};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::config::{ProcessUserInputContext, QueryEngineConfig};
use crate::observer::QueryObserverEvent;
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
        let existing_messages = self.state.messages.len();
        let new_messages = user_input.len();
        self.config.event_stream.emit(EngineEvent::QueryStarted {
            session_id: event_session_id(&context.session_id),
        });
        let _ = self
            .config
            .observer
            .on_event(QueryObserverEvent::QueryStarted {
                session_id: context.session_id.clone(),
                existing_messages,
                new_messages,
            })
            .await;
        let result = run_query_loop(&self.config, &mut self.state, user_input, &context).await;
        match &result {
            Ok(query_result) => {
                self.config.event_stream.emit(EngineEvent::QueryCompleted {
                    session_id: event_session_id(&context.session_id),
                    duration_ms: started.elapsed().as_millis() as u64,
                });
                let _ = self
                    .config
                    .observer
                    .on_event(QueryObserverEvent::QueryFinished {
                        stop_reason: query_result.stop_reason.clone(),
                        turns: query_result.turns,
                        final_text: query_result.final_text.clone(),
                        usage: usage_from_accumulator(&query_result.state.usage),
                    })
                    .await;
            }
            Err(error) => {
                self.config.event_stream.emit(EngineEvent::QueryAborted {
                    session_id: event_session_id(&context.session_id),
                });
                let _ = self
                    .config
                    .observer
                    .on_event(QueryObserverEvent::QueryFailed {
                        error: error.to_string(),
                        turns: self.state.turn,
                        consecutive_failures: self.state.consecutive_failures,
                        usage: usage_from_accumulator(&self.state.usage),
                    })
                    .await;
            }
        }
        result
    }
}

fn event_session_id(session_id: &SessionId) -> Uuid {
    session_id.try_as_uuid().unwrap_or_else(|_| Uuid::nil())
}

pub(crate) fn usage_from_accumulator(accumulator: &UsageAccumulator) -> Usage {
    Usage {
        input_tokens: accumulator.input_tokens,
        output_tokens: accumulator.output_tokens,
        cache_creation_input_tokens: accumulator.cache_creation_input_tokens,
        cache_read_input_tokens: accumulator.cache_read_input_tokens,
        total_tokens: accumulator.total_tokens(),
    }
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
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use anyhow::{Result, anyhow};
    use async_trait::async_trait;
    use rc_core::{
        ConversationEntry, PermissionMode, ProviderResponse, SessionId, SubAgentCompletion,
        ToolCall, ToolResult, UsageSummary,
    };
    use rc_engine_events::EngineEvent;
    use rc_provider::context::ContextWindowManager;
    use rc_provider::{ConversationBackend, StreamingCallbacks};
    use tokio::sync::broadcast::Receiver;

    use super::QueryEngine;
    use crate::config::{
        ProcessUserInputContext, ProviderInvocationMode, QueryEngineConfig, ToolRunResult,
        ToolRunner,
    };
    use crate::observer::{QueryCheckpointKind, QueryObserver, QueryObserverEvent};

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

    #[derive(Debug, Clone)]
    enum MockStreamingEvent {
        TextDelta(&'static str),
        ToolCallStart(&'static str, &'static str),
        ToolCallDelta(&'static str, &'static str),
        Usage(u64, u64),
    }

    struct MockBackend {
        responses: Mutex<VecDeque<ProviderResponse>>,
        stream_scripts: Mutex<VecDeque<Vec<MockStreamingEvent>>>,
        complete_calls: AtomicUsize,
        complete_streaming_calls: AtomicUsize,
    }

    impl MockBackend {
        fn new(responses: Vec<ProviderResponse>) -> Self {
            Self {
                responses: Mutex::new(VecDeque::from(responses)),
                stream_scripts: Mutex::new(VecDeque::new()),
                complete_calls: AtomicUsize::new(0),
                complete_streaming_calls: AtomicUsize::new(0),
            }
        }

        fn with_stream_scripts(
            responses: Vec<ProviderResponse>,
            stream_scripts: Vec<Vec<MockStreamingEvent>>,
        ) -> Self {
            Self {
                responses: Mutex::new(VecDeque::from(responses)),
                stream_scripts: Mutex::new(VecDeque::from(stream_scripts)),
                complete_calls: AtomicUsize::new(0),
                complete_streaming_calls: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl ConversationBackend for MockBackend {
        async fn complete(&self, _conversation: &[ConversationEntry]) -> Result<ProviderResponse> {
            self.complete_calls.fetch_add(1, Ordering::SeqCst);
            self.responses
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .pop_front()
                .ok_or_else(|| anyhow!("no more responses"))
        }

        async fn complete_streaming(
            &self,
            conversation: &[ConversationEntry],
            callbacks: Option<StreamingCallbacks>,
        ) -> Result<ProviderResponse> {
            self.complete_streaming_calls.fetch_add(1, Ordering::SeqCst);
            if let Some(script) = self
                .stream_scripts
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .pop_front()
                && let Some(callbacks) = callbacks.as_ref()
            {
                for event in script {
                    match event {
                        MockStreamingEvent::TextDelta(delta) => {
                            if let Some(callback) = callbacks.on_text_delta.as_ref() {
                                callback(delta);
                            }
                        }
                        MockStreamingEvent::ToolCallStart(tool_call_id, tool_name) => {
                            if let Some(callback) = callbacks.on_tool_call_start.as_ref() {
                                callback(tool_call_id, tool_name);
                            }
                        }
                        MockStreamingEvent::ToolCallDelta(tool_call_id, delta) => {
                            if let Some(callback) = callbacks.on_tool_call_delta.as_ref() {
                                callback(tool_call_id, delta);
                            }
                        }
                        MockStreamingEvent::Usage(input_tokens, output_tokens) => {
                            if let Some(callback) = callbacks.on_usage.as_ref() {
                                callback(input_tokens, output_tokens);
                            }
                        }
                    }
                }
            }
            self.responses
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .pop_front()
                .ok_or_else(|| anyhow!("no more responses for streaming call {conversation:?}"))
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
        ) -> Result<ToolRunResult> {
            Ok(ToolRunResult::from(ToolResult {
                content: format!("tool:{} ok", tool_call.name),
                is_error: false,
            }))
        }
    }

    #[derive(Default)]
    struct RecordingObserver {
        events: Mutex<Vec<QueryObserverEvent>>,
    }

    impl RecordingObserver {
        fn snapshot(&self) -> Vec<QueryObserverEvent> {
            self.events
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone()
        }
    }

    #[async_trait]
    impl QueryObserver for RecordingObserver {
        async fn on_event(&self, event: QueryObserverEvent) -> Result<()> {
            self.events
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push(event);
            Ok(())
        }
    }

    fn drain_engine_events(receiver: &mut Receiver<EngineEvent>) -> Vec<EngineEvent> {
        let mut events = Vec::new();
        while let Ok(event) = receiver.try_recv() {
            events.push(event);
        }
        events
    }

    #[tokio::test]
    async fn query_engine_completes_basic_tool_round_trip() {
        let session_id = SessionId::new();
        let observer = Arc::new(RecordingObserver::default());
        let backend = Arc::new(MockBackend::new(vec![
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
                request_id: None,
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
                request_id: None,
                usage: UsageSummary {
                    input_tokens: 3,
                    output_tokens: 7,
                    cache_read_input_tokens: 0,
                    cache_creation_input_tokens: 0,
                },
                stop_reason: "end_turn".to_owned(),
            },
        ]));
        let config = QueryEngineConfig::new(
            session_id.clone(),
            "mock-model",
            backend,
            Arc::new(MockToolRunner),
            rc_engine_events::EventStream::new(64),
        )
        .with_observer(observer.clone());
        let mut engine_events = config.event_stream.subscribe();
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

        let observer_events = observer.snapshot();
        assert!(observer_events.iter().any(|event| matches!(
            event,
            QueryObserverEvent::MessagesAppended { appended, .. } if appended.len() == 1
        )));
        assert!(observer_events.iter().any(|event| matches!(
            event,
            QueryObserverEvent::AssistantMessageCommitted { stop_reason, .. }
                if stop_reason == "tool_use"
        )));
        assert!(observer_events.iter().any(|event| matches!(
            event,
            QueryObserverEvent::ToolCallStarted { tool_call, .. } if tool_call.id == "tool-1"
        )));
        assert!(observer_events.iter().any(|event| matches!(
            event,
            QueryObserverEvent::ToolResultCommitted { tool_call, result, .. }
                if tool_call.id == "tool-1" && result.content == "tool:bash_command ok"
        )));
        assert!(observer_events.iter().any(|event| matches!(
            event,
            QueryObserverEvent::CheckpointCreated { checkpoint }
                if checkpoint.kind == QueryCheckpointKind::ResumeBoundary
                    && checkpoint.tool_use_ids == vec!["tool-1".to_owned()]
        )));
        assert!(observer_events.iter().any(|event| matches!(
            event,
            QueryObserverEvent::CheckpointCleared { checkpoint }
                if checkpoint.kind == QueryCheckpointKind::ToolBatch
        )));
        assert!(observer_events.iter().any(|event| matches!(
            event,
            QueryObserverEvent::QueryFinished { stop_reason, final_text, .. }
                if stop_reason == "end_turn" && final_text.as_deref() == Some("done")
        )));

        let engine_events = drain_engine_events(&mut engine_events);
        assert!(
            engine_events
                .iter()
                .any(|event| matches!(event, EngineEvent::QueryStarted { .. }))
        );
        assert!(engine_events.iter().any(|event| matches!(
            event,
            EngineEvent::ToolUseStarted { tool_use_id, .. } if tool_use_id == "tool-1"
        )));
        assert!(
            engine_events
                .iter()
                .any(|event| matches!(event, EngineEvent::QueryCompleted { .. }))
        );
    }

    #[tokio::test]
    async fn query_engine_reports_budget_stop_to_observer_and_event_stream() {
        let session_id = SessionId::new();
        let observer = Arc::new(RecordingObserver::default());
        let backend = Arc::new(MockBackend::new(Vec::new()));
        let config = QueryEngineConfig::new(
            session_id.clone(),
            "mock-model",
            backend,
            Arc::new(MockToolRunner),
            rc_engine_events::EventStream::new(16),
        )
        .with_observer(observer.clone());
        let mut engine_events = config.event_stream.subscribe();
        let mut engine = QueryEngine::new(
            config,
            vec![rc_core::Message::from(ConversationEntry::system("sys"))],
        );
        let mut context =
            ProcessUserInputContext::new(session_id, PermissionMode::Default, "mock-model");
        context.task_budget = Some(crate::TaskBudget {
            max_turns: Some(0),
            max_total_tokens: None,
        });

        let error = engine
            .submit_message(
                vec![rc_core::Message::from(ConversationEntry::user("hello"))],
                context,
            )
            .await
            .expect_err("budget stop should abort before provider call");

        match error {
            crate::EngineError::Stopped(reason) => {
                assert_eq!(reason, "turn budget exceeded (0)");
            }
            other => panic!("unexpected error: {other:?}"),
        }

        let observer_events = observer.snapshot();
        assert!(observer_events.iter().any(|event| matches!(
            event,
            QueryObserverEvent::BudgetExceeded { reason, .. }
                if reason == "turn budget exceeded (0)"
        )));
        assert!(observer_events.iter().any(|event| matches!(
            event,
            QueryObserverEvent::QueryFailed { error, .. }
                if error == "query stopped: turn budget exceeded (0)"
        )));

        let engine_events = drain_engine_events(&mut engine_events);
        assert!(
            engine_events
                .iter()
                .any(|event| matches!(event, EngineEvent::QueryAborted { .. }))
        );
    }

    #[tokio::test]
    async fn query_engine_emits_streaming_observer_events_when_opted_in() {
        let session_id = SessionId::new();
        let observer = Arc::new(RecordingObserver::default());
        let backend = Arc::new(MockBackend::with_stream_scripts(
            vec![
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
                    request_id: None,
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
                    request_id: None,
                    usage: UsageSummary {
                        input_tokens: 2,
                        output_tokens: 4,
                        cache_read_input_tokens: 0,
                        cache_creation_input_tokens: 0,
                    },
                    stop_reason: "end_turn".to_owned(),
                },
            ],
            vec![
                vec![
                    MockStreamingEvent::ToolCallStart("tool-1", "bash_command"),
                    MockStreamingEvent::ToolCallDelta("tool-1", "{\"command\":\"echo"),
                    MockStreamingEvent::ToolCallDelta("tool-1", " hi\"}"),
                    MockStreamingEvent::Usage(10, 5),
                ],
                vec![
                    MockStreamingEvent::TextDelta("do"),
                    MockStreamingEvent::TextDelta("ne"),
                    MockStreamingEvent::Usage(2, 4),
                ],
            ],
        ));
        let config = QueryEngineConfig::new(
            session_id.clone(),
            "mock-model",
            Arc::clone(&backend) as Arc<dyn ConversationBackend>,
            Arc::new(MockToolRunner),
            rc_engine_events::EventStream::new(32),
        )
        .with_observer(observer.clone())
        .with_provider_invocation_mode(ProviderInvocationMode::Streaming);
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
            .expect("streaming query should succeed");

        assert_eq!(result.final_text.as_deref(), Some("done"));
        assert_eq!(backend.complete_calls.load(Ordering::SeqCst), 0);
        assert_eq!(backend.complete_streaming_calls.load(Ordering::SeqCst), 2);

        let observer_events = observer.snapshot();
        assert!(observer_events.iter().any(|event| matches!(
            event,
            QueryObserverEvent::StreamingToolCallStarted {
                tool_call_id,
                tool_name,
                ..
            } if tool_call_id == "tool-1" && tool_name == "bash_command"
        )));
        assert!(observer_events.iter().any(|event| matches!(
            event,
            QueryObserverEvent::StreamingToolCallDelta {
                tool_call_id,
                delta,
                ..
            } if tool_call_id == "tool-1" && delta.contains("echo")
        )));
        assert!(observer_events.iter().any(|event| matches!(
            event,
            QueryObserverEvent::StreamingTextDelta {
                delta,
                accumulated_text,
                ..
            } if delta == "ne" && accumulated_text == "done"
        )));
        assert!(observer_events.iter().any(|event| matches!(
            event,
            QueryObserverEvent::StreamingUsageUpdated { usage, .. }
                if usage.input_tokens == 2 && usage.output_tokens == 4
        )));
    }

    #[tokio::test]
    async fn query_engine_emits_compaction_events_to_observer_and_stream() {
        let session_id = SessionId::new();
        let observer = Arc::new(RecordingObserver::default());
        let backend = Arc::new(MockBackend::new(vec![ProviderResponse {
            text: "done".to_owned(),
            history_text: None,
            thinking: None,
            content_blocks: Vec::new(),
            tool_calls: Vec::new(),
            request_id: None,
            usage: UsageSummary {
                input_tokens: 1,
                output_tokens: 1,
                cache_read_input_tokens: 0,
                cache_creation_input_tokens: 0,
            },
            stop_reason: "end_turn".to_owned(),
        }]));
        let mut config = QueryEngineConfig::new(
            session_id.clone(),
            "mock-model",
            backend,
            Arc::new(MockToolRunner),
            rc_engine_events::EventStream::new(16),
        )
        .with_observer(observer.clone());
        config.context_manager = ContextWindowManager::new(100, 20);
        let mut engine_events = config.event_stream.subscribe();

        let mut existing_messages = vec![rc_core::Message::from(ConversationEntry::system("sys"))];
        for index in 0..5 {
            existing_messages.push(rc_core::Message::from(ConversationEntry::user(format!(
                "user-{index}-{}",
                "a".repeat(200)
            ))));
            existing_messages.push(rc_core::Message::from(ConversationEntry::assistant(
                format!("assistant-{index}-{}", "b".repeat(200)),
            )));
        }

        let mut engine = QueryEngine::new(config, existing_messages);
        let context =
            ProcessUserInputContext::new(session_id, PermissionMode::Default, "mock-model");
        let result = engine
            .submit_message(
                vec![rc_core::Message::from(ConversationEntry::user(format!(
                    "latest-{}",
                    "c".repeat(200)
                )))],
                context,
            )
            .await
            .expect("query engine should succeed after compaction");

        assert_eq!(result.final_text.as_deref(), Some("done"));

        let observer_events = observer.snapshot();
        assert!(observer_events.iter().any(|event| matches!(
            event,
            QueryObserverEvent::ContextBudgetEvaluated { context, .. } if context.needs_compaction
        )));
        assert!(observer_events.iter().any(|event| matches!(
            event,
            QueryObserverEvent::ContextCompactionApplied {
                before_messages,
                after_messages,
                ..
            } if before_messages > after_messages
        )));

        let engine_events = drain_engine_events(&mut engine_events);
        assert!(engine_events.iter().any(|event| matches!(
            event,
            EngineEvent::CompactStarted { strategy } if strategy == "standard"
        )));
        assert!(engine_events.iter().any(|event| matches!(
            event,
            EngineEvent::CompactCompleted { result } if result.before_messages > result.after_messages
        )));
    }
}

use anyhow::Result;
use async_trait::async_trait;
use uuid::Uuid;

use crate::state::ServerState;
use crate::ws::protocol::{AgentStatus, ServerMessage, UsagePayload};

/// A QueryObserver that broadcasts ServerMessage events to WS clients.
///
/// Translates internal `QueryObserverEvent` variants into the wire-protocol
/// `ServerMessage` format that IM adapters understand.
pub struct WsQueryObserver {
    state: ServerState,
    session_id: Uuid,
}

impl WsQueryObserver {
    pub fn new(state: ServerState, session_id: Uuid) -> Self {
        Self { state, session_id }
    }
}

#[async_trait]
impl claude_query_engine::observer::QueryObserver for WsQueryObserver {
    async fn on_event(
        &self,
        event: claude_query_engine::observer::QueryObserverEvent,
    ) -> Result<()> {
        let messages = translate_event(&event);
        for msg in messages {
            self.state.broadcast_to_session(self.session_id, msg);
        }
        Ok(())
    }
}

/// Translate a query observer event into zero or more server messages.
fn translate_event(
    event: &claude_query_engine::observer::QueryObserverEvent,
) -> Vec<ServerMessage> {
    use claude_query_engine::observer::QueryObserverEvent;

    match event {
        QueryObserverEvent::QueryStarted { .. } => vec![ServerMessage::Status {
            state: AgentStatus::Thinking,
        }],

        QueryObserverEvent::StreamingTextDelta { delta, .. } => vec![ServerMessage::ContentDelta {
            text: Some(delta.clone()),
            tool_input: None,
        }],

        QueryObserverEvent::StreamingThinkingDelta { delta, .. } => vec![ServerMessage::Thinking {
            text: delta.clone(),
        }],

        QueryObserverEvent::StreamingToolCallStarted {
            tool_name,
            tool_call_id,
            ..
        } => vec![
            ServerMessage::ContentStart {
                block_type: "tool_use".to_owned(),
                tool_name: Some(tool_name.clone()),
                tool_use_id: Some(tool_call_id.clone()),
            },
            ServerMessage::Status {
                state: AgentStatus::ToolExecuting,
            },
        ],

        QueryObserverEvent::StreamingToolCallDelta { delta, .. } => {
            vec![ServerMessage::ContentDelta {
                text: None,
                tool_input: Some(delta.clone()),
            }]
        }

        QueryObserverEvent::ToolCallStarted { tool_call, .. } => {
            vec![ServerMessage::ToolUseComplete {
                tool_name: tool_call.name.clone(),
                tool_use_id: tool_call.id.clone(),
                input: tool_call.input.clone(),
            }]
        }

        QueryObserverEvent::ToolResultCommitted {
            tool_call, result, ..
        } => vec![ServerMessage::ToolResult {
            tool_use_id: tool_call.id.clone(),
            content: format_tool_result(result),
            is_error: result.is_error,
        }],

        QueryObserverEvent::QueryFinished { usage, .. } => vec![
            ServerMessage::MessageComplete {
                usage: UsagePayload {
                    input_tokens: usage.input_tokens,
                    output_tokens: usage.output_tokens,
                },
            },
            ServerMessage::Status {
                state: AgentStatus::Idle,
            },
        ],

        QueryObserverEvent::QueryFailed { error, .. } => vec![
            ServerMessage::Error {
                message: error.clone(),
                code: "query_failed".into(),
            },
            ServerMessage::Status {
                state: AgentStatus::Idle,
            },
        ],

        // Internal events not relevant to IM adapters.
        _ => vec![],
    }
}

fn format_tool_result(result: &claude_core::ToolResult) -> String {
    result.content.clone()
}

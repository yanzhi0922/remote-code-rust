use std::sync::Arc;
use std::sync::atomic::Ordering;

use axum::extract::{Path as AxumPath, State, WebSocketUpgrade};
use axum::response::Response;
use futures::{SinkExt, StreamExt};
use uuid::Uuid;

use crate::state::ServerState;
use crate::ws::protocol::{AgentStatus, ClientMessage, ServerMessage};

/// WS upgrade handler. Registered as GET /v1/sessions/{id}/ws.
pub async fn ws_upgrade(
    ws: WebSocketUpgrade,
    State(state): State<ServerState>,
    AxumPath(session_id): AxumPath<Uuid>,
) -> Response {
    ws.on_upgrade(move |socket| handle_ws(socket, state, session_id))
}

async fn handle_ws(socket: axum::extract::ws::WebSocket, state: ServerState, session_id: Uuid) {
    // Ensure the session exists in the store.
    if state.session_store.get_session_summary(session_id).is_err() {
        let _ = state.session_store.ensure_session(
            session_id,
            &state.config.default_work_dir,
            &state.config.default_provider,
            Some(&state.config.default_model),
            None,
        );
    }

    // Subscribe to the session's broadcast channel.
    let mut rx = state.ensure_active_session(session_id);

    // Send "connected" message.
    let connected = ServerMessage::Connected { session_id };
    let socket = match serde_json::to_string(&connected) {
        Ok(text) => {
            let mut s = socket;
            if s.send(axum::extract::ws::Message::Text(text.into()))
                .await
                .is_err()
            {
                return;
            }
            s
        }
        Err(_) => return,
    };

    // Split into sink and stream for concurrent read/write.
    let (mut sink, mut stream) = socket.split();

    // Write loop: forward broadcast messages to WS.
    let write_state = state.clone();
    let write_handle = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(msg) => {
                    let text = serde_json::to_string(&msg).unwrap_or_default();
                    if sink
                        .send(axum::extract::ws::Message::Text(text.into()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    let error_msg = ServerMessage::Error {
                        message: "event lag — some events dropped".into(),
                        code: "lag".into(),
                    };
                    let text = serde_json::to_string(&error_msg).unwrap_or_default();
                    let _ = sink
                        .send(axum::extract::ws::Message::Text(text.into()))
                        .await;
                }
                Err(_) => break,
            }
        }
        drop(write_state);
    });

    // Read loop: parse client messages and dispatch.
    while let Some(Ok(msg)) = stream.next().await {
        match msg {
            axum::extract::ws::Message::Text(text) => {
                if let Ok(client_msg) = serde_json::from_str::<ClientMessage>(&text) {
                    dispatch_client_message(&state, session_id, client_msg).await;
                }
            }
            axum::extract::ws::Message::Close(_) => break,
            _ => {}
        }
    }

    write_handle.abort();
}

async fn dispatch_client_message(state: &ServerState, session_id: Uuid, msg: ClientMessage) {
    match msg {
        ClientMessage::Ping => {
            state.broadcast_to_session(session_id, ServerMessage::Pong);
        }
        ClientMessage::UserMessage { content, .. } => {
            spawn_query(state, session_id, content);
        }
        ClientMessage::StopGeneration => {
            let sessions = state.active_sessions.read();
            if let Some(active) = sessions.get(&session_id) {
                active.interrupted.store(true, Ordering::SeqCst);
            }
        }
        ClientMessage::SetRuntimeConfig {
            provider_id,
            model_id,
        } => {
            tracing::info!(
                %session_id,
                ?provider_id,
                ?model_id,
                "runtime config change requested"
            );
        }
        ClientMessage::PermissionResponse {
            request_id,
            allowed,
            ..
        } => {
            tracing::info!(%session_id, %request_id, %allowed, "permission response received");
        }
    }
}

/// Spawn a query engine run in a background task for the given user message.
fn spawn_query(state: &ServerState, session_id: Uuid, content: String) {
    use claude_core::ConversationEntry;
    use claude_core::Message;
    use claude_core::SessionId;
    use claude_query_engine::config::{
        ProcessUserInputContext, ProviderInvocationMode, QueryEngineConfig,
    };
    use claude_query_engine::engine::QueryEngine;
    use rc_engine_events::EventStream;

    // Persist user message to transcript.
    let user_entry = ConversationEntry::user(&content);
    let _ = state
        .session_store
        .append_conversation_entry(session_id, &user_entry);

    // Reset interrupt flag and abort any previous query.
    {
        let mut sessions = state.active_sessions.write();
        let active = sessions.entry(session_id).or_insert_with(|| {
            let (tx, _) = tokio::sync::broadcast::channel(state.config.event_buffer_size);
            crate::state::ActiveSession {
                event_tx: tx,
                query_task: None,
                interrupted: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            }
        });
        // Abort any previous query task.
        if let Some(prev) = active.query_task.take() {
            prev.abort();
        }
        active.interrupted.store(false, Ordering::SeqCst);
    }

    let state_clone = state.clone();
    let model = state.config.default_model.clone();
    let session_id_for_result = session_id;

    let handle = tokio::spawn(async move {
        // Broadcast: thinking.
        state_clone.broadcast_to_session(
            session_id,
            ServerMessage::Status {
                state: AgentStatus::Thinking,
            },
        );

        // Load existing conversation.
        let conversation = match state_clone.session_store.load_conversation(session_id) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(%session_id, "failed to load conversation: {e}");
                state_clone.broadcast_to_session(
                    session_id,
                    ServerMessage::Error {
                        message: format!("failed to load conversation: {e}"),
                        code: "session_error".into(),
                    },
                );
                state_clone.broadcast_to_session(
                    session_id,
                    ServerMessage::Status {
                        state: AgentStatus::Idle,
                    },
                );
                return;
            }
        };

        let existing_messages: Vec<Message> = conversation.into_iter().map(Message::from).collect();

        // Build observer that broadcasts to WS clients.
        let observer = Arc::new(crate::events::WsQueryObserver::new(
            state_clone.clone(),
            session_id,
        ));

        // Build query engine config.
        let event_stream = EventStream::new(64);
        let query_config = QueryEngineConfig::new(
            SessionId::from(session_id),
            &model,
            state_clone.backend.clone(),
            state_clone.tool_runner.clone(),
            event_stream,
        )
        .with_observer(observer)
        .with_provider_invocation_mode(ProviderInvocationMode::Streaming);

        // Create engine.
        let mut engine = QueryEngine::new(query_config, existing_messages);

        // Build user message.
        let user_message = vec![Message::from(ConversationEntry::user(&content))];
        let context = ProcessUserInputContext::new(
            SessionId::from(session_id),
            claude_core::PermissionMode::BypassPermissions,
            &model,
        );

        // Run query.
        match engine.submit_message(user_message, context).await {
            Ok(result) => {
                tracing::info!(
                    %session_id,
                    turns = result.turns,
                    stop = %result.stop_reason,
                    "query completed"
                );
            }
            Err(e) => {
                tracing::error!(%session_id, "query failed: {e}");
                state_clone.broadcast_to_session(
                    session_id,
                    ServerMessage::Error {
                        message: format!("query failed: {e}"),
                        code: "query_failed".into(),
                    },
                );
            }
        }

        // Broadcast: idle (final state).
        state_clone.broadcast_to_session(
            session_id_for_result,
            ServerMessage::Status {
                state: AgentStatus::Idle,
            },
        );

        // Clear the query task handle.
        let mut sessions = state_clone.active_sessions.write();
        if let Some(active) = sessions.get_mut(&session_id_for_result) {
            active.query_task = None;
        }
    });

    // Store the task handle so StopGeneration can abort it.
    {
        let mut sessions = state.active_sessions.write();
        if let Some(active) = sessions.get_mut(&session_id) {
            active.query_task = Some(handle);
        }
    }
}

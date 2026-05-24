use std::sync::Arc;

use futures::{SinkExt, StreamExt};
use parking_lot::Mutex;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use url::Url;
use uuid::Uuid;

use claude_server::ws::protocol::{AgentStatus, ClientMessage, ServerMessage};

/// Maximum streaming buffer size (1 MB).  Truncates oldest data when exceeded.
const MAX_STREAMING_BUFFER: usize = 1 * 1024 * 1024;

type WsSink = futures::stream::SplitSink<
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>,
    Message,
>;

/// WebSocket client connected to a single claude-server session.
///
/// Maintains a background reader task that accumulates streaming text
/// and calls the provided callback when a complete response is ready.
pub struct WsClient {
    tx: Arc<tokio::sync::Mutex<WsSink>>,
    #[allow(dead_code)]
    buffer: Arc<Mutex<String>>,
    reader_handle: tokio::task::JoinHandle<()>,
}

impl WsClient {
    /// Connect to `ws://host/v1/sessions/{session_id}/ws`.
    pub async fn connect(
        server_url: &str,
        session_id: Uuid,
        auth_token: Option<&str>,
        on_complete: tokio::sync::mpsc::Sender<(Uuid, String)>,
    ) -> anyhow::Result<Self> {
        let mut url = Url::parse(&format!("{}/v1/sessions/{}/ws", server_url, session_id))?;
        if let Some(token) = auth_token {
            url.query_pairs_mut().append_pair("token", token);
        }

        let (ws_stream, _) = connect_async(url.to_string()).await?;
        let (sink, mut stream) = ws_stream.split();
        let tx = Arc::new(tokio::sync::Mutex::new(sink));

        // Wait for Connected message.
        if let Some(Ok(Message::Text(text))) = stream.next().await {
            if let Ok(ServerMessage::Connected { .. }) = serde_json::from_str(&text) {
                tracing::info!(%session_id, "WS connected to claude-server");
            }
        }

        let buffer = Arc::new(Mutex::new(String::new()));
        let buf_clone = buffer.clone();

        let reader_handle = tokio::spawn(async move {
            while let Some(Ok(msg)) = stream.next().await {
                let text = match msg {
                    Message::Text(t) => t,
                    Message::Close(_) => break,
                    _ => continue,
                };

                let server_msg = match serde_json::from_str::<ServerMessage>(&text) {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                match server_msg {
                    ServerMessage::ContentDelta {
                        text: Some(delta), ..
                    } => {
                        let mut buf = buf_clone.lock();
                        let delta_len = delta.len();
                        if buf.len() + delta_len > MAX_STREAMING_BUFFER {
                            let excess = buf.len() + delta_len - MAX_STREAMING_BUFFER;
                            buf.drain(..excess);
                        }
                        buf.push_str(&delta);
                    }
                    ServerMessage::ContentDelta { text: None, .. } => {}
                    ServerMessage::Error { message, code } => {
                        tracing::warn!(%session_id, %code, "server error: {message}");
                        let mut buf = buf_clone.lock();
                        let err_text = format!("\n[Error: {message}]");
                        let err_len = err_text.len();
                        if buf.len() + err_len > MAX_STREAMING_BUFFER {
                            let excess = buf.len() + err_len - MAX_STREAMING_BUFFER;
                            buf.drain(..excess);
                        }
                        buf.push_str(&err_text);
                    }
                    ServerMessage::Status {
                        state: AgentStatus::Idle,
                    } => {
                        let response: String = {
                            let mut buf = buf_clone.lock();
                            let s = buf.trim().to_owned();
                            buf.clear();
                            s
                        };
                        if !response.is_empty() {
                            if on_complete.send((session_id, response)).await.is_err() {
                                break;
                            }
                        }
                    }
                    ServerMessage::Thinking { text } => {
                        let mut buf = buf_clone.lock();
                        let text_len = text.len();
                        if buf.len() + text_len > MAX_STREAMING_BUFFER {
                            let excess = buf.len() + text_len - MAX_STREAMING_BUFFER;
                            buf.drain(..excess);
                        }
                        buf.push_str(&text);
                    }
                    _ => {}
                }
            }
            tracing::info!(%session_id, "WS reader exited");
        });

        Ok(Self {
            tx,
            buffer,
            reader_handle,
        })
    }

    /// Send a user message to claude-server.
    pub async fn send_user_message(&self, content: &str) -> anyhow::Result<()> {
        let msg = ClientMessage::UserMessage {
            content: content.to_owned(),
            attachments: Vec::new(),
        };
        let text = serde_json::to_string(&msg)?;
        let mut tx = self.tx.lock().await;
        tx.send(Message::Text(text.into())).await?;
        Ok(())
    }

    /// Send stop generation signal.
    pub async fn stop_generation(&self) -> anyhow::Result<()> {
        let msg = ClientMessage::StopGeneration;
        let text = serde_json::to_string(&msg)?;
        let mut tx = self.tx.lock().await;
        tx.send(Message::Text(text.into())).await?;
        Ok(())
    }
}

impl Drop for WsClient {
    fn drop(&mut self) {
        self.reader_handle.abort();
    }
}

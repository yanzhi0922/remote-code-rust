use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use parking_lot::RwLock;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::ws_client::WsClient;
use crate::{ImResponse, ImSender};

/// Shared HTTP client for IM bridge session creation.
static BRIDGE_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn bridge_client() -> &'static reqwest::Client {
    BRIDGE_CLIENT.get_or_init(reqwest::Client::new)
}

struct SessionEntry {
    #[allow(dead_code)]
    session_id: Uuid,
    client: Arc<WsClient>,
}

/// Bridge between an IM platform and claude-server.
///
/// Manages the lifecycle of WebSocket connections: one per active IM chat.
/// Routes inbound IM messages to claude-server and dispatches responses
/// back to the IM platform via the provided [`ImSender`].
pub struct ImBridge {
    server_url: String,
    auth_token: Option<String>,
    sessions: Arc<RwLock<HashMap<String, SessionEntry>>>,
    sender: Arc<dyn ImSender>,
    response_rx: parking_lot::Mutex<Option<mpsc::Receiver<(Uuid, String)>>>,
    response_tx: mpsc::Sender<(Uuid, String)>,
    /// Maximum characters per IM message.
    pub max_message_length: usize,
}

impl ImBridge {
    pub fn new(server_url: String, auth_token: Option<String>, sender: Arc<dyn ImSender>) -> Self {
        let (response_tx, response_rx) = mpsc::channel(64);
        Self {
            server_url,
            auth_token,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            sender,
            response_rx: parking_lot::Mutex::new(Some(response_rx)),
            response_tx,
            max_message_length: 4000,
        }
    }

    /// Handle an incoming IM message: ensure session exists, forward to claude-server.
    pub async fn on_message(&self, msg: crate::ImMessage) -> anyhow::Result<()> {
        let client = self.get_or_create_session(&msg.chat_id).await?;
        client.send_user_message(&msg.text).await?;
        Ok(())
    }

    /// Take the response receiver and run the dispatch loop in a background task.
    /// The task maps session IDs back to chat IDs via the sessions map, then
    /// sends each response (chunked) to the IM platform.
    pub fn spawn_response_dispatcher(self: &Arc<Self>) {
        let rx = self.response_rx.lock().take();
        let this = self.clone();

        tokio::spawn(async move {
            let Some(mut rx) = rx else {
                tracing::error!("response dispatcher already spawned");
                return;
            };

            while let Some((session_id, response_text)) = rx.recv().await {
                let chat_id = this.chat_id_for_session(session_id);
                let Some(chat_id) = chat_id else {
                    tracing::warn!(%session_id, "response for unknown session");
                    continue;
                };

                for chunk in chunk_text(&response_text, this.max_message_length) {
                    if let Err(e) = this
                        .sender
                        .send(ImResponse {
                            chat_id: chat_id.clone(),
                            text: chunk.to_owned(),
                        })
                        .await
                    {
                        tracing::error!("failed to send IM response: {e}");
                        break;
                    }
                }
            }
        });
    }

    async fn get_or_create_session(&self, chat_id: &str) -> anyhow::Result<Arc<WsClient>> {
        {
            let sessions = self.sessions.read();
            if let Some(entry) = sessions.get(chat_id) {
                return Ok(entry.client.clone());
            }
        }

        let session_id = self.create_session().await?;
        tracing::info!(%chat_id, %session_id, "created new claude-server session");

        let client = Arc::new(
            WsClient::connect(
                &self.server_url,
                session_id,
                self.auth_token.as_deref(),
                self.response_tx.clone(),
            )
            .await?,
        );

        {
            let mut sessions = self.sessions.write();
            sessions.insert(
                chat_id.to_owned(),
                SessionEntry {
                    session_id,
                    client: client.clone(),
                },
            );
        }

        Ok(client)
    }

    async fn create_session(&self) -> anyhow::Result<Uuid> {
        let client = bridge_client();
        let mut req = client.post(format!("{}/v1/sessions", self.server_url));

        if let Some(token) = &self.auth_token {
            req = req.bearer_auth(token);
        }

        let resp: serde_json::Value = req
            .json(&serde_json::json!({}))
            .send()
            .await?
            .json()
            .await?;

        let session_id: Uuid = resp["session_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("missing session_id in create response"))?
            .parse()?;
        Ok(session_id)
    }

    fn chat_id_for_session(&self, session_id: Uuid) -> Option<String> {
        let sessions = self.sessions.read();
        for (chat_id, entry) in sessions.iter() {
            if entry.session_id == session_id {
                return Some(chat_id.clone());
            }
        }
        None
    }
}

fn chunk_text(text: &str, max_len: usize) -> Vec<&str> {
    if text.len() <= max_len {
        return vec![text];
    }

    let mut chunks = Vec::new();
    let mut start = 0;

    while start < text.len() {
        let end = (start + max_len).min(text.len());
        let break_pos = text[start..end]
            .rfind('\n')
            .map(|i| start + i + 1)
            .unwrap_or(end);
        chunks.push(&text[start..break_pos]);
        start = break_pos;
    }

    chunks
}

//! IM adapter bridge for claude-server.
//!
//! Provides shared infrastructure used by all IM adapters:
//! - [`WsClient`] — WebSocket client connecting to claude-server
//! - [`ImSessionMap`] — maps IM platform chat IDs to claude-server session UUIDs
//! - [`ImBridge`] — orchestrates message flow between IM platform and claude-server
//!
//! Each adapter binary (telegram, feishu, dingtalk, wechat) uses these primitives
//! and adds platform-specific webhook handlers and message senders.

pub mod bridge;
pub mod webhook_auth;
pub mod ws_client;

pub use bridge::ImBridge;
pub use ws_client::WsClient;

/// Generic IM platform message metadata extracted from webhook payloads.
#[derive(Debug, Clone)]
pub struct ImMessage {
    /// Platform-specific chat/group/user ID (used as session key).
    pub chat_id: String,
    /// Message text content.
    pub text: String,
    /// Display name of the sender.
    pub sender_name: Option<String>,
}

/// Response to send back to the IM platform.
#[derive(Debug, Clone)]
pub struct ImResponse {
    /// Target chat ID.
    pub chat_id: String,
    /// Text to send.
    pub text: String,
}

/// Callback trait for sending a message to the IM platform.
#[async_trait::async_trait]
pub trait ImSender: Send + Sync {
    async fn send(&self, response: ImResponse) -> anyhow::Result<()>;
}

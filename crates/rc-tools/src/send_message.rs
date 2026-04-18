//! Multi-agent message sending tools: send_message, broadcast_message.
//!
//! Provides tools for sending messages to specific agents or broadcasting
//! to all agents in the multi-agent system via the Mailbox system.

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::ToolExecutionContext;
use rc_swarm::{MailboxMessage, MailboxMessageType, mailbox};

/// Message priority levels.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum MessagePriority {
    /// Low priority (informational).
    Low,
    /// Normal priority (default).
    #[default]
    Normal,
    /// High priority (urgent).
    High,
}

/// A structured agent message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentMessage {
    /// Unique message identifier.
    pub id: String,
    /// Sender agent name.
    pub from: String,
    /// Recipient agent name (or "all" for broadcast).
    pub to: String,
    /// Message content.
    pub content: String,
    /// Message priority.
    #[serde(default)]
    pub priority: MessagePriority,
    /// Unix timestamp (milliseconds).
    pub timestamp: i64,
    /// Optional correlation ID for request/response patterns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
}

impl AgentMessage {
    /// Create a new agent message.
    #[must_use]
    pub fn new(from: &str, to: &str, content: &str) -> Self {
        Self {
            id: format!("msg-{}", uuid::Uuid::new_v4().as_simple()),
            from: from.to_string(),
            to: to.to_string(),
            content: content.to_string(),
            priority: MessagePriority::default(),
            timestamp: chrono::Utc::now().timestamp_millis(),
            correlation_id: None,
        }
    }

    /// Set the message priority.
    #[must_use]
    pub fn with_priority(mut self, priority: MessagePriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set the correlation ID.
    #[must_use]
    pub fn with_correlation_id(mut self, id: &str) -> Self {
        self.correlation_id = Some(id.to_string());
        self
    }
}

/// Send a message to a specific agent.
///
/// The message is queued for delivery via the mailbox system.
///
/// # Errors
/// Returns an error if recipient or message is missing.
pub async fn send_message(input: &Value, _context: &ToolExecutionContext) -> Result<String> {
    let recipient = input["recipient"]
        .as_str()
        .ok_or_else(|| anyhow!("recipient is required"))?;
    let message = input["message"]
        .as_str()
        .ok_or_else(|| anyhow!("message is required"))?;

    if recipient.trim().is_empty() {
        return Err(anyhow!("recipient cannot be empty"));
    }
    if message.trim().is_empty() {
        return Err(anyhow!("message cannot be empty"));
    }

    let priority = parse_priority(input["priority"].as_str());
    let correlation_id = input["correlation_id"].as_str().map(String::from);
    let sender = input["sender"].as_str().unwrap_or("coordinator");
    let explicit_team_name = super::team_runtime::team_name_from_input(input);
    let team_name =
        super::team_runtime::resolve_single_team_name(explicit_team_name.as_deref()).await?;
    let team = super::team_runtime::load_team(&team_name).await?;
    let recipient_exists = recipient == team.lead_agent_id
        || team.members.iter().any(|member| member.name == recipient);
    if !recipient_exists {
        return Err(anyhow!(
            "recipient '{recipient}' is not a member of team '{team_name}'"
        ));
    }
    ensure_sender_is_allowed(&team, sender)?;

    let mut msg = AgentMessage::new(sender, recipient, message);
    msg.priority = priority;
    msg.correlation_id = correlation_id;

    let mut mailbox_message = MailboxMessage::new(
        sender,
        recipient,
        MailboxMessageType::Text,
        message.to_owned(),
    );
    mailbox_message.priority = Some(priority_label(priority).to_owned());
    mailbox_message.correlation_id = msg.correlation_id.clone();
    mailbox::send_message(&team_name, &mailbox_message).await?;

    Ok(json!({
        "type": "agent_message",
        "id": msg.id,
        "from": msg.from,
        "to": msg.to,
        "content": msg.content,
        "priority": serde_json::to_value(msg.priority).expect("priority serializes"),
        "timestamp": msg.timestamp,
        "correlation_id": msg.correlation_id,
        "team_name": team_name,
        "status": "queued",
        "delivery": "mailbox_written"
    })
    .to_string())
}

/// Broadcast a message to all agents in the system.
///
/// The message is queued for delivery to every registered agent.
///
/// # Errors
/// Returns an error if the message is missing.
pub async fn broadcast_message(input: &Value, _context: &ToolExecutionContext) -> Result<String> {
    let message = input["message"]
        .as_str()
        .ok_or_else(|| anyhow!("message is required for broadcast"))?;

    if message.trim().is_empty() {
        return Err(anyhow!("message cannot be empty"));
    }

    let priority = parse_priority(input["priority"].as_str());
    let sender = input["sender"].as_str().unwrap_or("coordinator");

    let explicit_team_name = super::team_runtime::team_name_from_input(input);
    let team_name =
        super::team_runtime::resolve_single_team_name(explicit_team_name.as_deref()).await?;
    let team = super::team_runtime::load_team(&team_name).await?;
    ensure_sender_is_allowed(&team, sender)?;
    let known_recipients: Vec<String> = std::iter::once(team.lead_agent_id.clone())
        .chain(team.members.iter().map(|member| member.name.clone()))
        .filter(|name| name != sender)
        .collect();
    let recipients: Vec<String> = if let Some(arr) = input["recipients"].as_array() {
        let parsed = arr
            .iter()
            .filter_map(Value::as_str)
            .map(String::from)
            .collect::<Vec<_>>();
        if parsed.is_empty() {
            known_recipients.clone()
        } else {
            parsed
        }
    } else {
        known_recipients.clone()
    };

    for recipient in &recipients {
        if !known_recipients.iter().any(|known| known == recipient) {
            return Err(anyhow!(
                "recipient '{recipient}' is not a member of team '{team_name}'"
            ));
        }
    }

    let broadcast_id = format!("broadcast-{}", uuid::Uuid::new_v4().as_simple());
    let mut messages = Vec::with_capacity(recipients.len());
    for recipient in &recipients {
        let mut mailbox_message = MailboxMessage::new(
            sender,
            recipient,
            MailboxMessageType::Text,
            message.to_owned(),
        );
        mailbox_message.priority = Some(priority_label(priority).to_owned());
        mailbox::send_message(&team_name, &mailbox_message).await?;
        messages.push(mailbox_message);
    }

    Ok(json!({
        "type": "broadcast_message",
        "broadcast_id": broadcast_id,
        "team_name": team_name,
        "from": sender,
        "content": message,
        "priority": serde_json::to_value(priority).expect("priority serializes"),
        "recipients": recipients,
        "message_ids": messages.iter().map(|entry| entry.id.clone()).collect::<Vec<_>>(),
        "timestamp": chrono::Utc::now().timestamp_millis(),
        "status": "queued",
        "delivery": "mailbox_written"
    })
    .to_string())
}

/// Parse a priority string into a `MessagePriority`.
fn parse_priority(priority: Option<&str>) -> MessagePriority {
    match priority.unwrap_or("normal") {
        "low" => MessagePriority::Low,
        "high" => MessagePriority::High,
        _ => MessagePriority::Normal,
    }
}

fn priority_label(priority: MessagePriority) -> &'static str {
    match priority {
        MessagePriority::Low => "low",
        MessagePriority::Normal => "normal",
        MessagePriority::High => "high",
    }
}

fn ensure_sender_is_allowed(team: &rc_swarm::TeamFile, sender: &str) -> Result<()> {
    if sender == "coordinator"
        || sender == team.lead_agent_id
        || team
            .members
            .iter()
            .any(|member| member.name == sender || member.agent_id == sender)
    {
        Ok(())
    } else {
        Err(anyhow!(
            "sender '{sender}' is not a member of team '{}'",
            team.name
        ))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rc_swarm::{TeamFile, TeamMember, mailbox, team_helpers};
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn test_context() -> ToolExecutionContext {
        ToolExecutionContext {
            cwd: PathBuf::from("/tmp"),
            timeout_ms: 30_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Arc::new(std::sync::Mutex::new(
                rc_core::task_stack::TaskStack::default(),
            )),
        }
    }

    struct TeamDirGuard;

    impl Drop for TeamDirGuard {
        fn drop(&mut self) {
            team_helpers::set_base_dir_override(None);
        }
    }

    async fn setup_team(temp: &TempDir, team_name: &str) -> TeamDirGuard {
        team_helpers::set_base_dir_override(Some(temp.path().to_path_buf()));
        let mut team = TeamFile::new(team_name, "lead");
        team.description = Some("test objective".to_owned());
        team.members
            .push(TeamMember::new("worker-1", "agent-1", "pane-1", "/tmp"));
        team.members
            .push(TeamMember::new("worker-2", "agent-2", "pane-2", "/tmp"));
        team_helpers::create_team(&team)
            .await
            .expect("create test team");
        TeamDirGuard
    }

    #[test]
    fn message_priority_default_is_normal() {
        assert_eq!(MessagePriority::default(), MessagePriority::Normal);
    }

    #[test]
    fn message_priority_serializes() {
        assert_eq!(
            serde_json::to_string(&MessagePriority::High).expect("serialize"),
            "\"high\""
        );
        assert_eq!(
            serde_json::to_string(&MessagePriority::Low).expect("serialize"),
            "\"low\""
        );
    }

    #[test]
    fn agent_message_new_generates_id() {
        let msg = AgentMessage::new("sender", "receiver", "hello");
        assert!(msg.id.starts_with("msg-"));
        assert_eq!(msg.from, "sender");
        assert_eq!(msg.to, "receiver");
        assert_eq!(msg.content, "hello");
    }

    #[test]
    fn agent_message_with_priority() {
        let msg = AgentMessage::new("a", "b", "hi").with_priority(MessagePriority::High);
        assert_eq!(msg.priority, MessagePriority::High);
    }

    #[test]
    fn agent_message_with_correlation_id() {
        let msg = AgentMessage::new("a", "b", "hi").with_correlation_id("corr-123");
        assert_eq!(msg.correlation_id.as_deref(), Some("corr-123"));
    }

    #[test]
    fn agent_message_round_trips_json() {
        let msg = AgentMessage::new("sender", "receiver", "test message")
            .with_priority(MessagePriority::High)
            .with_correlation_id("corr-1");
        let json = serde_json::to_string(&msg).expect("serialize");
        let parsed: AgentMessage = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.from, "sender");
        assert_eq!(parsed.to, "receiver");
        assert_eq!(parsed.content, "test message");
        assert_eq!(parsed.priority, MessagePriority::High);
        assert_eq!(parsed.correlation_id.as_deref(), Some("corr-1"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn send_message_requires_recipient() {
        let input = json!({"message": "hello"});
        let context = test_context();
        let result = send_message(&input, &context).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("recipient"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn send_message_requires_message() {
        let input = json!({"recipient": "agent-1"});
        let context = test_context();
        let result = send_message(&input, &context).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("message"));
    }

    #[tokio::test(flavor = "current_thread")]
    async fn send_message_rejects_empty_recipient() {
        let input = json!({"recipient": "", "message": "hello"});
        let context = test_context();
        let result = send_message(&input, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn send_message_rejects_empty_message() {
        let input = json!({"recipient": "agent-1", "message": ""});
        let context = test_context();
        let result = send_message(&input, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn send_message_returns_queued_status() {
        let temp = TempDir::new().expect("temp dir");
        let _guard = setup_team(&temp, "test-team").await;
        let input = json!({
            "team_name": "test-team",
            "recipient": "agent-1",
            "message": "hello"
        });
        let context = test_context();
        let result = send_message(&input, &context).await.unwrap();
        let parsed: Value = serde_json::from_str(&result).expect("valid json");
        assert_eq!(parsed["status"], "queued");
        assert_eq!(parsed["to"], "agent-1");
        assert_eq!(parsed["content"], "hello");
        assert!(parsed["id"].as_str().unwrap().starts_with("msg-"));
        let stored = mailbox::read_messages("test-team", "agent-1")
            .await
            .expect("read mailbox");
        assert_eq!(stored.len(), 1);
        assert_eq!(stored[0].content, "hello");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn send_message_with_priority() {
        let temp = TempDir::new().expect("temp dir");
        let _guard = setup_team(&temp, "priority-team").await;
        let input = json!({
            "team_name": "priority-team",
            "recipient": "agent-1",
            "message": "urgent!",
            "priority": "high"
        });
        let context = test_context();
        let result = send_message(&input, &context).await.unwrap();
        let parsed: Value = serde_json::from_str(&result).expect("valid json");
        assert_eq!(parsed["priority"], "high");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn send_message_with_correlation_id() {
        let temp = TempDir::new().expect("temp dir");
        let _guard = setup_team(&temp, "correlation-team").await;
        let input = json!({
            "team_name": "correlation-team",
            "recipient": "agent-1",
            "message": "response",
            "correlation_id": "corr-123"
        });
        let context = test_context();
        let result = send_message(&input, &context).await.unwrap();
        let parsed: Value = serde_json::from_str(&result).expect("valid json");
        assert_eq!(parsed["correlation_id"], "corr-123");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn send_message_default_sender_is_coordinator() {
        let temp = TempDir::new().expect("temp dir");
        let _guard = setup_team(&temp, "default-sender-team").await;
        let input = json!({
            "team_name": "default-sender-team",
            "recipient": "agent-1",
            "message": "hello"
        });
        let context = test_context();
        let result = send_message(&input, &context).await.unwrap();
        let parsed: Value = serde_json::from_str(&result).expect("valid json");
        assert_eq!(parsed["from"], "coordinator");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn send_message_custom_sender() {
        let temp = TempDir::new().expect("temp dir");
        let _guard = setup_team(&temp, "custom-sender-team").await;
        let input = json!({
            "team_name": "custom-sender-team",
            "recipient": "agent-1",
            "message": "hello",
            "sender": "worker-1"
        });
        let context = test_context();
        let result = send_message(&input, &context).await.unwrap();
        let parsed: Value = serde_json::from_str(&result).expect("valid json");
        assert_eq!(parsed["from"], "worker-1");
    }

    #[tokio::test(flavor = "current_thread")]
    async fn broadcast_message_requires_message() {
        let input = json!({});
        let context = test_context();
        let result = broadcast_message(&input, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn broadcast_message_rejects_empty_message() {
        let input = json!({"message": ""});
        let context = test_context();
        let result = broadcast_message(&input, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn broadcast_message_returns_queued_status() {
        let temp = TempDir::new().expect("temp dir");
        let _guard = setup_team(&temp, "broadcast-team").await;
        let input = json!({"team_name": "broadcast-team", "message": "Hello everyone!"});
        let context = test_context();
        let result = broadcast_message(&input, &context).await.unwrap();
        let parsed: Value = serde_json::from_str(&result).expect("valid json");
        assert_eq!(parsed["status"], "queued");
        assert_eq!(parsed["type"], "broadcast_message");
        assert!(
            parsed["broadcast_id"]
                .as_str()
                .unwrap()
                .starts_with("broadcast-")
        );
        let agent_1 = mailbox::read_messages("broadcast-team", "agent-1")
            .await
            .expect("agent-1 mailbox");
        let agent_2 = mailbox::read_messages("broadcast-team", "agent-2")
            .await
            .expect("agent-2 mailbox");
        assert_eq!(agent_1.len(), 1);
        assert_eq!(agent_2.len(), 1);
    }

    #[tokio::test(flavor = "current_thread")]
    async fn broadcast_message_with_recipients() {
        let temp = TempDir::new().expect("temp dir");
        let _guard = setup_team(&temp, "recipient-team").await;
        let input = json!({
            "team_name": "recipient-team",
            "message": "Hello team!",
            "recipients": ["agent-1", "agent-2", "agent-3"]
        });
        let context = test_context();
        let result = broadcast_message(&input, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test(flavor = "current_thread")]
    async fn broadcast_message_with_priority() {
        let temp = TempDir::new().expect("temp dir");
        let _guard = setup_team(&temp, "priority-broadcast-team").await;
        let input = json!({
            "team_name": "priority-broadcast-team",
            "message": "Urgent broadcast!",
            "priority": "high"
        });
        let context = test_context();
        let result = broadcast_message(&input, &context).await.unwrap();
        let parsed: Value = serde_json::from_str(&result).expect("valid json");
        assert_eq!(parsed["priority"], "high");
    }

    #[test]
    fn parse_priority_handles_all_values() {
        assert_eq!(parse_priority(Some("low")), MessagePriority::Low);
        assert_eq!(parse_priority(Some("normal")), MessagePriority::Normal);
        assert_eq!(parse_priority(Some("high")), MessagePriority::High);
        assert_eq!(parse_priority(None), MessagePriority::Normal);
        assert_eq!(parse_priority(Some("invalid")), MessagePriority::Normal);
    }
}

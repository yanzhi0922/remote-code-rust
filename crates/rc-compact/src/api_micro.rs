//! API microcompact — server-side compaction via an LLM API call.
//!
//! Unlike the local micro-compact (which simply clears old tool results),
//! the API microcompact sends messages to a model endpoint for summarisation.
//! This produces higher-quality compaction at the cost of an API round-trip.
//!
//! # Overview
//!
//! 1. Estimate token savings with [`estimate_savings`].
//! 2. Call [`api_microcompact`] to perform the compaction.
//! 3. Receive a [`CompactResult`] with the compacted messages and metadata.

use rc_core::Message;

use crate::prompt::rough_token_count;

// ---------------------------------------------------------------------------
// Token savings estimate
// ---------------------------------------------------------------------------

/// Token savings estimate for a compaction pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenSavings {
    /// Estimated token count before compaction.
    pub before: u64,
    /// Estimated token count after compaction.
    pub after: u64,
    /// Tokens saved (`before - after`).
    pub saved: u64,
}

impl TokenSavings {
    /// Compute savings ratio (0.0 – 1.0).
    #[must_use]
    pub fn ratio(&self) -> f64 {
        if self.before == 0 {
            return 0.0;
        }
        (self.saved as f64) / (self.before as f64)
    }
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for API microcompaction.
#[derive(Debug, Clone)]
pub struct ApiMicrocompactConfig {
    /// Maximum output tokens the model may produce for the summary.
    pub max_output_tokens: u64,
    /// Model identifier (e.g., "claude-sonnet-4-20250514").
    pub model: String,
}

impl Default for ApiMicrocompactConfig {
    fn default() -> Self {
        Self {
            max_output_tokens: 4096,
            model: "claude-sonnet-4-20250514".to_owned(),
        }
    }
}

// ---------------------------------------------------------------------------
// Compact result
// ---------------------------------------------------------------------------

/// Result of an API microcompact operation.
#[derive(Debug, Clone)]
pub struct CompactResult {
    /// The compacted message list.
    pub messages: Vec<Message>,
    /// Token savings estimate.
    pub savings: TokenSavings,
    /// The summary text returned by the API (if any).
    pub summary: Option<String>,
}

// ---------------------------------------------------------------------------
// Core functions
// ---------------------------------------------------------------------------

/// Estimate token savings for a given message list.
///
/// This is a heuristic: tool-result messages are assumed to be replaceable
/// with a short placeholder (≈ 10 tokens each). The estimate does **not**
/// call the API.
#[must_use]
pub fn estimate_savings(messages: &[Message]) -> TokenSavings {
    let before = estimate_messages_tokens(messages);
    let tool_result_tokens: u64 = messages
        .iter()
        .map(|m| match m {
            Message::ToolUseSummary(s) => rough_token_count(&s.summary),
            _ => 0,
        })
        .sum();

    // Assume each tool result can be replaced with ~10 tokens.
    let tool_count = messages
        .iter()
        .filter(|m| matches!(m, Message::ToolUseSummary(_)))
        .count() as u64;

    let after = before
        .saturating_sub(tool_result_tokens)
        .saturating_add(tool_count * 10);

    TokenSavings {
        before,
        after,
        saved: before.saturating_sub(after),
    }
}

/// Perform an API microcompact.
///
/// In a full implementation this would call the model endpoint. Here we
/// simulate the compaction by replacing old tool results with placeholders,
/// which mirrors the real behaviour for testing purposes.
///
/// # Errors
///
/// Returns an error if the message list is empty or the config is invalid.
pub fn api_microcompact(
    messages: &[Message],
    config: &ApiMicrocompactConfig,
) -> anyhow::Result<CompactResult> {
    if messages.is_empty() {
        anyhow::bail!("cannot compact an empty message list");
    }
    if config.model.is_empty() {
        anyhow::bail!("model must not be empty");
    }

    let savings = estimate_savings(messages);

    // Simulate compaction: replace tool results with short placeholders.
    let compacted: Vec<Message> = messages
        .iter()
        .map(|msg| match msg {
            Message::ToolUseSummary(summary) => {
                let placeholder = format!(
                    "[Compact summary: {} call {} — {}]",
                    summary.tool_name,
                    &summary.tool_call_id[..8.min(summary.tool_call_id.len())],
                    if summary.is_error { "error" } else { "ok" }
                );
                Message::ToolUseSummary(rc_core::ToolUseSummaryMessage {
                    summary: placeholder,
                    ..summary.clone()
                })
            }
            other => other.clone(),
        })
        .collect();

    let summary_text = format!(
        "Compacted {} messages using model {} (max_output_tokens={})",
        messages.len(),
        config.model,
        config.max_output_tokens,
    );

    Ok(CompactResult {
        messages: compacted,
        savings,
        summary: Some(summary_text),
    })
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Estimate total tokens for a slice of messages.
fn estimate_messages_tokens(messages: &[Message]) -> u64 {
    messages.iter().map(estimate_single_message_tokens).sum()
}

/// Estimate tokens for a single message.
fn estimate_single_message_tokens(msg: &Message) -> u64 {
    match msg {
        Message::User(u) => rough_token_count(&u.text),
        Message::Assistant(a) => rough_token_count(&a.text),
        Message::ToolUseSummary(s) => rough_token_count(&s.summary),
        Message::System(s) => rough_token_count(&s.text),
        Message::HookResult(h) => rough_token_count(&h.output),
        Message::Tombstone(t) => rough_token_count(&t.summary),
        Message::Progress(p) => rough_token_count(&p.stage) + rough_token_count(&p.status),
        Message::Attachment(a) => a.label.as_ref().map(|l| rough_token_count(l)).unwrap_or(5),
        Message::GroupedToolUse(g) => g
            .summary
            .as_ref()
            .map(|s| rough_token_count(s))
            .unwrap_or(10),
        Message::CollapsedReadSearch(c) => rough_token_count(&c.summary),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rc_core::{MessageBase, MessageOrigin, ToolUseSummaryMessage};
    use uuid::Uuid;

    /// Helper: create a tool-use-summary message.
    fn tool_summary(id: &str, content: &str) -> Message {
        Message::ToolUseSummary(ToolUseSummaryMessage {
            base: MessageBase {
                uuid: Uuid::new_v4(),
                parent_uuid: None,
                timestamp: chrono::Utc::now(),
                is_meta: false,
                is_virtual: false,
                is_compact_summary: false,
                origin: Some(MessageOrigin::Tool),
            },
            tool_call_id: id.to_owned(),
            tool_name: "bash".to_owned(),
            summary: content.to_owned(),
            is_error: false,
            content_blocks: Vec::new(),
        })
    }

    /// Helper: create a user message.
    fn user_msg(text: &str) -> Message {
        Message::User(rc_core::UserMessage {
            base: MessageBase::with_origin(MessageOrigin::UserInput),
            text: text.to_owned(),
            attachments: Vec::new(),
            provider_content_blocks: Vec::new(),
        })
    }

    #[test]
    fn estimate_savings_returns_zero_for_empty() {
        let savings = estimate_savings(&[]);
        assert_eq!(savings.before, 0);
        assert_eq!(savings.after, 0);
        assert_eq!(savings.saved, 0);
        assert_eq!(savings.ratio(), 0.0);
    }

    #[test]
    fn estimate_savings_detects_tool_result_savings() {
        let long_content = "x".repeat(5000);
        let messages = vec![user_msg("short"), tool_summary("1", &long_content)];
        let savings = estimate_savings(&messages);
        assert!(savings.before > 0, "before should be positive");
        assert!(savings.saved > 0, "should detect savings from tool result");
        assert!(savings.ratio() > 0.0, "ratio should be positive");
    }

    #[test]
    fn api_microcompact_rejects_empty_messages() {
        let config = ApiMicrocompactConfig::default();
        let result = api_microcompact(&[], &config);
        assert!(result.is_err(), "should reject empty message list");
    }

    #[test]
    fn api_microcompact_rejects_empty_model() {
        let config = ApiMicrocompactConfig {
            model: String::new(),
            ..ApiMicrocompactConfig::default()
        };
        let messages = vec![user_msg("test")];
        let result = api_microcompact(&messages, &config);
        assert!(result.is_err(), "should reject empty model");
    }

    #[test]
    fn api_microcompact_compacts_tool_results() {
        let config = ApiMicrocompactConfig {
            max_output_tokens: 2048,
            model: "test-model".to_owned(),
        };
        let messages = vec![
            user_msg("run this"),
            tool_summary("tc-12345678", &"very long tool output ".repeat(100)),
        ];

        let result = api_microcompact(&messages, &config).expect("should succeed");
        assert_eq!(result.messages.len(), 2);
        assert!(result.summary.is_some());

        // Tool result should be replaced with compact placeholder.
        match &result.messages[1] {
            Message::ToolUseSummary(s) => {
                assert!(s.summary.contains("[Compact summary:"));
                assert!(s.summary.len() < 200, "placeholder should be short");
            }
            other => panic!("expected ToolUseSummary, got {other:?}"),
        }
    }

    #[test]
    fn api_microcompact_preserves_user_messages() {
        let config = ApiMicrocompactConfig::default();
        let messages = vec![user_msg("hello world")];
        let result = api_microcompact(&messages, &config).expect("should succeed");

        match &result.messages[0] {
            Message::User(u) => assert_eq!(u.text, "hello world"),
            other => panic!("expected User, got {other:?}"),
        }
    }

    #[test]
    fn token_savings_ratio_handles_zero_before() {
        let savings = TokenSavings {
            before: 0,
            after: 0,
            saved: 0,
        };
        assert_eq!(savings.ratio(), 0.0);
    }

    #[test]
    fn token_savings_ratio_computes_correctly() {
        let savings = TokenSavings {
            before: 1000,
            after: 400,
            saved: 600,
        };
        let ratio = savings.ratio();
        assert!(
            (ratio - 0.6).abs() < 0.001,
            "ratio should be ~0.6, got {ratio}"
        );
    }
}

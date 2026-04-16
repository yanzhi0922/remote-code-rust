//! Snip Compact strategy.
//!
//! Trims oversized tool outputs by replacing them with shorter placeholders.
//! Mirrors `services/compact/snipCompact.ts`.

use rc_core::Message;

use crate::prompt::rough_token_count;
use crate::strategy::{
    CompactOptions, CompactProgressEvent, CompactStrategy, CompactStrategyType, CompactionResult,
    ProgressCallback, SummaryProvider,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default maximum token length for a single tool result before snipping.
pub const DEFAULT_SNIP_THRESHOLD_TOKENS: u64 = 10_000;

/// Placeholder text used to replace snipped content.
pub const SNIPPED_CONTENT_MARKER: &str =
    "[... content snipped for length; use Read to see the full output if needed]";

// ---------------------------------------------------------------------------
// Snip compact config
// ---------------------------------------------------------------------------

/// Configuration for snip compaction.
#[derive(Debug, Clone)]
pub struct SnipCompactConfig {
    /// Maximum tokens per tool result before it gets snipped.
    pub snip_threshold_tokens: u64,
    /// Whether snip compact is enabled.
    pub enabled: bool,
}

impl Default for SnipCompactConfig {
    fn default() -> Self {
        Self {
            snip_threshold_tokens: DEFAULT_SNIP_THRESHOLD_TOKENS,
            enabled: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Snip compact strategy
// ---------------------------------------------------------------------------

/// Snip-compact strategy that trims oversized tool outputs.
#[derive(Default)]
pub struct SnipCompactStrategy {
    /// Configuration for this strategy.
    pub config: SnipCompactConfig,
}


impl SnipCompactStrategy {
    /// Create a new snip-compact strategy with custom config.
    pub fn new(config: SnipCompactConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl CompactStrategy for SnipCompactStrategy {
    fn strategy_type(&self) -> CompactStrategyType {
        CompactStrategyType::Snip
    }

    async fn compact(
        &self,
        messages: &[Message],
        _options: &CompactOptions,
        _provider: &dyn SummaryProvider,
        progress: Option<&ProgressCallback>,
    ) -> Result<CompactionResult, anyhow::Error> {
        snip_compact(messages, &self.config, progress)
    }
}

// ---------------------------------------------------------------------------
// Core snip-compact implementation
// ---------------------------------------------------------------------------

/// Perform snip compaction on the given messages.
///
/// This does **not** call the LLM — it directly trims oversized tool outputs.
/// Returns the (potentially modified) messages along with a compaction result.
pub fn snip_compact(
    messages: &[Message],
    config: &SnipCompactConfig,
    progress: Option<&ProgressCallback>,
) -> Result<CompactionResult, anyhow::Error> {
    if !config.enabled {
        return Ok(CompactionResult {
            summary: "Snip compact disabled".into(),
            messages_removed: 0,
            tokens_saved: 0,
            strategy_used: CompactStrategyType::Snip,
            preserved_segments: Vec::new(),
            pre_compact_token_count: None,
            post_compact_token_count: None,
            messages_to_keep: messages.to_vec(),
            attachments: Vec::new(),
            hook_results: Vec::new(),
            user_display_message: None,
        });
    }

    if let Some(sink) = progress {
        sink(CompactProgressEvent::Started {
            strategy: CompactStrategyType::Snip,
        });
    }

    let pre_compact_tokens = estimate_message_tokens(messages);
    let mut snipped_count: usize = 0;
    let mut tokens_saved: u64 = 0;

    let mut modified_messages: Vec<Message> = messages.to_vec();

    for msg in &mut modified_messages {
        if let Message::User(user_msg) = msg {
            let token_count = rough_token_count(&user_msg.text);
            if token_count > config.snip_threshold_tokens {
                let saved = token_count.saturating_sub(rough_token_count(SNIPPED_CONTENT_MARKER));
                user_msg.text = SNIPPED_CONTENT_MARKER.to_string();
                tokens_saved += saved;
                snipped_count += 1;
            }
        }
    }

    let post_compact_tokens = pre_compact_tokens.saturating_sub(tokens_saved);

    if let Some(sink) = progress {
        sink(CompactProgressEvent::Summarizing {
            messages_processed: snipped_count,
        });
    }

    let result = CompactionResult {
        summary: format!(
            "Snip compact: trimmed {snipped_count} oversized outputs, saved ~{tokens_saved} tokens"
        ),
        messages_removed: snipped_count,
        tokens_saved,
        strategy_used: CompactStrategyType::Snip,
        preserved_segments: Vec::new(),
        pre_compact_token_count: Some(pre_compact_tokens),
        post_compact_token_count: Some(post_compact_tokens),
        messages_to_keep: modified_messages,
        attachments: Vec::new(),
        hook_results: Vec::new(),
        user_display_message: None,
    };

    if let Some(sink) = progress {
        sink(CompactProgressEvent::Completed(result.clone()));
    }

    Ok(result)
}

/// Check if a message is a snip boundary marker.
pub fn is_snip_boundary_message(_msg: &Message) -> bool {
    // Currently snip compact doesn't create boundary messages
    false
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Estimate total tokens across all messages.
fn estimate_message_tokens(messages: &[Message]) -> u64 {
    let mut total: u64 = 0;
    for msg in messages {
        total += estimate_single_message_tokens(msg);
    }
    total
}

/// Estimate tokens for a single message.
fn estimate_single_message_tokens(msg: &Message) -> u64 {
    match msg {
        Message::User(m) => rough_token_count(&m.text),
        Message::Assistant(m) => rough_token_count(&m.text),
        Message::System(m) => rough_token_count(&m.text),
        Message::Progress(m) => rough_token_count(&m.status),
        Message::Attachment(m) => {
            let mut t = m.label.as_deref().map_or(0, rough_token_count);
            for att in &m.attachments {
                t += rough_token_count(&att.data);
            }
            t
        }
        Message::HookResult(m) => rough_token_count(&m.output),
        Message::ToolUseSummary(m) => rough_token_count(&m.summary),
        Message::Tombstone(m) => rough_token_count(&m.summary),
        Message::GroupedToolUse(m) => m.summary.as_deref().map_or(0, rough_token_count),
        Message::CollapsedReadSearch(m) => rough_token_count(&m.summary),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rc_core::{MessageBase, UserMessage};

    #[test]
    fn snip_compact_disabled() {
        let config = SnipCompactConfig {
            enabled: false,
            ..SnipCompactConfig::default()
        };
        let messages = vec![Message::User(UserMessage {
            base: MessageBase::default(),
            text: "x".repeat(100_000),
            attachments: Vec::new(),
        })];
        let result = snip_compact(&messages, &config, None).expect("should succeed");
        assert_eq!(result.messages_removed, 0);
        assert_eq!(result.tokens_saved, 0);
    }

    #[test]
    fn snip_compact_trims_long_content() {
        let config = SnipCompactConfig {
            snip_threshold_tokens: 100,
            ..SnipCompactConfig::default()
        };
        // Create a message with > 100 tokens worth of text (~400+ chars)
        let messages = vec![Message::User(UserMessage {
            base: MessageBase::default(),
            text: "x".repeat(1000),
            attachments: Vec::new(),
        })];
        let result = snip_compact(&messages, &config, None).expect("should succeed");
        assert_eq!(result.messages_removed, 1);
        assert!(result.tokens_saved > 0);
        // Check that the kept messages have been modified
        let kept = &result.messages_to_keep;
        if let Some(Message::User(u)) = kept.first() {
            assert_eq!(u.text, SNIPPED_CONTENT_MARKER);
        }
    }

    #[test]
    fn snip_compact_preserves_short_content() {
        let config = SnipCompactConfig::default();
        let messages = vec![Message::User(UserMessage {
            base: MessageBase::default(),
            text: "short".into(),
            attachments: Vec::new(),
        })];
        let result = snip_compact(&messages, &config, None).expect("should succeed");
        assert_eq!(result.messages_removed, 0);
    }
}

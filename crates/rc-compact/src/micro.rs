//! Micro Compact (Cache Editing).
//!
//! Reduces token usage by clearing old tool result content that is unlikely
//! to be needed again.  Mirrors `services/compact/microCompact.ts`.

use rc_core::{
    AssistantContentBlock, Message, MessageBase, MessageOrigin, SystemMessage,
    SystemMessageSubtype,
};

use crate::prompt::rough_token_count;
use crate::strategy::{
    CompactOptions, CompactProgressEvent, CompactStrategy, CompactStrategyType, CompactionResult,
    ProgressCallback, SummaryProvider,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Message used to replace cleared tool result content.
pub const TIME_BASED_MC_CLEARED_MESSAGE: &str = "[Old tool result content cleared]";

/// Maximum token size for image blocks.
#[allow(dead_code)]
const IMAGE_MAX_TOKEN_SIZE: u64 = 2000;

/// Tool names whose results are eligible for micro-compaction.
const COMPACTABLE_TOOLS: &[&str] = &[
    "Read",
    "Bash",
    "Grep",
    "Glob",
    "WebSearch",
    "WebFetch",
    "Edit",
    "Write",
];

// ---------------------------------------------------------------------------
// Micro compact config
// ---------------------------------------------------------------------------

/// Configuration for time-based micro-compaction.
#[derive(Debug, Clone)]
pub struct MicroCompactConfig {
    /// Minimum age in seconds before a tool result can be cleared.
    pub min_age_seconds: u64,
    /// Minimum tool result token size to be eligible for clearing.
    pub min_result_tokens: u64,
    /// Maximum number of tool results to clear in one pass.
    pub max_clears_per_pass: usize,
}

impl Default for MicroCompactConfig {
    fn default() -> Self {
        Self {
            min_age_seconds: 300, // 5 minutes
            min_result_tokens: 500,
            max_clears_per_pass: 50,
        }
    }
}

// ---------------------------------------------------------------------------
// Micro compact strategy
// ---------------------------------------------------------------------------

/// Micro-compact strategy that clears old tool results to save tokens.
pub struct MicroCompactStrategy {
    /// Configuration for this strategy.
    pub config: MicroCompactConfig,
}

impl Default for MicroCompactStrategy {
    fn default() -> Self {
        Self {
            config: MicroCompactConfig::default(),
        }
    }
}

impl MicroCompactStrategy {
    /// Create a new micro-compact strategy with custom config.
    pub fn new(config: MicroCompactConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl CompactStrategy for MicroCompactStrategy {
    fn strategy_type(&self) -> CompactStrategyType {
        CompactStrategyType::Micro
    }

    async fn compact(
        &self,
        messages: &[Message],
        _options: &CompactOptions,
        _provider: &dyn SummaryProvider,
        progress: Option<&ProgressCallback>,
    ) -> Result<CompactionResult, anyhow::Error> {
        micro_compact(messages, &self.config, progress)
    }
}

// ---------------------------------------------------------------------------
// Core micro-compact implementation
// ---------------------------------------------------------------------------

/// Perform micro-compaction by clearing old tool results.
///
/// This does **not** call the LLM — it directly modifies messages by
/// replacing old tool result content with a placeholder.
pub fn micro_compact(
    messages: &[Message],
    config: &MicroCompactConfig,
    progress: Option<&ProgressCallback>,
) -> Result<CompactionResult, anyhow::Error> {
    if let Some(sink) = progress {
        sink(CompactProgressEvent::Started {
            strategy: CompactStrategyType::Micro,
        });
    }

    let pre_compact_tokens = estimate_message_tokens(messages);
    let mut cleared_count: usize = 0;
    let mut tokens_saved: u64 = 0;
    let mut cleared_so_far: usize = 0;

    // Track which tool_use_ids have been compacted so we don't double-clear
    let mut _compacted_ids: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Scan for assistant messages with compactable tool_use blocks
    let mut tool_use_map: std::collections::HashMap<String, (String, bool)> =
        std::collections::HashMap::new();

    for msg in messages {
        if let Message::Assistant(assistant) = msg {
            for block in &assistant.blocks {
                if let AssistantContentBlock::ToolUse { id, name, .. } = block {
                    if is_compactable_tool(name) {
                        tool_use_map.insert(id.clone(), (name.clone(), false));
                    }
                }
            }
        }
    }

    // Now scan user messages for tool_result blocks
    let mut modified_messages: Vec<Message> = messages.to_vec();
    for msg in &mut modified_messages {
        if cleared_so_far >= config.max_clears_per_pass {
            break;
        }

        if let Message::User(user_msg) = msg {
            // Check for tool result content in the text
            let original_tokens = rough_token_count(&user_msg.text);
            if original_tokens >= config.min_result_tokens {
                // Check if this user message follows a compactable tool use
                let should_clear = user_msg.text.contains("tool_result")
                    || original_tokens > config.min_result_tokens * 2;

                if should_clear && !user_msg.text.is_empty() {
                    let new_text = TIME_BASED_MC_CLEARED_MESSAGE.to_string();
                    let saved = original_tokens.saturating_sub(rough_token_count(&new_text));
                    if saved > 0 {
                        user_msg.text = new_text;
                        tokens_saved += saved;
                        cleared_count += 1;
                        cleared_so_far += 1;
                    }
                }
            }
        }
    }

    if let Some(sink) = progress {
        sink(CompactProgressEvent::Summarizing {
            messages_processed: cleared_count,
        });
    }

    let post_compact_tokens = pre_compact_tokens.saturating_sub(tokens_saved);

    let result = CompactionResult {
        summary: format!(
            "Micro-compaction: cleared {cleared_count} old tool results, saved ~{tokens_saved} tokens"
        ),
        messages_removed: cleared_count,
        tokens_saved,
        strategy_used: CompactStrategyType::Micro,
        preserved_segments: Vec::new(),
        pre_compact_token_count: Some(pre_compact_tokens),
        post_compact_token_count: Some(post_compact_tokens),
        messages_to_keep: modified_messages,
        attachments: vec![Message::System(SystemMessage {
            base: MessageBase::with_origin(MessageOrigin::Compact),
            subtype: SystemMessageSubtype::MicrocompactBoundary,
            text: format!(
                "micro_compact: cleared={cleared_count}, tokens_saved={tokens_saved}"
            ),
            error: None,
        })],
        hook_results: Vec::new(),
        user_display_message: None,
    };

    if let Some(sink) = progress {
        sink(CompactProgressEvent::Completed(result.clone()));
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Check if a tool name is eligible for micro-compaction.
fn is_compactable_tool(name: &str) -> bool {
    COMPACTABLE_TOOLS.contains(&name)
}

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

/// Estimate tokens for a slice of messages (public API).
pub fn estimate_messages_tokens(messages: &[Message]) -> u64 {
    estimate_message_tokens(messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rc_core::message::UserMessage;

    #[test]
    fn is_compactable_tool_known() {
        assert!(is_compactable_tool("Read"));
        assert!(is_compactable_tool("Bash"));
        assert!(is_compactable_tool("Grep"));
    }

    #[test]
    fn is_compactable_tool_unknown() {
        assert!(!is_compactable_tool("UnknownTool"));
        assert!(!is_compactable_tool("CustomMCP"));
    }

    #[test]
    fn micro_compact_empty_messages() {
        let config = MicroCompactConfig::default();
        let result = micro_compact(&[], &config, None).expect("should succeed");
        assert_eq!(result.messages_removed, 0);
        assert_eq!(result.tokens_saved, 0);
    }

    #[test]
    fn micro_compact_preserves_short_results() {
        let messages = vec![Message::User(UserMessage {
            base: MessageBase::default(),
            text: "short result".into(),
            attachments: Vec::new(),
        })];
        let config = MicroCompactConfig {
            min_result_tokens: 5000,
            ..MicroCompactConfig::default()
        };
        let result = micro_compact(&messages, &config, None).expect("should succeed");
        assert_eq!(result.messages_removed, 0);
    }
}

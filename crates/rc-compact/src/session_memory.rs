//! Session Memory Compact strategy.
//!
//! Preserves key information (session memory) while compressing the rest of
//! the conversation.  Mirrors `services/compact/sessionMemoryCompact.ts`.

use rc_core::Message;

use crate::engine::compact_conversation;
use crate::prompt::rough_token_count;
use crate::strategy::{
    CompactOptions, CompactProgressEvent, CompactStrategy, CompactStrategyType, CompactionResult,
    ProgressCallback, SummaryProvider,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default minimum tokens to preserve after session-memory compaction.
pub const DEFAULT_SM_COMPACT_MIN_TOKENS: u64 = 10_000;

/// Default minimum number of messages with text blocks to keep.
pub const DEFAULT_SM_COMPACT_MIN_TEXT_BLOCK_MESSAGES: usize = 5;

/// Default maximum tokens to preserve after session-memory compaction.
pub const DEFAULT_SM_COMPACT_MAX_TOKENS: u64 = 40_000;

// ---------------------------------------------------------------------------
// Session memory compact config
// ---------------------------------------------------------------------------

/// Configuration for session-memory compaction thresholds.
#[derive(Debug, Clone)]
pub struct SessionMemoryCompactConfig {
    /// Minimum tokens to preserve after compaction.
    pub min_tokens: u64,
    /// Minimum number of messages with text blocks to keep.
    pub min_text_block_messages: usize,
    /// Maximum tokens to preserve after compaction (hard cap).
    pub max_tokens: u64,
}

impl Default for SessionMemoryCompactConfig {
    fn default() -> Self {
        Self {
            min_tokens: DEFAULT_SM_COMPACT_MIN_TOKENS,
            min_text_block_messages: DEFAULT_SM_COMPACT_MIN_TEXT_BLOCK_MESSAGES,
            max_tokens: DEFAULT_SM_COMPACT_MAX_TOKENS,
        }
    }
}

// ---------------------------------------------------------------------------
// Session memory compact strategy
// ---------------------------------------------------------------------------

/// Session-memory compact strategy that preserves key facts while compressing.
#[derive(Default)]
pub struct SessionMemoryCompactStrategy {
    /// Configuration for this strategy.
    pub config: SessionMemoryCompactConfig,
    /// Optional session memory content to inject into the compact prompt.
    pub session_memory_content: Option<String>,
}


impl SessionMemoryCompactStrategy {
    /// Create a new session-memory compact strategy with custom config.
    pub fn new(config: SessionMemoryCompactConfig) -> Self {
        Self {
            config,
            session_memory_content: None,
        }
    }

    /// Create with session memory content.
    pub fn with_session_memory(mut self, content: String) -> Self {
        self.session_memory_content = Some(content);
        self
    }
}

#[async_trait::async_trait]
impl CompactStrategy for SessionMemoryCompactStrategy {
    fn strategy_type(&self) -> CompactStrategyType {
        CompactStrategyType::SessionMemory
    }

    async fn compact(
        &self,
        messages: &[Message],
        options: &CompactOptions,
        provider: &dyn SummaryProvider,
        progress: Option<&ProgressCallback>,
    ) -> Result<CompactionResult, anyhow::Error> {
        session_memory_compact(
            messages,
            &self.config,
            self.session_memory_content.as_deref(),
            options,
            provider,
            progress,
        )
        .await
    }
}

// ---------------------------------------------------------------------------
// Core session-memory compact implementation
// ---------------------------------------------------------------------------

/// Perform session-memory compaction.
///
/// This strategy finds the optimal split point in the conversation that
/// preserves enough recent messages (per the config) while summarising the
/// rest.  If session memory content is available, it's injected into the
/// compact prompt as additional context.
pub async fn session_memory_compact(
    messages: &[Message],
    config: &SessionMemoryCompactConfig,
    session_memory_content: Option<&str>,
    options: &CompactOptions,
    provider: &dyn SummaryProvider,
    progress: Option<&ProgressCallback>,
) -> Result<CompactionResult, anyhow::Error> {
    if messages.is_empty() {
        return Err(anyhow::anyhow!("Not enough messages to compact."));
    }

    if let Some(sink) = progress {
        sink(CompactProgressEvent::Started {
            strategy: CompactStrategyType::SessionMemory,
        });
    }

    // Find the split point: keep enough recent messages to satisfy
    // min_text_block_messages and stay within max_tokens
    let split_index = find_split_point(messages, config);

    if split_index == 0 {
        // Nothing to summarize — all messages are kept
        return Ok(CompactionResult {
            summary: "Session-memory compact: no messages to summarize".into(),
            messages_removed: 0,
            tokens_saved: 0,
            strategy_used: CompactStrategyType::SessionMemory,
            preserved_segments: Vec::new(),
            pre_compact_token_count: None,
            post_compact_token_count: None,
            messages_to_keep: messages.to_vec(),
            attachments: Vec::new(),
            hook_results: Vec::new(),
            user_display_message: None,
        });
    }

    let messages_to_summarize: Vec<Message> =
        messages.iter().take(split_index).cloned().collect();
    let messages_to_keep: Vec<Message> = messages.iter().skip(split_index).cloned().collect();

    // Build custom instructions including session memory
    let custom_instructions = match (options.custom_instructions.as_deref(), session_memory_content)
    {
        (Some(ci), Some(sm)) => Some(format!("{ci}\n\nSession memory:\n{sm}")),
        (Some(ci), None) => Some(ci.to_string()),
        (None, Some(sm)) => Some(format!("Session memory:\n{sm}")),
        (None, None) => None,
    };

    let sm_options = CompactOptions {
        custom_instructions,
        preserve_recent_messages: config.min_text_block_messages,
        ..options.clone()
    };

    if let Some(sink) = progress {
        sink(CompactProgressEvent::Summarizing {
            messages_processed: messages_to_summarize.len(),
        });
    }

    let mut result =
        compact_conversation(&messages_to_summarize, &sm_options, provider, None).await?;
    result.strategy_used = CompactStrategyType::SessionMemory;
    result.messages_to_keep = messages_to_keep;

    if let Some(sink) = progress {
        sink(CompactProgressEvent::Completed(result.clone()));
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Find the split point in the message list.
///
/// Walks backward from the end of the messages, counting text-block messages
/// and accumulated tokens, until both thresholds are satisfied.
fn find_split_point(messages: &[Message], config: &SessionMemoryCompactConfig) -> usize {
    let total = messages.len();
    let mut text_block_count: usize = 0;
    let mut accumulated_tokens: u64 = 0;

    for i in (0..total).rev() {
        let msg = &messages[i];
        if has_text_blocks(msg) {
            text_block_count += 1;
        }
        accumulated_tokens += estimate_single_message_tokens(msg);

        // Stop when we have enough text-block messages AND tokens
        if text_block_count >= config.min_text_block_messages
            && accumulated_tokens >= config.min_tokens
        {
            // Don't exceed max_tokens — if we're over, move the split forward
            if accumulated_tokens <= config.max_tokens {
                return i;
            }
            // Otherwise keep going to find a smaller set
        }
    }

    // Always keep everything from the earliest possible point
    0
}

/// Check if a message contains text blocks (user or assistant text content).
pub fn has_text_blocks(msg: &Message) -> bool {
    match msg {
        Message::User(m) => !m.text.is_empty(),
        Message::Assistant(m) => !m.text.is_empty(),
        _ => false,
    }
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
    fn session_memory_config_default() {
        let config = SessionMemoryCompactConfig::default();
        assert_eq!(config.min_tokens, DEFAULT_SM_COMPACT_MIN_TOKENS);
        assert_eq!(
            config.min_text_block_messages,
            DEFAULT_SM_COMPACT_MIN_TEXT_BLOCK_MESSAGES
        );
        assert_eq!(config.max_tokens, DEFAULT_SM_COMPACT_MAX_TOKENS);
    }

    #[test]
    fn has_text_blocks_user() {
        let msg = Message::User(UserMessage {
            base: MessageBase::default(),
            text: "hello".into(),
            attachments: Vec::new(),
        });
        assert!(has_text_blocks(&msg));
    }

    #[test]
    fn has_text_blocks_empty() {
        let msg = Message::User(UserMessage {
            base: MessageBase::default(),
            text: String::new(),
            attachments: Vec::new(),
        });
        assert!(!has_text_blocks(&msg));
    }

    #[test]
    fn find_split_point_returns_zero_for_few_messages() {
        let messages = vec![Message::User(UserMessage {
            base: MessageBase::default(),
            text: "hello".into(),
            attachments: Vec::new(),
        })];
        let config = SessionMemoryCompactConfig {
            min_text_block_messages: 5,
            ..SessionMemoryCompactConfig::default()
        };
        assert_eq!(find_split_point(&messages, &config), 0);
    }
}

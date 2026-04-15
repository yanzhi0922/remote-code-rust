//! Compact main engine.
//!
//! Implements the [`FullCompactStrategy`] and [`PartialCompactStrategy`] that
//! orchestrate the full and partial compaction flows.  The engine accepts a
//! [`SummaryProvider`] callback to call the LLM — it does **not** depend on
//! any specific provider crate.

use rc_core::{
    Message, MessageBase, MessageOrigin, SystemMessage, SystemMessageSubtype, UserMessage,
};

use crate::prompt::{
    build_compact_prompt, build_compact_user_summary_message, build_partial_compact_prompt,
    format_compact_summary, rough_token_count, COMPACT_SYSTEM_PROMPT, PartialCompactDirection,
};
use crate::strategy::{
    CompactOptions, CompactProgressEvent, CompactStrategy, CompactStrategyType, CompactionResult,
    PreservedSegment, ProgressCallback, SummaryProvider,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Error message when there are not enough messages to compact.
pub const ERROR_MESSAGE_NOT_ENOUGH_MESSAGES: &str = "Not enough messages to compact.";

/// Error message when the conversation is too long even after retry.
pub const ERROR_MESSAGE_PROMPT_TOO_LONG: &str =
    "Conversation too long. Press esc twice to go up a few messages and try again.";

/// Error message when the user aborts the compaction.
pub const ERROR_MESSAGE_USER_ABORT: &str = "API Error: Request was aborted.";

/// Error message when the compaction response is incomplete.
pub const ERROR_MESSAGE_INCOMPLETE_RESPONSE: &str =
    "Compaction interrupted · This may be due to network issues — please try again.";

/// Maximum number of prompt-too-long retries.
const MAX_PTL_RETRIES: u32 = 3;

/// Marker inserted when truncating for a PTL retry.
const PTL_RETRY_MARKER: &str = "[earlier conversation truncated for compaction retry]";

// ---------------------------------------------------------------------------
// Full compact strategy
// ---------------------------------------------------------------------------

/// Full conversation compaction strategy.
///
/// Summarises the entire conversation (minus the most recent tail) into a
/// single summary message, then appends the preserved tail and attachments.
pub struct FullCompactStrategy;

#[async_trait::async_trait]
impl CompactStrategy for FullCompactStrategy {
    fn strategy_type(&self) -> CompactStrategyType {
        CompactStrategyType::Full
    }

    async fn compact(
        &self,
        messages: &[Message],
        options: &CompactOptions,
        provider: &dyn SummaryProvider,
        progress: Option<&ProgressCallback>,
    ) -> Result<CompactionResult, anyhow::Error> {
        compact_conversation(messages, options, provider, progress).await
    }
}

// ---------------------------------------------------------------------------
// Partial compact strategy
// ---------------------------------------------------------------------------

/// Partial compaction strategy.
///
/// Compacts only one side of a pivot point, keeping the other side intact.
pub struct PartialCompactStrategy {
    /// Index of the pivot message.
    pub pivot_index: usize,
    /// Direction: `From` = summarise after pivot, `UpTo` = summarise before pivot.
    pub direction: PartialCompactDirection,
    /// Optional user feedback to include in the compact prompt.
    pub user_feedback: Option<String>,
}

#[async_trait::async_trait]
impl CompactStrategy for PartialCompactStrategy {
    fn strategy_type(&self) -> CompactStrategyType {
        CompactStrategyType::Partial
    }

    async fn compact(
        &self,
        messages: &[Message],
        options: &CompactOptions,
        provider: &dyn SummaryProvider,
        progress: Option<&ProgressCallback>,
    ) -> Result<CompactionResult, anyhow::Error> {
        partial_compact_conversation(
            messages,
            self.pivot_index,
            self.direction,
            self.user_feedback.as_deref(),
            options,
            provider,
            progress,
        )
        .await
    }
}

// ---------------------------------------------------------------------------
// Core compact implementation
// ---------------------------------------------------------------------------

/// Perform a full compaction of the conversation.
///
/// Mirrors `compactConversation()` from the TypeScript reference.
pub async fn compact_conversation(
    messages: &[Message],
    options: &CompactOptions,
    provider: &dyn SummaryProvider,
    progress: Option<&ProgressCallback>,
) -> Result<CompactionResult, anyhow::Error> {
    if messages.is_empty() {
        return Err(anyhow::anyhow!(ERROR_MESSAGE_NOT_ENOUGH_MESSAGES));
    }

    let pre_compact_token_count = estimate_message_tokens(messages);

    emit_progress(&progress, CompactProgressEvent::Started {
        strategy: CompactStrategyType::Full,
    });

    // Build the compact prompt
    let user_prompt = build_compact_prompt(options.custom_instructions.as_deref());

    emit_progress(&progress, CompactProgressEvent::Summarizing {
        messages_processed: 0,
    });

    // Call the LLM to generate the summary
    let mut messages_to_summarize = messages.to_vec();
    let mut summary = None;
    let mut ptl_attempts = 0;

    for _ in 0..=(MAX_PTL_RETRIES + 1) {
        let result = provider
            .generate_summary(
                &messages_to_summarize,
                COMPACT_SYSTEM_PROMPT,
                &user_prompt,
            )
            .await;

        match result {
            Ok(text) => {
                if text.starts_with(ERROR_MESSAGE_PROMPT_TOO_LONG) {
                    ptl_attempts += 1;
                    if ptl_attempts <= MAX_PTL_RETRIES {
                        let truncated = truncate_head_for_ptl_retry(&messages_to_summarize);
                        if let Some(trunc) = truncated {
                            messages_to_summarize = trunc;
                            continue;
                        }
                    }
                    return Err(anyhow::anyhow!(ERROR_MESSAGE_PROMPT_TOO_LONG));
                }
                if text.is_empty() {
                    return Err(anyhow::anyhow!(
                        "Failed to generate conversation summary - response did not contain valid text content"
                    ));
                }
                summary = Some(text);
                break;
            }
            Err(e) => {
                return Err(e);
            }
        }
    }

    let summary = summary.ok_or_else(|| {
        anyhow::anyhow!(
            "Failed to generate conversation summary - response did not contain valid text content"
        )
    })?;

    emit_progress(&progress, CompactProgressEvent::Summarizing {
        messages_processed: messages.len(),
    });

    // Determine how many messages to keep from the tail
    let preserve_count = options.preserve_recent_messages.min(messages.len());
    let messages_removed = messages.len().saturating_sub(preserve_count);

    // Build the compact boundary system message
    let boundary_marker = Message::System(SystemMessage {
        base: MessageBase::with_origin(MessageOrigin::Compact),
        subtype: SystemMessageSubtype::CompactBoundary,
        text: format!(
            "compact_boundary: type={}, pre_tokens={}",
            if options.is_auto_compact {
                "auto"
            } else {
                "manual"
            },
            pre_compact_token_count,
        ),
        error: None,
    });

    // Build the summary user message
    let formatted_summary = format_compact_summary(&summary);
    let summary_text = build_compact_user_summary_message(
        &summary,
        true, // suppress follow-up questions
        None, // transcript path — caller can provide via options
        preserve_count > 0,
    );

    let summary_message = Message::User(UserMessage {
        base: {
            let mut base = MessageBase::with_origin(MessageOrigin::Compact);
            base.is_compact_summary = true;
            base
        },
        text: summary_text,
        attachments: Vec::new(),
    });

    // Estimate post-compact tokens
    let post_compact_token_count =
        rough_token_count(&formatted_summary) + preserve_count as u64 * 100;

    let tokens_saved = pre_compact_token_count.saturating_sub(post_compact_token_count);

    // Build preserved segments
    let preserved_segments = if preserve_count > 0 {
        let kept: Vec<&Message> = messages.iter().rev().take(preserve_count).collect();
        vec![PreservedSegment {
            head_uuid: kept
                .last()
                .map(|m| m.uuid())
                .unwrap_or_default(),
            anchor_uuid: summary_message.uuid(),
            tail_uuid: kept
                .first()
                .map(|m| m.uuid())
                .unwrap_or_default(),
        }]
    } else {
        Vec::new()
    };

    let result = CompactionResult {
        summary: formatted_summary,
        messages_removed,
        tokens_saved,
        strategy_used: CompactStrategyType::Full,
        preserved_segments,
        pre_compact_token_count: Some(pre_compact_token_count),
        post_compact_token_count: Some(post_compact_token_count),
        messages_to_keep: messages
            .iter()
            .rev()
            .take(preserve_count)
            .cloned()
            .collect(),
        attachments: vec![boundary_marker, summary_message],
        hook_results: Vec::new(),
        user_display_message: None,
    };

    emit_progress(&progress, CompactProgressEvent::Completed(result.clone()));

    Ok(result)
}

/// Perform a partial compaction around a pivot message.
///
/// Mirrors `partialCompactConversation()` from the TypeScript reference.
pub async fn partial_compact_conversation(
    all_messages: &[Message],
    pivot_index: usize,
    direction: PartialCompactDirection,
    user_feedback: Option<&str>,
    options: &CompactOptions,
    provider: &dyn SummaryProvider,
    progress: Option<&ProgressCallback>,
) -> Result<CompactionResult, anyhow::Error> {
    if all_messages.is_empty() {
        return Err(anyhow::anyhow!(ERROR_MESSAGE_NOT_ENOUGH_MESSAGES));
    }

    let (messages_to_summarize, messages_to_keep): (Vec<Message>, Vec<Message>) = match direction {
        PartialCompactDirection::UpTo => {
            let to_summarize: Vec<Message> = all_messages
                .iter()
                .take(pivot_index)
                .cloned()
                .collect();
            let to_keep: Vec<Message> = all_messages
                .iter()
                .skip(pivot_index)
                .filter(|m| !matches!(m, Message::Progress(_)))
                .cloned()
                .collect();
            (to_summarize, to_keep)
        }
        PartialCompactDirection::From => {
            let to_summarize: Vec<Message> = all_messages
                .iter()
                .skip(pivot_index)
                .cloned()
                .collect();
            let to_keep: Vec<Message> = all_messages
                .iter()
                .take(pivot_index)
                .filter(|m| !matches!(m, Message::Progress(_)))
                .cloned()
                .collect();
            (to_summarize, to_keep)
        }
    };

    if messages_to_summarize.is_empty() {
        return Err(anyhow::anyhow!(
            "Nothing to summarize {} the selected message.",
            match direction {
                PartialCompactDirection::UpTo => "before",
                PartialCompactDirection::From => "after",
            }
        ));
    }

    let pre_compact_token_count = estimate_message_tokens(all_messages);

    emit_progress(&progress, CompactProgressEvent::Started {
        strategy: CompactStrategyType::Partial,
    });

    // Build custom instructions from user feedback + hook instructions
    let custom_instructions = match (options.custom_instructions.as_deref(), user_feedback) {
        (Some(hook), Some(feedback)) => Some(format!("{hook}\n\nUser context: {feedback}")),
        (Some(hook), None) => Some(hook.to_string()),
        (None, Some(feedback)) => Some(format!("User context: {feedback}")),
        (None, None) => None,
    };

    let user_prompt =
        build_partial_compact_prompt(custom_instructions.as_deref(), direction);

    emit_progress(&progress, CompactProgressEvent::Summarizing {
        messages_processed: 0,
    });

    // Call the LLM
    let summary = provider
        .generate_summary(
            &messages_to_summarize,
            COMPACT_SYSTEM_PROMPT,
            &user_prompt,
        )
        .await?;

    if summary.is_empty() {
        return Err(anyhow::anyhow!(
            "Failed to generate conversation summary - response did not contain valid text content"
        ));
    }

    emit_progress(&progress, CompactProgressEvent::Summarizing {
        messages_processed: messages_to_summarize.len(),
    });

    let formatted_summary = format_compact_summary(&summary);
    let _ = build_compact_user_summary_message(
        &summary,
        false,
        None,
        !messages_to_keep.is_empty(),
    );

    let summary_message = Message::User(UserMessage {
        base: {
            let mut base = MessageBase::with_origin(MessageOrigin::Compact);
            base.is_compact_summary = true;
            base
        },
        text: formatted_summary.clone(),
        attachments: Vec::new(),
    });

    // Build boundary marker
    let boundary_marker = Message::System(SystemMessage {
        base: MessageBase::with_origin(MessageOrigin::Compact),
        subtype: SystemMessageSubtype::CompactBoundary,
        text: format!(
            "compact_boundary: type=partial, direction={}, pre_tokens={}, summarized={}, kept={}",
            match direction {
                PartialCompactDirection::From => "from",
                PartialCompactDirection::UpTo => "up_to",
            },
            pre_compact_token_count,
            messages_to_summarize.len(),
            messages_to_keep.len(),
        ),
        error: None,
    });

    let post_compact_token_count = rough_token_count(&formatted_summary)
        + estimate_message_tokens(&messages_to_keep);

    let tokens_saved = pre_compact_token_count.saturating_sub(post_compact_token_count);

    let preserved_segments = if !messages_to_keep.is_empty() {
        let anchor_uuid = match direction {
            PartialCompactDirection::UpTo => summary_message.uuid(),
            PartialCompactDirection::From => boundary_marker.uuid(),
        };
        vec![PreservedSegment {
            head_uuid: messages_to_keep
                .first()
                .map(|m| m.uuid())
                .unwrap_or_default(),
            anchor_uuid,
            tail_uuid: messages_to_keep
                .last()
                .map(|m| m.uuid())
                .unwrap_or_default(),
        }]
    } else {
        Vec::new()
    };

    let result = CompactionResult {
        summary: formatted_summary,
        messages_removed: messages_to_summarize.len(),
        tokens_saved,
        strategy_used: CompactStrategyType::Partial,
        preserved_segments,
        pre_compact_token_count: Some(pre_compact_token_count),
        post_compact_token_count: Some(post_compact_token_count),
        messages_to_keep,
        attachments: vec![boundary_marker, summary_message],
        hook_results: Vec::new(),
        user_display_message: None,
    };

    emit_progress(&progress, CompactProgressEvent::Completed(result.clone()));

    Ok(result)
}

// ---------------------------------------------------------------------------
// Build post-compact messages
// ---------------------------------------------------------------------------

/// Build the ordered list of messages that replaces the conversation after
/// compaction.
///
/// Mirrors `buildPostCompactMessages()` from the TypeScript reference.
pub fn build_post_compact_messages(result: &CompactionResult) -> Vec<Message> {
    let mut messages = Vec::new();

    // Boundary marker (first attachment)
    for msg in &result.attachments {
        if matches!(msg, Message::System(s) if s.subtype == SystemMessageSubtype::CompactBoundary) {
            messages.push(msg.clone());
            break;
        }
    }

    // Summary messages
    for msg in &result.attachments {
        if matches!(msg, Message::User(u) if u.base.is_compact_summary) {
            messages.push(msg.clone());
        }
    }

    // Preserved messages
    messages.extend(result.messages_to_keep.clone());

    // Remaining attachments (non-boundary, non-summary)
    for msg in &result.attachments {
        let is_boundary =
            matches!(msg, Message::System(s) if s.subtype == SystemMessageSubtype::CompactBoundary);
        let is_summary =
            matches!(msg, Message::User(u) if u.base.is_compact_summary);
        if !is_boundary && !is_summary {
            messages.push(msg.clone());
        }
    }

    // Hook results
    messages.extend(result.hook_results.clone());

    messages
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Emit a progress event if a sink is provided.
fn emit_progress(
    sink: &Option<&ProgressCallback>,
    event: CompactProgressEvent,
) {
    if let Some(sink) = sink {
        sink(event);
    }
}

/// Rough token estimation for a slice of messages.
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

/// Attempt to truncate the oldest messages to recover from a prompt-too-long
/// error during compaction.
///
/// Returns `None` if there's nothing to drop.
fn truncate_head_for_ptl_retry(messages: &[Message]) -> Option<Vec<Message>> {
    // Need at least 2 messages to drop something
    if messages.len() < 2 {
        return None;
    }

    // Drop the oldest 20% of messages (at least 1)
    let drop_count = std::cmp::max(1, messages.len() / 5);
    let remaining = messages.len().saturating_sub(drop_count);

    if remaining == 0 {
        return None;
    }

    let truncated: Vec<Message> = messages.iter().skip(drop_count).cloned().collect();

    // If the first message is now an assistant message, prepend a synthetic user marker
    if matches!(truncated.first(), Some(Message::Assistant(_))) {
        let mut result = vec![Message::User(UserMessage {
            base: {
                let mut base = MessageBase::with_origin(MessageOrigin::Compact);
                base.is_meta = true;
                base
            },
            text: PTL_RETRY_MARKER.to_string(),
            attachments: Vec::new(),
        })];
        result.extend(truncated);
        Some(result)
    } else {
        Some(truncated)
    }
}

/// Create a compact boundary system message.
pub fn create_compact_boundary_message(
    trigger: &str,
    pre_compact_token_count: u64,
    last_message_uuid: Option<uuid::Uuid>,
) -> Message {
    Message::System(SystemMessage {
        base: MessageBase::with_origin(MessageOrigin::Compact),
        subtype: SystemMessageSubtype::CompactBoundary,
        text: format!(
            "compact_boundary: type={trigger}, pre_tokens={pre_compact_token_count}, last_uuid={}",
            last_message_uuid
                .map(|u| u.to_string())
                .unwrap_or_default()
        ),
        error: None,
    })
}

/// Merge user-supplied custom instructions with hook-provided instructions.
pub fn merge_hook_instructions(
    user_instructions: Option<&str>,
    hook_instructions: Option<&str>,
) -> Option<String> {
    match (user_instructions, hook_instructions) {
        (Some(user), Some(hook)) => {
            let trimmed_user = user.trim();
            let trimmed_hook = hook.trim();
            if trimmed_user.is_empty() && trimmed_hook.is_empty() {
                None
            } else if trimmed_user.is_empty() {
                Some(trimmed_hook.to_string())
            } else if trimmed_hook.is_empty() {
                Some(trimmed_user.to_string())
            } else {
                Some(format!("{trimmed_user}\n\n{trimmed_hook}"))
            }
        }
        (Some(user), None) => {
            let trimmed = user.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        (None, Some(hook)) => {
            let trimmed = hook.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
        (None, None) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_hook_instructions_both() {
        let result = merge_hook_instructions(Some("user"), Some("hook"));
        assert_eq!(result.as_deref(), Some("user\n\nhook"));
    }

    #[test]
    fn merge_hook_instructions_user_only() {
        let result = merge_hook_instructions(Some("user"), None);
        assert_eq!(result.as_deref(), Some("user"));
    }

    #[test]
    fn merge_hook_instructions_empty() {
        let result = merge_hook_instructions(Some(""), Some(""));
        assert!(result.is_none());
    }

    #[test]
    fn truncate_head_for_ptl_retry_returns_none_for_single() {
        let msgs = vec![Message::User(UserMessage {
            base: MessageBase::default(),
            text: "hello".into(),
            attachments: Vec::new(),
        })];
        assert!(truncate_head_for_ptl_retry(&msgs).is_none());
    }

    #[test]
    fn build_post_compact_messages_orders_correctly() {
        let boundary = create_compact_boundary_message("manual", 1000, None);
        let summary = Message::User(UserMessage {
            base: {
                let mut b = MessageBase::default();
                b.is_compact_summary = true;
                b
            },
            text: "summary".into(),
            attachments: Vec::new(),
        });

        let result = CompactionResult {
            summary: "summary".into(),
            messages_removed: 5,
            tokens_saved: 500,
            strategy_used: CompactStrategyType::Full,
            preserved_segments: Vec::new(),
            pre_compact_token_count: Some(1000),
            post_compact_token_count: Some(500),
            messages_to_keep: vec![Message::User(UserMessage {
                base: MessageBase::default(),
                text: "kept".into(),
                attachments: Vec::new(),
            })],
            attachments: vec![boundary.clone(), summary.clone()],
            hook_results: Vec::new(),
            user_display_message: None,
        };

        let built = build_post_compact_messages(&result);
        assert!(matches!(built.first(), Some(Message::System(_))));
        assert!(
            built
                .iter()
                .any(|m| matches!(m, Message::User(u) if u.base.is_compact_summary))
        );
        assert!(
            built
                .iter()
                .any(|m| matches!(m, Message::User(u) if u.text == "kept"))
        );
    }
}

//! Context management: condensation and fallback truncation.
//!
//! Conditionally manages the conversation context when approaching limits.
//! Attempts intelligent condensation of prior messages when thresholds are reached.
//! Falls back to sliding window truncation if condensation is unavailable or fails.
//!
//! Source: `src/core/context-management/index.ts` — `manageContext`, `willManageContext`

use std::collections::HashMap;
use std::sync::Arc;

use roo_condense::{
    MAX_CONDENSE_THRESHOLD, MIN_CONDENSE_THRESHOLD, SummarizeConversationOptions,
    summarize_conversation,
};
use roo_provider::handler::{CreateMessageMetadata, Provider};
use roo_types::api::{ApiMessage, ContentBlock};

/// Fallback context window size when no provider-specific max tokens are available.
///
/// The previous value (8192) was Anthropic's completion/output limit, not a context window size.
/// When no provider info is available, use a reasonable default context window of 200K tokens.
/// This is used as the fallback for `reserved_tokens` in context management calculations.
///
/// Source: `src/core/context-management/index.ts` — uses `ANTHROPIC_DEFAULT_MAX_TOKENS` which
/// is actually Anthropic's max output tokens (8192), not the context window.
const CONTEXT_WINDOW_FALLBACK: u64 = 200_000;

use crate::TOKEN_BUFFER_PERCENTAGE;
use crate::token::estimate_token_count;
use crate::truncation::truncate_conversation;

/// Options for checking if context management will likely run.
///
/// Source: `src/core/context-management/index.ts` — `WillManageContextOptions`
#[derive(Debug, Clone)]
pub struct WillManageContextOptions {
    pub total_tokens: usize,
    pub context_window: usize,
    pub max_tokens: Option<usize>,
    pub auto_condense_context: bool,
    pub auto_condense_context_percent: f64,
    pub profile_thresholds: HashMap<String, f64>,
    pub current_profile_id: String,
    pub last_message_tokens: usize,
}

/// Options for context management (condensation and fallback truncation).
///
/// Source: `src/core/context-management/index.ts` — `ContextManagementOptions`
pub struct ContextManagementOptions {
    pub messages: Vec<ApiMessage>,
    pub total_tokens: usize,
    pub context_window: usize,
    pub max_tokens: Option<usize>,
    pub api_handler: Arc<dyn Provider>,
    pub auto_condense_context: bool,
    pub auto_condense_context_percent: f64,
    pub system_prompt: String,
    pub task_id: String,
    pub custom_condensing_prompt: Option<String>,
    pub profile_thresholds: HashMap<String, f64>,
    pub current_profile_id: String,
    pub metadata: Option<CreateMessageMetadata>,
    pub environment_details: Option<String>,
    pub files_read_by_roo: Option<Vec<String>>,
    pub cwd: Option<String>,
}

/// Result of context management.
///
/// Source: `src/core/context-management/index.ts` — `ContextManagementResult`
#[derive(Debug, Clone)]
pub struct ContextManagementResult {
    /// The messages after context management.
    pub messages: Vec<ApiMessage>,
    /// The summary text (empty if no condensation occurred).
    pub summary: String,
    /// The cost of the condensation operation.
    pub cost: f64,
    /// The token count before context management.
    pub prev_context_tokens: usize,
    /// Error message if condensation failed.
    pub error: Option<String>,
    /// Detailed error information.
    pub error_details: Option<String>,
    /// The truncation ID if truncation occurred.
    pub truncation_id: Option<String>,
    /// Number of messages removed by truncation.
    pub messages_removed: Option<usize>,
    /// Token count after truncation.
    pub new_context_tokens_after_truncation: Option<usize>,
}

/// Checks whether context management (condensation or truncation) will likely
/// run based on current token usage.
///
/// This is useful for showing UI indicators before `manage_context` is actually
/// called, without duplicating the threshold calculation logic.
///
/// Source: `src/core/context-management/index.ts` — `willManageContext`
pub fn will_manage_context(options: &WillManageContextOptions) -> bool {
    let WillManageContextOptions {
        total_tokens,
        context_window,
        max_tokens,
        auto_condense_context,
        auto_condense_context_percent,
        profile_thresholds,
        current_profile_id,
        last_message_tokens,
    } = options;

    let reserved_tokens = max_tokens.unwrap_or(CONTEXT_WINDOW_FALLBACK as usize);
    let prev_context_tokens = total_tokens + last_message_tokens;
    let allowed_tokens =
        (*context_window as f64 * (1.0 - TOKEN_BUFFER_PERCENTAGE)) as usize - reserved_tokens;

    if !auto_condense_context {
        // When auto-condense is disabled, only truncation can occur
        return prev_context_tokens > allowed_tokens;
    }

    // Determine the effective threshold to use
    let mut effective_threshold = *auto_condense_context_percent;
    if let Some(&profile_threshold) = profile_thresholds.get(current_profile_id) {
        if profile_threshold == -1.0 {
            // Special case: -1 means inherit from global setting
            effective_threshold = *auto_condense_context_percent;
        } else if (MIN_CONDENSE_THRESHOLD..=MAX_CONDENSE_THRESHOLD).contains(&profile_threshold) {
            // Valid custom threshold
            effective_threshold = profile_threshold;
        }
        // Invalid values fall back to global setting (effective_threshold already set)
    }

    let context_percent = (100.0 * prev_context_tokens as f64) / *context_window as f64;
    context_percent >= effective_threshold || prev_context_tokens > allowed_tokens
}

/// Conditionally manages conversation context (condense and fallback truncation).
///
/// Attempts intelligent condensation of prior messages when thresholds are reached.
/// Falls back to sliding window truncation if condensation is unavailable or fails.
///
/// Source: `src/core/context-management/index.ts` — `manageContext`
pub async fn manage_context(
    options: ContextManagementOptions,
) -> anyhow::Result<ContextManagementResult> {
    let ContextManagementOptions {
        messages,
        total_tokens,
        context_window,
        max_tokens,
        api_handler,
        auto_condense_context,
        auto_condense_context_percent,
        system_prompt,
        task_id,
        custom_condensing_prompt,
        profile_thresholds,
        current_profile_id,
        metadata,
        environment_details,
        files_read_by_roo,
        cwd,
    } = options;

    let mut error: Option<String> = None;
    let mut error_details: Option<String> = None;
    let mut cost = 0.0f64;

    // Calculate the maximum tokens reserved for response
    let reserved_tokens = max_tokens.unwrap_or(CONTEXT_WINDOW_FALLBACK as usize);

    // Estimate tokens for the last message (which is always a user message)
    let last_message = messages.last().expect("messages should not be empty");
    let last_message_tokens =
        estimate_token_count(&last_message.content, api_handler.as_ref()).await? as usize;

    // Calculate total effective tokens (totalTokens never includes the last message)
    let prev_context_tokens = total_tokens + last_message_tokens;

    // Calculate available tokens for conversation history
    // Truncate if we're within TOKEN_BUFFER_PERCENTAGE of the context window
    let allowed_tokens =
        (context_window as f64 * (1.0 - TOKEN_BUFFER_PERCENTAGE)) as usize - reserved_tokens;

    // Determine the effective threshold to use
    let mut effective_threshold = auto_condense_context_percent;
    if let Some(&profile_threshold) = profile_thresholds.get(&current_profile_id) {
        if profile_threshold == -1.0 {
            // Special case: -1 means inherit from global setting
            effective_threshold = auto_condense_context_percent;
        } else if (MIN_CONDENSE_THRESHOLD..=MAX_CONDENSE_THRESHOLD).contains(&profile_threshold) {
            // Valid custom threshold
            effective_threshold = profile_threshold;
        } else {
            // Invalid threshold value, fall back to global setting
            tracing::warn!(
                "Invalid profile threshold {} for profile \"{}\". Using global default of {}%",
                profile_threshold,
                current_profile_id,
                auto_condense_context_percent
            );
            effective_threshold = auto_condense_context_percent;
        }
    }

    if auto_condense_context {
        let context_percent = (100.0 * prev_context_tokens as f64) / context_window as f64;
        if context_percent >= effective_threshold || prev_context_tokens > allowed_tokens {
            // Attempt to intelligently condense the context
            let condense_options = SummarizeConversationOptions {
                messages: messages.clone(),
                api_handler: api_handler.clone(),
                system_prompt: system_prompt.clone(),
                task_id: task_id.clone(),
                is_automatic_trigger: true,
                custom_condensing_prompt: custom_condensing_prompt.clone(),
                metadata: metadata.clone(),
                environment_details: environment_details.clone(),
                files_read_by_roo: files_read_by_roo.clone(),
                cwd: cwd.clone(),
            };

            let result = summarize_conversation(condense_options).await?;
            if result.error.is_some() {
                error = result.error;
                error_details = result.error_details;
                cost = result.cost;
            } else {
                return Ok(ContextManagementResult {
                    messages: result.messages,
                    summary: result.summary,
                    cost: result.cost,
                    prev_context_tokens,
                    error: None,
                    error_details: None,
                    truncation_id: None,
                    messages_removed: None,
                    new_context_tokens_after_truncation: result
                        .new_context_tokens
                        .map(|t| t as usize),
                });
            }
        }
    }

    // Fall back to sliding window truncation if needed
    if prev_context_tokens > allowed_tokens {
        let truncation_result = truncate_conversation(&messages, 0.5, &task_id);

        // Calculate new context tokens after truncation by counting non-truncated messages
        let effective_messages: Vec<&ApiMessage> = truncation_result
            .messages
            .iter()
            .filter(|msg| {
                msg.truncation_parent.is_none() && !msg.is_truncation_marker.unwrap_or(false)
            })
            .collect();

        // Include system prompt tokens
        let system_prompt_blocks = vec![ContentBlock::Text {
            text: system_prompt.clone(),
        }];
        let mut new_context_tokens =
            estimate_token_count(&system_prompt_blocks, api_handler.as_ref()).await? as usize;

        for msg in &effective_messages {
            let msg_tokens =
                estimate_token_count(&msg.content, api_handler.as_ref()).await? as usize;
            new_context_tokens += msg_tokens;
        }

        return Ok(ContextManagementResult {
            messages: truncation_result.messages,
            prev_context_tokens,
            summary: String::new(),
            cost,
            error,
            error_details,
            truncation_id: Some(truncation_result.truncation_id),
            messages_removed: Some(truncation_result.messages_removed),
            new_context_tokens_after_truncation: Some(new_context_tokens),
        });
    }

    // No truncation or condensation needed
    Ok(ContextManagementResult {
        messages,
        summary: String::new(),
        cost,
        prev_context_tokens,
        error,
        error_details,
        truncation_id: None,
        messages_removed: None,
        new_context_tokens_after_truncation: None,
    })
}

// ===========================================================================
// getEffectiveApiHistory — filters condensed/truncated messages for API calls
// ===========================================================================

/// Filter the API conversation history to only include messages that should be
/// sent to the API, removing condensed and truncated messages.
///
/// This implements the "fresh start" model: when a summary exists, only the
/// summary and messages after it are included. Orphaned tool_result blocks
/// (referencing tool_use IDs that were condensed away) are also removed.
///
/// When no summary exists, messages with `condense_parent` or
/// `truncation_parent` pointing to existing summaries/markers are filtered out.
/// Messages with orphaned parent references (summary/marker was deleted) are
/// included.
///
/// Source: `src/core/condense/index.ts` — `getEffectiveApiHistory`
pub fn get_effective_api_history(messages: &[ApiMessage]) -> Vec<ApiMessage> {
    use roo_types::api::{ContentBlock, MessageRole};
    use std::collections::HashSet;

    // Find the most recent summary message
    let last_summary_idx = messages
        .iter()
        .rposition(|msg| msg.is_summary.unwrap_or(false));

    if let Some(summary_idx) = last_summary_idx {
        // Fresh start model: return only messages from the summary onwards
        let mut messages_from_summary: Vec<ApiMessage> = messages[summary_idx..].to_vec();

        // Collect all tool_use IDs from assistant messages in the result.
        // This is needed to filter out orphan tool_result blocks that reference
        // tool_use IDs from messages that were condensed away.
        let mut tool_use_ids: HashSet<String> = HashSet::new();
        for msg in &messages_from_summary {
            if msg.role == MessageRole::Assistant {
                for block in &msg.content {
                    if let ContentBlock::ToolUse { id, .. } = block {
                        tool_use_ids.insert(id.clone());
                    }
                }
            }
        }

        // Filter out orphan tool_result blocks from user messages
        messages_from_summary = messages_from_summary
            .into_iter()
            .filter_map(|mut msg| {
                if msg.role == MessageRole::User {
                    let original_len = msg.content.len();
                    msg.content.retain(|block| {
                        if let ContentBlock::ToolResult { tool_use_id, .. } = block {
                            tool_use_ids.contains(tool_use_id)
                        } else {
                            true
                        }
                    });
                    // If all content was filtered out, remove the message
                    if msg.content.is_empty() {
                        return None;
                    }
                    // Return updated message if content was filtered
                    let _ = original_len; // content was modified in place
                }
                Some(msg)
            })
            .collect();

        // Still need to filter out any truncated messages within this range
        let mut existing_truncation_ids: HashSet<String> = HashSet::new();
        for msg in &messages_from_summary {
            if msg.is_truncation_marker.unwrap_or(false)
                && let Some(ref tid) = msg.truncation_id
            {
                existing_truncation_ids.insert(tid.clone());
            }
        }

        messages_from_summary
            .into_iter()
            .filter(|msg| {
                // Filter out truncated messages if their truncation marker exists
                if let Some(ref parent) = msg.truncation_parent
                    && existing_truncation_ids.contains(parent)
                {
                    return false;
                }
                true
            })
            .collect()
    } else {
        // No summary - filter based on condenseParent and truncationParent.
        // This handles the case of orphaned condenseParent tags (summary was deleted via rewind).

        // Collect all condenseIds of summaries that exist in the current history
        let mut existing_summary_ids: HashSet<String> = HashSet::new();
        // Collect all truncationIds of truncation markers that exist in the current history
        let mut existing_truncation_ids: HashSet<String> = HashSet::new();

        for msg in messages {
            if msg.is_summary.unwrap_or(false)
                && let Some(ref cid) = msg.condense_id
            {
                existing_summary_ids.insert(cid.clone());
            }
            if msg.is_truncation_marker.unwrap_or(false)
                && let Some(ref tid) = msg.truncation_id
            {
                existing_truncation_ids.insert(tid.clone());
            }
        }

        // Filter out messages whose condenseParent points to an existing summary
        // or whose truncationParent points to an existing truncation marker.
        // Messages with orphaned parents (summary/marker was deleted) are included.
        messages
            .iter()
            .filter(|msg| {
                // Filter out condensed messages if their summary exists
                if let Some(ref parent) = msg.condense_parent
                    && existing_summary_ids.contains(parent)
                {
                    return false;
                }
                // Filter out truncated messages if their truncation marker exists
                if let Some(ref parent) = msg.truncation_parent
                    && existing_truncation_ids.contains(parent)
                {
                    return false;
                }
                true
            })
            .cloned()
            .collect()
    }
}

// ===========================================================================
// cleanupAfterTruncation — clear orphaned parent references
// ===========================================================================

/// Clean up orphaned `condense_parent` and `truncation_parent` references
/// after a truncation operation (rewind/delete).
///
/// When a summary message or truncation marker is deleted, messages that were
/// tagged with its ID should have their parent reference cleared so they
/// become active again.
///
/// This function should be called after any operation that truncates the API
/// history to ensure messages are properly restored when their summary or
/// truncation marker is deleted.
///
/// Source: `src/core/condense/index.ts` — `cleanupAfterTruncation`
pub fn cleanup_after_truncation(messages: &[ApiMessage]) -> Vec<ApiMessage> {
    use std::collections::HashSet;

    // Collect all condenseIds of summaries that still exist
    let mut existing_summary_ids: HashSet<String> = HashSet::new();
    // Collect all truncationIds of truncation markers that still exist
    let mut existing_truncation_ids: HashSet<String> = HashSet::new();

    for msg in messages {
        if msg.is_summary.unwrap_or(false)
            && let Some(ref cid) = msg.condense_id
        {
            existing_summary_ids.insert(cid.clone());
        }
        if msg.is_truncation_marker.unwrap_or(false)
            && let Some(ref tid) = msg.truncation_id
        {
            existing_truncation_ids.insert(tid.clone());
        }
    }

    // Clear orphaned parent references for messages whose summary or
    // truncation marker was deleted
    messages
        .iter()
        .map(|msg| {
            let mut needs_update = false;

            // Check for orphaned condenseParent
            if let Some(ref parent) = msg.condense_parent
                && !existing_summary_ids.contains(parent)
            {
                needs_update = true;
            }

            // Check for orphaned truncationParent
            if let Some(ref parent) = msg.truncation_parent
                && !existing_truncation_ids.contains(parent)
            {
                needs_update = true;
            }

            if needs_update {
                let mut updated = msg.clone();
                // Keep condenseParent only if its summary still exists
                if let Some(ref parent) = updated.condense_parent
                    && !existing_summary_ids.contains(parent)
                {
                    updated.condense_parent = None;
                }
                // Keep truncationParent only if its truncation marker still exists
                if let Some(ref parent) = updated.truncation_parent
                    && !existing_truncation_ids.contains(parent)
                {
                    updated.truncation_parent = None;
                }
                updated
            } else {
                msg.clone()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_will_manage_context_no_condense_below_threshold() {
        let options = WillManageContextOptions {
            total_tokens: 1000,
            context_window: 10000,
            max_tokens: Some(8192),
            auto_condense_context: true,
            auto_condense_context_percent: 50.0,
            profile_thresholds: HashMap::new(),
            current_profile_id: "default".to_string(),
            last_message_tokens: 100,
        };
        // prevContextTokens = 1100, allowedTokens = 10000*0.9 - 8192 = 808
        // contextPercent = 100*1100/10000 = 11%, which is < 50%
        // But prevContextTokens (1100) > allowedTokens (808)
        assert!(will_manage_context(&options));
    }

    #[test]
    fn test_will_manage_context_no_condense_disabled() {
        let options = WillManageContextOptions {
            total_tokens: 500,
            context_window: 10000,
            max_tokens: Some(8192),
            auto_condense_context: false,
            auto_condense_context_percent: 50.0,
            profile_thresholds: HashMap::new(),
            current_profile_id: "default".to_string(),
            last_message_tokens: 100,
        };
        // prevContextTokens = 600, allowedTokens = 10000*0.9 - 8192 = 808
        // 600 < 808, so no management needed
        assert!(!will_manage_context(&options));
    }

    #[test]
    fn test_will_manage_context_with_profile_threshold() {
        let mut profile_thresholds = HashMap::new();
        profile_thresholds.insert("custom".to_string(), 10.0);

        let options = WillManageContextOptions {
            total_tokens: 500,
            context_window: 10000,
            max_tokens: Some(1000),
            auto_condense_context: true,
            auto_condense_context_percent: 50.0,
            profile_thresholds,
            current_profile_id: "custom".to_string(),
            last_message_tokens: 100,
        };
        // prevContextTokens = 600, allowedTokens = 10000*0.9 - 1000 = 8000
        // contextPercent = 100*600/10000 = 6%, which is < 10% (profile threshold)
        // 600 < 8000, so no management needed
        assert!(!will_manage_context(&options));
    }

    #[test]
    fn test_will_manage_context_profile_threshold_minus_one() {
        let mut profile_thresholds = HashMap::new();
        profile_thresholds.insert("custom".to_string(), -1.0);

        let options = WillManageContextOptions {
            total_tokens: 500,
            context_window: 10000,
            max_tokens: Some(1000),
            auto_condense_context: true,
            auto_condense_context_percent: 50.0,
            profile_thresholds,
            current_profile_id: "custom".to_string(),
            last_message_tokens: 100,
        };
        // -1 means inherit from global: effective_threshold = 50.0
        // contextPercent = 6% < 50%, 600 < 8000
        assert!(!will_manage_context(&options));
    }

    // ---- get_effective_api_history tests ----

    use roo_types::api::{ContentBlock, MessageRole};

    fn make_msg(role: MessageRole, text: &str) -> ApiMessage {
        ApiMessage {
            role,
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
            reasoning: None,
            ts: None,
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        }
    }

    fn make_summary_msg(text: &str, condense_id: &str) -> ApiMessage {
        ApiMessage {
            role: MessageRole::User,
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
            reasoning: None,
            ts: None,
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: Some(true),
            condense_id: Some(condense_id.to_string()),
            reasoning_details: None,
        }
    }

    fn make_condensed_msg(role: MessageRole, text: &str, condense_parent: &str) -> ApiMessage {
        ApiMessage {
            role,
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
            reasoning: None,
            ts: None,
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: Some(condense_parent.to_string()),
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        }
    }

    #[test]
    fn test_get_effective_api_history_no_summary() {
        // No summary → all messages are included
        let messages = vec![
            make_msg(MessageRole::User, "hello"),
            make_msg(MessageRole::Assistant, "hi"),
            make_msg(MessageRole::User, "how are you"),
        ];
        let effective = get_effective_api_history(&messages);
        assert_eq!(effective.len(), 3);
    }

    #[test]
    fn test_get_effective_api_history_with_summary_fresh_start() {
        // Summary exists → only summary and messages after it
        let messages = vec![
            make_msg(MessageRole::User, "old message 1"),
            make_msg(MessageRole::Assistant, "old response 1"),
            make_summary_msg("Summary of conversation", "condense-1"),
            make_msg(MessageRole::User, "new message"),
            make_msg(MessageRole::Assistant, "new response"),
        ];
        let effective = get_effective_api_history(&messages);
        // Should contain: summary, new message, new response (3)
        assert_eq!(effective.len(), 3);
        assert_eq!(effective[0].is_summary, Some(true));
    }

    #[test]
    fn test_get_effective_api_history_filters_condensed() {
        // Messages with condense_parent pointing to existing summary are filtered
        let messages = vec![
            make_condensed_msg(MessageRole::User, "condensed msg", "condense-1"),
            make_summary_msg("Summary", "condense-1"),
            make_msg(MessageRole::User, "after summary"),
        ];
        let effective = get_effective_api_history(&messages);
        // Condensed msg is filtered (its condense_parent "condense-1" matches summary's condense_id)
        // Summary + after summary = 2
        assert_eq!(effective.len(), 2);
    }

    #[test]
    fn test_get_effective_api_history_orphaned_condense_parent_included() {
        // Messages with condense_parent pointing to non-existent summary are included
        let messages = vec![
            make_condensed_msg(MessageRole::User, "orphaned msg", "deleted-condense"),
            make_msg(MessageRole::User, "normal msg"),
        ];
        let effective = get_effective_api_history(&messages);
        // Both messages included because "deleted-condense" has no matching summary
        assert_eq!(effective.len(), 2);
    }

    #[test]
    fn test_get_effective_api_history_empty() {
        let effective = get_effective_api_history(&[]);
        assert!(effective.is_empty());
    }

    #[test]
    fn test_get_effective_api_history_filters_orphan_tool_results() {
        // When summary exists, tool_results referencing tool_use IDs before the
        // summary should be filtered out.
        let messages = vec![
            // These are before the summary, but included in fresh-start slice
            ApiMessage {
                role: MessageRole::Assistant,
                content: vec![ContentBlock::ToolUse {
                    id: "tool-1".to_string(),
                    name: "read_file".to_string(),
                    input: serde_json::json!({}),
                }],
                reasoning: None,
                ts: None,
                truncation_parent: None,
                is_truncation_marker: None,
                truncation_id: None,
                condense_parent: None,
                is_summary: Some(true), // This is the summary message
                condense_id: Some("c1".to_string()),
                reasoning_details: None,
            },
            // User message with tool_result referencing a tool_use that doesn't exist in slice
            ApiMessage {
                role: MessageRole::User,
                content: vec![
                    ContentBlock::ToolResult {
                        tool_use_id: "tool-old".to_string(), // orphaned
                        content: vec![],
                        is_error: None,
                    },
                    ContentBlock::Text {
                        text: "some text".to_string(),
                    },
                ],
                reasoning: None,
                ts: None,
                truncation_parent: None,
                is_truncation_marker: None,
                truncation_id: None,
                condense_parent: None,
                is_summary: None,
                condense_id: None,
                reasoning_details: None,
            },
        ];
        let effective = get_effective_api_history(&messages);
        // The tool_result referencing "tool-old" should be filtered,
        // but the text block remains, so the message is kept
        assert_eq!(effective.len(), 2);
        // Check that the user message only has the text block
        if let ContentBlock::Text { ref text } = effective[1].content[0] {
            assert_eq!(text, "some text");
        } else {
            panic!("Expected text block");
        }
    }

    // ---- cleanup_after_truncation tests ----

    #[test]
    fn test_cleanup_after_truncation_clears_orphaned_condense_parent() {
        let messages = vec![
            make_condensed_msg(MessageRole::User, "orphaned", "deleted-summary"),
            make_msg(MessageRole::User, "normal"),
        ];
        let cleaned = cleanup_after_truncation(&messages);
        // Orphaned condense_parent should be cleared
        assert!(cleaned[0].condense_parent.is_none());
        assert!(cleaned[1].condense_parent.is_none());
    }

    #[test]
    fn test_cleanup_after_truncation_keeps_valid_condense_parent() {
        let messages = vec![
            make_condensed_msg(MessageRole::User, "condensed", "condense-1"),
            make_summary_msg("Summary", "condense-1"),
        ];
        let cleaned = cleanup_after_truncation(&messages);
        // condense_parent "condense-1" still has a matching summary, so it's kept
        assert_eq!(cleaned[0].condense_parent, Some("condense-1".to_string()));
    }

    #[test]
    fn test_cleanup_after_truncation_clears_orphaned_truncation_parent() {
        let mut msg = make_msg(MessageRole::User, "truncated");
        msg.truncation_parent = Some("deleted-marker".to_string());
        let messages = vec![msg];
        let cleaned = cleanup_after_truncation(&messages);
        assert!(cleaned[0].truncation_parent.is_none());
    }
}

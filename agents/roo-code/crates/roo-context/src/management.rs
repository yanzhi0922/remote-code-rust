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
    summarize_conversation, SummarizeConversationOptions, MIN_CONDENSE_THRESHOLD,
    MAX_CONDENSE_THRESHOLD,
};
use roo_provider::handler::{CreateMessageMetadata, Provider};
use roo_types::api::{ApiMessage, ContentBlock};

/// Default max tokens for Anthropic models when not specified.
const ANTHROPIC_DEFAULT_MAX_TOKENS: u64 = 8192;

use crate::token::estimate_token_count;
use crate::truncation::truncate_conversation;
use crate::TOKEN_BUFFER_PERCENTAGE;

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

    let reserved_tokens = max_tokens.unwrap_or(ANTHROPIC_DEFAULT_MAX_TOKENS as usize);
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
        } else if profile_threshold >= MIN_CONDENSE_THRESHOLD
            && profile_threshold <= MAX_CONDENSE_THRESHOLD
        {
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
pub async fn manage_context(options: ContextManagementOptions) -> anyhow::Result<ContextManagementResult> {
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
    let reserved_tokens = max_tokens.unwrap_or(ANTHROPIC_DEFAULT_MAX_TOKENS as usize);

    // Estimate tokens for the last message (which is always a user message)
    let last_message = messages.last().expect("messages should not be empty");
    let last_message_tokens = estimate_token_count(&last_message.content, api_handler.as_ref()).await? as usize;

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
        } else if profile_threshold >= MIN_CONDENSE_THRESHOLD
            && profile_threshold <= MAX_CONDENSE_THRESHOLD
        {
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
                    new_context_tokens_after_truncation: result.new_context_tokens.map(|t| t as usize),
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
                msg.truncation_parent.is_none()
                    && !msg.is_truncation_marker.unwrap_or(false)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_will_manage_context_no_condense_below_threshold() {
        let options = WillManageContextOptions {
            total_tokens: 1000,
            context_window: 10000,
            max_tokens: None,
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
            max_tokens: None,
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
}

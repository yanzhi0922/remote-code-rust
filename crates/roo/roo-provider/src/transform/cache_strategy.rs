/// Cache strategy for API request caching.
/// Mirrors src/api/transform/cache-strategy/*.ts
///
/// Implements the Bedrock MultiPoint cache strategy that decides where to place
/// `cachePoint` markers in the Converse API request payload.
use serde::{Deserialize, Serialize};
use serde_json::Value;

use roo_types::api::{ApiMessage, ContentBlock, MessageRole};
use roo_types::model::ModelInfo;

// ---------------------------------------------------------------------------
// Legacy types (preserved for backward compatibility)
// ---------------------------------------------------------------------------

/// Cache strategy types for different providers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheStrategyType {
    /// No caching.
    None,
    /// Anthropic-style prompt caching.
    Anthropic,
    /// Gemini-style context caching.
    Gemini,
    /// Vertex AI-style caching.
    Vertex,
    /// Vercel AI Gateway caching.
    VercelAiGateway,
}

/// A cache breakpoint in a message.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CacheBreakpoint {
    /// The index in the messages array where caching should be applied.
    pub index: usize,
    /// The type of cache strategy.
    pub strategy: CacheStrategyType,
}

/// Configuration for cache strategy.
#[derive(Clone, Debug)]
pub struct CacheStrategyConfig {
    /// The type of cache strategy to use.
    pub strategy_type: CacheStrategyType,
    /// Minimum number of tokens required before caching kicks in.
    pub min_token_threshold: usize,
    /// Maximum number of cache breakpoints per request.
    pub max_breakpoints: usize,
}

impl Default for CacheStrategyConfig {
    fn default() -> Self {
        Self {
            strategy_type: CacheStrategyType::None,
            min_token_threshold: 1024,
            max_breakpoints: 4,
        }
    }
}

/// Apply cache breakpoints to messages for the given strategy.
/// Returns the messages with cache control markers added.
pub fn apply_cache_breakpoints(
    messages: &mut [Value],
    config: &CacheStrategyConfig,
) -> Vec<CacheBreakpoint> {
    if config.strategy_type == CacheStrategyType::None {
        return vec![];
    }

    let mut breakpoints = Vec::new();

    match config.strategy_type {
        CacheStrategyType::Anthropic => {
            let user_indices: Vec<usize> = messages
                .iter()
                .enumerate()
                .filter(|(_, m)| m["role"].as_str() == Some("user"))
                .map(|(i, _)| i)
                .collect();

            for &idx in user_indices.iter().rev().take(config.max_breakpoints) {
                if let Some(msg) = messages.get_mut(idx)
                    && let Some(content) = msg.get_mut("content")
                {
                    if let Some(arr) = content.as_array_mut() {
                        if let Some(last_block) = arr.last_mut() {
                            last_block["cache_control"] = serde_json::json!({"type": "ephemeral"});
                        }
                    } else {
                        let text = content.as_str().unwrap_or("").to_string();
                        *content = serde_json::json!([
                            {"type": "text", "text": text, "cache_control": {"type": "ephemeral"}}
                        ]);
                    }
                }
                breakpoints.push(CacheBreakpoint {
                    index: idx,
                    strategy: CacheStrategyType::Anthropic,
                });
            }
        }
        CacheStrategyType::Gemini => {
            if let Some(system_msg) = messages
                .iter_mut()
                .find(|m| m["role"].as_str() == Some("system"))
            {
                system_msg["cached_content"] = Value::String("auto".to_string());
                breakpoints.push(CacheBreakpoint {
                    index: 0,
                    strategy: CacheStrategyType::Gemini,
                });
            }
        }
        CacheStrategyType::Vertex => {
            if let Some(system_msg) = messages
                .iter_mut()
                .find(|m| m["role"].as_str() == Some("system"))
            {
                system_msg["cached_content"] = Value::String("auto".to_string());
                breakpoints.push(CacheBreakpoint {
                    index: 0,
                    strategy: CacheStrategyType::Vertex,
                });
            }
        }
        CacheStrategyType::VercelAiGateway => {
            for msg in messages.iter_mut() {
                if msg["role"].as_str() == Some("system") {
                    msg["cache_control"] = serde_json::json!({"type": "ephemeral"});
                    breakpoints.push(CacheBreakpoint {
                        index: 0,
                        strategy: CacheStrategyType::VercelAiGateway,
                    });
                    break;
                }
            }
        }
        CacheStrategyType::None => {}
    }

    breakpoints
}

// ---------------------------------------------------------------------------
// Bedrock MultiPoint cache strategy
// ---------------------------------------------------------------------------

/// Model-specific cache capabilities for Bedrock.
///
/// Claude models typically support 4 cache points with a 1024-token minimum.
/// Nova models support 1 cache point with a 1-token minimum.
#[derive(Clone, Debug)]
pub struct BedrockCacheModelInfo {
    /// Maximum number of cache points the model supports.
    pub max_cache_points: usize,
    /// Minimum tokens required per cache point.
    pub min_tokens_per_cache_point: usize,
    /// Fields that can be cached (e.g. "system", "messages").
    pub cacheable_fields: Vec<String>,
}

impl BedrockCacheModelInfo {
    /// Build from a [`ModelInfo`] by detecting model family.
    pub fn from_model_info(model_info: &ModelInfo) -> Self {
        // Prefer explicit values from ModelInfo when present.
        let max_cache_points = model_info.max_cache_points.map(|v| v as usize).unwrap_or(4);
        let min_tokens = model_info
            .min_tokens_per_cache_point
            .map(|v| v as usize)
            .unwrap_or(1024);
        let cacheable_fields = model_info
            .cachable_fields
            .clone()
            .unwrap_or_else(|| vec!["system".into(), "messages".into()]);

        Self {
            max_cache_points,
            min_tokens_per_cache_point: min_tokens,
            cacheable_fields,
        }
    }

    /// Claude model defaults: 4 cache points, 1024 min tokens.
    pub fn claude_defaults() -> Self {
        Self {
            max_cache_points: 4,
            min_tokens_per_cache_point: 1024,
            cacheable_fields: vec!["system".into(), "messages".into()],
        }
    }

    /// Nova model defaults: 1 cache point, 1 min token.
    pub fn nova_defaults() -> Self {
        Self {
            max_cache_points: 1,
            min_tokens_per_cache_point: 1,
            cacheable_fields: vec!["system".into(), "messages".into()],
        }
    }
}

/// Represents the position and metadata for a cache point placement.
#[derive(Clone, Debug, PartialEq)]
pub struct CachePointPlacement {
    /// Message index where the cache point is placed.
    pub index: usize,
    /// Type of cache point.
    pub placement_type: CachePointPlacementType,
    /// Number of tokens this cache point covers.
    pub tokens_covered: usize,
}

/// Type of cache point placement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CachePointPlacementType {
    System,
    Message,
}

/// Result of applying the cache strategy.
#[derive(Clone, Debug)]
pub struct CacheResult {
    /// System content blocks (may include a cachePoint block at the end).
    pub system: Vec<Value>,
    /// Message content blocks (may have cachePoint blocks appended).
    pub messages: Vec<BedrockMessage>,
    /// Cache point placements for messages (for tracking across calls).
    pub message_cache_point_placements: Vec<CachePointPlacement>,
}

/// A single Bedrock Converse-format message with role and content blocks.
#[derive(Clone, Debug)]
pub struct BedrockMessage {
    /// "user" or "assistant".
    pub role: String,
    /// Content blocks for this message.
    pub content: Vec<Value>,
}

/// Configuration for the Bedrock MultiPoint cache strategy.
#[derive(Clone, Debug)]
pub struct MultiPointStrategyConfig {
    /// Model-specific cache capabilities.
    pub model_info: BedrockCacheModelInfo,
    /// System prompt text (may be empty).
    pub system_prompt: Option<String>,
    /// Messages to process.
    pub messages: Vec<ApiMessage>,
    /// Whether prompt caching is enabled.
    pub use_prompt_cache: bool,
    /// Previous cache point placements from the last call.
    pub previous_cache_point_placements: Option<Vec<CachePointPlacement>>,
}

// ---------------------------------------------------------------------------
// MultiPointStrategy
// ---------------------------------------------------------------------------

/// Strategy for placing Bedrock `cachePoint` markers.
///
/// Places cache points after messages as soon as uncached tokens exceed
/// `min_tokens_per_cache_point`. The system prompt always gets a cache point
/// when caching is enabled.
///
/// This faithfully mirrors the TypeScript `MultiPointStrategy` class in
/// `src/api/transform/cache-strategy/multi-point-strategy.ts`.
pub struct MultiPointStrategy {
    config: MultiPointStrategyConfig,
    system_token_count: usize,
}

impl MultiPointStrategy {
    /// Create a new strategy from the given configuration.
    pub fn new(config: MultiPointStrategyConfig) -> Self {
        let system_token_count = config
            .system_prompt
            .as_deref()
            .map(|t| Self::estimate_text_tokens(t) + 5)
            .unwrap_or(0);
        Self {
            config,
            system_token_count,
        }
    }

    /// Determine optimal cache point placements and return the formatted result.
    pub fn determine_optimal_cache_points(&self) -> CacheResult {
        // If prompt caching is disabled or no messages, return without cache points.
        if !self.config.use_prompt_cache || self.config.messages.is_empty() {
            return self.format_without_cache_points();
        }

        let supports_system_cache = self
            .config
            .model_info
            .cacheable_fields
            .contains(&"system".to_string());
        let supports_message_cache = self
            .config
            .model_info
            .cacheable_fields
            .contains(&"messages".to_string());
        let min_tokens_per_point = self.config.model_info.min_tokens_per_cache_point;
        let mut remaining_cache_points: usize = self.config.model_info.max_cache_points;

        // Determine if we will use a system cache point.
        let use_system_cache = supports_system_cache
            && self.config.system_prompt.is_some()
            && self.meets_min_token_threshold(self.system_token_count);

        // Build system blocks.
        let mut system_blocks: Vec<Value> = Vec::new();
        if let Some(ref prompt) = self.config.system_prompt {
            system_blocks.push(serde_json::json!({ "text": prompt }));
            if use_system_cache {
                system_blocks.push(Self::create_cache_point());
                remaining_cache_points -= 1;
            }
        }

        // If message caching isn't supported, return with just system caching.
        if !supports_message_cache {
            let messages = self.messages_to_bedrock_messages(&self.config.messages);
            return CacheResult {
                system: system_blocks,
                messages,
                message_cache_point_placements: vec![],
            };
        }

        let placements =
            self.determine_message_cache_points(min_tokens_per_point, remaining_cache_points);
        let mut messages = self.messages_to_bedrock_messages(&self.config.messages);
        self.apply_cache_points(&mut messages, &placements);

        CacheResult {
            system: system_blocks,
            messages,
            message_cache_point_placements: placements,
        }
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// Format result without any cache points.
    fn format_without_cache_points(&self) -> CacheResult {
        let system_blocks: Vec<Value> = self
            .config
            .system_prompt
            .as_deref()
            .map(|p| vec![serde_json::json!({ "text": p })])
            .unwrap_or_default();

        let messages = self.messages_to_bedrock_messages(&self.config.messages);

        CacheResult {
            system: system_blocks,
            messages,
            message_cache_point_placements: vec![],
        }
    }

    /// Determine optimal cache point placements for messages.
    fn determine_message_cache_points(
        &self,
        min_tokens_per_point: usize,
        mut remaining_cache_points: usize,
    ) -> Vec<CachePointPlacement> {
        if self.config.messages.len() <= 1 {
            return vec![];
        }

        let mut placements: Vec<CachePointPlacement> = Vec::new();
        let total_messages = self.config.messages.len();
        let previous_placements = self
            .config
            .previous_cache_point_placements
            .clone()
            .unwrap_or_default();

        // Special case: no previous placements — place initial cache points.
        if previous_placements.is_empty() {
            let mut current_index = 0;

            while current_index < total_messages && remaining_cache_points > 0 {
                if let Some(new_placement) = self.find_optimal_placement_for_range(
                    current_index,
                    total_messages - 1,
                    min_tokens_per_point,
                    &previous_placements,
                ) {
                    placements.push(new_placement.clone());
                    current_index = new_placement.index + 1;
                    remaining_cache_points -= 1;
                } else {
                    break;
                }
            }

            return placements;
        }

        // Growing conversation: calculate tokens in new messages.
        let last_previous_index = previous_placements.last().unwrap().index;
        let new_messages_tokens: usize = self.config.messages[last_previous_index + 1..]
            .iter()
            .map(|m| self.estimate_message_tokens(m))
            .sum();

        if new_messages_tokens >= min_tokens_per_point {
            // Enough new tokens for a cache point.
            if remaining_cache_points > previous_placements.len() {
                // Keep all previous placements.
                for placement in &previous_placements {
                    if placement.index < total_messages {
                        placements.push(placement.clone());
                    }
                }

                // Add a new placement for the new messages.
                if let Some(new_placement) = self.find_optimal_placement_for_range(
                    last_previous_index + 1,
                    total_messages - 1,
                    min_tokens_per_point,
                    &previous_placements,
                ) {
                    placements.push(new_placement);
                }
            } else {
                // Need to decide which previous cache points to keep / combine.
                // Analyze the token distribution between previous cache points.
                let mut tokens_between_placements: Vec<usize> = Vec::new();
                let mut start_idx = 0;

                for placement in &previous_placements {
                    let tokens: usize = self.config.messages[start_idx..=placement.index]
                        .iter()
                        .map(|m| self.estimate_message_tokens(m))
                        .sum();
                    tokens_between_placements.push(tokens);
                    start_idx = placement.index + 1;
                }

                // Find the two consecutive placements with the smallest token gap.
                let mut smallest_gap_index = 0;
                let mut smallest_gap = usize::MAX;

                for i in 0..tokens_between_placements.len().saturating_sub(1) {
                    let gap = tokens_between_placements[i] + tokens_between_placements[i + 1];
                    if gap < smallest_gap {
                        smallest_gap = gap;
                        smallest_gap_index = i;
                    }
                }

                // 20% increase required to justify reallocation.
                let required_percentage_increase: f64 = 1.2;
                let required_token_threshold =
                    (smallest_gap as f64 * required_percentage_increase).ceil() as usize;

                if new_messages_tokens >= required_token_threshold {
                    // It's beneficial to combine cache points.
                    tracing::info!(
                        new_messages_tokens,
                        smallest_gap,
                        required_token_threshold,
                        "Combining cache points is beneficial"
                    );

                    // Combine the two placements with the smallest gap.
                    let mut i = 0;
                    while i < previous_placements.len() {
                        if i != smallest_gap_index && i != smallest_gap_index + 1 {
                            // Keep this placement.
                            if previous_placements[i].index < total_messages {
                                placements.push(previous_placements[i].clone());
                            }
                        } else if i == smallest_gap_index {
                            // Replace with a combined placement.
                            let combined_end_index = previous_placements[i + 1].index;
                            let start_of_range = if i == 0 {
                                0
                            } else {
                                previous_placements[i - 1].index + 1
                            };

                            if let Some(combined_placement) = self.find_optimal_placement_for_range(
                                start_of_range,
                                combined_end_index,
                                min_tokens_per_point,
                                &previous_placements,
                            ) {
                                placements.push(combined_placement);
                            }
                            // Skip the next placement as we've combined it.
                            i += 1;
                        }
                        i += 1;
                    }

                    // If we freed up a cache point, use it for the new messages.
                    if placements.len() < remaining_cache_points
                        && let Some(new_placement) = self.find_optimal_placement_for_range(
                            last_previous_index + 1,
                            total_messages - 1,
                            min_tokens_per_point,
                            &previous_placements,
                        )
                    {
                        placements.push(new_placement);
                    }
                } else {
                    // Not beneficial to combine — keep all previous placements.
                    tracing::info!(
                        new_messages_tokens,
                        smallest_gap,
                        "Combining cache points is not beneficial, keeping existing"
                    );
                    for placement in &previous_placements {
                        if placement.index < total_messages {
                            placements.push(placement.clone());
                        }
                    }
                }
            }

            placements
        } else {
            // New messages don't have enough tokens — keep all previous placements.
            for placement in &previous_placements {
                if placement.index < total_messages {
                    placements.push(placement.clone());
                }
            }
            placements
        }
    }

    /// Find the optimal placement for a cache point within a range.
    ///
    /// Strategy: find the last user message in the range whose accumulated
    /// token count meets the minimum threshold.
    fn find_optimal_placement_for_range(
        &self,
        start_index: usize,
        end_index: usize,
        min_tokens_per_point: usize,
        previous_placements: &[CachePointPlacement],
    ) -> Option<CachePointPlacement> {
        if start_index >= end_index {
            return None;
        }

        // Find the last user message in the range.
        let mut last_user_message_index: Option<usize> = None;
        for i in (start_index..=end_index).rev() {
            if self.config.messages[i].role == MessageRole::User {
                last_user_message_index = Some(i);
                break;
            }
        }

        let last_user_idx = last_user_message_index?;

        // Find the previous cache point index before start_index.
        let mut previous_cache_point_index: isize = -1;
        for placement in previous_placements {
            if placement.index < start_index {
                let idx = placement.index as isize;
                if idx > previous_cache_point_index {
                    previous_cache_point_index = idx;
                }
            }
        }

        let token_start_index = (previous_cache_point_index + 1) as usize;
        let total_tokens_covered: usize = self.config.messages[token_start_index..=last_user_idx]
            .iter()
            .map(|m| self.estimate_message_tokens(m))
            .sum();

        // Guard: enough tokens to justify a cache point.
        if total_tokens_covered < min_tokens_per_point {
            return None;
        }

        Some(CachePointPlacement {
            index: last_user_idx,
            placement_type: CachePointPlacementType::Message,
            tokens_covered: total_tokens_covered,
        })
    }

    /// Apply cache points to Bedrock messages by inserting cachePoint blocks.
    fn apply_cache_points(
        &self,
        messages: &mut [BedrockMessage],
        placements: &[CachePointPlacement],
    ) {
        for placement in placements {
            if let Some(msg) = messages.get_mut(placement.index) {
                msg.content.push(Self::create_cache_point());
            }
        }
    }

    /// Convert `ApiMessage` slice to `BedrockMessage` vec.
    fn messages_to_bedrock_messages(&self, msgs: &[ApiMessage]) -> Vec<BedrockMessage> {
        msgs.iter()
            .map(|msg| {
                let role = match msg.role {
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                };

                let content: Vec<Value> = msg
                    .content
                    .iter()
                    .map(|block| match block {
                        ContentBlock::Text { text } => serde_json::json!({ "text": text }),
                        ContentBlock::Image { source } => {
                            if let roo_types::api::ImageSource::Base64 { data, media_type } = source
                            {
                                serde_json::json!({
                                    "image": {
                                        "source": { "bytes": data },
                                        "format": media_type,
                                    }
                                })
                            } else {
                                serde_json::json!({ "text": "[image]" })
                            }
                        }
                        ContentBlock::ToolUse { id, name, input } => serde_json::json!({
                            "toolUse": {
                                "toolUseId": id,
                                "name": name,
                                "input": input,
                            }
                        }),
                        ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            is_error,
                        } => {
                            let tool_content: Vec<Value> = content
                                .iter()
                                .map(|c| match c {
                                    roo_types::api::ToolResultContent::Text { text } => {
                                        serde_json::json!({ "text": text })
                                    }
                                    roo_types::api::ToolResultContent::Image { source } => {
                                        if let roo_types::api::ImageSource::Base64 {
                                            data,
                                            media_type,
                                        } = source
                                        {
                                            serde_json::json!({
                                                "image": {
                                                    "source": { "bytes": data },
                                                    "format": media_type,
                                                }
                                            })
                                        } else {
                                            serde_json::json!({ "text": "[image]" })
                                        }
                                    }
                                })
                                .collect();
                            let status = if is_error.unwrap_or(false) {
                                "error"
                            } else {
                                "success"
                            };
                            serde_json::json!({
                                "toolResult": {
                                    "toolUseId": tool_use_id,
                                    "content": tool_content,
                                    "status": status,
                                }
                            })
                        }
                        ContentBlock::Thinking { thinking, .. } => serde_json::json!({
                            "reasoningContent": {
                                "reasoningText": { "text": thinking }
                            }
                        }),
                        ContentBlock::RedactedThinking { data } => serde_json::json!({
                            "reasoningContent": { "redactedContent": data }
                        }),
                    })
                    .collect();

                BedrockMessage {
                    role: role.to_string(),
                    content,
                }
            })
            .collect()
    }

    /// Create a `cachePoint` content block.
    fn create_cache_point() -> Value {
        serde_json::json!({ "cachePoint": { "type": "default" } })
    }

    /// Check if a token count meets the minimum threshold for caching.
    fn meets_min_token_threshold(&self, token_count: usize) -> bool {
        token_count >= self.config.model_info.min_tokens_per_cache_point
    }

    /// Estimate token count for a single message.
    fn estimate_message_tokens(&self, message: &ApiMessage) -> usize {
        let mut total_tokens = 0usize;
        for block in &message.content {
            match block {
                ContentBlock::Text { text } => {
                    total_tokens += Self::estimate_text_tokens(text);
                }
                ContentBlock::Image { .. } => {
                    // Conservative image estimate.
                    total_tokens += 300;
                }
                ContentBlock::ToolUse { name, input, .. } => {
                    total_tokens += Self::estimate_text_tokens(name);
                    total_tokens += Self::estimate_text_tokens(&input.to_string());
                }
                ContentBlock::ToolResult { content, .. } => {
                    for c in content {
                        match c {
                            roo_types::api::ToolResultContent::Text { text } => {
                                total_tokens += Self::estimate_text_tokens(text);
                            }
                            roo_types::api::ToolResultContent::Image { .. } => {
                                total_tokens += 300;
                            }
                        }
                    }
                }
                ContentBlock::Thinking { thinking, .. } => {
                    total_tokens += Self::estimate_text_tokens(thinking);
                }
                ContentBlock::RedactedThinking { .. } => {
                    total_tokens += 10;
                }
            }
        }
        // Overhead for message structure.
        total_tokens + 10
    }

    /// Estimate token count for a text string using the same heuristic as the
    /// TypeScript source:
    ///   - words * 1.3
    ///   - punctuation chars * 0.3
    ///   - newlines * 0.5
    fn estimate_text_tokens(text: &str) -> usize {
        if text.is_empty() {
            return 0;
        }

        let words = text.split_whitespace().count() as f64;
        let punct_count = text
            .chars()
            .filter(|c| {
                matches!(
                    c,
                    '.' | ','
                        | '!'
                        | '?'
                        | ';'
                        | ':'
                        | '('
                        | ')'
                        | '['
                        | ']'
                        | '{'
                        | '}'
                        | '"'
                        | '\''
                        | '`'
                )
            })
            .count() as f64;
        let newline_count = text.chars().filter(|c| *c == '\n').count() as f64;

        let tokens = words * 1.3 + punct_count * 0.3 + newline_count * 0.5;
        tokens.ceil() as usize
    }
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // Legacy tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_default_config() {
        let config = CacheStrategyConfig::default();
        assert_eq!(CacheStrategyType::None, config.strategy_type);
        assert_eq!(1024, config.min_token_threshold);
        assert_eq!(4, config.max_breakpoints);
    }

    #[test]
    fn test_no_caching() {
        let mut messages = vec![serde_json::json!({"role": "user", "content": "hello"})];
        let config = CacheStrategyConfig::default();
        let breakpoints = apply_cache_breakpoints(&mut messages, &config);
        assert!(breakpoints.is_empty());
    }

    #[test]
    fn test_anthropic_caching() {
        let mut messages = vec![
            serde_json::json!({"role": "system", "content": "You are helpful."}),
            serde_json::json!({"role": "user", "content": "hello"}),
        ];
        let config = CacheStrategyConfig {
            strategy_type: CacheStrategyType::Anthropic,
            ..Default::default()
        };
        let breakpoints = apply_cache_breakpoints(&mut messages, &config);
        assert!(!breakpoints.is_empty());
    }

    #[test]
    fn test_gemini_caching() {
        let mut messages = vec![
            serde_json::json!({"role": "system", "content": "You are helpful."}),
            serde_json::json!({"role": "user", "content": "hello"}),
        ];
        let config = CacheStrategyConfig {
            strategy_type: CacheStrategyType::Gemini,
            ..Default::default()
        };
        let breakpoints = apply_cache_breakpoints(&mut messages, &config);
        assert!(!breakpoints.is_empty());
        assert!(messages[0].get("cached_content").is_some());
    }

    #[test]
    fn test_cache_strategy_type_serde() {
        let t = CacheStrategyType::Anthropic;
        let json = serde_json::to_string(&t).unwrap();
        assert!(json.contains("anthropic"));
        let deserialized: CacheStrategyType = serde_json::from_str(&json).unwrap();
        assert_eq!(t, deserialized);
    }

    // -----------------------------------------------------------------------
    // MultiPointStrategy tests
    // -----------------------------------------------------------------------

    /// Helper: build a text-only user message.
    fn user_msg(text: &str) -> ApiMessage {
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
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        }
    }

    /// Helper: build a text-only assistant message.
    fn assistant_msg(text: &str) -> ApiMessage {
        ApiMessage {
            role: MessageRole::Assistant,
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

    /// Helper: build a Claude-style config.
    fn claude_config(
        messages: Vec<ApiMessage>,
        system_prompt: Option<String>,
        use_prompt_cache: bool,
        previous: Option<Vec<CachePointPlacement>>,
    ) -> MultiPointStrategyConfig {
        MultiPointStrategyConfig {
            model_info: BedrockCacheModelInfo::claude_defaults(),
            system_prompt,
            messages,
            use_prompt_cache,
            previous_cache_point_placements: previous,
        }
    }

    /// Generate a long string that exceeds the Claude min token threshold (1024).
    /// ~27 words * 1.3 = 35 tokens per sentence; repeat 40x for ~1400 tokens.
    fn long_text() -> String {
        (0..40)
            .map(|_| {
                "This is a sample sentence used to pad out the token count \
                 significantly beyond the minimum threshold for cache points \
                 in the Bedrock prompt caching strategy implementation."
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn test_estimate_text_tokens_basic() {
        let tokens = MultiPointStrategy::estimate_text_tokens("Hello world");
        // 2 words * 1.3 = 2.6 -> ceil = 3
        assert_eq!(tokens, 3);
    }

    #[test]
    fn test_estimate_text_tokens_empty() {
        assert_eq!(MultiPointStrategy::estimate_text_tokens(""), 0);
    }

    #[test]
    fn test_estimate_text_tokens_with_punctuation() {
        let tokens = MultiPointStrategy::estimate_text_tokens("Hello, world!");
        // 2 words * 1.3 + 2 punct * 0.3 = 2.6 + 0.6 = 3.2 -> ceil = 4
        assert_eq!(tokens, 4);
    }

    #[test]
    fn test_estimate_text_tokens_with_newlines() {
        let tokens = MultiPointStrategy::estimate_text_tokens("Hello\nworld");
        // 2 words * 1.3 + 1 newline * 0.5 = 2.6 + 0.5 = 3.1 -> ceil = 4
        assert_eq!(tokens, 4);
    }

    #[test]
    fn test_strategy_disabled_returns_no_cache_points() {
        let config = claude_config(
            vec![user_msg("Hello")],
            Some("System prompt".to_string()),
            false,
            None,
        );
        let strategy = MultiPointStrategy::new(config);
        let result = strategy.determine_optimal_cache_points();

        assert!(result.message_cache_point_placements.is_empty());
        // System blocks should have no cache point.
        assert!(result.system.iter().all(|b| b.get("cachePoint").is_none()));
    }

    #[test]
    fn test_strategy_empty_messages_returns_no_cache_points() {
        let config = claude_config(vec![], Some("System prompt".to_string()), true, None);
        let strategy = MultiPointStrategy::new(config);
        let result = strategy.determine_optimal_cache_points();
        assert!(result.message_cache_point_placements.is_empty());
        assert!(result.messages.is_empty());
    }

    #[test]
    fn test_system_prompt_gets_cache_point_when_enabled() {
        let long_system = long_text();
        let config = claude_config(
            vec![user_msg(&long_text()), assistant_msg(&long_text())],
            Some(long_system),
            true,
            None,
        );
        let strategy = MultiPointStrategy::new(config);
        let result = strategy.determine_optimal_cache_points();

        // System blocks should end with a cachePoint.
        assert!(result.system.len() >= 2);
        assert!(result.system.last().unwrap().get("cachePoint").is_some());
    }

    #[test]
    fn test_system_prompt_no_cache_point_when_too_short() {
        let config = claude_config(
            vec![user_msg(&long_text()), assistant_msg(&long_text())],
            Some("Short".to_string()),
            true,
            None,
        );
        let strategy = MultiPointStrategy::new(config);
        let result = strategy.determine_optimal_cache_points();

        // System block should NOT have a cache point (too short for Claude's 1024 min).
        assert_eq!(result.system.len(), 1);
        assert!(result.system[0].get("cachePoint").is_none());
    }

    #[test]
    fn test_single_message_no_message_cache_points() {
        let config = claude_config(vec![user_msg(&long_text())], Some(long_text()), true, None);
        let strategy = MultiPointStrategy::new(config);
        let result = strategy.determine_optimal_cache_points();

        // With only 1 message, no message cache points are placed.
        assert!(result.message_cache_point_placements.is_empty());
    }

    #[test]
    fn test_initial_placement_finds_user_message() {
        let long = long_text();
        let config = claude_config(
            vec![
                user_msg(&long),
                assistant_msg(&long),
                user_msg(&long),
                assistant_msg(&long),
                user_msg(&long),
            ],
            Some(long.clone()),
            true,
            None,
        );
        let strategy = MultiPointStrategy::new(config);
        let result = strategy.determine_optimal_cache_points();

        // Should have at least one message cache point placement.
        assert!(!result.message_cache_point_placements.is_empty());

        // Placements should be on user messages.
        for placement in &result.message_cache_point_placements {
            assert_eq!(
                result.messages[placement.index].role, "user",
                "Cache points should be placed on user messages"
            );
        }
    }

    #[test]
    fn test_cache_points_inserted_into_message_content() {
        let long = long_text();
        let config = claude_config(
            vec![user_msg(&long), assistant_msg(&long), user_msg(&long)],
            Some(long),
            true,
            None,
        );
        let strategy = MultiPointStrategy::new(config);
        let result = strategy.determine_optimal_cache_points();

        // Verify cachePoint blocks are in the content arrays at placement indices.
        for placement in &result.message_cache_point_placements {
            let msg = &result.messages[placement.index];
            let has_cache_point = msg.content.iter().any(|b| b.get("cachePoint").is_some());
            assert!(
                has_cache_point,
                "Message at index {} should have a cachePoint block",
                placement.index
            );
        }
    }

    #[test]
    fn test_nova_model_single_cache_point() {
        let long = long_text();
        let config = MultiPointStrategyConfig {
            model_info: BedrockCacheModelInfo::nova_defaults(),
            system_prompt: Some(long.clone()),
            messages: vec![user_msg(&long), assistant_msg(&long), user_msg(&long)],
            use_prompt_cache: true,
            previous_cache_point_placements: None,
        };
        let strategy = MultiPointStrategy::new(config);
        let result = strategy.determine_optimal_cache_points();

        // Nova: max 1 cache point total. System prompt gets it if it qualifies,
        // leaving 0 for messages (Nova min tokens is 1 so system always qualifies).
        // Total message placements + system placements <= 1.
        let system_cache_points = result
            .system
            .iter()
            .filter(|b| b.get("cachePoint").is_some())
            .count();
        let total = system_cache_points + result.message_cache_point_placements.len();
        assert!(
            total <= 1,
            "Nova should have at most 1 cache point, got {}",
            total
        );
    }

    #[test]
    fn test_previous_placements_preserved_when_no_new_tokens() {
        let long = long_text();
        let previous = vec![CachePointPlacement {
            index: 0,
            placement_type: CachePointPlacementType::Message,
            tokens_covered: 2000,
        }];

        // Short new message that doesn't meet the threshold.
        let config = claude_config(
            vec![
                user_msg(&long),
                assistant_msg(&long),
                user_msg("short"), // new short message
            ],
            Some(long),
            true,
            Some(previous),
        );
        let strategy = MultiPointStrategy::new(config);
        let result = strategy.determine_optimal_cache_points();

        // The previous placement at index 0 should be preserved.
        assert!(
            result
                .message_cache_point_placements
                .iter()
                .any(|p| p.index == 0)
        );
    }

    #[test]
    fn test_previous_placements_extended_with_new_tokens() {
        let long = long_text();
        let previous = vec![CachePointPlacement {
            index: 0,
            placement_type: CachePointPlacementType::Message,
            tokens_covered: 2000,
        }];

        // Long new messages that meet the threshold.
        let config = claude_config(
            vec![
                user_msg(&long),      // index 0 (has previous placement)
                assistant_msg(&long), // index 1
                user_msg(&long),      // index 2 (new long message)
            ],
            Some(long),
            true,
            Some(previous),
        );
        let strategy = MultiPointStrategy::new(config);
        let result = strategy.determine_optimal_cache_points();

        // Should have the previous placement plus a new one.
        // Claude allows 4 cache points, system uses 1, so 3 remaining.
        // 1 previous + 1 new should fit.
        assert!(
            result.message_cache_point_placements.len() >= 2,
            "Expected at least 2 placements, got {}",
            result.message_cache_point_placements.len()
        );
    }

    #[test]
    fn test_bedrock_cache_model_info_claude_defaults() {
        let info = BedrockCacheModelInfo::claude_defaults();
        assert_eq!(info.max_cache_points, 4);
        assert_eq!(info.min_tokens_per_cache_point, 1024);
        assert!(info.cacheable_fields.contains(&"system".to_string()));
        assert!(info.cacheable_fields.contains(&"messages".to_string()));
    }

    #[test]
    fn test_bedrock_cache_model_info_nova_defaults() {
        let info = BedrockCacheModelInfo::nova_defaults();
        assert_eq!(info.max_cache_points, 1);
        assert_eq!(info.min_tokens_per_cache_point, 1);
    }

    #[test]
    fn test_bedrock_cache_model_info_from_model_info_claude() {
        let mi = ModelInfo {
            max_cache_points: Some(4),
            min_tokens_per_cache_point: Some(1024),
            cachable_fields: Some(vec!["system".into(), "messages".into()]),
            ..Default::default()
        };
        let info = BedrockCacheModelInfo::from_model_info(&mi);
        assert_eq!(info.max_cache_points, 4);
        assert_eq!(info.min_tokens_per_cache_point, 1024);
    }

    #[test]
    fn test_bedrock_cache_model_info_from_model_info_defaults() {
        let mi = ModelInfo::default();
        let info = BedrockCacheModelInfo::from_model_info(&mi);
        // Should fall back to Claude defaults.
        assert_eq!(info.max_cache_points, 4);
        assert_eq!(info.min_tokens_per_cache_point, 1024);
    }

    #[test]
    fn test_create_cache_point_format() {
        let cp = MultiPointStrategy::create_cache_point();
        assert_eq!(cp["cachePoint"]["type"], "default");
    }

    #[test]
    fn test_messages_to_bedrock_messages() {
        let config = claude_config(
            vec![user_msg("Hello"), assistant_msg("Hi there")],
            None,
            true,
            None,
        );
        let strategy = MultiPointStrategy::new(config);
        let result = strategy.determine_optimal_cache_points();

        assert_eq!(result.messages.len(), 2);
        assert_eq!(result.messages[0].role, "user");
        assert_eq!(result.messages[1].role, "assistant");
        assert_eq!(result.messages[0].content.len(), 1);
        assert_eq!(result.messages[0].content[0]["text"], "Hello");
    }

    #[test]
    fn test_max_cache_point_limit_respected() {
        let long = long_text();
        // Create many messages to try to exceed the 4-point limit.
        let mut msgs = Vec::new();
        for _ in 0..10 {
            msgs.push(user_msg(&long));
            msgs.push(assistant_msg(&long));
        }

        let config = claude_config(msgs, Some(long), true, None);
        let strategy = MultiPointStrategy::new(config);
        let result = strategy.determine_optimal_cache_points();

        // System uses 1, so message placements <= 3. Total <= 4.
        let system_cps = result
            .system
            .iter()
            .filter(|b| b.get("cachePoint").is_some())
            .count();
        let total = system_cps + result.message_cache_point_placements.len();
        assert!(
            total <= 4,
            "Total cache points should not exceed 4, got {}",
            total
        );
    }
}

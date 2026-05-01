/// Cache strategy for API request caching.
/// Mirrors src/api/transform/cache-strategy/*.ts

use serde::{Deserialize, Serialize};
use serde_json::Value;

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
            // Anthropic: add cache_control to the last two user messages
            let user_indices: Vec<usize> = messages
                .iter()
                .enumerate()
                .filter(|(_, m)| m["role"].as_str() == Some("user"))
                .map(|(i, _)| i)
                .collect();

            for &idx in user_indices.iter().rev().take(config.max_breakpoints) {
                if let Some(msg) = messages.get_mut(idx) {
                    if let Some(content) = msg.get_mut("content") {
                        if let Some(arr) = content.as_array_mut() {
                            if let Some(last_block) = arr.last_mut() {
                                last_block["cache_control"] = serde_json::json!({"type": "ephemeral"});
                            }
                        } else {
                            // String content - convert to array
                            let text = content.as_str().unwrap_or("").to_string();
                            *content = serde_json::json!([
                                {"type": "text", "text": text, "cache_control": {"type": "ephemeral"}}
                            ]);
                        }
                    }
                }
                breakpoints.push(CacheBreakpoint {
                    index: idx,
                    strategy: CacheStrategyType::Anthropic,
                });
            }
        }
        CacheStrategyType::Gemini => {
            // Gemini: mark system instruction for caching
            if let Some(system_msg) = messages.iter_mut().find(|m| m["role"].as_str() == Some("system")) {
                system_msg["cached_content"] = Value::String("auto".to_string());
                breakpoints.push(CacheBreakpoint {
                    index: 0,
                    strategy: CacheStrategyType::Gemini,
                });
            }
        }
        CacheStrategyType::Vertex => {
            // Similar to Gemini but with Vertex-specific markers
            if let Some(system_msg) = messages.iter_mut().find(|m| m["role"].as_str() == Some("system")) {
                system_msg["cached_content"] = Value::String("auto".to_string());
                breakpoints.push(CacheBreakpoint {
                    index: 0,
                    strategy: CacheStrategyType::Vertex,
                });
            }
        }
        CacheStrategyType::VercelAiGateway => {
            // Vercel AI Gateway: add cache headers
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_default_config() {
        let config = CacheStrategyConfig::default();
        assert_eq!(CacheStrategyType::None, config.strategy_type);
        assert_eq!(1024, config.min_token_threshold);
        assert_eq!(4, config.max_breakpoints);
    }

    #[test]
    fn test_no_caching() {
        let mut messages = vec![json!({"role": "user", "content": "hello"})];
        let config = CacheStrategyConfig::default();
        let breakpoints = apply_cache_breakpoints(&mut messages, &config);
        assert!(breakpoints.is_empty());
    }

    #[test]
    fn test_anthropic_caching() {
        let mut messages = vec![
            json!({"role": "system", "content": "You are helpful."}),
            json!({"role": "user", "content": "hello"}),
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
            json!({"role": "system", "content": "You are helpful."}),
            json!({"role": "user", "content": "hello"}),
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
}

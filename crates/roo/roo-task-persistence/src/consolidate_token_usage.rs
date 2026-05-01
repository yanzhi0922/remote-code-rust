//! Consolidate Token Usage
//!
//! Consolidates token usage metrics from an array of messages.
//! Mirrors `consolidateTokenUsage.ts`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Parsed API request started text.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedApiReqStartedText {
    pub tokens_in: Option<u64>,
    pub tokens_out: Option<u64>,
    pub cache_writes: Option<u64>,
    pub cache_reads: Option<u64>,
    pub cost: Option<f64>,
}

/// Consolidated token usage result.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    pub total_tokens_in: u64,
    pub total_tokens_out: u64,
    pub total_cache_writes: Option<u64>,
    pub total_cache_reads: Option<u64>,
    pub total_cost: f64,
    pub context_tokens: u64,
}

/// A simplified message type for token usage consolidation.
#[derive(Debug, Clone)]
pub struct ClineMessageRef<'a> {
    pub msg_type: &'a str,
    pub say: Option<&'a str>,
    pub text: Option<&'a str>,
    pub context_condense_cost: Option<f64>,
    pub context_condense_new_context_tokens: Option<u64>,
}

// ---------------------------------------------------------------------------
// Consolidation
// ---------------------------------------------------------------------------

/// Consolidate token usage from a list of messages.
///
/// Source: `.research/Roo-Code/packages/core/src/message-utils/consolidateTokenUsage.ts`
pub fn consolidate_token_usage(messages: &[ClineMessageRef<'_>]) -> TokenUsage {
    let mut result = TokenUsage::default();

    // Calculate running totals
    for message in messages {
        if message.msg_type == "say" && message.say == Some("api_req_started") {
            if let Some(text) = message.text {
                if let Ok(parsed) = serde_json::from_str::<ParsedApiReqStartedText>(text) {
                    if let Some(tokens_in) = parsed.tokens_in {
                        result.total_tokens_in += tokens_in;
                    }
                    if let Some(tokens_out) = parsed.tokens_out {
                        result.total_tokens_out += tokens_out;
                    }
                    if let Some(cache_writes) = parsed.cache_writes {
                        result.total_cache_writes =
                            Some(result.total_cache_writes.unwrap_or(0) + cache_writes);
                    }
                    if let Some(cache_reads) = parsed.cache_reads {
                        result.total_cache_reads =
                            Some(result.total_cache_reads.unwrap_or(0) + cache_reads);
                    }
                    if let Some(cost) = parsed.cost {
                        result.total_cost += cost;
                    }
                }
            }
        } else if message.msg_type == "say" && message.say == Some("condense_context") {
            result.total_cost += message.context_condense_cost.unwrap_or(0.0);
        }
    }

    // Calculate context tokens from the last API request or condense
    result.context_tokens = 0;

    for message in messages.iter().rev() {
        if message.msg_type == "say" && message.say == Some("api_req_started") {
            if let Some(text) = message.text {
                if let Ok(parsed) = serde_json::from_str::<ParsedApiReqStartedText>(text) {
                    let tokens_in = parsed.tokens_in.unwrap_or(0);
                    let tokens_out = parsed.tokens_out.unwrap_or(0);
                    result.context_tokens = tokens_in + tokens_out;
                }
            }
        } else if message.msg_type == "say" && message.say == Some("condense_context") {
            result.context_tokens = message.context_condense_new_context_tokens.unwrap_or(0);
        }

        if result.context_tokens > 0 {
            break;
        }
    }

    result
}

/// Check if token usage has changed.
///
/// Source: `consolidateTokenUsage.ts` — `hasTokenUsageChanged`
pub fn has_token_usage_changed(current: &TokenUsage, snapshot: Option<&TokenUsage>) -> bool {
    match snapshot {
        None => true,
        Some(snap) => {
            current.total_tokens_in != snap.total_tokens_in
                || current.total_tokens_out != snap.total_tokens_out
                || current.total_cache_writes != snap.total_cache_writes
                || current.total_cache_reads != snap.total_cache_reads
                || (current.total_cost - snap.total_cost).abs() > f64::EPSILON
                || current.context_tokens != snap.context_tokens
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_messages() {
        let result = consolidate_token_usage(&[]);
        assert_eq!(result.total_tokens_in, 0);
        assert_eq!(result.total_tokens_out, 0);
        assert_eq!(result.total_cost, 0.0);
        assert_eq!(result.context_tokens, 0);
    }

    #[test]
    fn test_single_api_req() {
        let messages = vec![ClineMessageRef {
            msg_type: "say",
            say: Some("api_req_started"),
            text: Some(r#"{"tokensIn":10,"tokensOut":20,"cost":0.005}"#),
            context_condense_cost: None,
            context_condense_new_context_tokens: None,
        }];
        let result = consolidate_token_usage(&messages);
        assert_eq!(result.total_tokens_in, 10);
        assert_eq!(result.total_tokens_out, 20);
        assert!((result.total_cost - 0.005).abs() < f64::EPSILON);
        assert_eq!(result.context_tokens, 30);
    }

    #[test]
    fn test_multiple_api_reqs() {
        let messages = vec![
            ClineMessageRef {
                msg_type: "say",
                say: Some("api_req_started"),
                text: Some(r#"{"tokensIn":10,"tokensOut":20,"cost":0.005}"#),
                context_condense_cost: None,
                context_condense_new_context_tokens: None,
            },
            ClineMessageRef {
                msg_type: "say",
                say: Some("api_req_started"),
                text: Some(r#"{"tokensIn":5,"tokensOut":10,"cost":0.002}"#),
                context_condense_cost: None,
                context_condense_new_context_tokens: None,
            },
        ];
        let result = consolidate_token_usage(&messages);
        assert_eq!(result.total_tokens_in, 15);
        assert_eq!(result.total_tokens_out, 30);
        assert!((result.total_cost - 0.007).abs() < f64::EPSILON);
    }

    #[test]
    fn test_condense_context() {
        let messages = vec![
            ClineMessageRef {
                msg_type: "say",
                say: Some("api_req_started"),
                text: Some(r#"{"tokensIn":100,"tokensOut":50}"#),
                context_condense_cost: None,
                context_condense_new_context_tokens: None,
            },
            ClineMessageRef {
                msg_type: "say",
                say: Some("condense_context"),
                text: None,
                context_condense_cost: Some(0.001),
                context_condense_new_context_tokens: Some(200),
            },
        ];
        let result = consolidate_token_usage(&messages);
        assert!((result.total_cost - 0.001).abs() < f64::EPSILON);
        assert_eq!(result.context_tokens, 200);
    }

    #[test]
    fn test_has_token_usage_changed_none_snapshot() {
        let current = TokenUsage::default();
        assert!(has_token_usage_changed(&current, None));
    }

    #[test]
    fn test_has_token_usage_changed_same() {
        let current = TokenUsage {
            total_tokens_in: 10,
            ..Default::default()
        };
        let snapshot = TokenUsage {
            total_tokens_in: 10,
            ..Default::default()
        };
        assert!(!has_token_usage_changed(&current, Some(&snapshot)));
    }

    #[test]
    fn test_has_token_usage_changed_different() {
        let current = TokenUsage {
            total_tokens_in: 10,
            ..Default::default()
        };
        let snapshot = TokenUsage {
            total_tokens_in: 5,
            ..Default::default()
        };
        assert!(has_token_usage_changed(&current, Some(&snapshot)));
    }

    #[test]
    fn test_invalid_json_ignored() {
        let messages = vec![ClineMessageRef {
            msg_type: "say",
            say: Some("api_req_started"),
            text: Some("invalid json"),
            context_condense_cost: None,
            context_condense_new_context_tokens: None,
        }];
        let result = consolidate_token_usage(&messages);
        assert_eq!(result.total_tokens_in, 0);
    }
}

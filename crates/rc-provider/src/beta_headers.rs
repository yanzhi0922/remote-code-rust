//! Beta headers and extra body parameters for API requests.
//!
//! Manages the `anthropic-beta` header and extra body parameters that enable
//! experimental API features.  Includes special handling for Bedrock and
//! Vertex AI providers.
//!
//! Based on upstream Claude Code's `getExtraBodyParams`, `getMergedBetas`,
//! and `getBedrockExtraBodyParamsBetas` in `utils/betas.ts` and
//! `services/api/claude.ts`.

use reqwest::header::{HeaderName, HeaderValue};
use serde_json::{Value, json};

// ---------------------------------------------------------------------------
// Beta header constants
// ---------------------------------------------------------------------------

/// Prompt caching beta.
pub const PROMPT_CACHING_BETA: &str = "prompt-caching-2024-07-31";

/// PDF support beta.
pub const PDFS_BETA: &str = "pdfs-2024-09-25";

/// Extended thinking beta.
pub const INTERLEAVED_THINKING_BETA: &str = "interleaved-thinking-2025-05-14";

/// Structured outputs beta.
pub const STRUCTURED_OUTPUTS_BETA: &str = "structured-outputs-2025-05-14";

/// Token-efficient tool use beta.
pub const TOKEN_EFFICIENT_TOOLS_BETA: &str = "token-efficient-tools-2025-02-19";

/// Default beta headers for Anthropic first-party requests.
pub const DEFAULT_BETA_HEADERS: &[&str] = &[
    PROMPT_CACHING_BETA,
    PDFS_BETA,
];

// ---------------------------------------------------------------------------
// get_extra_body_params
// ---------------------------------------------------------------------------

/// Assemble extra body parameters for the API request.
///
/// Parses the `CLAUDE_CODE_EXTRA_BODY` environment variable (if present) as
/// a JSON object and merges it with any beta headers.
///
/// # Arguments
///
/// * `beta_headers` — Optional list of beta header strings to include.
///
/// # Returns
///
/// A JSON object representing the extra body parameters.
#[must_use]
pub fn get_extra_body_params(beta_headers: Option<&[String]>) -> Value {
    let mut result = json!({});

    // Parse user-supplied extra body parameters.
    if let Ok(extra_body_str) = std::env::var("CLAUDE_CODE_EXTRA_BODY")
        && !extra_body_str.is_empty()
            && let Ok(parsed) = serde_json::from_str::<Value>(&extra_body_str)
                && parsed.is_object() {
                    // Shallow clone — we don't want to mutate the original.
                    result = parsed;
                }

    // Merge beta headers into anthropic_beta array.
    if let Some(headers) = beta_headers
        && !headers.is_empty() {
            let existing: Vec<String> = result
                .get("anthropic_beta")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(Value::as_str)
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default();

            let merged: Vec<String> = existing
                .into_iter()
                .chain(headers.iter().cloned())
                .collect();

            result["anthropic_beta"] = json!(merged);
        }

    result
}

// ---------------------------------------------------------------------------
// get_beta_headers
// ---------------------------------------------------------------------------

/// Build the list of beta header strings for the API request.
///
/// Includes the default betas and any model-specific betas.  For Bedrock and
/// Vertex AI providers, additional betas may be included.
///
/// # Arguments
///
/// * `model` — The model identifier.
/// * `is_bedrock` — Whether the request targets Amazon Bedrock.
/// * `is_vertex` — Whether the request targets Google Vertex AI.
/// * `enable_caching` — Whether prompt caching is enabled.
/// * `enable_thinking` — Whether extended thinking is enabled.
///
/// # Returns
///
/// A vector of beta header strings.
#[must_use]
pub fn get_beta_headers(
    model: &str,
    is_bedrock: bool,
    is_vertex: bool,
    enable_caching: bool,
    enable_thinking: bool,
) -> Vec<String> {
    let mut betas = Vec::new();

    // Default betas.
    if enable_caching {
        betas.push(PROMPT_CACHING_BETA.to_owned());
    }
    betas.push(PDFS_BETA.to_owned());

    // Model-specific betas.
    let model_lower = model.to_ascii_lowercase();

    // Extended thinking for Claude models.
    if enable_thinking && model_lower.contains("claude") {
        betas.push(INTERLEAVED_THINKING_BETA.to_owned());
    }

    // Token-efficient tools for newer Claude models.
    if model_lower.contains("claude-sonnet-4")
        || model_lower.contains("claude-opus-4")
        || model_lower.contains("claude-3-7")
        || model_lower.contains("claude-3.7")
    {
        betas.push(TOKEN_EFFICIENT_TOOLS_BETA.to_owned());
    }

    // Structured outputs for Claude models.
    if model_lower.contains("claude") {
        betas.push(STRUCTURED_OUTPUTS_BETA.to_owned());
    }

    // Bedrock-specific betas.
    if is_bedrock {
        // Bedrock may need additional betas for compatibility.
        if enable_caching && !betas.contains(&PROMPT_CACHING_BETA.to_owned()) {
            betas.push(PROMPT_CACHING_BETA.to_owned());
        }
    }

    // Vertex AI-specific betas.
    if is_vertex {
        // Vertex AI may need additional betas for compatibility.
        if enable_caching && !betas.contains(&PROMPT_CACHING_BETA.to_owned()) {
            betas.push(PROMPT_CACHING_BETA.to_owned());
        }
    }

    // Deduplicate.
    betas.sort();
    betas.dedup();

    betas
}

/// Build the `anthropic-beta` header value from a list of beta strings.
///
/// # Errors
///
/// Returns an error if the header value contains invalid bytes.
pub fn build_beta_header_value(betas: &[String]) -> Result<HeaderValue, reqwest::header::InvalidHeaderValue> {
    let value = betas.join(",");
    HeaderValue::from_str(&value)
}

/// Build the `anthropic-beta` header as a `(HeaderName, HeaderValue)` pair.
///
/// # Errors
///
/// Returns an error if the header value contains invalid bytes.
pub fn build_beta_header_pair(
    betas: &[String],
) -> Result<(HeaderName, HeaderValue), reqwest::header::InvalidHeaderValue> {
    let name = HeaderName::from_static("anthropic-beta");
    let value = build_beta_header_value(betas)?;
    Ok((name, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_beta_headers_includes_defaults() {
        let betas = get_beta_headers("claude-sonnet-4", false, false, true, false);
        assert!(betas.contains(&PROMPT_CACHING_BETA.to_owned()));
        assert!(betas.contains(&PDFS_BETA.to_owned()));
    }

    #[test]
    fn get_beta_headers_includes_thinking_for_claude() {
        let betas = get_beta_headers("claude-sonnet-4", false, false, true, true);
        assert!(betas.contains(&INTERLEAVED_THINKING_BETA.to_owned()));
    }

    #[test]
    fn get_beta_headers_no_thinking_for_non_claude() {
        let betas = get_beta_headers("gpt-4o", false, false, true, true);
        assert!(!betas.contains(&INTERLEAVED_THINKING_BETA.to_owned()));
    }

    #[test]
    fn get_beta_headers_deduplicates() {
        let betas = get_beta_headers("claude-sonnet-4", true, false, true, false);
        let count = betas.iter().filter(|b| **b == PROMPT_CACHING_BETA).count();
        assert_eq!(count, 1);
    }

    #[test]
    fn get_extra_body_params_without_env_returns_betas() {
        // Test that without CLAUDE_CODE_EXTRA_BODY, only betas are merged.
        let betas = vec!["test-beta-1".to_owned(), "test-beta-2".to_owned()];
        let params = get_extra_body_params(Some(&betas));
        let anthropic_beta = params
            .get("anthropic_beta")
            .and_then(Value::as_array)
            .expect("should have anthropic_beta");
        assert_eq!(anthropic_beta.len(), 2);
    }

    #[test]
    fn get_extra_body_params_none_betas_returns_empty() {
        // When no betas and no env var, result should be empty or have no anthropic_beta.
        let params = get_extra_body_params(None);
        assert!(params
            .get("anthropic_beta")
            .and_then(Value::as_array)
            .is_none_or(|a| a.is_empty()));
    }

    #[test]
    fn build_beta_header_value_joins_with_comma() {
        let betas = vec!["beta-a".to_owned(), "beta-b".to_owned()];
        let value = build_beta_header_value(&betas).expect("should build");
        assert_eq!(value.to_str().expect("utf8"), "beta-a,beta-b");
    }
}

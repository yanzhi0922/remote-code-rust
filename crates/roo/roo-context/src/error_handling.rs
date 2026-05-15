//! Context window error detection.
//!
//! Ported from `.research/Roo-Code/src/core/context/context-management/context-error-handling.ts`.
//!
//! Detects whether a provider error indicates that the context window was exceeded,
//! matching error patterns from OpenAI, OpenRouter, and Anthropic.

use std::sync::LazyLock;

use regex::Regex;
use roo_provider::error::ProviderError;

// ---------------------------------------------------------------------------
// OpenRouter patterns (status 400 + message pattern)
// ---------------------------------------------------------------------------

static OPENROUTER_CONTEXT_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        Regex::new(r"(?i)\bcontext\s*(?:length|window)\b").unwrap(),
        Regex::new(r"(?i)\bmaximum\s*context\b").unwrap(),
        Regex::new(r"(?i)\b(?:input\s*)?tokens?\s*exceed\b").unwrap(),
        Regex::new(r"(?i)\btoo\s*many\s*tokens?\b").unwrap(),
    ]
});

// ---------------------------------------------------------------------------
// Anthropic patterns (invalid_request_error + message pattern)
// ---------------------------------------------------------------------------

static ANTHROPIC_CONTEXT_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    vec![
        Regex::new(r"(?i)prompt is too long").unwrap(),
        Regex::new(r"(?i)maximum.*tokens").unwrap(),
        Regex::new(r"(?i)context.*too.*long").unwrap(),
        Regex::new(r"(?i)exceeds.*context").unwrap(),
        Regex::new(r"(?i)token.*limit").unwrap(),
        Regex::new(r"(?i)context_length_exceeded").unwrap(),
        Regex::new(r"(?i)max_tokens_to_sample").unwrap(),
    ]
});

// ---------------------------------------------------------------------------
// OpenAI known substrings
// ---------------------------------------------------------------------------

const OPENAI_CONTEXT_ERROR_SUBSTRINGS: &[&str] = &["token", "context length"];

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Check if an error indicates the context window was exceeded.
///
/// Matches error patterns from three providers:
/// - **OpenAI**: `LengthFinishReasonError` name, or API error with code 400
///   and "token" / "context length" in the message.
/// - **OpenRouter**: Status 400 combined with regex patterns on the message
///   (context length/window, maximum context, tokens exceed, too many tokens).
/// - **Anthropic**: `invalid_request_error` type combined with patterns on the
///   message (prompt too long, maximum tokens, context too long, etc.).
///
/// Ported from `checkContextWindowExceededError()` in the TS source.
pub fn check_context_window_exceeded_error(error: &ProviderError) -> bool {
    check_is_openai_context_error(error)
        || check_is_openrouter_context_error(error)
        || check_is_anthropic_context_error(error)
}

// ---------------------------------------------------------------------------
// Provider-specific checkers
// ---------------------------------------------------------------------------

/// Check for OpenAI-specific context window errors.
///
/// In the TS source this checks for:
/// 1. `error.name === "LengthFinishReasonError"` — in Rust we match on the
///    error message containing that string.
/// 2. `error instanceof APIError && error.code === 400 && message includes
///    "token" or "context length"` — we match on `ApiErrorResponse` with
///    status 400 and the same substring checks.
fn check_is_openai_context_error(error: &ProviderError) -> bool {
    let message = error_to_message(error);

    // Check for LengthFinishReasonError by name in the message.
    if message.contains("LengthFinishReasonError") {
        return true;
    }

    // Check for API error with code 400 and known substrings.
    match error {
        ProviderError::ApiError(_, msg) => OPENAI_CONTEXT_ERROR_SUBSTRINGS
            .iter()
            .any(|substr| msg.contains(substr)),
        ProviderError::ApiErrorResponse(_, status, msg) if *status == 400 => {
            OPENAI_CONTEXT_ERROR_SUBSTRINGS
                .iter()
                .any(|substr| msg.contains(substr))
        }
        _ => false,
    }
}

/// Check for OpenRouter-specific context window errors.
///
/// In the TS source this checks for:
/// - `status === "400"` (as a string comparison)
/// - Message matches one of several regex patterns.
///
/// In Rust we check `ApiErrorResponse` with status 400 or `ApiError` (which
/// may carry status info in the message), and match the same regex patterns.
fn check_is_openrouter_context_error(error: &ProviderError) -> bool {
    let message = error_to_message(error);
    let is_status_400 = match error {
        ProviderError::ApiErrorResponse(_, status, _) => *status == 400,
        ProviderError::Other(msg) => msg.contains("400"),
        _ => false,
    };

    if !is_status_400 {
        return false;
    }

    OPENROUTER_CONTEXT_PATTERNS
        .iter()
        .any(|pattern| pattern.is_match(&message))
}

/// Check for Anthropic-specific context window errors.
///
/// In the TS source this checks:
/// - `res.error.error.type === "invalid_request_error"`
/// - Message matches one of several regex patterns.
/// - Additionally checks for `error.error.code` being
///   `"context_length_exceeded"` or `"invalid_request_error"`.
///
/// In Rust we look at the error message for the type indicator and the
/// patterns. The `ApiErrorResponse` variant carries status and message,
/// which is where Anthropic errors surface.
fn check_is_anthropic_context_error(error: &ProviderError) -> bool {
    let message = error_to_message(error);

    // Check if the error looks like an Anthropic invalid_request_error.
    let is_anthropic_invalid_request = match error {
        ProviderError::ApiErrorResponse(provider, _status, msg) => {
            // Anthropic errors come through as ApiErrorResponse.
            let is_anthropic_provider = provider.to_lowercase().contains("anthropic")
                || msg.contains("invalid_request_error");
            is_anthropic_provider
        }
        ProviderError::ApiError(_, msg) => msg.contains("invalid_request_error"),
        ProviderError::Other(msg) => msg.contains("invalid_request_error"),
        _ => false,
    };

    if !is_anthropic_invalid_request {
        return false;
    }

    // Check if any context-window pattern matches.
    ANTHROPIC_CONTEXT_PATTERNS
        .iter()
        .any(|pattern| pattern.is_match(&message))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the message string from a `ProviderError`.
fn error_to_message(error: &ProviderError) -> String {
    match error {
        ProviderError::ApiError(_, msg) => msg.clone(),
        ProviderError::ApiErrorResponse(_, _, msg) => msg.clone(),
        ProviderError::StreamError(msg) => msg.clone(),
        ProviderError::ParseError(msg) => msg.clone(),
        ProviderError::Timeout(_) => String::new(),
        ProviderError::RateLimitExceeded => String::new(),
        ProviderError::ApiKeyRequired => String::new(),
        ProviderError::UnsupportedModel(msg) => msg.clone(),
        ProviderError::RetiredProvider => String::new(),
        ProviderError::Other(msg) => msg.clone(),
        ProviderError::Reqwest(e) => e.to_string(),
        ProviderError::Json(e) => e.to_string(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- OpenAI tests ---

    #[test]
    fn test_openai_length_finish_reason_error() {
        let error = ProviderError::Other("LengthFinishReasonError: max tokens reached".to_string());
        assert!(check_is_openai_context_error(&error));
        assert!(check_context_window_exceeded_error(&error));
    }

    #[test]
    fn test_openai_api_error_with_token() {
        let error =
            ProviderError::ApiError("openai".to_string(), "max token limit reached".to_string());
        assert!(check_is_openai_context_error(&error));
    }

    #[test]
    fn test_openai_api_error_response_with_context_length() {
        let error = ProviderError::ApiErrorResponse(
            "openai".to_string(),
            400,
            "This model's maximum context length is 4096 tokens".to_string(),
        );
        assert!(check_is_openai_context_error(&error));
    }

    #[test]
    fn test_openai_api_error_response_wrong_status() {
        let error = ProviderError::ApiErrorResponse(
            "openai".to_string(),
            429,
            "max token limit reached".to_string(),
        );
        assert!(!check_is_openai_context_error(&error));
    }

    #[test]
    fn test_openai_api_error_no_token() {
        let error = ProviderError::ApiError("openai".to_string(), "server error".to_string());
        assert!(!check_is_openai_context_error(&error));
    }

    // --- OpenRouter tests ---

    #[test]
    fn test_openrouter_context_length() {
        let error = ProviderError::ApiErrorResponse(
            "openrouter".to_string(),
            400,
            "context length exceeded".to_string(),
        );
        assert!(check_is_openrouter_context_error(&error));
    }

    #[test]
    fn test_openrouter_context_window() {
        let error = ProviderError::ApiErrorResponse(
            "openrouter".to_string(),
            400,
            "context window is full".to_string(),
        );
        assert!(check_is_openrouter_context_error(&error));
    }

    #[test]
    fn test_openrouter_maximum_context() {
        let error = ProviderError::ApiErrorResponse(
            "openrouter".to_string(),
            400,
            "maximum context reached".to_string(),
        );
        assert!(check_is_openrouter_context_error(&error));
    }

    #[test]
    fn test_openrouter_tokens_exceed() {
        let error = ProviderError::ApiErrorResponse(
            "openrouter".to_string(),
            400,
            "input tokens exceed the limit".to_string(),
        );
        assert!(check_is_openrouter_context_error(&error));
    }

    #[test]
    fn test_openrouter_too_many_tokens() {
        let error = ProviderError::ApiErrorResponse(
            "openrouter".to_string(),
            400,
            "too many tokens in request".to_string(),
        );
        assert!(check_is_openrouter_context_error(&error));
    }

    #[test]
    fn test_openrouter_wrong_status() {
        let error = ProviderError::ApiErrorResponse(
            "openrouter".to_string(),
            500,
            "context length exceeded".to_string(),
        );
        assert!(!check_is_openrouter_context_error(&error));
    }

    #[test]
    fn test_openrouter_wrong_message() {
        let error = ProviderError::ApiErrorResponse(
            "openrouter".to_string(),
            400,
            "internal server error".to_string(),
        );
        assert!(!check_is_openrouter_context_error(&error));
    }

    // --- Anthropic tests ---

    #[test]
    fn test_anthropic_prompt_too_long() {
        let error = ProviderError::ApiErrorResponse(
            "anthropic".to_string(),
            400,
            "prompt is too long: 300000 tokens > 200000 maximum".to_string(),
        );
        assert!(check_is_anthropic_context_error(&error));
    }

    #[test]
    fn test_anthropic_maximum_tokens() {
        let error = ProviderError::ApiErrorResponse(
            "anthropic".to_string(),
            400,
            "maximum tokens exceeded in request".to_string(),
        );
        assert!(check_is_anthropic_context_error(&error));
    }

    #[test]
    fn test_anthropic_context_too_long() {
        let error = ProviderError::ApiErrorResponse(
            "anthropic".to_string(),
            400,
            "context is too long for this model".to_string(),
        );
        assert!(check_is_anthropic_context_error(&error));
    }

    #[test]
    fn test_anthropic_context_length_exceeded() {
        let error = ProviderError::ApiErrorResponse(
            "anthropic".to_string(),
            400,
            "context_length_exceeded: request too large".to_string(),
        );
        assert!(check_is_anthropic_context_error(&error));
    }

    #[test]
    fn test_anthropic_invalid_request_error_in_message() {
        let error = ProviderError::Other("invalid_request_error: prompt is too long".to_string());
        assert!(check_is_anthropic_context_error(&error));
    }

    #[test]
    fn test_anthropic_no_match() {
        let error = ProviderError::ApiErrorResponse(
            "anthropic".to_string(),
            400,
            "some other error".to_string(),
        );
        assert!(!check_is_anthropic_context_error(&error));
    }

    // --- Combined check tests ---

    #[test]
    fn test_combined_openai_error() {
        let error = ProviderError::ApiError(
            "openai".to_string(),
            "context length exceeded for model".to_string(),
        );
        assert!(check_context_window_exceeded_error(&error));
    }

    #[test]
    fn test_combined_non_context_error() {
        let error = ProviderError::RateLimitExceeded;
        assert!(!check_context_window_exceeded_error(&error));
    }

    #[test]
    fn test_combined_timeout_error() {
        let error = ProviderError::Timeout(30000);
        assert!(!check_context_window_exceeded_error(&error));
    }
}

//! Attribution header construction and commit/PR attribution strings.
//!
//! Builds the `x-attribution` HTTP header and the `x-anthropic-billing-header`
//! text block embedded in the system prompt.
//!
//! Also provides [`get_commit_attribution`] and [`get_pr_attribution`] matching
//! TS `getAttributionTexts()` for commit message `Co-Authored-By` lines and
//! PR description footers.
//!
//! Based on upstream Claude Code's `getAttributionHeader` in
//! `constants/system.ts` and `computeFingerprint` in `utils/fingerprint.ts`.

use reqwest::header::{HeaderName, HeaderValue};

/// The HTTP attribution header name.
pub const ATTRIBUTION_HEADER: &str = "x-attribution";

/// Build the `x-attribution` HTTP header value.
///
/// The official CLI sends a JSON object with `client` and `version` keys.
/// We match the exact format used by Claude Code.
pub fn build_attribution_header() -> Result<HeaderValue, reqwest::header::InvalidHeaderValue> {
    let value = format!(
        r#"{{"client":"claude-code","version":"{}"}}"#,
        claude_config::runtime_version()
    );
    HeaderValue::from_str(&value)
}

/// Build the attribution header as a `(HeaderName, HeaderValue)` pair.
pub fn build_attribution_header_pair()
-> Result<(HeaderName, HeaderValue), reqwest::header::InvalidHeaderValue> {
    let name = HeaderName::from_static(ATTRIBUTION_HEADER);
    let value = build_attribution_header()?;
    Ok((name, value))
}

/// Build the `x-anthropic-billing-header` text that is prepended as the
/// **first** block of the system prompt array.
///
/// Format: `x-anthropic-billing-header: cc_version=VERSION.FINGERPRINT; cc_entrypoint=ENTRYPOINT;`
///
/// This matches the TS reference `getAttributionHeader` in `constants/system.ts`.
pub fn build_billing_attribution_text(fingerprint: &str) -> String {
    let version = claude_config::runtime_version();
    let entrypoint = std::env::var("CLAUDE_CODE_ENTRYPOINT")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "cli".to_owned());
    format!(
        "x-anthropic-billing-header: cc_version={version}.{fingerprint}; cc_entrypoint={entrypoint};"
    )
}

// ---------------------------------------------------------------------------
// Commit / PR attribution — mirrors TS `getAttributionTexts()`
// ---------------------------------------------------------------------------

/// Product URL for attribution footers.
const PRODUCT_URL: &str = "https://claude.com/claude-code";

/// Get the public-facing model name for attribution.
///
/// Matches TS `getPublicModelName()` — maps internal IDs to user-facing names.
/// Unknown models fall back to "Claude Opus 4.7" to avoid leaking codenames.
fn get_public_model_name(model: &str) -> String {
    let lower = model.to_ascii_lowercase();
    if lower.contains("claude-opus-4-7") {
        return "Claude Opus 4.7".to_owned();
    }
    if lower.contains("claude-opus-4-6") {
        return "Claude Opus 4.6".to_owned();
    }
    if lower.contains("claude-sonnet-4-6") {
        return "Claude Sonnet 4.6".to_owned();
    }
    if lower.contains("claude-sonnet-4-5") {
        return "Claude Sonnet 4.5".to_owned();
    }
    if lower.contains("claude-haiku-4-5") {
        return "Claude Haiku 4.5".to_owned();
    }
    if lower.contains("claude-sonnet-4") {
        return "Claude Sonnet 4".to_owned();
    }
    if lower.contains("claude-opus-4") {
        return "Claude Opus 4".to_owned();
    }
    // @[MODEL LAUNCH]: Update the fallback name below to avoid leaking codenames.
    "Claude Opus 4.7".to_owned()
}

/// Get the commit attribution string (`Co-Authored-By: ...`).
///
/// Matches TS `getAttributionTexts().commit`. Returns an empty string when
/// `CLAUDE_CODE_INCLUDE_CO_AUTHORED_BY` is set to `"false"`.
pub fn get_commit_attribution(model: &str) -> String {
    // Backward compatibility: deprecated includeCoAuthoredBy setting.
    if std::env::var("CLAUDE_CODE_INCLUDE_CO_AUTHORED_BY").as_deref() == Ok("false") {
        return String::new();
    }
    let model_name = get_public_model_name(model);
    format!("Co-Authored-By: {model_name} <noreply@anthropic.com>")
}

/// Get the PR attribution footer string.
///
/// Matches TS `getAttributionTexts().pr`. Returns an empty string when
/// `CLAUDE_CODE_INCLUDE_CO_AUTHORED_BY` is set to `"false"`.
pub fn get_pr_attribution(_model: &str) -> String {
    // Backward compatibility: deprecated includeCoAuthoredBy setting.
    if std::env::var("CLAUDE_CODE_INCLUDE_CO_AUTHORED_BY").as_deref() == Ok("false") {
        return String::new();
    }
    format!("Generated with [Claude Code]({PRODUCT_URL})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribution_header_format() {
        let header = build_attribution_header().expect("should build header");
        let value = header.to_str().expect("should be valid UTF-8");
        assert!(value.contains("\"client\":\"claude-code\""));
        assert!(value.contains("\"version\""));
    }

    #[test]
    fn attribution_header_pair_is_valid() {
        let (name, value) = build_attribution_header_pair().expect("should build pair");
        assert_eq!(name.as_str(), ATTRIBUTION_HEADER);
        assert!(value.to_str().is_ok());
    }

    #[test]
    fn billing_attribution_text_format() {
        let text = build_billing_attribution_text("abc");
        assert!(text.starts_with("x-anthropic-billing-header: cc_version="));
        assert!(text.contains(".abc;"));
        assert!(text.contains("cc_entrypoint="));
    }

    #[test]
    fn commit_attribution_contains_co_authored_by() {
        let text = get_commit_attribution("claude-sonnet-4-6");
        assert!(text.contains("Co-Authored-By:"));
        assert!(text.contains("noreply@anthropic.com"));
    }

    #[test]
    fn commit_attribution_uses_public_model_name() {
        let text = get_commit_attribution("claude-opus-4-7");
        assert!(text.contains("Claude Opus 4.7"));
    }

    #[test]
    fn commit_attribution_unknown_model_uses_fallback() {
        let text = get_commit_attribution("some-unknown-model");
        assert!(text.contains("Claude Opus 4.7"));
    }

    #[test]
    fn pr_attribution_contains_product_url() {
        let text = get_pr_attribution("claude-sonnet-4-6");
        assert!(text.contains("Generated with"));
        assert!(text.contains("claude.com"));
    }

    #[test]
    fn commit_attribution_respects_disable_env() {
        // SAFETY: test-only, single-threaded.
        unsafe { std::env::set_var("CLAUDE_CODE_INCLUDE_CO_AUTHORED_BY", "false"); }
        let text = get_commit_attribution("claude-sonnet-4-6");
        assert!(text.is_empty());
        unsafe { std::env::remove_var("CLAUDE_CODE_INCLUDE_CO_AUTHORED_BY"); }
    }

    #[test]
    fn pr_attribution_respects_disable_env() {
        // SAFETY: test-only, single-threaded.
        unsafe { std::env::set_var("CLAUDE_CODE_INCLUDE_CO_AUTHORED_BY", "false"); }
        let text = get_pr_attribution("claude-sonnet-4-6");
        assert!(text.is_empty());
        unsafe { std::env::remove_var("CLAUDE_CODE_INCLUDE_CO_AUTHORED_BY"); }
    }
}

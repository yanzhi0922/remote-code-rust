//! Attribution header construction.
//!
//! Builds the `x-attribution` header sent with API requests to identify the
//! client type, version, and session context.
//!
//! Based on upstream Claude Code's `getAttributionHeader` in
//! `constants/system.ts`.

use reqwest::header::{HeaderName, HeaderValue};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// The attribution header name.
pub const ATTRIBUTION_HEADER: &str = "x-attribution";

/// Client identifier.
const CLIENT_ID: &str = "remote-code-rust";

// ---------------------------------------------------------------------------
// build_attribution_header
// ---------------------------------------------------------------------------

/// Build the attribution header value.
///
/// The value is a JSON object containing:
/// - `client`: The client identifier (`"remote-code-rust"`)
/// - `version`: The crate version from `CARGO_PKG_VERSION`
///
/// # Errors
///
/// Returns an error if the header value contains invalid bytes.
pub fn build_attribution_header() -> Result<HeaderValue, reqwest::header::InvalidHeaderValue> {
    let value = format!(
        r#"{{"client":"{CLIENT_ID}","version":"{}"}}"#,
        env!("CARGO_PKG_VERSION")
    );
    HeaderValue::from_str(&value)
}

/// Build the attribution header as a `(HeaderName, HeaderValue)` pair.
///
/// Convenience function for adding to a `HeaderMap`.
///
/// # Errors
///
/// Returns an error if the header value contains invalid bytes.
pub fn build_attribution_header_pair(
) -> Result<(HeaderName, HeaderValue), reqwest::header::InvalidHeaderValue> {
    let name = HeaderName::from_static(ATTRIBUTION_HEADER);
    let value = build_attribution_header()?;
    Ok((name, value))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribution_header_contains_client_id() {
        let header = build_attribution_header().expect("should build header");
        let value = header.to_str().expect("should be valid UTF-8");
        assert!(value.contains(CLIENT_ID));
        assert!(value.contains("version"));
    }

    #[test]
    fn attribution_header_pair_is_valid() {
        let (name, value) = build_attribution_header_pair().expect("should build pair");
        assert_eq!(name.as_str(), ATTRIBUTION_HEADER);
        assert!(value.to_str().is_ok());
    }
}

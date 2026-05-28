//! Webhook input sanitization — validate and clean incoming webhook payloads.
//!
//! Native Rust implementation of `claude-code-rev/src/bridge/webhookSanitizer.ts`.
//! Provides structured validation for webhook payloads from GitHub, Slack, and
//! generic JSON sources.

use std::fmt::Write;

use anyhow::{Result, anyhow};
use serde_json::Value;

/// Maximum allowed webhook payload size (10 MB).
const MAX_WEBHOOK_PAYLOAD_SIZE: usize = 10 * 1024 * 1024;

/// Maximum nesting depth for JSON payloads.
const MAX_NESTING_DEPTH: usize = 20;

/// Sanitize a webhook payload by validating size, depth, and content type.
///
/// Returns the sanitized payload or an error if validation fails.
pub fn sanitize_webhook_payload(body: &[u8], content_type: Option<&str>) -> Result<Value> {
    // 1. Size check
    if body.len() > MAX_WEBHOOK_PAYLOAD_SIZE {
        return Err(anyhow!(
            "webhook payload too large: {} bytes (max {})",
            body.len(),
            MAX_WEBHOOK_PAYLOAD_SIZE
        ));
    }

    // 2. Content-type check
    if let Some(ct) = content_type {
        if !ct.contains("json") && !ct.contains("application/x-www-form-urlencoded") {
            return Err(anyhow!("unsupported content type: {ct}"));
        }
    }

    // 3. Parse JSON
    let value: Value = serde_json::from_slice(body)?;

    // 4. Depth check
    check_depth(&value, 0)?;

    // 5. Strip sensitive fields
    Ok(strip_sensitive_fields(value))
}

/// Check that nesting depth does not exceed the limit.
fn check_depth(value: &Value, depth: usize) -> Result<()> {
    if depth > MAX_NESTING_DEPTH {
        return Err(anyhow!(
            "webhook payload nesting exceeds {MAX_NESTING_DEPTH} levels"
        ));
    }
    match value {
        Value::Object(map) => {
            for v in map.values() {
                check_depth(v, depth + 1)?;
            }
        }
        Value::Array(arr) => {
            for v in arr {
                check_depth(v, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Strip sensitive fields from a webhook payload.
fn strip_sensitive_fields(mut value: Value) -> Value {
    if let Value::Object(ref mut map) = value {
        let sensitive_keys = [
            "secret",
            "token",
            "password",
            "api_key",
            "apiKey",
            "access_token",
            "accessToken",
            "private_key",
            "privateKey",
            "ssh_key",
            "sshKey",
        ];
        map.retain(|k, _| !sensitive_keys.contains(&k.as_str()));
        for v in map.values_mut() {
            *v = strip_sensitive_fields(v.clone());
        }
    }
    if let Value::Array(arr) = &value {
        return Value::Array(
            arr.iter()
                .map(|v| strip_sensitive_fields(v.clone()))
                .collect(),
        );
    }
    value
}

/// GitHub webhook event types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitHubEvent {
    Push,
    PullRequest,
    Issues,
    IssueComment,
    Release,
    Unknown,
}

/// Identify the GitHub webhook event type from headers.
pub fn identify_github_event(event_header: Option<&str>) -> GitHubEvent {
    match event_header {
        Some("push") => GitHubEvent::Push,
        Some("pull_request") => GitHubEvent::PullRequest,
        Some("issues") => GitHubEvent::Issues,
        Some("issue_comment") => GitHubEvent::IssueComment,
        Some("release") => GitHubEvent::Release,
        _ => GitHubEvent::Unknown,
    }
}

/// Validate a GitHub webhook signature using HMAC-SHA256.
///
/// Returns `true` if the signature is valid. When no secret is configured,
/// validation is skipped (returns `true`).
pub fn validate_github_signature(
    body: &[u8],
    signature_header: Option<&str>,
    secret: Option<&str>,
) -> bool {
    let Some(sec) = secret else {
        // No secret configured — skip validation
        return true;
    };
    let Some(sig) = signature_header else {
        // Secret configured but no signature — reject
        return false;
    };

    let expected = hex_encode_hmac_sha256(body, sec);
    let provided = sig.strip_prefix("sha256=").unwrap_or(sig);
    constant_time_eq(expected.as_bytes(), provided.as_bytes())
}

fn hex_encode_hmac_sha256(body: &[u8], secret: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    let mut mac = Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC key");
    mac.update(body);
    let result = mac.finalize();
    let bytes = result.into_bytes();
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in &bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

// Constant-time comparison to prevent timing attacks
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_oversized_payload() {
        let large = vec![0u8; MAX_WEBHOOK_PAYLOAD_SIZE + 1];
        assert!(sanitize_webhook_payload(&large, Some("application/json")).is_err());
    }

    #[test]
    fn rejects_deeply_nested_payload() {
        let mut val = serde_json::json!({"a": {"b": {"c": {"d": {"e": {}}}}}});
        for _ in 0..25 {
            val = serde_json::json!({"a": val});
        }
        let bytes = serde_json::to_vec(&val).unwrap();
        assert!(sanitize_webhook_payload(&bytes, Some("application/json")).is_err());
    }

    #[test]
    fn strips_sensitive_fields() {
        let payload = serde_json::json!({
            "action": "push",
            "token": "secret123",
            "ref": "main"
        });
        let result = sanitize_webhook_payload(
            &serde_json::to_vec(&payload).unwrap(),
            Some("application/json"),
        )
        .unwrap();
        assert!(result.get("token").is_none());
        assert_eq!(result["action"], "push");
    }

    #[test]
    fn identifies_github_events() {
        assert_eq!(identify_github_event(Some("push")), GitHubEvent::Push);
        assert_eq!(identify_github_event(None), GitHubEvent::Unknown);
    }

    #[test]
    fn validates_signature() {
        let body = b"{\"test\":true}";
        let secret = "my-secret";
        let sig = format!("sha256={}", hex_encode_hmac_sha256(body, secret));
        assert!(validate_github_signature(body, Some(&sig), Some(secret)));
        assert!(!validate_github_signature(
            body,
            Some(&sig),
            Some("wrong-secret")
        ));
    }

    #[test]
    fn valid_no_signature_when_no_secret() {
        let body = b"{}";
        assert!(validate_github_signature(body, Some("sha256=bad"), None));
    }
}

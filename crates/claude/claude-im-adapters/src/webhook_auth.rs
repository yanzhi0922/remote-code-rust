use anyhow::{Result, anyhow};
use sha2::{Digest, Sha256};

const MIN_WEBHOOK_SECRET_LEN: usize = 16;

pub fn validate_webhook_secret(secret: &str) -> Result<()> {
    if secret.len() < MIN_WEBHOOK_SECRET_LEN {
        return Err(anyhow!(
            "webhook secret must be at least {MIN_WEBHOOK_SECRET_LEN} characters"
        ));
    }
    if !secret
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(anyhow!(
            "webhook secret may contain only ASCII letters, digits, '-', '_' or '.'"
        ));
    }
    Ok(())
}

pub fn verify_webhook_secret(provided: &str, expected: &str) -> bool {
    let provided_digest = Sha256::digest(provided.as_bytes());
    let expected_digest = Sha256::digest(expected.as_bytes());
    constant_time_eq::constant_time_eq_32(&provided_digest.into(), &expected_digest.into())
}

pub fn append_secret_to_webhook_url(webhook_url: &str, secret: &str) -> Result<String> {
    validate_webhook_secret(secret)?;
    let url = webhook_url.trim_end_matches('/');
    let suffix = format!("/{secret}");
    if url.ends_with(&suffix) {
        return Ok(url.to_owned());
    }
    Ok(format!("{url}{suffix}"))
}

#[cfg(test)]
mod tests {
    use super::{append_secret_to_webhook_url, validate_webhook_secret, verify_webhook_secret};

    #[test]
    fn webhook_secret_requires_minimum_entropy() {
        assert!(validate_webhook_secret("short").is_err());
        assert!(validate_webhook_secret("valid-secret_1234").is_ok());
        assert!(validate_webhook_secret("invalid secret value").is_err());
    }

    #[test]
    fn webhook_secret_comparison_matches_only_expected_value() {
        assert!(verify_webhook_secret(
            "valid-secret_1234",
            "valid-secret_1234"
        ));
        assert!(!verify_webhook_secret(
            "valid-secret_1234",
            "different-secret"
        ));
    }

    #[test]
    fn webhook_url_appends_secret_once() {
        assert_eq!(
            append_secret_to_webhook_url("https://example.invalid/webhook", "valid-secret_1234")
                .expect("url should append"),
            "https://example.invalid/webhook/valid-secret_1234"
        );
        assert_eq!(
            append_secret_to_webhook_url(
                "https://example.invalid/webhook/valid-secret_1234",
                "valid-secret_1234"
            )
            .expect("url should stay stable"),
            "https://example.invalid/webhook/valid-secret_1234"
        );
    }
}

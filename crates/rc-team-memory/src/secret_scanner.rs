//! Secret scanning for memory safety.
//!
//! Scans content for common secret patterns (API keys, passwords, tokens,
//! private keys, connection strings) using regex-based detection.

use regex::Regex;
use serde::{Deserialize, Serialize};

/// The kind of secret detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SecretKind {
    /// API key patterns (e.g. `sk-...`, `AKIA...`).
    ApiKey,
    /// Password assignments in config files.
    Password,
    /// Bearer / OAuth tokens.
    Token,
    /// PEM-encoded private keys.
    PrivateKey,
    /// Database or service connection strings.
    ConnectionString,
}

impl std::fmt::Display for SecretKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ApiKey => write!(f, "api_key"),
            Self::Password => write!(f, "password"),
            Self::Token => write!(f, "token"),
            Self::PrivateKey => write!(f, "private_key"),
            Self::ConnectionString => write!(f, "connection_string"),
        }
    }
}

/// A single secret finding within scanned content.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretFinding {
    /// What kind of secret was detected.
    pub kind: SecretKind,
    /// Byte offset where the secret starts.
    pub start: usize,
    /// Byte offset where the secret ends.
    pub end: usize,
    /// Confidence score in `[0.0, 1.0]`.
    pub confidence: f64,
}

/// Result of scanning content for secrets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretScanResult {
    /// Whether any secrets were found.
    pub has_secrets: bool,
    /// All findings.
    pub findings: Vec<SecretFinding>,
}

impl SecretScanResult {
    /// An empty result with no findings.
    pub fn clean() -> Self {
        Self {
            has_secrets: false,
            findings: Vec::new(),
        }
    }
}

/// Scanner that detects secrets in text content using regex patterns.
pub struct SecretScanner {
    patterns: Vec<(SecretKind, Regex, f64)>,
}

impl SecretScanner {
    /// Create a new scanner with built-in patterns.
    pub fn new() -> Self {
        let patterns: Vec<(SecretKind, Regex, f64)> = vec![
            // AWS Access Key ID
            (
                SecretKind::ApiKey,
                Regex::new(r"AKIA[0-9A-Z]{16}").expect("regex"),
                0.95,
            ),
            // Generic API key patterns
            (
                SecretKind::ApiKey,
                Regex::new(r#"(?i)(api[_-]?key|apikey)\s*[:=]\s*['"]?[A-Za-z0-9_\-]{20,}['"]?"#)
                    .expect("regex"),
                0.80,
            ),
            // OpenAI-style keys
            (
                SecretKind::ApiKey,
                Regex::new(r"sk-[A-Za-z0-9]{20,}").expect("regex"),
                0.90,
            ),
            // Password assignments
            (
                SecretKind::Password,
                Regex::new(r#"(?i)(password|passwd|pwd)\s*[:=]\s*['"]?[^\s'"]{8,}['"]?"#)
                    .expect("regex"),
                0.75,
            ),
            // Bearer tokens
            (
                SecretKind::Token,
                Regex::new(r"(?i)bearer\s+[A-Za-z0-9\-._~+/]+=*").expect("regex"),
                0.90,
            ),
            // Generic token assignments
            (
                SecretKind::Token,
                Regex::new(r#"(?i)(token|access_token|secret_token)\s*[:=]\s*['"]?[A-Za-z0-9_\-]{20,}['"]?"#)
                    .expect("regex"),
                0.75,
            ),
            // GitHub tokens
            (
                SecretKind::Token,
                Regex::new(r"gh[ps]_[A-Za-z0-9]{36}").expect("regex"),
                0.95,
            ),
            // PEM private keys
            (
                SecretKind::PrivateKey,
                Regex::new(r"-----BEGIN\s+(?:RSA\s+)?PRIVATE\s+KEY-----").expect("regex"),
                0.99,
            ),
            // Connection strings
            (
                SecretKind::ConnectionString,
                Regex::new(r#"(?i)(?:mongodb|postgres|mysql|redis|amqp)://[^\s'"]{10,}"#)
                    .expect("regex"),
                0.85,
            ),
        ];

        Self { patterns }
    }

    /// Scan content for secrets.
    pub fn scan_content(&self, content: &str) -> SecretScanResult {
        let mut findings = Vec::new();

        for (kind, regex, confidence) in &self.patterns {
            for mat in regex.find_iter(content) {
                findings.push(SecretFinding {
                    kind: *kind,
                    start: mat.start(),
                    end: mat.end(),
                    confidence: *confidence,
                });
            }
        }

        // Sort by position
        findings.sort_by_key(|f| f.start);

        let has_secrets = !findings.is_empty();
        SecretScanResult {
            has_secrets,
            findings,
        }
    }

    /// Check whether content is safe (contains no detected secrets).
    pub fn is_safe(&self, content: &str) -> bool {
        !self.scan_content(content).has_secrets
    }
}

impl Default for SecretScanner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_content() {
        let scanner = SecretScanner::new();
        let result = scanner.scan_content("Hello, this is safe content.");
        assert!(!result.has_secrets);
        assert!(result.findings.is_empty());
    }

    #[test]
    fn detect_aws_key() {
        let scanner = SecretScanner::new();
        let result = scanner.scan_content(r"key=AKIAIOSFODNN7EXAMPLE");
        assert!(result.has_secrets);
        assert!(result.findings.iter().any(|f| f.kind == SecretKind::ApiKey));
    }

    #[test]
    fn detect_openai_key() {
        let scanner = SecretScanner::new();
        let result = scanner.scan_content(r"sk-abcdefghijklmnopqrstuvwxyz1234567890");
        assert!(result.has_secrets);
        assert!(result.findings.iter().any(|f| f.kind == SecretKind::ApiKey));
    }

    #[test]
    fn detect_password() {
        let scanner = SecretScanner::new();
        let result = scanner.scan_content(r#"password = 'supersecretpassword123'"#);
        assert!(result.has_secrets);
        assert!(
            result
                .findings
                .iter()
                .any(|f| f.kind == SecretKind::Password)
        );
    }

    #[test]
    fn detect_bearer_token() {
        let scanner = SecretScanner::new();
        let result = scanner.scan_content(r"Authorization: bearer abc123def456ghi789");
        assert!(result.has_secrets);
        assert!(result.findings.iter().any(|f| f.kind == SecretKind::Token));
    }

    #[test]
    fn detect_github_token() {
        let scanner = SecretScanner::new();
        let result = scanner.scan_content(r"ghp_ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghij");
        assert!(result.has_secrets);
        assert!(result.findings.iter().any(|f| f.kind == SecretKind::Token));
    }

    #[test]
    fn detect_private_key() {
        let scanner = SecretScanner::new();
        let result = scanner.scan_content(
            r"-----BEGIN RSA PRIVATE KEY-----
MIIE...",
        );
        assert!(result.has_secrets);
        assert!(
            result
                .findings
                .iter()
                .any(|f| f.kind == SecretKind::PrivateKey)
        );
    }

    #[test]
    fn detect_connection_string() {
        let scanner = SecretScanner::new();
        let result = scanner.scan_content(r"postgres://user:pass@host:5432/mydb");
        assert!(result.has_secrets);
        assert!(
            result
                .findings
                .iter()
                .any(|f| f.kind == SecretKind::ConnectionString)
        );
    }

    #[test]
    fn is_safe_method() {
        let scanner = SecretScanner::new();
        assert!(scanner.is_safe("normal text"));
        assert!(!scanner.is_safe(r#"password = 'secret12345'"#));
    }

    #[test]
    fn multiple_findings() {
        let scanner = SecretScanner::new();
        let content = r#"password = 'secret12345'
api_key=abcdef1234567890abcdef1234567890"#;
        let result = scanner.scan_content(content);
        assert!(result.findings.len() >= 2);
    }

    #[test]
    fn secret_kind_display() {
        assert_eq!(SecretKind::ApiKey.to_string(), "api_key");
        assert_eq!(SecretKind::Password.to_string(), "password");
        assert_eq!(SecretKind::Token.to_string(), "token");
        assert_eq!(SecretKind::PrivateKey.to_string(), "private_key");
        assert_eq!(
            SecretKind::ConnectionString.to_string(),
            "connection_string"
        );
    }

    #[test]
    fn clean_result() {
        let result = SecretScanResult::clean();
        assert!(!result.has_secrets);
        assert!(result.findings.is_empty());
    }

    #[test]
    fn default_trait() {
        let scanner = SecretScanner::default();
        let result = scanner.scan_content("safe");
        assert!(!result.has_secrets);
    }
}

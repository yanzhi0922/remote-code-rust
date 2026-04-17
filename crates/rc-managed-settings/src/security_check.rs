//! Settings security validation.
//!
//! Validates settings against security policies to prevent unsafe or
//! unauthorized configuration changes.

use regex::Regex;
use serde::{Deserialize, Serialize};

/// Risk level of a setting change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskLevel {
    /// No risk — safe to apply.
    Low,
    /// Minor concern — should be logged.
    Medium,
    /// Significant risk — requires confirmation.
    High,
    /// Must be blocked.
    Critical,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Low => write!(f, "low"),
            Self::Medium => write!(f, "medium"),
            Self::High => write!(f, "high"),
            Self::Critical => write!(f, "critical"),
        }
    }
}

impl RiskLevel {
    /// Numeric severity for comparison.
    pub fn severity(&self) -> u32 {
        match self {
            Self::Low => 0,
            Self::Medium => 1,
            Self::High => 2,
            Self::Critical => 3,
        }
    }
}

/// Result of a security check on a setting.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityCheckResult {
    /// Whether the setting is allowed.
    pub allowed: bool,
    /// Human-readable reason if not allowed.
    pub reason: Option<String>,
    /// Assessed risk level.
    pub risk_level: RiskLevel,
}

impl SecurityCheckResult {
    /// A passing result with low risk.
    pub fn allowed() -> Self {
        Self {
            allowed: true,
            reason: None,
            risk_level: RiskLevel::Low,
        }
    }

    /// A denied result with the given reason and risk level.
    pub fn denied(reason: impl Into<String>, risk_level: RiskLevel) -> Self {
        Self {
            allowed: false,
            reason: Some(reason.into()),
            risk_level,
        }
    }
}

/// Validates settings against security policies.
pub struct SecurityChecker {
    restricted_key_patterns: Vec<Regex>,
    enterprise_keys: Vec<String>,
}

impl SecurityChecker {
    /// Create a new checker with default restricted patterns.
    pub fn new() -> Self {
        let restricted_key_patterns = vec![
            Regex::new(r"(?i)(api[_-]?key|apikey)").expect("regex"),
            Regex::new(r"(?i)(secret|secret[_-]?key)").expect("regex"),
            Regex::new(r"(?i)(password|passwd|pwd)").expect("regex"),
            Regex::new(r"(?i)(token|access[_-]?token|auth[_-]?token)").expect("regex"),
            Regex::new(r"(?i)(private[_-]?key|signing[_-]?key)").expect("regex"),
            Regex::new(r"(?i)(credential|cred)").expect("regex"),
        ];

        Self {
            restricted_key_patterns,
            enterprise_keys: Vec::new(),
        }
    }

    /// Create a checker with specific enterprise-managed keys.
    pub fn with_enterprise_keys(keys: Vec<String>) -> Self {
        let mut checker = Self::new();
        checker.enterprise_keys = keys;
        checker
    }

    /// Check whether a setting is allowed.
    ///
    /// Returns a `SecurityCheckResult` indicating whether the setting can be
    /// applied and at what risk level.
    pub fn check_setting(
        &self,
        key: &str,
        value: &serde_json::Value,
    ) -> SecurityCheckResult {
        // Check if the key is enterprise-managed
        if self.enterprise_keys.contains(&key.to_lowercase()) {
            return SecurityCheckResult::denied(
                format!("'{key}' is managed by enterprise policy and cannot be overridden locally"),
                RiskLevel::Critical,
            );
        }

        // Check against restricted key patterns
        for pattern in &self.restricted_key_patterns {
            if pattern.is_match(key) {
                return SecurityCheckResult::denied(
                    format!("'{key}' matches a restricted key pattern — secrets should not be stored in settings"),
                    RiskLevel::Critical,
                );
            }
        }

        // Check if the value looks like a secret
        if let Some(s) = value.as_str()
            && looks_like_secret(s)
        {
            return SecurityCheckResult::denied(
                "Setting value appears to contain a secret".to_owned(),
                RiskLevel::High,
            );
        }

        // Check for suspiciously large values
        let value_str = value.to_string();
        if value_str.len() > 10_000 {
            return SecurityCheckResult {
                allowed: true,
                reason: Some("Setting value is unusually large".to_owned()),
                risk_level: RiskLevel::Medium,
            };
        }

        SecurityCheckResult::allowed()
    }

    /// Check if a key is restricted (matches restricted patterns).
    pub fn is_restricted_key(&self, key: &str) -> bool {
        if self.enterprise_keys.contains(&key.to_lowercase()) {
            return true;
        }
        self.restricted_key_patterns.iter().any(|p| p.is_match(key))
    }
}

impl Default for SecurityChecker {
    fn default() -> Self {
        Self::new()
    }
}

/// Heuristic check for whether a string value looks like a secret.
fn looks_like_secret(s: &str) -> bool {
    // Base64-encoded strings of 20+ chars
    let base64_re = Regex::new(r"^[A-Za-z0-9+/]{20,}={0,2}$").expect("regex");
    // Hex strings of 32+ chars
    let hex_re = Regex::new(r"^[0-9a-fA-F]{32,}$").expect("regex");
    // Common secret prefixes
    let prefix_re =
        Regex::new(r"^(sk-|ghp_|gho_|AKIA|eyJ)[A-Za-z0-9]").expect("regex");

    base64_re.is_match(s) || hex_re.is_match(s) || prefix_re.is_match(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn allowed_result() {
        let r = SecurityCheckResult::allowed();
        assert!(r.allowed);
        assert_eq!(r.risk_level, RiskLevel::Low);
        assert!(r.reason.is_none());
    }

    #[test]
    fn denied_result() {
        let r = SecurityCheckResult::denied("bad", RiskLevel::Critical);
        assert!(!r.allowed);
        assert_eq!(r.risk_level, RiskLevel::Critical);
        assert_eq!(r.reason.as_deref(), Some("bad"));
    }

    #[test]
    fn check_safe_setting() {
        let checker = SecurityChecker::new();
        let result = checker.check_setting("editor.tab_size", &json!(4));
        assert!(result.allowed);
        assert_eq!(result.risk_level, RiskLevel::Low);
    }

    #[test]
    fn check_restricted_key_api_key() {
        let checker = SecurityChecker::new();
        let result = checker.check_setting("api_key", &json!("sk-abc"));
        assert!(!result.allowed);
        assert_eq!(result.risk_level, RiskLevel::Critical);
    }

    #[test]
    fn check_restricted_key_password() {
        let checker = SecurityChecker::new();
        let result = checker.check_setting("password", &json!("secret"));
        assert!(!result.allowed);
    }

    #[test]
    fn check_restricted_key_token() {
        let checker = SecurityChecker::new();
        let result = checker.check_setting("auth_token", &json!("val"));
        assert!(!result.allowed);
    }

    #[test]
    fn check_secret_value() {
        let checker = SecurityChecker::new();
        let result = checker.check_setting("my_config", &json!("sk-abcdefghijklmnopqrstuvwxyz123456"));
        assert!(!result.allowed);
        assert_eq!(result.risk_level, RiskLevel::High);
    }

    #[test]
    fn check_enterprise_key() {
        let checker =
            SecurityChecker::with_enterprise_keys(vec!["security.policy".to_owned()]);
        let result = checker.check_setting("security.policy", &json!("strict"));
        assert!(!result.allowed);
        assert_eq!(result.risk_level, RiskLevel::Critical);
    }

    #[test]
    fn check_large_value() {
        let checker = SecurityChecker::new();
        // Use a value with spaces so it does not match secret patterns
        let big_val = json!(format!("large value {}", "data ".repeat(3000)));
        let result = checker.check_setting("some.key", &big_val);
        assert!(result.allowed);
        assert_eq!(result.risk_level, RiskLevel::Medium);
    }

    #[test]
    fn is_restricted_key() {
        let checker = SecurityChecker::new();
        assert!(checker.is_restricted_key("api_key"));
        assert!(checker.is_restricted_key("password"));
        assert!(checker.is_restricted_key("secret"));
        assert!(!checker.is_restricted_key("tab_size"));
    }

    #[test]
    fn risk_level_severity_order() {
        assert!(RiskLevel::Critical.severity() > RiskLevel::High.severity());
        assert!(RiskLevel::High.severity() > RiskLevel::Medium.severity());
        assert!(RiskLevel::Medium.severity() > RiskLevel::Low.severity());
    }

    #[test]
    fn risk_level_display() {
        assert_eq!(RiskLevel::Low.to_string(), "low");
        assert_eq!(RiskLevel::Medium.to_string(), "medium");
        assert_eq!(RiskLevel::High.to_string(), "high");
        assert_eq!(RiskLevel::Critical.to_string(), "critical");
    }

    #[test]
    fn default_trait() {
        let checker = SecurityChecker::default();
        let result = checker.check_setting("safe_key", &json!(1));
        assert!(result.allowed);
    }
}

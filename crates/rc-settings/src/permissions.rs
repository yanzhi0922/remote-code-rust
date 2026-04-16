//! Permission settings types.
//!
//! Corresponds to `src/utils/settings/types.ts` (PermissionsSchema, lines 42–85).

use serde::{Deserialize, Serialize};

/// Permission settings configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionSettings {
    /// List of permission rules for allowed operations.
    pub allow: Option<Vec<String>>,
    /// List of permission rules for denied operations.
    pub deny: Option<Vec<String>>,
    /// List of permission rules that should always prompt.
    pub ask: Option<Vec<String>>,
    /// Default permission mode.
    pub default_mode: Option<String>,
    /// Disable bypass permissions mode.
    pub disable_bypass_permissions_mode: Option<String>,
    /// Disable auto mode.
    pub disable_auto_mode: Option<String>,
    /// Additional directories to include in the permission scope.
    pub additional_directories: Option<Vec<String>>,
}

impl PermissionSettings {
    /// Create new empty permission settings.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a rule is in the allow list.
    #[must_use]
    pub fn is_allowed(&self, rule: &str) -> bool {
        self.allow
            .as_ref()
            .is_some_and(|rules| rules.iter().any(|r| r == rule))
    }

    /// Check if a rule is in the deny list.
    #[must_use]
    pub fn is_denied(&self, rule: &str) -> bool {
        self.deny
            .as_ref()
            .is_some_and(|rules| rules.iter().any(|r| r == rule))
    }

    /// Check if a rule is in the ask list.
    #[must_use]
    pub fn is_ask(&self, rule: &str) -> bool {
        self.ask
            .as_ref()
            .is_some_and(|rules| rules.iter().any(|r| r == rule))
    }

    /// Add a rule to the allow list.
    pub fn allow(&mut self, rule: String) {
        self.allow.get_or_insert_with(Vec::new).push(rule);
    }

    /// Add a rule to the deny list.
    pub fn deny(&mut self, rule: String) {
        self.deny.get_or_insert_with(Vec::new).push(rule);
    }

    /// Get the total number of rules across all lists.
    #[must_use]
    pub fn total_rules(&self) -> usize {
        self.allow.as_ref().map_or(0, Vec::len)
            + self.deny.as_ref().map_or(0, Vec::len)
            + self.ask.as_ref().map_or(0, Vec::len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty() {
        let p = PermissionSettings::default();
        assert!(p.allow.is_none());
        assert!(p.deny.is_none());
        assert!(p.ask.is_none());
        assert_eq!(p.total_rules(), 0);
    }

    #[test]
    fn add_and_check_rules() {
        let mut p = PermissionSettings::new();
        p.allow("Bash(*)".to_string());
        p.deny("Edit(/etc/*)".to_string());

        assert!(p.is_allowed("Bash(*)"));
        assert!(!p.is_allowed("Edit(/etc/*)"));
        assert!(p.is_denied("Edit(/etc/*)"));
        assert!(!p.is_denied("Bash(*)"));
    }

    #[test]
    fn ask_rules() {
        let mut p = PermissionSettings::new();
        p.ask = Some(vec!["Write(*)".to_string()]);
        assert!(p.is_ask("Write(*)"));
        assert!(!p.is_ask("Bash(*)"));
    }

    #[test]
    fn total_rules_count() {
        let p = PermissionSettings {
            allow: Some(vec!["a".to_string(), "b".to_string()]),
            deny: Some(vec!["c".to_string()]),
            ask: None,
            ..Default::default()
        };
        assert_eq!(p.total_rules(), 3);
    }

    #[test]
    fn serialization_roundtrip() {
        let p = PermissionSettings {
            allow: Some(vec!["Bash(*)".to_string()]),
            deny: Some(vec!["Edit(/etc/*)".to_string()]),
            default_mode: Some("default".to_string()),
            ..Default::default()
        };
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("Bash(*)"));
        assert!(json.contains("defaultMode"));

        let deserialized: PermissionSettings = serde_json::from_str(&json).unwrap();
        assert!(deserialized.is_allowed("Bash(*)"));
    }
}

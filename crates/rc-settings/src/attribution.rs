//! Attribution settings for commits and PRs.
//!
//! Corresponds to `src/utils/settings/types.ts` (attribution field, lines 366–387).

use serde::{Deserialize, Serialize};

/// Attribution settings for git commits and pull requests.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttributionSettings {
    /// Attribution text for git commits, including any trailers.
    /// Empty string hides attribution.
    pub commit: Option<String>,
    /// Attribution text for pull request descriptions.
    /// Empty string hides attribution.
    pub pr: Option<String>,
}

impl AttributionSettings {
    /// Get the effective commit attribution text.
    #[must_use]
    pub fn effective_commit_attribution<'a>(&'a self, default: &'a str) -> &'a str {
        match &self.commit {
            Some(s) if !s.is_empty() => s.as_str(),
            _ => default,
        }
    }

    /// Get the effective PR attribution text.
    #[must_use]
    pub fn effective_pr_attribution<'a>(&'a self, default: &'a str) -> &'a str {
        match &self.pr {
            Some(s) if !s.is_empty() => s.as_str(),
            _ => default,
        }
    }

    /// Check if commit attribution is hidden (explicitly set to empty string).
    #[must_use]
    pub fn is_commit_attribution_hidden(&self) -> bool {
        self.commit.as_deref() == Some("")
    }

    /// Check if PR attribution is hidden.
    #[must_use]
    pub fn is_pr_attribution_hidden(&self) -> bool {
        self.pr.as_deref() == Some("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_none() {
        let a = AttributionSettings::default();
        assert!(a.commit.is_none());
        assert!(a.pr.is_none());
    }

    #[test]
    fn effective_attribution_with_default() {
        let a = AttributionSettings::default();
        assert_eq!(
            a.effective_commit_attribution("Generated with Claude Code"),
            "Generated with Claude Code"
        );
    }

    #[test]
    fn effective_attribution_with_override() {
        let a = AttributionSettings {
            commit: Some("Custom attribution".to_string()),
            pr: None,
        };
        assert_eq!(
            a.effective_commit_attribution("default"),
            "Custom attribution"
        );
    }

    #[test]
    fn hidden_attribution() {
        let a = AttributionSettings {
            commit: Some(String::new()),
            pr: Some(String::new()),
        };
        assert!(a.is_commit_attribution_hidden());
        assert!(a.is_pr_attribution_hidden());
    }

    #[test]
    fn not_hidden_when_set() {
        let a = AttributionSettings {
            commit: Some("text".to_string()),
            pr: None,
        };
        assert!(!a.is_commit_attribution_hidden());
        assert!(!a.is_pr_attribution_hidden());
    }

    #[test]
    fn serialization_roundtrip() {
        let a = AttributionSettings {
            commit: Some("test".to_string()),
            pr: Some("pr-test".to_string()),
        };
        let json = serde_json::to_string(&a).unwrap();
        let deserialized: AttributionSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.commit, a.commit);
        assert_eq!(deserialized.pr, a.pr);
    }
}

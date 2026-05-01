use serde::{Deserialize, Serialize};

/// Permission behavior used by v2 permission providers and audits.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PermissionBehavior {
    Allow,
    Deny,
    Ask,
}

/// Result returned from a permission check.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum PermissionResult {
    Allow,
    Deny { reason: String },
    Ask { prompt: String },
}

/// Source of an active permission rule.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PermissionRuleSource {
    UserSettings,
    ProjectSettings,
    LocalSettings,
    FlagSettings,
    PolicySettings,
    CliArg,
    Command,
    Session,
}

/// Structured permission rule used for provenance-aware evaluation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PermissionRule {
    pub source: PermissionRuleSource,
    pub behavior: PermissionBehavior,
    pub tool_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_content: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::{PermissionBehavior, PermissionResult, PermissionRule, PermissionRuleSource};

    #[test]
    fn permission_rule_serializes_with_stable_shape() {
        let rule = PermissionRule {
            source: PermissionRuleSource::ProjectSettings,
            behavior: PermissionBehavior::Ask,
            tool_name: "bash_command".to_owned(),
            rule_content: Some("Bash(git status *)".to_owned()),
        };
        let value = serde_json::to_value(&rule).expect("rule should serialize");
        assert_eq!(value["source"], "project_settings");
        assert_eq!(value["behavior"], "ask");
    }

    #[test]
    fn permission_result_preserves_reason_and_prompt() {
        let deny = PermissionResult::Deny {
            reason: "outside workspace".to_owned(),
        };
        let ask = PermissionResult::Ask {
            prompt: "Allow write?".to_owned(),
        };
        assert!(
            serde_json::to_string(&deny)
                .expect("deny should serialize")
                .contains("outside")
        );
        assert!(
            serde_json::to_string(&ask)
                .expect("ask should serialize")
                .contains("Allow")
        );
    }
}

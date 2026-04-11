use anyhow::{Result, anyhow};
use async_trait::async_trait;
use rc_core::PermissionMode;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionClass {
    Read,
    Edit,
    Command,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub tool_name: String,
    pub tool_use_id: String,
    pub title: String,
    pub description: String,
    pub input: Value,
    pub blocked_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionDecision {
    pub allowed: bool,
    pub message: Option<String>,
}

impl PermissionDecision {
    #[must_use]
    pub fn allow() -> Self {
        Self {
            allowed: true,
            message: None,
        }
    }

    pub fn deny(message: impl Into<String>) -> Self {
        Self {
            allowed: false,
            message: Some(message.into()),
        }
    }
}

#[async_trait]
pub trait PermissionBroker: Send + Sync {
    fn mode(&self) -> PermissionMode;

    async fn decide(&self, request: PermissionRequest) -> PermissionDecision;
}

#[derive(Debug, Clone)]
pub struct StaticPermissionBroker {
    mode: PermissionMode,
}

impl StaticPermissionBroker {
    #[must_use]
    pub fn new(mode: PermissionMode) -> Self {
        Self { mode }
    }
}

#[async_trait]
impl PermissionBroker for StaticPermissionBroker {
    fn mode(&self) -> PermissionMode {
        self.mode
    }

    async fn decide(&self, request: PermissionRequest) -> PermissionDecision {
        if auto_allows(self.mode, classify_tool(&request.tool_name)) {
            PermissionDecision::allow()
        } else {
            PermissionDecision::deny(format!(
                "Permission mode {} denied {}.",
                self.mode.as_legacy_str(),
                request.tool_name
            ))
        }
    }
}

// ── Fine-grained permission rules ──────────────────────────────────────────

/// A single permission rule that matches tool calls by name and optional
/// input pattern.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    /// Tool name pattern, e.g. `"Bash"`, `"Bash(git *)"`, `"Edit(src/**)"`.
    pub tool_pattern: String,
    /// Whether to allow or deny matching calls.
    pub decision: PermissionDecision,
}

/// Rule-based permission engine that evaluates tool calls against an ordered
/// list of rules, returning the first match.
#[derive(Debug, Clone)]
pub struct RuleEngine {
    rules: Vec<PermissionRule>,
}

impl RuleEngine {
    /// Create a new rule engine with the given rules.
    #[must_use]
    pub fn new(rules: Vec<PermissionRule>) -> Self {
        Self { rules }
    }

    /// Create an empty rule engine (no rules → always returns `None`).
    #[must_use]
    pub fn empty() -> Self {
        Self { rules: Vec::new() }
    }

    /// Check whether a tool call matches any rule. Returns the decision of
    /// the first matching rule, or `None` if nothing matched.
    pub fn check(&self, tool_name: &str, tool_input: &Value) -> Option<PermissionDecision> {
        for rule in &self.rules {
            if rule_matches(&rule.tool_pattern, tool_name, tool_input) {
                return Some(rule.decision.clone());
            }
        }
        None
    }

    /// Parse a rule string into a `PermissionRule`.
    ///
    /// Supported formats:
    /// - `"ToolName"` — matches any call to that tool.
    /// - `"ToolName(pattern)"` — matches calls where the tool input contains
    ///   a string matching `pattern` (wildcard `*` supported).
    pub fn parse_rule(
        rule_str: &str,
        decision: PermissionDecision,
    ) -> Result<PermissionRule> {
        let trimmed = rule_str.trim();
        if trimmed.is_empty() {
            return Err(anyhow!("rule string must not be empty"));
        }
        Ok(PermissionRule {
            tool_pattern: trimmed.to_owned(),
            decision,
        })
    }
}

/// Check whether a tool pattern matches a given tool call.
fn rule_matches(pattern: &str, tool_name: &str, tool_input: &Value) -> bool {
    // Split "ToolName(sub-pattern)" into parts.
    let (name_part, input_pattern) = if let Some(open) = pattern.find('(') {
        let close = pattern.rfind(')').unwrap_or(pattern.len());
        let name = &pattern[..open];
        let sub = &pattern[open + 1..close];
        (name, Some(sub))
    } else {
        (pattern, None)
    };

    // Name must match exactly (case-insensitive for convenience).
    if !name_part.eq_ignore_ascii_case(tool_name) {
        return false;
    }

    // If there's no sub-pattern, the name match is sufficient.
    let Some(sub_pattern) = input_pattern else {
        return true;
    };

    // Try to match the sub-pattern against any string value in the input.
    wildcard_match_values(sub_pattern, tool_input)
}

/// Recursively search a JSON value for any string that matches the pattern.
fn wildcard_match_values(pattern: &str, value: &Value) -> bool {
    match value {
        Value::String(s) => wildcard_match(pattern, s),
        Value::Array(arr) => arr.iter().any(|v| wildcard_match_values(pattern, v)),
        Value::Object(map) => map.values().any(|v| wildcard_match_values(pattern, v)),
        _ => false,
    }
}

/// Simple wildcard matching: `*` matches any sequence of characters.
fn wildcard_match(pattern: &str, text: &str) -> bool {
    let p = pattern.as_bytes();
    let t = text.as_bytes();
    let pn = p.len();
    let tn = t.len();

    // DP table: dp[i][j] = pattern[..i] matches text[..j]
    let mut dp = vec![vec![false; tn + 1]; pn + 1];
    dp[0][0] = true;

    // Handle leading *s
    for i in 1..=pn {
        if p[i - 1] == b'*' {
            dp[i][0] = dp[i - 1][0];
        } else {
            break;
        }
    }

    for i in 1..=pn {
        for j in 1..=tn {
            if p[i - 1] == b'*' {
                // * matches zero chars (dp[i-1][j]) or one+ chars (dp[i][j-1])
                dp[i][j] = dp[i - 1][j] || dp[i][j - 1];
            } else if p[i - 1] == b'?' || p[i - 1] == t[j - 1] {
                dp[i][j] = dp[i - 1][j - 1];
            }
        }
    }

    dp[pn][tn]
}

/// A `PermissionBroker` that uses a `RuleEngine` for fine-grained matching,
/// falling back to a default decision when no rule matches.
#[derive(Debug, Clone)]
pub struct RuleBasedPermissionBroker {
    engine: RuleEngine,
    default_decision: PermissionDecision,
    mode: PermissionMode,
}

impl RuleBasedPermissionBroker {
    /// Create a new rule-based broker.
    ///
    /// - `rules`: ordered list of permission rules (first match wins).
    /// - `default_decision`: decision when no rule matches.
    /// - `mode`: the permission mode to report.
    #[must_use]
    pub fn new(
        rules: Vec<PermissionRule>,
        default_decision: PermissionDecision,
        mode: PermissionMode,
    ) -> Self {
        Self {
            engine: RuleEngine::new(rules),
            default_decision,
            mode,
        }
    }
}

#[async_trait]
impl PermissionBroker for RuleBasedPermissionBroker {
    fn mode(&self) -> PermissionMode {
        self.mode
    }

    async fn decide(&self, request: PermissionRequest) -> PermissionDecision {
        // First check the rule engine.
        if let Some(decision) = self.engine.check(&request.tool_name, &request.input) {
            return decision;
        }

        // Fall back to the default decision.
        self.default_decision.clone()
    }
}

#[must_use]
pub fn classify_tool(name: &str) -> PermissionClass {
    match name {
        "list_directory" | "read_file" | "search_text" | "glob" | "grep" => PermissionClass::Read,
        "write_file" | "replace_in_file" | "edit_file" => PermissionClass::Edit,
        _ => PermissionClass::Command,
    }
}

#[must_use]
pub fn auto_allows(mode: PermissionMode, class: PermissionClass) -> bool {
    match mode {
        PermissionMode::BypassPermissions => true,
        PermissionMode::AcceptEdits => {
            matches!(class, PermissionClass::Read | PermissionClass::Edit)
        }
        PermissionMode::Default => matches!(class, PermissionClass::Read),
        PermissionMode::DontAsk | PermissionMode::Plan => matches!(class, PermissionClass::Read),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn wildcard_match_exact() {
        assert!(wildcard_match("hello", "hello"));
        assert!(!wildcard_match("hello", "world"));
    }

    #[test]
    fn wildcard_match_star() {
        assert!(wildcard_match("git *", "git status"));
        assert!(wildcard_match("git *", "git commit -m \"test\""));
        assert!(!wildcard_match("git *", "npm install"));
    }

    #[test]
    fn wildcard_match_star_only() {
        assert!(wildcard_match("*", "anything"));
        assert!(wildcard_match("*", ""));
    }

    #[test]
    fn wildcard_match_question_mark() {
        assert!(wildcard_match("fi?e", "file"));
        assert!(wildcard_match("fi?e", "fire"));
        assert!(!wildcard_match("fi?e", "files"));
    }

    #[test]
    fn rule_engine_matches_simple_name() {
        let engine = RuleEngine::new(vec![PermissionRule {
            tool_pattern: "Bash".to_owned(),
            decision: PermissionDecision::allow(),
        }]);
        let result = engine.check("Bash", &json!({"command": "ls"}));
        assert!(result.is_some());
        assert!(result.expect("check should return Some").allowed);
    }

    #[test]
    fn rule_engine_matches_with_pattern() {
        let engine = RuleEngine::new(vec![PermissionRule {
            tool_pattern: "Bash(git *)".to_owned(),
            decision: PermissionDecision::allow(),
        }]);
        // Should match: command starts with "git "
        assert!(engine
            .check("Bash", &json!({"command": "git status"}))
            .is_some());
        // Should not match: command doesn't match "git *"
        assert!(engine
            .check("Bash", &json!({"command": "npm install"}))
            .is_none());
    }

    #[test]
    fn rule_engine_no_match_returns_none() {
        let engine = RuleEngine::new(vec![PermissionRule {
            tool_pattern: "Edit".to_owned(),
            decision: PermissionDecision::allow(),
        }]);
        assert!(engine.check("Bash", &json!({})).is_none());
    }

    #[test]
    fn parse_rule_valid() {
        let rule = RuleEngine::parse_rule("Bash(git *)", PermissionDecision::allow())
            .expect("should parse");
        assert_eq!(rule.tool_pattern, "Bash(git *)");
        assert!(rule.decision.allowed);
    }

    #[test]
    fn parse_rule_empty_fails() {
        assert!(RuleEngine::parse_rule("", PermissionDecision::allow()).is_err());
    }

    #[tokio::test]
    async fn rule_based_broker_uses_rules() {
        let broker = RuleBasedPermissionBroker::new(
            vec![PermissionRule {
                tool_pattern: "bash_command(git *)".to_owned(),
                decision: PermissionDecision::allow(),
            }],
            PermissionDecision::deny("no matching rule"),
            PermissionMode::Default,
        );

        let decision = broker
            .decide(PermissionRequest {
                tool_name: "bash_command".to_owned(),
                tool_use_id: "1".to_owned(),
                title: "Bash".to_owned(),
                description: "Run command".to_owned(),
                input: json!({"command": "git status"}),
                blocked_path: None,
            })
            .await;
        assert!(decision.allowed);
    }

    #[tokio::test]
    async fn rule_based_broker_falls_back_to_default() {
        let broker = RuleBasedPermissionBroker::new(
            vec![],
            PermissionDecision::deny("denied by default"),
            PermissionMode::Default,
        );

        let decision = broker
            .decide(PermissionRequest {
                tool_name: "bash_command".to_owned(),
                tool_use_id: "1".to_owned(),
                title: "Bash".to_owned(),
                description: "Run command".to_owned(),
                input: json!({"command": "ls"}),
                blocked_path: None,
            })
            .await;
        assert!(!decision.allowed);
    }
}

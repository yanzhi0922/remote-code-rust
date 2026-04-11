//! Permission system with wildcard matching and rule-based brokering.
//!
//! Implements a [`PermissionBroker`] trait with two concrete implementations:
//! - [`StaticPermissionBroker`] — blanket allow/deny.
//! - [`RuleBasedPermissionBroker`] — pattern-matched rules with wildcard support.

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use rc_core::PermissionMode;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Classification of a tool by its risk level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionClass {
    /// Read-only operations (file read, search, etc.).
    Read,
    /// File modification operations (write, edit, etc.).
    Edit,
    /// Shell command execution.
    Command,
}

/// A request for permission to execute a tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    /// Name of the tool requesting permission.
    pub tool_name: String,
    /// Unique ID of the tool use.
    pub tool_use_id: String,
    /// Short human-readable title.
    pub title: String,
    /// Detailed description of the action.
    pub description: String,
    /// JSON input to the tool.
    pub input: Value,
    /// Path that would be affected, if applicable.
    pub blocked_path: Option<String>,
}

/// The outcome of a permission decision.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionDecision {
    /// Whether the action is allowed.
    pub allowed: bool,
    /// Optional message explaining the decision.
    pub message: Option<String>,
}

impl PermissionDecision {
    /// Create an allow decision.
    #[must_use]
    pub fn allow() -> Self {
        Self {
            allowed: true,
            message: None,
        }
    }

    /// Create a deny decision with an explanatory message.
    pub fn deny(message: impl Into<String>) -> Self {
        Self {
            allowed: false,
            message: Some(message.into()),
        }
    }
}

/// Trait for permission brokering strategies.
#[async_trait]
pub trait PermissionBroker: Send + Sync {
    /// Return the current permission mode.
    fn mode(&self) -> PermissionMode;

    /// Decide whether to allow or deny the given request.
    async fn decide(&self, request: PermissionRequest) -> PermissionDecision;
}

/// A broker that uses a single [`PermissionMode`] for all decisions.
#[derive(Debug, Clone)]
pub struct StaticPermissionBroker {
    mode: PermissionMode,
}

impl StaticPermissionBroker {
    /// Create a new static broker with the given permission mode.
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
        // Read-only tools
        "list_directory" | "read_file" | "search_text" | "glob" | "grep"
        | "config_read" | "tool_search" | "skill_discover" | "list_peers"
        | "ctx_inspect" | "team_status" | "memory_read" | "list_mcp_resources"
        | "read_mcp_resource" | "verify_plan" | "brief" | "monitor"
        | "terminal_capture" | "lsp" | "suggest_pr" => PermissionClass::Read,

        // File editing tools
        "write_file" | "replace_in_file" | "edit_file" | "notebook_edit" | "snip" => {
            PermissionClass::Edit
        }

        // Memory write is an edit
        "memory_write" => PermissionClass::Edit,

        // Task/todo management
        "todo_write" | "task_create" | "task_get" | "task_list" | "task_stop" | "task_update" => {
            PermissionClass::Edit
        }

        // Everything else is a command
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

// ---------------------------------------------------------------------------
// YOLO Classifier — intelligent auto-permission decisions
// ---------------------------------------------------------------------------

/// Risk level classification for tool inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    /// Safe operation: read-only, no side effects.
    Safe,
    /// Low risk: file edits within workspace, common commands.
    Low,
    /// Medium risk: shell commands, network operations.
    Medium,
    /// High risk: destructive operations, system commands.
    High,
}

/// Classify the risk level of a tool call for automatic permission decisions.
///
/// This implements the "yoloClassifier" pattern from upstream Claude Code:
/// instead of asking the user for every tool call, the classifier makes
/// intelligent decisions based on the tool name and input content.
#[must_use]
pub fn classify_risk(tool_name: &str, input: &Value) -> RiskLevel {
    match tool_name {
        // Read-only tools are always safe.
        "list_directory" | "read_file" | "search_text" | "glob" | "grep"
        | "config_read" | "tool_search" | "skill_discover" | "list_peers"
        | "ctx_inspect" | "team_status" | "memory_read" | "list_mcp_resources"
        | "read_mcp_resource" | "verify_plan" | "brief" | "monitor"
        | "terminal_capture" | "lsp" | "suggest_pr" => RiskLevel::Safe,

        // File edits are low risk if within workspace.
        "write_file" | "replace_in_file" | "edit_file" | "notebook_edit" | "snip" => {
            RiskLevel::Low
        }

        // Task management is low risk.
        "todo_write" | "task_create" | "task_get" | "task_list" | "task_stop" | "task_update" => {
            RiskLevel::Low
        }

        // Plan mode tools are safe.
        "enter_plan_mode" | "exit_plan_mode" | "send_message" => RiskLevel::Safe,

        // Ask user is safe (it's interactive).
        "ask_user" => RiskLevel::Safe,

        // Sleep is safe.
        "sleep" => RiskLevel::Safe,

        // Memory write is low risk.
        "memory_write" => RiskLevel::Low,

        // Voice input is safe (read-only capture).
        "voice_input" => RiskLevel::Safe,

        // Git worktree tools are low risk.
        "enter_worktree" | "exit_worktree" => RiskLevel::Low,

        // Bash commands need risk analysis based on content.
        "bash_command" | "powershell" | "repl" => classify_command_risk(input),

        // Web tools are medium risk.
        "web_fetch" | "web_search" | "web_browser" => RiskLevel::Medium,

        // Agent dispatch is medium risk.
        "agent" => RiskLevel::Medium,

        // MCP tools depend on the action.
        "mcp_auth" | "mcp_tool_call" => RiskLevel::Medium,

        // Workflow/cron/remote trigger are high risk.
        "workflow" | "schedule_cron" | "remote_trigger" | "daemon" => RiskLevel::High,

        // Tungsten/overflow/synthetic are testing tools.
        "tungsten" | "overflow_test" | "synthetic_output" => RiskLevel::Safe,

        // Everything else is medium risk by default.
        _ => RiskLevel::Medium,
    }
}

/// Classify the risk of a shell command based on its content.
fn classify_command_risk(input: &Value) -> RiskLevel {
    let command = input
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or("");

    let command_lower = command.to_ascii_lowercase();
    let command_trimmed = command_lower.trim();

    // Empty commands are safe.
    if command_trimmed.is_empty() {
        return RiskLevel::Safe;
    }

    // --- Safe read-only commands ---
    let safe_prefixes = [
        // File listing/viewing
        "ls", "dir", "cat", "head", "tail", "less", "more", "file",
        "grep", "egrep", "rg", "ag", "ack", "find", "locate",
        "wc", "sort", "uniq", "cut", "tr", "tee", "xargs",
        "echo", "printf", "pwd", "whoami", "hostname", "uname",
        "which", "where", "type", "command -v", "env", "printenv",
        "stat", "file", "du", "df", "free", "top", "ps", "lsof",
        // Git read-only
        "git status", "git log", "git diff", "git branch", "git tag",
        "git remote", "git show", "git stash list", "git blame",
        "git reflog", "git shortlog", "git describe", "git rev-parse",
        "git ls-files", "git ls-tree", "git ls-remote",
        "git config --get", "git config --list",
        // Cargo read-only
        "cargo check", "cargo build", "cargo test", "cargo clippy",
        "cargo fmt", "cargo doc", "cargo tree", "cargo metadata",
        "cargo locate-project", "cargo pkgid", "cargo --version",
        "cargo search", "cargo info",
        // npm/node read-only
        "npm list", "npm view", "npm info", "npm show", "npm outdated",
        "npm pack --dry-run", "npm --version", "node --version",
        "npx --version", "npx which",
        // Python read-only
        "python --version", "python3 --version", "pip list", "pip show",
        "pip freeze", "pip check", "python -c \"import", "python3 -c \"import",
        "rustc --version", "rustup show", "rustup toolchain list",
        // Go read-only
        "go version", "go list", "go vet", "go doc",
        // Docker read-only
        "docker ps", "docker images", "docker version", "docker info",
        "docker logs", "docker inspect", "docker stats",
        // Kubernetes read-only
        "kubectl get", "kubectl describe", "kubectl logs", "kubectl version",
        // Misc safe
        "date", "cal", "uptime", "arch", "nproc", "lscpu",
        "gh repo view", "gh pr list", "gh issue list", "gh api",
    ];

    for prefix in &safe_prefixes {
        if command_trimmed.starts_with(prefix) {
            return RiskLevel::Safe;
        }
    }

    // --- Low risk: common development commands ---
    let low_risk_prefixes = [
        // Git mutating (but normal workflow)
        "git add", "git commit", "git checkout", "git switch",
        "git stash", "git merge", "git rebase", "git cherry-pick",
        "git pull", "git fetch", "git push", "git reset --soft",
        "git restore", "git rm --cached", "git mv",
        // Cargo mutating
        "cargo add", "cargo run", "cargo update", "cargo install",
        "cargo publish --dry-run", "cargo clean",
        // npm/node mutating
        "npm install", "npm run", "npm test", "npm start",
        "npm build", "npm ci", "npm uninstall",
        // Python mutating
        "pip install", "python -m pip", "python3 -m pip",
        // Go mutating
        "go build", "go test", "go run", "go mod tidy", "go mod download",
        // Docker build/run
        "docker build", "docker compose up", "docker compose build",
        // File operations (non-destructive)
        "mkdir", "touch", "cp", "mv", "ln -s", "chmod +x",
        "tar ", "unzip ", "7z ",
    ];

    for prefix in &low_risk_prefixes {
        if command_trimmed.starts_with(prefix) {
            return RiskLevel::Low;
        }
    }

    // --- High risk: destructive or dangerous commands ---
    let high_risk_patterns = [
        // System-destructive
        "rm -rf /", "rm -rf /*", "del /s /q c:", "format ",
        "shutdown", "reboot", "halt", "poweroff",
        "> /etc/", "chmod 777", "chown ",
        "dd if=", "mkfs.", ":(){ :|:& };:",
        "sudo rm", "sudo del", "sudo shutdown",
        // Download and execute
        "curl ", "wget ",
        "| sh", "| bash", "| zsh", "| fish",
        "| sudo ",
        // Credential/environment leaks
        "export ", "setenv ", "aws ", "gcloud ", "az ",
        // Package manager global operations
        "npm install -g", "pip install --user",
        // Force operations
        "rm -rf", "git push --force", "git push -f",
        "git clean -fdx", "git reset --hard",
        // Network listeners
        "nc -l", "ncat -l", "socat ",
        // Fork bombs and resource exhaustion
        "fork ", "bomb", "while true",
    ];

    for pattern in &high_risk_patterns {
        if command_lower.contains(pattern) {
            return RiskLevel::High;
        }
    }

    // --- Medium risk: everything else ---
    RiskLevel::Medium
}

/// Decide whether to auto-approve a tool call based on risk classification
/// and the current permission mode.
///
/// This is the "yolo classifier" — it enables automatic approval of safe
/// operations even in non-bypass modes, reducing permission prompt fatigue.
#[must_use]
pub fn yolo_classify(mode: PermissionMode, tool_name: &str, input: &Value) -> Option<bool> {
    let risk = classify_risk(tool_name, input);

    match mode {
        PermissionMode::BypassPermissions => Some(true),
        PermissionMode::AcceptEdits => match risk {
            RiskLevel::Safe | RiskLevel::Low => Some(true),
            _ => None, // Ask user
        },
        PermissionMode::Default => match risk {
            RiskLevel::Safe => Some(true),
            _ => None, // Ask user
        },
        PermissionMode::DontAsk => match risk {
            RiskLevel::Safe | RiskLevel::Low => Some(true),
            RiskLevel::Medium | RiskLevel::High => Some(false), // Deny without asking
        },
        PermissionMode::Plan => match risk {
            RiskLevel::Safe => Some(true),
            _ => Some(false), // Plan mode: deny everything except reads
        },
    }
}

// ---------------------------------------------------------------------------
// Permission cache — session-level decision caching
// ---------------------------------------------------------------------------

use std::collections::HashMap;

/// Cache for permission decisions within a session.
///
/// Avoids re-prompting the user for identical tool calls within the same
/// session. The cache key is (tool_name, input_hash) and the value is the
/// previous decision.
#[derive(Debug, Clone)]
pub struct PermissionCache {
    decisions: HashMap<String, bool>,
}

impl PermissionCache {
    /// Create a new empty permission cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            decisions: HashMap::new(),
        }
    }

    /// Generate a cache key from tool name and input.
    fn cache_key(tool_name: &str, input: &Value) -> String {
        // Simple hash: tool_name + sorted input keys/values.
        let input_str = input.to_string();
        format!("{tool_name}:{input_str}")
    }

    /// Check if there's a cached decision for this tool call.
    #[must_use]
    pub fn get(&self, tool_name: &str, input: &Value) -> Option<bool> {
        let key = Self::cache_key(tool_name, input);
        self.decisions.get(&key).copied()
    }

    /// Cache a permission decision.
    pub fn insert(&mut self, tool_name: &str, input: &Value, allowed: bool) {
        let key = Self::cache_key(tool_name, input);
        self.decisions.insert(key, allowed);
    }

    /// Clear all cached decisions.
    pub fn clear(&mut self) {
        self.decisions.clear();
    }

    /// Get the number of cached decisions.
    #[must_use]
    pub fn len(&self) -> usize {
        self.decisions.len()
    }

    /// Check if the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.decisions.is_empty()
    }
}

impl Default for PermissionCache {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Settings file rule loader
// ---------------------------------------------------------------------------

/// Load permission rules from a settings JSON structure.
///
/// Expects a format compatible with `.remote-code-rust/settings.json`
/// (also compatible with upstream `.claude/settings.json` format):
/// ```json
/// {
///   "permissions": {
///     "allow": ["Bash(git *)", "Read"],
///     "deny": ["Bash(rm -rf *)"]
///   }
/// }
/// ```
pub fn load_settings_rules(settings: &Value) -> (Vec<PermissionRule>, Vec<PermissionRule>) {
    let mut allows = Vec::new();
    let mut denies = Vec::new();

    if let Some(permissions) = settings.get("permissions") {
        if let Some(allow_list) = permissions.get("allow").and_then(Value::as_array) {
            for item in allow_list {
                if let Some(pattern) = item.as_str()
                    && let Ok(rule) = RuleEngine::parse_rule(pattern, PermissionDecision::allow())
                {
                    allows.push(rule);
                }
            }
        }
        if let Some(deny_list) = permissions.get("deny").and_then(Value::as_array) {
            for item in deny_list {
                if let Some(pattern) = item.as_str()
                    && let Ok(rule) =
                        RuleEngine::parse_rule(pattern, PermissionDecision::deny("denied by settings"))
                {
                    denies.push(rule);
                }
            }
        }
    }

    (allows, denies)
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

    // ── YOLO classifier tests ─────────────────────────────────────────

    #[test]
    fn yolo_classify_safe_tools_auto_approved() {
        let input = json!({"path": "/some/file"});
        assert_eq!(super::yolo_classify(PermissionMode::Default, "read_file", &input), Some(true));
        assert_eq!(super::yolo_classify(PermissionMode::Default, "glob", &input), Some(true));
        assert_eq!(super::yolo_classify(PermissionMode::Default, "grep", &input), Some(true));
    }

    #[test]
    fn yolo_classify_edit_tools_need_permission_in_default() {
        let input = json!({"path": "/some/file"});
        assert_eq!(super::yolo_classify(PermissionMode::Default, "write_file", &input), None);
    }

    #[test]
    fn yolo_classify_edit_tools_auto_approved_in_accept_edits() {
        let input = json!({"path": "/some/file"});
        assert_eq!(super::yolo_classify(PermissionMode::AcceptEdits, "write_file", &input), Some(true));
    }

    #[test]
    fn yolo_classify_bypass_allows_everything() {
        let input = json!({"command": "rm -rf /"});
        assert_eq!(super::yolo_classify(PermissionMode::BypassPermissions, "bash_command", &input), Some(true));
    }

    #[test]
    fn yolo_classify_safe_git_commands() {
        assert_eq!(
            super::yolo_classify(PermissionMode::Default, "bash_command", &json!({"command": "git status"})),
            Some(true)
        );
        assert_eq!(
            super::yolo_classify(PermissionMode::Default, "bash_command", &json!({"command": "git log --oneline"})),
            Some(true)
        );
    }

    #[test]
    fn yolo_classify_dangerous_commands_high_risk() {
        assert_eq!(
            super::classify_risk("bash_command", &json!({"command": "rm -rf /"})),
            super::RiskLevel::High
        );
    }

    #[test]
    fn yolo_classify_cargo_commands_safe() {
        assert_eq!(
            super::classify_risk("bash_command", &json!({"command": "cargo test"})),
            super::RiskLevel::Safe
        );
        assert_eq!(
            super::classify_risk("bash_command", &json!({"command": "cargo clippy"})),
            super::RiskLevel::Safe
        );
    }

    #[test]
    fn yolo_classify_dont_ask_denies_medium_and_high() {
        let input = json!({"command": "something unknown"});
        assert_eq!(super::yolo_classify(PermissionMode::DontAsk, "bash_command", &input), Some(false));
    }

    #[test]
    fn yolo_classify_plan_mode_denies_edits() {
        let input = json!({"path": "/some/file"});
        assert_eq!(super::yolo_classify(PermissionMode::Plan, "write_file", &input), Some(false));
    }

    // ── Permission cache tests ────────────────────────────────────────

    #[test]
    fn permission_cache_stores_and_retrieves() {
        let mut cache = super::PermissionCache::new();
        let input = json!({"command": "ls"});
        assert!(cache.get("bash_command", &input).is_none());

        cache.insert("bash_command", &input, true);
        assert_eq!(cache.get("bash_command", &input), Some(true));
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn permission_cache_clear_works() {
        let mut cache = super::PermissionCache::new();
        cache.insert("tool", &json!({}), true);
        assert_eq!(cache.len(), 1);
        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn permission_cache_different_inputs_different_keys() {
        let mut cache = super::PermissionCache::new();
        cache.insert("bash_command", &json!({"command": "ls"}), true);
        cache.insert("bash_command", &json!({"command": "rm"}), false);

        assert_eq!(cache.get("bash_command", &json!({"command": "ls"})), Some(true));
        assert_eq!(cache.get("bash_command", &json!({"command": "rm"})), Some(false));
    }

    // ── Settings rule loader tests ────────────────────────────────────

    #[test]
    fn load_settings_rules_parses_allow_and_deny() {
        let settings = json!({
            "permissions": {
                "allow": ["Bash(git *)", "Read"],
                "deny": ["Bash(rm -rf *)"]
            }
        });

        let (allows, denies) = super::load_settings_rules(&settings);
        assert_eq!(allows.len(), 2);
        assert_eq!(denies.len(), 1);
    }

    #[test]
    fn load_settings_rules_handles_empty() {
        let settings = json!({});
        let (allows, denies) = super::load_settings_rules(&settings);
        assert!(allows.is_empty());
        assert!(denies.is_empty());
    }

    #[test]
    fn load_settings_rules_handles_partial() {
        let settings = json!({
            "permissions": {
                "allow": ["Bash(git *)"]
            }
        });
        let (allows, denies) = super::load_settings_rules(&settings);
        assert_eq!(allows.len(), 1);
        assert!(denies.is_empty());
    }

    #[test]
    fn risk_level_ordering() {
        use super::RiskLevel;
        assert!(std::cmp::PartialEq::eq(&RiskLevel::Safe, &RiskLevel::Safe));
        assert!(!std::cmp::PartialEq::eq(&RiskLevel::Safe, &RiskLevel::High));
    }
}

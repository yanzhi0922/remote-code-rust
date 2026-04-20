//! # rc-permissions — Permission System
//!
//! Full permission system matching Claude Code's permission architecture.
//! Combines the original permission broker system with the V2 advanced features.
//!
//! ## Original Features (V1)
//! - **Permission Broker**: Trait-based permission decision system
//! - **Static Broker**: Simple allow/deny based on configuration
//! - **Layered Broker**: Multi-source rule evaluation with session rules
//! - **Rule Engine**: Pattern-based tool matching with wildcards
//! - **Risk Classification**: Tool and command risk assessment
//! - **Permission Cache**: Caching for repeated permission checks
//!
//! ## V2 Features
//! - **7 Permission Modes**: default, plan, acceptEdits, bypassPermissions, dontAsk, auto, bubble
//! - **YOLO Classifier**: Auto-approves safe read operations and known-safe bash commands
//! - **Bash Classifier**: Categorizes bash commands by safety level
//! - **Dangerous Pattern Detection**: Detects rm -rf /, sudo, curl|sh, force-push, etc.
//! - **Auto Mode**: Classifier-based automatic approval with state tracking
//! - **Shadowed Rule Detection**: Identifies rules that will never be evaluated
//! - **Bypass Killswitch**: Remote safety mechanism to disable bypass permissions
//! - **Filesystem Checks**: Validates file operations are within allowed scope
//! - **Path Validation**: Detects path traversal attacks and null bytes
//! - **Shell Matching**: Glob-style matching for bash command rules
//! - **Denial Tracking**: Tracks repeated denials to prevent prompt spam
//! - **Permission Explainer**: Human-readable explanations for decisions
//! - **Multiple Handlers**: Interactive, Coordinator, and SwarmWorker strategies
//!
//! ## Example
//! ```ignore
//! use rc_permissions::classifier::YoloClassifier;
//! use rc_permissions::dangerous_patterns::is_critically_dangerous;
//!
//! assert!(is_critically_dangerous("rm -rf /"));
//! assert!(!is_critically_dangerous("git status"));
//! ```

// ── Original V1 modules ──────────────────────────────────────
pub mod audit;
pub mod rule_parser;
pub mod rules;
pub mod shell_rules;

// ── V2 modules ────────────────────────────────────────────────
pub mod auto_mode;
pub mod bypass_killswitch;
pub mod classifier;
pub mod dangerous_patterns;
pub mod decision;
pub mod denial_tracking;
pub mod explainer;
pub mod filesystem;
pub mod handler;
pub mod loader;
pub mod mode;
pub mod path_validation;
pub mod rule;
pub mod setup;
pub mod shadowed_detection;
pub mod shell_matching;

// ── V1 re-exports ─────────────────────────────────────────────
pub use audit::PermissionAuditRecord;
pub use rules::{LayeredRuleEngine, RuleAction, RuleMatch, RuleSource, SourceAwarePermissionRule};

// ── V2 re-exports ─────────────────────────────────────────────
pub use auto_mode::{AutoModeManager, AutoModeState};
pub use bypass_killswitch::BypassKillswitchManager;
pub use classifier::{
    BashClassifier, BashCommandCategory, ClassifierResult, PermissionClassifier, YoloClassifier,
};
pub use dangerous_patterns::{
    DangerLevel, DangerousPattern, detect_dangerous_patterns, has_dangerous_patterns,
    is_critically_dangerous,
};
pub use decision::{
    AllowDecision, AskDecision, DecisionReason, DenyDecision, PassthroughDecision,
    PermissionDecisionV2, PermissionUpdate, PermissionUpdateDestination,
};
pub use denial_tracking::{DenialTracker, SharedDenialTracker};
pub use explainer::explain_permission;
pub use filesystem::{
    FilesystemCheckResult, FilesystemOperation, assess_filesystem_access,
    check_filesystem_permission, get_paths_for_permission_check,
};
pub use handler::{
    CoordinatorHandler, InteractiveHandler, PermissionCheckContext, PermissionHandler,
    SwarmWorkerHandler,
};
pub use loader::{load_rules_from_file, merge_rules, parse_rule_string};
pub use mode::{ExtendedPermissionMode, ModeColorKey, PermissionModeConfig};
pub use path_validation::{
    PathValidation, clean_path_input, path_requires_manual_approval, validate_path,
};
pub use rule::PermissionRuleV2;
pub use setup::{PermissionSetup, PermissionSetupConfig, get_next_permission_mode};
pub use shadowed_detection::{ShadowReason, ShadowedRule, detect_shadowed_rules};
pub use shell_matching::shell_command_matches_pattern;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

// ── V1 core types (kept for backward compatibility) ───────────

/// Permission class for categorizing tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PermissionClass {
    Read,
    Edit,
    Bash,
    Mcp,
    Agent,
}

/// A permission request for a specific tool invocation.
#[derive(Debug, Clone)]
pub struct PermissionRequest {
    pub tool_name: String,
    pub permission_class: Option<PermissionClass>,
    pub tool_input: serde_json::Value,
    pub working_directory: Option<String>,
    /// Optional tool use ID for tracking.
    pub tool_use_id: Option<String>,
    /// Optional human-readable title.
    pub title: Option<String>,
    /// Optional description of the operation.
    pub description: Option<String>,
    /// Optional blocked path (set when a path violation is detected).
    pub blocked_path: Option<String>,
}

impl PermissionRequest {
    #[must_use]
    pub fn resolved_permission_class(&self) -> PermissionClass {
        self.permission_class
            .unwrap_or_else(|| classify_tool(&self.tool_name))
    }
}

/// A permission decision (allow or deny).
#[derive(Debug, Clone)]
pub struct PermissionDecision {
    pub allowed: bool,
    pub message: Option<String>,
}

impl PermissionDecision {
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

/// Trait for permission brokers.
#[async_trait]
pub trait PermissionBroker: Send + Sync {
    /// Decide whether to allow or deny a permission request.
    async fn decide(&self, request: PermissionRequest) -> PermissionDecision;
    /// Add a session-scoped rule.
    fn add_session_rule(&self, _action: RuleAction, _tool_pattern: String) -> Result<()> {
        Ok(())
    }
    /// Clear all session-scoped rules, returning the count removed.
    fn clear_session_rules(&self) -> Result<usize> {
        Ok(0)
    }
    /// Return the current permission mode, if known.
    fn mode(&self) -> Option<rc_core::PermissionMode> {
        None
    }
    /// Return audit records collected so far.
    fn audit_records(&self) -> Vec<PermissionAuditRecord> {
        Vec::new()
    }
    /// Return the layered rules, if this broker has any.
    fn layered_rules(&self) -> Vec<SourceAwarePermissionRule> {
        Vec::new()
    }
}

/// Static permission broker that allows or denies everything.
#[derive(Debug)]
pub struct StaticPermissionBroker {
    pub allow_all: bool,
    mode: Option<rc_core::PermissionMode>,
}

impl StaticPermissionBroker {
    /// Create a broker that either allows or denies everything.
    pub fn new(allow_all: bool) -> Self {
        Self {
            allow_all,
            mode: None,
        }
    }

    /// Create a broker from a [`rc_core::PermissionMode`].
    ///
    /// `BypassPermissions` and `AcceptEdits` auto-approve certain classes;
    /// all other modes deny by default.
    pub fn from_mode(mode: rc_core::PermissionMode) -> Self {
        let allow_all = matches!(mode, rc_core::PermissionMode::BypassPermissions);
        Self {
            allow_all,
            mode: Some(mode),
        }
    }
}

#[async_trait]
impl PermissionBroker for StaticPermissionBroker {
    async fn decide(&self, request: PermissionRequest) -> PermissionDecision {
        if self.allow_all {
            return PermissionDecision::allow();
        }
        let requires_explicit_confirmation = request.working_directory.is_some()
            && request.blocked_path.is_some()
            && !matches!(self.mode, Some(rc_core::PermissionMode::BypassPermissions));
        if requires_explicit_confirmation {
            return PermissionDecision::deny("Permission denied by static broker");
        }
        // If we have a mode, check auto_allows for the tool class.
        if let Some(mode) = self.mode {
            let class = request.resolved_permission_class();
            if auto_allows(mode, class) {
                return PermissionDecision::allow();
            }
        }
        PermissionDecision::deny("Permission denied by static broker")
    }

    fn mode(&self) -> Option<rc_core::PermissionMode> {
        self.mode
    }
}

/// Layered permission broker with multi-source rules.
pub struct LayeredPermissionBroker<B> {
    fallback: B,
    rules: Vec<SourceAwarePermissionRule>,
    session_rules: std::sync::RwLock<Vec<SourceAwarePermissionRule>>,
    audit: std::sync::Mutex<Vec<PermissionAuditRecord>>,
}

impl<B: PermissionBroker> LayeredPermissionBroker<B> {
    pub fn new(fallback: B, rules: Vec<SourceAwarePermissionRule>) -> Self {
        Self {
            fallback,
            rules,
            session_rules: std::sync::RwLock::new(Vec::new()),
            audit: std::sync::Mutex::new(Vec::new()),
        }
    }

    pub fn rules(&self) -> Vec<SourceAwarePermissionRule> {
        self.rules.clone()
    }

    pub fn audit_records(&self) -> Vec<PermissionAuditRecord> {
        self.audit.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn push_audit(&self, record: PermissionAuditRecord) {
        self.audit
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(record);
    }
}

#[async_trait]
impl<B: PermissionBroker + std::fmt::Debug> PermissionBroker for LayeredPermissionBroker<B> {
    async fn decide(&self, request: PermissionRequest) -> PermissionDecision {
        // Check session rules first (highest priority)
        {
            let session = self.session_rules.read().unwrap_or_else(|e| e.into_inner());
            for rule in session.iter() {
                if shell_rules::rule_matches_request(&rule.tool_pattern, &request) {
                    return match rule.action {
                        RuleAction::Allow => PermissionDecision::allow(),
                        RuleAction::Deny => PermissionDecision::deny(format!(
                            "Denied by session rule: {}",
                            rule.tool_pattern
                        )),
                        RuleAction::Ask => continue,
                    };
                }
            }
        }

        // Check persistent rules
        for rule in &self.rules {
            if shell_rules::rule_matches_request(&rule.tool_pattern, &request) {
                self.push_audit(PermissionAuditRecord {
                    tool_name: request.tool_name.clone(),
                    tool_use_id: String::new(),
                    source: Some(rule.source),
                    matched_pattern: Some(rule.tool_pattern.clone()),
                    action: rule.action,
                    final_allowed: rule.action == RuleAction::Allow,
                    reason: None,
                });
                return match rule.action {
                    RuleAction::Allow => PermissionDecision::allow(),
                    RuleAction::Deny => PermissionDecision::deny(format!(
                        "Denied by rule from {:?}: {}",
                        rule.source, rule.tool_pattern
                    )),
                    RuleAction::Ask => continue,
                };
            }
        }

        // Fall back to the inner broker
        self.fallback.decide(request).await
    }

    fn add_session_rule(&self, action: RuleAction, tool_pattern: String) -> Result<()> {
        let mut session = self
            .session_rules
            .write()
            .unwrap_or_else(|e| e.into_inner());
        session.push(SourceAwarePermissionRule {
            action,
            tool_pattern,
            source: RuleSource::Session,
        });
        Ok(())
    }

    fn clear_session_rules(&self) -> Result<usize> {
        let mut session = self
            .session_rules
            .write()
            .unwrap_or_else(|e| e.into_inner());
        let count = session.len();
        session.clear();
        Ok(count)
    }

    fn audit_records(&self) -> Vec<PermissionAuditRecord> {
        self.audit.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn layered_rules(&self) -> Vec<SourceAwarePermissionRule> {
        self.rules.clone()
    }

    fn mode(&self) -> Option<rc_core::PermissionMode> {
        self.fallback.mode()
    }
}

/// Load layered permission rules from a set of settings files.
///
/// Discovers rule files, parses them, and returns a merged list sorted by source priority.
pub fn load_layered_rules(
    cwd: &std::path::Path,
    profile_dir: &std::path::Path,
    settings_files: &[std::path::PathBuf],
    cli_settings_files: &[std::path::PathBuf],
) -> Vec<SourceAwarePermissionRule> {
    let files = rule_parser::discover_permission_rule_files(
        cwd,
        profile_dir,
        settings_files,
        cli_settings_files,
    );
    let mut all_rules = Vec::new();
    for (path, source) in &files {
        match rule_parser::load_permission_rules_from_file(path, *source) {
            Ok(rules) => {
                all_rules.extend(rules);
            }
            Err(_) => continue,
        }
    }
    all_rules.sort_by_key(|r| match r.source {
        RuleSource::Cli => 0,
        RuleSource::Session => 1,
        RuleSource::Project => 2,
        RuleSource::User => 3,
    });
    all_rules
}

pub fn rule_matches_pattern(pattern: &str, tool_name: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return pattern == tool_name;
    }
    let parts: Vec<&str> = pattern.split('*').collect();
    let mut idx = 0;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            if !tool_name[idx..].starts_with(part) {
                return false;
            }
            idx += part.len();
        } else if i == parts.len() - 1 {
            if !tool_name.ends_with(part) {
                return false;
            }
        } else {
            match tool_name[idx..].find(part) {
                Some(pos) => idx += pos + part.len(),
                None => return false,
            }
        }
    }
    true
}

/// Classify a tool by name.
///
/// Accepts both PascalCase permission names (e.g. `"Bash"`, `"Read"`) used in
/// `ToolSpec::permission_tool_name` and snake_case internal names (e.g.
/// `"bash_command"`, `"read_file"`) used in `ToolSpec::name`.
pub fn classify_tool(name: &str) -> PermissionClass {
    match name {
        // PascalCase (permission_tool_name / rule patterns)
        "Read" | "Glob" | "LS" => PermissionClass::Read,
        "Edit" | "Write" | "MultiEdit" => PermissionClass::Edit,
        "Bash" | "WebFetch" | "WebSearch" | "WebBrowser" | "TerminalCapture" | "RemoteTrigger"
        | "Daemon" => PermissionClass::Bash,
        "mcp" | "McpServerList" | "McpAuth" | "McpCall" => PermissionClass::Mcp,
        "Agent" | "Task" | "ExecuteSkill" | "TeamDelete" => PermissionClass::Agent,
        "Config" | "LSP" | "ToolSearch" | "VerifyPlan" | "Snip" => PermissionClass::Read,
        // snake_case (internal tool names) — Bash
        "bash_command" | "powershell" | "repl" | "tungsten" | "web_fetch" | "web_search"
        | "web_browser" | "terminal_capture" | "remote_trigger" | "daemon" => PermissionClass::Bash,
        // snake_case — Edit
        "write_file" | "replace_in_file" | "edit_file" | "notebook_edit" | "memory_write"
        | "schedule_cron" | "enter_worktree" | "exit_worktree" => PermissionClass::Edit,
        // snake_case — Agent
        "agent" | "team_delete" => PermissionClass::Agent,
        // snake_case — Mcp
        "mcp_call" | "mcp_auth" => PermissionClass::Mcp,
        // Everything else defaults to Read (safe / least-privilege fallback)
        _ => PermissionClass::Read,
    }
}

/// Check whether a given permission mode auto-allows a permission class.
///
/// This is used as a fast-path: if the mode auto-allows the class,
/// the broker can skip the interactive prompt entirely.
pub fn auto_allows(mode: rc_core::PermissionMode, class: PermissionClass) -> bool {
    use rc_core::PermissionMode;
    match mode {
        PermissionMode::BypassPermissions => true,
        PermissionMode::AcceptEdits => {
            matches!(class, PermissionClass::Read | PermissionClass::Edit)
        }
        PermissionMode::Default | PermissionMode::Plan | PermissionMode::DontAsk => {
            matches!(class, PermissionClass::Read)
        }
    }
}

/// Risk level for operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_broker_allow_all() -> anyhow::Result<()> {
        let broker = StaticPermissionBroker::new(true);
        let rt = tokio::runtime::Runtime::new()?;
        let decision = rt.block_on(broker.decide(PermissionRequest {
            tool_name: "Bash".to_owned(),
            permission_class: None,
            tool_input: serde_json::json!({"command": "ls"}),
            working_directory: None,
            tool_use_id: None,
            title: None,
            description: None,
            blocked_path: None,
        }));
        assert!(decision.allowed);
        Ok(())
    }

    #[test]
    fn static_broker_deny_all() -> anyhow::Result<()> {
        let broker = StaticPermissionBroker::new(false);
        let rt = tokio::runtime::Runtime::new()?;
        let decision = rt.block_on(broker.decide(PermissionRequest {
            tool_name: "Bash".to_owned(),
            permission_class: None,
            tool_input: serde_json::json!({"command": "rm -rf /"}),
            working_directory: None,
            tool_use_id: None,
            title: None,
            description: None,
            blocked_path: None,
        }));
        assert!(!decision.allowed);
        Ok(())
    }

    #[test]
    fn layered_broker_falls_through() -> anyhow::Result<()> {
        let fallback = StaticPermissionBroker::new(true);
        let layered = LayeredPermissionBroker::new(fallback, vec![]);
        let rt = tokio::runtime::Runtime::new()?;
        let decision = rt.block_on(layered.decide(PermissionRequest {
            tool_name: "Read".to_owned(),
            permission_class: None,
            tool_input: serde_json::json!({"path": "/tmp/a"}),
            working_directory: None,
            tool_use_id: None,
            title: None,
            description: None,
            blocked_path: None,
        }));
        assert!(decision.allowed);
        Ok(())
    }

    #[test]
    fn layered_broker_rule_deny() -> anyhow::Result<()> {
        let fallback = StaticPermissionBroker::new(true);
        let rules = vec![SourceAwarePermissionRule {
            tool_pattern: "Bash".to_owned(),
            action: RuleAction::Deny,
            source: RuleSource::Project,
        }];
        let layered = LayeredPermissionBroker::new(fallback, rules);
        let rt = tokio::runtime::Runtime::new()?;
        let decision = rt.block_on(layered.decide(PermissionRequest {
            tool_name: "Bash".to_owned(),
            permission_class: None,
            tool_input: serde_json::json!({"command": "ls"}),
            working_directory: None,
            tool_use_id: None,
            title: None,
            description: None,
            blocked_path: None,
        }));
        assert!(!decision.allowed);
        Ok(())
    }

    #[test]
    fn layered_broker_wildcard_pattern() -> anyhow::Result<()> {
        let fallback = StaticPermissionBroker::new(false);
        let rules = vec![SourceAwarePermissionRule {
            tool_pattern: "Read*".to_owned(),
            action: RuleAction::Allow,
            source: RuleSource::User,
        }];
        let layered = LayeredPermissionBroker::new(fallback, rules);
        let rt = tokio::runtime::Runtime::new()?;
        let decision = rt.block_on(layered.decide(PermissionRequest {
            tool_name: "ReadFile".to_owned(),
            permission_class: None,
            tool_input: serde_json::json!({"path": "/tmp/a"}),
            working_directory: None,
            tool_use_id: None,
            title: None,
            description: None,
            blocked_path: None,
        }));
        assert!(decision.allowed);
        Ok(())
    }

    #[test]
    fn layered_broker_rule_matches_tool_input_path() -> anyhow::Result<()> {
        let fallback = StaticPermissionBroker::new(false);
        let rules = vec![SourceAwarePermissionRule {
            tool_pattern: "Read(src/**)".to_owned(),
            action: RuleAction::Allow,
            source: RuleSource::User,
        }];
        let layered = LayeredPermissionBroker::new(fallback, rules);
        let rt = tokio::runtime::Runtime::new()?;
        let allowed = rt.block_on(layered.decide(PermissionRequest {
            tool_name: "read_file".to_owned(),
            permission_class: None,
            tool_input: serde_json::json!({"path": "src/main.rs"}),
            working_directory: None,
            tool_use_id: None,
            title: None,
            description: None,
            blocked_path: None,
        }));
        let denied = rt.block_on(layered.decide(PermissionRequest {
            tool_name: "read_file".to_owned(),
            permission_class: None,
            tool_input: serde_json::json!({"path": "tests/main.rs"}),
            working_directory: None,
            tool_use_id: None,
            title: None,
            description: None,
            blocked_path: None,
        }));

        assert!(allowed.allowed);
        assert!(!denied.allowed);
        Ok(())
    }

    #[test]
    fn rule_matches_pattern_exact() {
        assert!(rule_matches_pattern("Bash", "Bash"));
        assert!(!rule_matches_pattern("Bash", "Read"));
    }

    #[test]
    fn rule_matches_pattern_wildcard() {
        assert!(rule_matches_pattern("*", "anything"));
        assert!(rule_matches_pattern("Read*", "ReadFile"));
        assert!(rule_matches_pattern("*File", "ReadFile"));
        assert!(rule_matches_pattern("Read*File", "ReadMyFile"));
        assert!(!rule_matches_pattern("Read*File", "WriteMyFile"));
    }

    #[test]
    fn classify_tool_returns_correct_class() {
        assert_eq!(classify_tool("Read"), PermissionClass::Read);
        assert_eq!(classify_tool("Glob"), PermissionClass::Read);
        assert_eq!(classify_tool("Edit"), PermissionClass::Edit);
        assert_eq!(classify_tool("Write"), PermissionClass::Edit);
        assert_eq!(classify_tool("Bash"), PermissionClass::Bash);
        assert_eq!(classify_tool("WebFetch"), PermissionClass::Bash);
        assert_eq!(classify_tool("McpCall"), PermissionClass::Mcp);
        assert_eq!(classify_tool("Daemon"), PermissionClass::Bash);
        assert_eq!(classify_tool("mcp"), PermissionClass::Mcp);
        assert_eq!(classify_tool("Agent"), PermissionClass::Agent);
        assert_eq!(classify_tool("Unknown"), PermissionClass::Read);
    }

    #[test]
    fn permission_request_can_override_classification() {
        let request = PermissionRequest {
            tool_name: "workflow".to_owned(),
            permission_class: Some(PermissionClass::Bash),
            tool_input: serde_json::json!({"action": "run"}),
            working_directory: None,
            tool_use_id: None,
            title: None,
            description: None,
            blocked_path: None,
        };
        assert_eq!(request.resolved_permission_class(), PermissionClass::Bash);
    }

    #[test]
    fn session_rules_highest_priority() -> anyhow::Result<()> {
        let fallback = StaticPermissionBroker::new(false);
        // Persistent rule allows Bash
        let rules = vec![SourceAwarePermissionRule {
            tool_pattern: "Bash".to_owned(),
            action: RuleAction::Allow,
            source: RuleSource::User,
        }];
        let layered = LayeredPermissionBroker::new(fallback, rules);
        // Session rule denies Bash
        layered.add_session_rule(RuleAction::Deny, "Bash".to_owned())?;
        let rt = tokio::runtime::Runtime::new()?;
        let decision = rt.block_on(layered.decide(PermissionRequest {
            tool_name: "Bash".to_owned(),
            permission_class: None,
            tool_input: serde_json::json!({"command": "ls"}),
            working_directory: None,
            tool_use_id: None,
            title: None,
            description: None,
            blocked_path: None,
        }));
        assert!(!decision.allowed);
        Ok(())
    }

    #[test]
    fn clear_session_rules_returns_count() -> anyhow::Result<()> {
        let fallback = StaticPermissionBroker::new(true);
        let layered = LayeredPermissionBroker::new(fallback, vec![]);
        layered.add_session_rule(RuleAction::Allow, "Read".to_owned())?;
        layered.add_session_rule(RuleAction::Deny, "Bash".to_owned())?;
        let count = layered.clear_session_rules()?;
        assert_eq!(count, 2);
        Ok(())
    }

    #[test]
    fn audit_records_tracked() -> anyhow::Result<()> {
        let fallback = StaticPermissionBroker::new(true);
        let rules = vec![SourceAwarePermissionRule {
            tool_pattern: "Read".to_owned(),
            action: RuleAction::Allow,
            source: RuleSource::Project,
        }];
        let layered = LayeredPermissionBroker::new(fallback, rules);
        let rt = tokio::runtime::Runtime::new()?;
        let _ = rt.block_on(layered.decide(PermissionRequest {
            tool_name: "Read".to_owned(),
            permission_class: None,
            tool_input: serde_json::json!({"path": "/tmp"}),
            working_directory: None,
            tool_use_id: None,
            title: None,
            description: None,
            blocked_path: None,
        }));
        let records = layered.audit_records();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].tool_name, "Read");
        assert!(records[0].final_allowed);
        Ok(())
    }
}

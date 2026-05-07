//! YOLO Classifier — auto-approve safe operations based on rule sets.
//!
//! Corresponds to `.research/cc-haha/src/utils/permissions/yoloClassifier.ts`.
//! Provides rule-based classification for tool use in auto-permission mode,
//! allowing known-safe operations and soft-denying dangerous ones.
//!
//! Also includes a 2-stage LLM-based classification pipeline matching TS:
//! - Stage 1 ("fast"): max_tokens=64, stop_sequences=['</block>'], parses XML
//! - Stage 2 ("thinking"): max_tokens=4096, full chain-of-thought reasoning
//! - Fail-closed: any ambiguity → block
//! - Safe tools allowlist bypasses the classifier entirely

use std::future::Future;
use std::pin::Pin;

use serde::{Deserialize, Serialize};

/// Result of a YOLO classifier evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum YoloClassifierResult {
    /// The operation is known-safe and should be auto-approved.
    Allow,
    /// The operation is potentially dangerous; deny with a reason.
    Deny(String),
    /// The operation needs user confirmation; ask with a prompt.
    Ask(String),
}

/// Rules for auto mode classification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoModeRules {
    /// Glob / exact patterns for tools/commands that are always allowed.
    pub allow: Vec<String>,
    /// Patterns for commands that should be soft-denied (user must confirm).
    pub soft_deny: Vec<String>,
    /// Environment variables that are safe to inspect.
    pub environment: Vec<String>,
}

impl Default for AutoModeRules {
    fn default() -> Self {
        Self::external()
    }
}

impl AutoModeRules {
    /// Default rules for external / third-party models.
    ///
    /// Conservative: only allow read-only git, build commands, and file reads.
    pub fn external() -> Self {
        Self {
            allow: vec![
                // Git read operations
                "git status".into(),
                "git log".into(),
                "git diff".into(),
                "git branch --list".into(),
                "git remote -v".into(),
                "git show".into(),
                "git stash list".into(),
                "git tag --list".into(),
                // Build / package commands
                "cargo build".into(),
                "cargo check".into(),
                "cargo test".into(),
                "cargo clippy".into(),
                "cargo doc".into(),
                "npm install".into(),
                "npm run build".into(),
                "npm test".into(),
                "npm ci".into(),
                "node ".into(),
                "npx ".into(),
                "yarn install".into(),
                "yarn build".into(),
                "yarn test".into(),
                "pnpm install".into(),
                "pnpm build".into(),
                "pnpm test".into(),
                "pip install".into(),
                "python ".into(),
                "python3 ".into(),
                "make ".into(),
                "cmake ".into(),
                // File read operations
                "cat ".into(),
                "head ".into(),
                "tail ".into(),
                "less ".into(),
                "wc ".into(),
                "file ".into(),
                "stat ".into(),
                // Directory listing
                "ls".into(),
                "dir".into(),
                "find ".into(),
                "tree".into(),
                "pwd".into(),
                // Safe utilities
                "echo ".into(),
                "which ".into(),
                "whoami".into(),
                "uname".into(),
                "date".into(),
                "env".into(),
                "printenv".into(),
                "id".into(),
                "hostname".into(),
                "df -h".into(),
                "du ".into(),
                "ps ".into(),
                "top -bn1".into(),
            ],
            soft_deny: vec![
                "rm -rf".into(),
                "rm -r".into(),
                "rmdir".into(),
                "git push --force".into(),
                "git push -f".into(),
                "git push --force-with-lease".into(),
                "git reset --hard".into(),
                "git clean -fd".into(),
                "drop database".into(),
                "DROP DATABASE".into(),
                "format disk".into(),
                "mkfs".into(),
                "sudo ".into(),
                "su ".into(),
                "chmod 777".into(),
                "chown ".into(),
                "dd if=".into(),
                "> /dev/sd".into(),
                "shutdown".into(),
                "reboot".into(),
                "halt".into(),
                "poweroff".into(),
                "systemctl stop".into(),
                "service stop".into(),
                "kill -9".into(),
                "killall".into(),
                "iptables".into(),
                "ufw ".into(),
            ],
            environment: vec![
                "PATH".into(),
                "HOME".into(),
                "USER".into(),
                "SHELL".into(),
                "LANG".into(),
                "TERM".into(),
                "PWD".into(),
                "EDITOR".into(),
                "VISUAL".into(),
                "RUSTUP_HOME".into(),
                "CARGO_HOME".into(),
                "NODE_VERSION".into(),
                "NVM_DIR".into(),
                "JAVA_HOME".into(),
                "GOPATH".into(),
                "PYTHONPATH".into(),
                "VIRTUAL_ENV".into(),
                "CONDA_DEFAULT_ENV".into(),
                "DOCKER_HOST".into(),
            ],
        }
    }

    /// Default rules for Anthropic-hosted models.
    ///
    /// More permissive: allows additional safe operations.
    pub fn anthropic() -> Self {
        let mut rules = Self::external();
        // Additional allowed operations for first-party models
        rules.allow.extend_from_slice(&[
            "git add ".into(),
            "git commit".into(),
            "git checkout ".into(),
            "git switch ".into(),
            "git merge ".into(),
            "git rebase ".into(),
            "git pull".into(),
            "git fetch".into(),
            "git stash".into(),
            "git tag ".into(),
            "mkdir ".into(),
            "touch ".into(),
            "cp ".into(),
            "mv ".into(),
            "chmod ".into(),
            "curl ".into(),
            "wget ".into(),
            "docker build".into(),
            "docker run".into(),
            "docker ps".into(),
            "docker-compose up".into(),
            "docker compose up".into(),
        ]);
        rules
    }

    /// Check if a command matches any allow rule.
    pub fn is_allowed(&self, command: &str) -> bool {
        let command_lower = command.to_lowercase();
        self.allow.iter().any(|pattern| {
            let pattern_lower = pattern.to_lowercase();
            command_lower == pattern_lower
                || command_lower.starts_with(&format!("{pattern_lower} "))
                || command_lower.starts_with(&pattern_lower)
                    && (pattern.ends_with(' ') || !command_lower.contains(' '))
        })
    }

    /// Check if a command matches any soft-deny rule.
    pub fn is_soft_denied(&self, command: &str) -> bool {
        let command_lower = command.to_lowercase();
        self.soft_deny.iter().any(|pattern| {
            let pattern_lower = pattern.to_lowercase();
            command_lower.contains(&pattern_lower)
        })
    }

    /// Check if an environment variable is safe to inspect.
    pub fn is_env_allowed(&self, var_name: &str) -> bool {
        self.environment
            .iter()
            .any(|v| v.eq_ignore_ascii_case(var_name))
    }
}

/// Get the default external auto mode rules.
#[must_use]
pub fn get_default_external_auto_mode_rules() -> AutoModeRules {
    AutoModeRules::external()
}

/// Get the default Anthropic auto mode rules.
#[must_use]
pub fn get_default_anthropic_auto_mode_rules() -> AutoModeRules {
    AutoModeRules::anthropic()
}

/// Classify a tool use for auto-permission decisions.
///
/// Takes a tool name, its JSON input, and the active auto mode rules,
/// and returns whether to allow, deny, or ask the user.
pub fn classify_tool_use(
    tool_name: &str,
    tool_input: &serde_json::Value,
    rules: &AutoModeRules,
) -> YoloClassifierResult {
    // Check if the tool itself is a known read-only tool
    if is_read_only_tool(tool_name) {
        return YoloClassifierResult::Allow;
    }

    // For Bash/Shell tools, classify the command
    if (tool_name == "Bash" || tool_name == "Shell" || tool_name == "bash" || tool_name == "shell")
        && let Some(command) = tool_input.get("command").and_then(|v| v.as_str())
    {
        return classify_bash_in_yolo(command, rules);
    }

    // For file write tools, ask for confirmation
    if is_write_tool(tool_name) {
        if let Some(path) = extract_file_path(tool_input) {
            // Allow writes to non-critical paths
            if is_safe_write_path(&path) {
                return YoloClassifierResult::Allow;
            }
            return YoloClassifierResult::Ask(format!(
                "Write operation to '{path}' requires confirmation"
            ));
        }
        return YoloClassifierResult::Ask("File write operation requires confirmation".into());
    }

    // For MCP tools, ask by default
    if tool_name.starts_with("mcp__") {
        return YoloClassifierResult::Ask(format!(
            "MCP tool '{tool_name}' requires user confirmation"
        ));
    }

    // Default: ask for unknown tools
    YoloClassifierResult::Ask(format!(
        "Unknown tool '{tool_name}' requires user confirmation"
    ))
}

/// Apply auto mode rules to a tool invocation.
///
/// This is the main entry point for the auto-mode permission system.
pub fn apply_auto_mode_rules(
    tool_name: &str,
    tool_input: &serde_json::Value,
    rules: &AutoModeRules,
) -> YoloClassifierResult {
    classify_tool_use(tool_name, tool_input, rules)
}

/// Classify a bash command within the YOLO classifier context.
fn classify_bash_in_yolo(command: &str, rules: &AutoModeRules) -> YoloClassifierResult {
    let trimmed = command.trim();

    // Check soft-deny patterns first (safety first)
    if rules.is_soft_denied(trimmed) {
        return YoloClassifierResult::Deny(format!(
            "Command matches a dangerous pattern: '{trimmed}'"
        ));
    }

    // Pipe chains and redirections need review (check BEFORE allow patterns)
    if trimmed.contains('|') || trimmed.contains('>') || trimmed.contains(">>") {
        return YoloClassifierResult::Ask(format!(
            "Pipe/redirect in command requires review: '{trimmed}'"
        ));
    }

    // Commands with variable expansion need review (check BEFORE allow patterns)
    if trimmed.contains("$(") || trimmed.contains('`') {
        return YoloClassifierResult::Ask(format!(
            "Command substitution requires review: '{trimmed}'"
        ));
    }

    // Check allow patterns
    if rules.is_allowed(trimmed) {
        return YoloClassifierResult::Allow;
    }

    // Default: ask for unrecognized commands
    YoloClassifierResult::Ask(format!("Unrecognized command requires review: '{trimmed}'"))
}

/// Check if a tool is known to be read-only.
fn is_read_only_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "Read"
            | "Grep"
            | "Glob"
            | "LS"
            | "WebFetch"
            | "WebSearch"
            | "read"
            | "grep"
            | "glob"
            | "ls"
            | "list_files"
            | "search"
    )
}

/// Check if a tool performs write operations.
fn is_write_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "Write" | "Edit" | "write" | "edit" | "write_to_file" | "apply_diff" | "create_file"
    )
}

/// Extract a file path from tool input JSON.
fn extract_file_path(tool_input: &serde_json::Value) -> Option<String> {
    tool_input
        .get("path")
        .or_else(|| tool_input.get("file_path"))
        .or_else(|| tool_input.get("filePath"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_owned())
}

/// Check if a file path is considered safe for writes.
fn is_safe_write_path(path: &str) -> bool {
    // Block writes to critical system paths
    let blocked_prefixes = [
        "/etc/",
        "/usr/",
        "/bin/",
        "/sbin/",
        "/System/",
        "/Library/System/",
        "C:\\Windows\\",
        "C:\\Program Files\\",
        "C:\\Program Files (x86)\\",
    ];

    for prefix in &blocked_prefixes {
        if path.starts_with(prefix) {
            return false;
        }
    }

    // Block writes to hidden files in home directory
    if path.starts_with("~/.") || path.contains("/.") {
        // Allow .remote-code-rust project files
        if path.contains(".remote-code-rust") || path.contains(".remote-code") {
            return true;
        }
        return false;
    }

    true
}

// ---------------------------------------------------------------------------
// 2-Stage LLM-based YOLO Classifier
// ---------------------------------------------------------------------------

/// System prompt for the 2-stage LLM classifier.
/// Mirrors TS `yoloClassifier.ts` system prompt.
pub const YOLO_CLASSIFIER_SYSTEM_PROMPT: &str = r#"You are a security classifier for tool use in an AI coding assistant. Your job is to determine whether a tool invocation is safe to auto-approve.

Respond with an XML block:
<block>yes</block> — if the operation is safe to auto-approve
<block>no</block> — if the operation should be blocked or requires user confirmation

Guidelines:
- Reading files, searching code, and listing directories are always safe
- Writing to files within the project directory is generally safe
- Running build commands, tests, and linters is generally safe
- Commands that delete files, force-push, modify system configs, or access credentials are unsafe
- When in doubt, err on the side of blocking (fail-closed)
- Never execute the command yourself — you are only classifying it"#;

/// Request for a single LLM classification call.
#[derive(Debug, Clone)]
pub struct YoloLlmRequest {
    /// The tool name being classified.
    pub tool_name: String,
    /// The JSON input of the tool call.
    pub tool_input: serde_json::Value,
    /// Conversation context (recent messages for the classifier to consider).
    pub context: String,
    /// Maximum tokens for the response.
    pub max_tokens: u32,
    /// Optional stop sequences.
    pub stop_sequences: Vec<String>,
    /// Suffix hint appended to the prompt.
    pub suffix_hint: Option<String>,
}

/// Response from a single LLM classification call.
#[derive(Debug, Clone)]
pub struct YoloLlmResponse {
    /// The raw text output from the LLM.
    pub text: String,
}

/// Trait for LLM-backed classification. Implemented by the query engine layer.
pub trait YoloLlmClient: Send + Sync {
    fn classify(
        &self,
        request: YoloLlmRequest,
    ) -> Pin<Box<dyn Future<Output = anyhow::Result<YoloLlmResponse>> + Send + '_>>;
}

/// 2-stage LLM-based YOLO classifier.
///
/// Stage 1 ("fast"): max_tokens=64, stop_sequences=['</block>'], parses `<block>yes|no</block>`.
/// If allow, returns immediately. If block/unparseable, escalates to Stage 2.
///
/// Stage 2 ("thinking"): max_tokens=4096, no stop sequences, full chain-of-thought.
/// Both share the same system prompt with XML output format.
pub struct YoloLlmClassifier {
    client: Option<Box<dyn YoloLlmClient>>,
}

impl YoloLlmClassifier {
    /// Create a new LLM classifier with an optional client.
    /// When no client is provided, classification falls back to rule-based only.
    pub fn new(client: Option<Box<dyn YoloLlmClient>>) -> Self {
        Self { client }
    }

    /// Classify a tool use through the 2-stage LLM pipeline.
    ///
    /// First checks rule-based bypasses (safe tools allowlist), then
    /// invokes the 2-stage LLM classifier if a client is available.
    /// Returns `None` if no LLM client is configured (caller should
    /// fall back to rule-based classification).
    pub async fn classify(
        &self,
        tool_name: &str,
        tool_input: &serde_json::Value,
        context: &str,
    ) -> Option<YoloClassifierResult> {
        let client = self.client.as_ref()?;

        // Safe tools bypass the classifier entirely
        if is_read_only_tool(tool_name) {
            return Some(YoloClassifierResult::Allow);
        }

        let input_str = serde_json::to_string_pretty(tool_input)
            .unwrap_or_else(|_| tool_input.to_string());

        // Stage 1: Fast classification
        let stage1_request = YoloLlmRequest {
            tool_name: tool_name.to_owned(),
            tool_input: tool_input.clone(),
            context: context.to_owned(),
            max_tokens: 64,
            stop_sequences: vec!["</block>".to_owned()],
            suffix_hint: Some("Err on the side of blocking.".to_owned()),
        };

        let stage1_response = client.classify(stage1_request).await.ok()?;

        if let Some(decision) = parse_block_xml(&stage1_response.text) {
            match decision {
                BlockDecision::Allow => return Some(YoloClassifierResult::Allow),
                BlockDecision::Block => {
                    // Escalate to stage 2 for reasoning
                }
            }
        }

        // Stage 2: Thinking classification (full chain-of-thought)
        let stage2_request = YoloLlmRequest {
            tool_name: tool_name.to_owned(),
            tool_input: tool_input.clone(),
            context: context.to_owned(),
            max_tokens: 4096,
            stop_sequences: vec![],
            suffix_hint: None,
        };

        let stage2_response = match client.classify(stage2_request).await {
            Ok(resp) => resp,
            Err(_) => {
                // Fail-closed: if stage 2 fails, block
                return Some(YoloClassifierResult::Deny(
                    "LLM classifier stage 2 failed — fail-closed".to_owned(),
                ));
            }
        };

        // Parse stage 2 response
        if let Some(decision) = parse_block_xml(&stage2_response.text) {
            match decision {
                BlockDecision::Allow => return Some(YoloClassifierResult::Allow),
                BlockDecision::Block => {
                    return Some(YoloClassifierResult::Deny(
                        "LLM classifier blocked the operation".to_owned(),
                    ));
                }
            }
        }

        // Fail-closed: unparseable response → block
        Some(YoloClassifierResult::Deny(
            "LLM classifier returned unparseable response — fail-closed".to_owned(),
        ))
    }
}

/// Parsed decision from XML block output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockDecision {
    Allow,
    Block,
}

/// Parse `<block>yes</block>` or `<block>no</block>` from LLM output.
fn parse_block_xml(text: &str) -> Option<BlockDecision> {
    // Try to find the last <block>...</block> in the text
    let lower = text.to_ascii_lowercase();

    // Find the last occurrence of <block>
    if let Some(idx) = lower.rfind("<block>") {
        let after_open = &lower[idx + 7..];
        if let Some(close_idx) = after_open.find("</block>") {
            let content = after_open[..close_idx].trim();
            return match content {
                "yes" | "allow" | "safe" => Some(BlockDecision::Allow),
                "no" | "block" | "deny" | "unsafe" => Some(BlockDecision::Block),
                _ => None,
            };
        }
    }

    None
}

/// Resolve the model to use for the YOLO classifier.
///
/// Mirrors TS model selection: env var → GrowthBook → main loop model.
pub fn resolve_yolo_model(main_model: &str) -> String {
    // 1. Check env var override
    if let Ok(model) = std::env::var("CLAUDE_CODE_AUTO_MODE_MODEL") {
        if !model.is_empty() {
            return model;
        }
    }

    // 2. Fall back to main loop model
    // (GrowthBook feature flag would be checked here in the full impl)
    main_model.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn yolo_classifier_result_allow_equality() {
        assert_eq!(YoloClassifierResult::Allow, YoloClassifierResult::Allow);
    }

    #[test]
    fn yolo_classifier_result_deny_equality() {
        assert_eq!(
            YoloClassifierResult::Deny("reason".into()),
            YoloClassifierResult::Deny("reason".into())
        );
    }

    #[test]
    fn yolo_classifier_result_ask_equality() {
        assert_eq!(
            YoloClassifierResult::Ask("prompt".into()),
            YoloClassifierResult::Ask("prompt".into())
        );
    }

    #[test]
    fn read_only_tools_are_allowed() {
        let rules = AutoModeRules::external();
        for tool in &["Read", "Grep", "Glob", "LS", "WebFetch"] {
            let result = classify_tool_use(tool, &json!({}), &rules);
            assert!(
                matches!(result, YoloClassifierResult::Allow),
                "Expected Allow for tool '{tool}', got {result:?}"
            );
        }
    }

    #[test]
    fn safe_git_commands_are_allowed() {
        let rules = AutoModeRules::external();
        let result = classify_tool_use("Bash", &json!({"command": "git status"}), &rules);
        assert!(matches!(result, YoloClassifierResult::Allow));
    }

    #[test]
    fn dangerous_commands_are_denied() {
        let rules = AutoModeRules::external();
        let result = classify_tool_use("Bash", &json!({"command": "rm -rf /"}), &rules);
        assert!(matches!(result, YoloClassifierResult::Deny(_)));
    }

    #[test]
    fn sudo_commands_are_denied() {
        let rules = AutoModeRules::external();
        let result = classify_tool_use(
            "Bash",
            &json!({"command": "sudo apt install something"}),
            &rules,
        );
        assert!(matches!(result, YoloClassifierResult::Deny(_)));
    }

    #[test]
    fn force_push_is_denied() {
        let rules = AutoModeRules::external();
        let result = classify_tool_use(
            "Bash",
            &json!({"command": "git push --force origin main"}),
            &rules,
        );
        assert!(matches!(result, YoloClassifierResult::Deny(_)));
    }

    #[test]
    fn unknown_commands_ask() {
        let rules = AutoModeRules::external();
        let result = classify_tool_use(
            "Bash",
            &json!({"command": "some-unknown-tool arg1 arg2"}),
            &rules,
        );
        assert!(matches!(result, YoloClassifierResult::Ask(_)));
    }

    #[test]
    fn pipe_commands_ask() {
        let rules = AutoModeRules::external();
        let result = classify_tool_use(
            "Bash",
            &json!({"command": "cat file.txt | grep pattern"}),
            &rules,
        );
        assert!(matches!(result, YoloClassifierResult::Ask(_)));
    }

    #[test]
    fn write_tool_to_safe_path_is_allowed() {
        let rules = AutoModeRules::external();
        let result = classify_tool_use(
            "Write",
            &json!({"path": "/home/user/project/src/main.rs"}),
            &rules,
        );
        assert!(matches!(result, YoloClassifierResult::Allow));
    }

    #[test]
    fn write_tool_to_system_path_is_asked() {
        let rules = AutoModeRules::external();
        let result = classify_tool_use("Write", &json!({"path": "/etc/passwd"}), &rules);
        assert!(matches!(result, YoloClassifierResult::Ask(_)));
    }

    #[test]
    fn mcp_tools_ask() {
        let rules = AutoModeRules::external();
        let result = classify_tool_use(
            "mcp__filesystem__read",
            &json!({"path": "/tmp/test"}),
            &rules,
        );
        assert!(matches!(result, YoloClassifierResult::Ask(_)));
    }

    #[test]
    fn anthropic_rules_allow_more_operations() {
        let rules = AutoModeRules::anthropic();
        let result = classify_tool_use("Bash", &json!({"command": "git add src/main.rs"}), &rules);
        assert!(matches!(result, YoloClassifierResult::Allow));
    }

    #[test]
    fn external_rules_deny_git_add() {
        let rules = AutoModeRules::external();
        let result = classify_tool_use("Bash", &json!({"command": "git add src/main.rs"}), &rules);
        // git add is not in external allow list, so it should ask
        assert!(matches!(result, YoloClassifierResult::Ask(_)));
    }

    #[test]
    fn auto_mode_rules_is_allowed() {
        let rules = AutoModeRules::external();
        assert!(rules.is_allowed("git status"));
        assert!(rules.is_allowed("cargo build"));
        assert!(rules.is_allowed("ls"));
        assert!(!rules.is_allowed("rm -rf /"));
    }

    #[test]
    fn auto_mode_rules_is_soft_denied() {
        let rules = AutoModeRules::external();
        assert!(rules.is_soft_denied("rm -rf /"));
        assert!(rules.is_soft_denied("sudo something"));
        assert!(!rules.is_soft_denied("git status"));
    }

    #[test]
    fn auto_mode_rules_env_allowed() {
        let rules = AutoModeRules::external();
        assert!(rules.is_env_allowed("PATH"));
        assert!(rules.is_env_allowed("HOME"));
        assert!(rules.is_env_allowed("Cargo_Home")); // case-insensitive
        assert!(!rules.is_env_allowed("AWS_SECRET_ACCESS_KEY"));
    }

    #[test]
    fn command_substitution_asks() {
        let rules = AutoModeRules::external();
        let result = classify_tool_use(
            "Bash",
            &json!({"command": "echo $(cat /etc/passwd)"}),
            &rules,
        );
        assert!(matches!(result, YoloClassifierResult::Ask(_)));
    }

    #[test]
    fn apply_auto_mode_rules_delegates_to_classify() {
        let rules = AutoModeRules::external();
        let result = apply_auto_mode_rules("Read", &json!({"path": "/tmp/test"}), &rules);
        assert!(matches!(result, YoloClassifierResult::Allow));
    }

    #[test]
    fn default_rules_are_external() {
        let default = AutoModeRules::default();
        let external = AutoModeRules::external();
        assert_eq!(default.allow.len(), external.allow.len());
    }

    // ---- XML block parsing tests ----

    #[test]
    fn parse_block_xml_yes() {
        assert_eq!(parse_block_xml("<block>yes</block>"), Some(BlockDecision::Allow));
    }

    #[test]
    fn parse_block_xml_no() {
        assert_eq!(parse_block_xml("<block>no</block>"), Some(BlockDecision::Block));
    }

    #[test]
    fn parse_block_xml_with_reasoning() {
        let text = "Let me analyze this...\n<block>no</block>";
        assert_eq!(parse_block_xml(text), Some(BlockDecision::Block));
    }

    #[test]
    fn parse_block_xml_case_insensitive() {
        assert_eq!(parse_block_xml("<Block>YES</Block>"), Some(BlockDecision::Allow));
    }

    #[test]
    fn parse_block_xml_invalid() {
        assert_eq!(parse_block_xml("no xml here"), None);
    }

    #[test]
    fn parse_block_xml_empty() {
        assert_eq!(parse_block_xml("<block></block>"), None);
    }

    #[test]
    fn parse_block_xml_trailing() {
        // Only last <block> matters
        let text = "<block>yes</block> actually wait <block>no</block>";
        assert_eq!(parse_block_xml(text), Some(BlockDecision::Block));
    }

    #[test]
    fn parse_block_xml_allow_aliases() {
        assert_eq!(parse_block_xml("<block>allow</block>"), Some(BlockDecision::Allow));
        assert_eq!(parse_block_xml("<block>safe</block>"), Some(BlockDecision::Allow));
    }

    #[test]
    fn parse_block_xml_block_aliases() {
        assert_eq!(parse_block_xml("<block>block</block>"), Some(BlockDecision::Block));
        assert_eq!(parse_block_xml("<block>deny</block>"), Some(BlockDecision::Block));
        assert_eq!(parse_block_xml("<block>unsafe</block>"), Some(BlockDecision::Block));
    }

    #[test]
    fn resolve_yolo_model_defaults_to_main() {
        // Clear env var to ensure no override
        #[allow(unsafe_code)]
        unsafe { std::env::remove_var("CLAUDE_CODE_AUTO_MODE_MODEL"); }
        assert_eq!(resolve_yolo_model("claude-sonnet-4-6"), "claude-sonnet-4-6");
    }

    #[test]
    fn yolo_llm_classifier_no_client_returns_none() {
        let classifier = YoloLlmClassifier::new(None);
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let result = rt.block_on(classifier.classify("Bash", &json!({"command": "echo hi"}), ""));
        assert!(result.is_none(), "without client, should return None");
    }

    #[test]
    fn yolo_llm_classifier_read_only_bypasses_llm() {
        struct MockClient;
        impl YoloLlmClient for MockClient {
            fn classify(
                &self,
                _request: YoloLlmRequest,
            ) -> Pin<Box<dyn Future<Output = anyhow::Result<YoloLlmResponse>> + Send + '_>> {
                // Should never be called for read-only tools
                Box::pin(async { panic!("should not call LLM for read-only tools") })
            }
        }

        let classifier = YoloLlmClassifier::new(Some(Box::new(MockClient)));
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let result = rt.block_on(classifier.classify("Read", &json!({"path": "/tmp/test"}), ""));
        assert!(matches!(result, Some(YoloClassifierResult::Allow)));
    }

    #[test]
    fn yolo_llm_classifier_stage1_allow() {
        struct AllowClient;
        impl YoloLlmClient for AllowClient {
            fn classify(
                &self,
                request: YoloLlmRequest,
            ) -> Pin<Box<dyn Future<Output = anyhow::Result<YoloLlmResponse>> + Send + '_>> {
                let max_tokens = request.max_tokens;
                Box::pin(async move {
                    // Stage 1 (max_tokens=64) returns allow
                    if max_tokens == 64 {
                        Ok(YoloLlmResponse { text: "<block>yes</block>".to_owned() })
                    } else {
                        panic!("stage 2 should not be called when stage 1 allows")
                    }
                })
            }
        }

        let classifier = YoloLlmClassifier::new(Some(Box::new(AllowClient)));
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let result = rt.block_on(classifier.classify("Bash", &json!({"command": "echo hi"}), ""));
        assert!(matches!(result, Some(YoloClassifierResult::Allow)));
    }

    #[test]
    fn yolo_llm_classifier_stage2_block() {
        struct BlockClient;
        impl YoloLlmClient for BlockClient {
            fn classify(
                &self,
                request: YoloLlmRequest,
            ) -> Pin<Box<dyn Future<Output = anyhow::Result<YoloLlmResponse>> + Send + '_>> {
                let max_tokens = request.max_tokens;
                Box::pin(async move {
                    if max_tokens == 64 {
                        // Stage 1 returns block → escalate to stage 2
                        Ok(YoloLlmResponse { text: "<block>no</block>".to_owned() })
                    } else {
                        // Stage 2 confirms block with reasoning
                        Ok(YoloLlmResponse {
                            text: "This command is dangerous because...\n<block>no</block>".to_owned(),
                        })
                    }
                })
            }
        }

        let classifier = YoloLlmClassifier::new(Some(Box::new(BlockClient)));
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let result = rt.block_on(classifier.classify("Bash", &json!({"command": "rm -rf /"}), ""));
        assert!(matches!(result, Some(YoloClassifierResult::Deny(_))));
    }

    #[test]
    fn yolo_llm_classifier_fail_closed_on_unparseable() {
        struct UnparseableClient;
        impl YoloLlmClient for UnparseableClient {
            fn classify(
                &self,
                _request: YoloLlmRequest,
            ) -> Pin<Box<dyn Future<Output = anyhow::Result<YoloLlmResponse>> + Send + '_>> {
                Box::pin(async {
                    Ok(YoloLlmResponse { text: "I cannot determine...".to_owned() })
                })
            }
        }

        let classifier = YoloLlmClassifier::new(Some(Box::new(UnparseableClient)));
        let rt = tokio::runtime::Runtime::new().expect("runtime");
        let result = rt.block_on(classifier.classify("Bash", &json!({"command": "unknown"}), ""));
        assert!(matches!(result, Some(YoloClassifierResult::Deny(_))));
    }
}

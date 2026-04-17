//! Classifier system for automatic permission decisions.
//!
//! Corresponds to `src/utils/permissions/yoloClassifier.ts`,
//! `src/utils/permissions/bashClassifier.ts`, and
//! `src/utils/permissions/classifierShared.ts`.

use crate::dangerous_patterns::{has_dangerous_patterns, is_critically_dangerous};
use async_trait::async_trait;

/// Result from a classifier evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClassifierResult {
    /// Whether the classifier recommends allowing.
    pub should_allow: bool,
    /// Confidence level (0-100).
    pub confidence: u8,
    /// Human-readable reason for the decision.
    pub reason: String,
    /// The classifier that produced this result.
    pub classifier_name: String,
}

impl ClassifierResult {
    /// Create an allow result.
    pub fn allow(reason: impl Into<String>, confidence: u8) -> Self {
        Self {
            should_allow: true,
            confidence,
            reason: reason.into(),
            classifier_name: String::new(),
        }
    }

    /// Create a deny result.
    pub fn deny(reason: impl Into<String>, confidence: u8) -> Self {
        Self {
            should_allow: false,
            confidence,
            reason: reason.into(),
            classifier_name: String::new(),
        }
    }
}

/// Trait for permission classifiers.
#[async_trait]
pub trait PermissionClassifier: Send + Sync {
    /// Name of the classifier.
    fn name(&self) -> &str;

    /// Classify a tool invocation for permission.
    async fn classify(
        &self,
        tool_name: &str,
        input: &serde_json::Value,
        cwd: &str,
    ) -> ClassifierResult;
}

/// YOLO classifier — automatically approves safe operations.
///
/// Corresponds to `src/utils/permissions/yoloClassifier.ts`.
/// This classifier uses a list of known-safe patterns to auto-approve
/// read-only operations and common safe commands.
pub struct YoloClassifier {
    /// Whether the classifier is enabled.
    enabled: bool,
}

impl YoloClassifier {
    /// Create a new YOLO classifier.
    #[must_use]
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    /// Known-safe read-only tools that can always be auto-approved.
    const SAFE_READ_TOOLS: &[&str] = &["Read", "Grep", "Glob", "LS", "WebFetch", "WebSearch"];

    /// Known-safe bash command prefixes.
    const SAFE_BASH_PREFIXES: &[&str] = &[
        "git status",
        "git log",
        "git diff",
        "git branch",
        "git remote",
        "ls",
        "cat",
        "head",
        "tail",
        "wc",
        "echo",
        "pwd",
        "which",
        "env",
        "printenv",
        "node --version",
        "python --version",
        "rustc --version",
        "cargo --version",
        "npm --version",
        "npm list",
        "npm view",
        "pip list",
        "pip show",
        "cargo check",
        "cargo test",
        "cargo build",
        "cargo clippy",
        "cargo doc",
        "cargo tree",
        "df",
        "du",
        "free",
        "uname",
        "date",
        "whoami",
        "id",
        "hostname",
    ];

    /// Check if a tool is a safe read-only tool.
    #[must_use]
    pub fn is_safe_read_tool(tool_name: &str) -> bool {
        Self::SAFE_READ_TOOLS.contains(&tool_name)
    }

    /// Check if a bash command is safe.
    #[must_use]
    pub fn is_safe_bash_command(command: &str) -> bool {
        let trimmed = command.trim();

        // Reject if dangerous patterns detected
        if is_critically_dangerous(trimmed) {
            return false;
        }

        // Check safe prefixes
        for prefix in Self::SAFE_BASH_PREFIXES {
            if trimmed.starts_with(prefix) {
                // Ensure no shell chaining with dangerous commands
                if trimmed.contains("&&") || trimmed.contains("|") || trimmed.contains(";") {
                    // Only allow if the entire command matches the safe prefix
                    return trimmed == *prefix;
                }
                return true;
            }
        }

        false
    }
}

#[async_trait]
impl PermissionClassifier for YoloClassifier {
    fn name(&self) -> &str {
        "yolo"
    }

    async fn classify(
        &self,
        tool_name: &str,
        input: &serde_json::Value,
        _cwd: &str,
    ) -> ClassifierResult {
        if !self.enabled {
            return ClassifierResult::deny("YOLO classifier disabled", 100);
        }

        // Safe read-only tools
        if Self::is_safe_read_tool(tool_name) {
            return ClassifierResult {
                should_allow: true,
                confidence: 95,
                reason: format!("{} is a safe read-only tool", tool_name),
                classifier_name: self.name().to_string(),
            };
        }

        // Bash commands
        if (tool_name == "Bash" || tool_name == "BashCommand")
            && let Some(command) = input.get("command").and_then(|v| v.as_str())
        {
            if Self::is_safe_bash_command(command) {
                return ClassifierResult {
                    should_allow: true,
                    confidence: 90,
                    reason: format!(
                        "Bash command '{}' is in safe list",
                        truncate_command(command, 40)
                    ),
                    classifier_name: self.name().to_string(),
                };
            }
            if is_critically_dangerous(command) {
                return ClassifierResult {
                    should_allow: false,
                    confidence: 99,
                    reason: format!(
                        "Bash command '{}' matches critical danger pattern",
                        truncate_command(command, 40)
                    ),
                    classifier_name: self.name().to_string(),
                };
            }
        }

        // File write tools — check path safety
        if (tool_name == "Write" || tool_name == "Edit" || tool_name == "MultiEdit")
            && let Some(path) = input
                .get("file_path")
                .or_else(|| input.get("path"))
                .and_then(|v| v.as_str())
        {
            // Reject writes outside cwd
            if path.starts_with("/etc/") || path.starts_with("/usr/") || path.starts_with("/sys/") {
                return ClassifierResult {
                    should_allow: false,
                    confidence: 95,
                    reason: format!("Write to system path '{}' blocked", path),
                    classifier_name: self.name().to_string(),
                };
            }
            // Allow writes within project
            if !path.starts_with("..") && !path.starts_with("~") {
                return ClassifierResult {
                    should_allow: true,
                    confidence: 80,
                    reason: format!(
                        "Write to project file '{}' allowed",
                        truncate_command(path, 40)
                    ),
                    classifier_name: self.name().to_string(),
                };
            }
        }

        ClassifierResult {
            should_allow: false,
            confidence: 50,
            reason: format!("No safe pattern match for tool '{}'", tool_name),
            classifier_name: self.name().to_string(),
        }
    }
}

/// Bash-specific classifier for more fine-grained command analysis.
///
/// Corresponds to `src/utils/permissions/bashClassifier.ts`.
pub struct BashClassifier;

impl BashClassifier {
    /// Classify a bash command into a safety category.
    #[must_use]
    pub fn classify_command(command: &str) -> BashCommandCategory {
        let trimmed = command.trim();

        if is_critically_dangerous(trimmed) {
            return BashCommandCategory::Critical;
        }

        if has_dangerous_patterns(trimmed) {
            return BashCommandCategory::Dangerous;
        }

        // Check for common safe patterns
        if YoloClassifier::is_safe_bash_command(trimmed) {
            return BashCommandCategory::Safe;
        }

        // Check for common development commands
        let dev_prefixes = [
            "npm run",
            "npm test",
            "npm start",
            "npm build",
            "cargo run",
            "cargo test",
            "cargo build",
            "cargo check",
            "cargo clippy",
            "make",
            "pytest",
            "jest",
            "vitest",
            "go test",
            "go build",
            "go run",
            "python ",
            "python3 ",
            "node ",
            "git add",
            "git commit",
            "git checkout",
            "git stash",
            "docker compose",
            "docker-compose",
            "kubectl",
        ];

        for prefix in dev_prefixes {
            if trimmed.starts_with(prefix) {
                return BashCommandCategory::Development;
            }
        }

        BashCommandCategory::Unknown
    }
}

/// Safety category for bash commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BashCommandCategory {
    /// Safe read-only or well-known safe command.
    Safe,
    /// Common development command (build, test, etc.).
    Development,
    /// Unknown command — needs review.
    Unknown,
    /// Potentially dangerous command.
    Dangerous,
    /// Critically dangerous command.
    Critical,
}

/// Truncate a command string for display.
fn truncate_command(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn yolo_allows_read_tools() {
        let classifier = YoloClassifier::new(true);
        for tool in YoloClassifier::SAFE_READ_TOOLS {
            let result = classifier
                .classify(tool, &serde_json::json!({}), "/tmp")
                .await;
            assert!(result.should_allow, "{} should be allowed", tool);
        }
    }

    #[tokio::test]
    async fn yolo_allows_safe_bash() {
        let classifier = YoloClassifier::new(true);
        let result = classifier
            .classify(
                "Bash",
                &serde_json::json!({"command": "git status"}),
                "/tmp",
            )
            .await;
        assert!(result.should_allow);
    }

    #[tokio::test]
    async fn yolo_blocks_critical_commands() {
        let classifier = YoloClassifier::new(true);
        let result = classifier
            .classify("Bash", &serde_json::json!({"command": "rm -rf /"}), "/tmp")
            .await;
        assert!(!result.should_allow);
    }

    #[tokio::test]
    async fn yolo_disabled_blocks_everything() {
        let classifier = YoloClassifier::new(false);
        let result = classifier
            .classify("Read", &serde_json::json!({}), "/tmp")
            .await;
        assert!(!result.should_allow);
    }

    #[tokio::test]
    async fn yolo_allows_project_writes() {
        let classifier = YoloClassifier::new(true);
        let result = classifier
            .classify(
                "Write",
                &serde_json::json!({"file_path": "src/main.rs"}),
                "/tmp",
            )
            .await;
        assert!(result.should_allow);
    }

    #[tokio::test]
    async fn yolo_blocks_system_writes() {
        let classifier = YoloClassifier::new(true);
        let result = classifier
            .classify(
                "Write",
                &serde_json::json!({"file_path": "/etc/passwd"}),
                "/tmp",
            )
            .await;
        assert!(!result.should_allow);
    }

    #[test]
    fn bash_classifier_categories() {
        assert_eq!(
            BashClassifier::classify_command("git status"),
            BashCommandCategory::Safe
        );
        // "cargo test" is in SAFE_BASH_PREFIXES, so it's Safe not Development
        assert_eq!(
            BashClassifier::classify_command("cargo test"),
            BashCommandCategory::Safe
        );
        assert_eq!(
            BashClassifier::classify_command("npm run build"),
            BashCommandCategory::Development
        );
        assert_eq!(
            BashClassifier::classify_command("rm -rf /"),
            BashCommandCategory::Critical
        );
        assert_eq!(
            BashClassifier::classify_command("sudo rm foo"),
            BashCommandCategory::Dangerous
        );
        assert_eq!(
            BashClassifier::classify_command("some-unknown-cmd"),
            BashCommandCategory::Unknown
        );
    }

    #[test]
    fn safe_bash_command_no_chaining() {
        assert!(YoloClassifier::is_safe_bash_command("git status"));
        assert!(!YoloClassifier::is_safe_bash_command(
            "git status && rm -rf /"
        ));
    }
}

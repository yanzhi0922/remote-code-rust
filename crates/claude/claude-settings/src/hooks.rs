//! Hook settings types — extended to support all 26 hook events.
//!
//! Corresponds to `src/utils/settings/types.ts` (hooks field) and
//! `src/schemas/hooks.ts` (HookMatcherSchema, HooksSchema).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Shell type for command hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookShellType {
    /// POSIX-compatible shell.
    Bash,
    /// Windows PowerShell.
    PowerShell,
}

impl HookShellType {
    /// Return the platform default shell.
    #[must_use]
    pub fn platform_default() -> Self {
        if cfg!(windows) {
            Self::PowerShell
        } else {
            Self::Bash
        }
    }
}

/// Hook command configuration — corresponds to the discriminated union
/// `HookCommandSchema` from `schemas/hooks.ts`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum HookCommandConfig {
    /// Shell command hook.
    Command(BashCommandHookConfig),
    /// LLM prompt hook.
    Prompt(PromptHookConfig),
    /// Agent verifier hook.
    Agent(AgentHookConfig),
    /// HTTP POST hook.
    Http(HttpHookConfig),
}

/// Bash command hook configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BashCommandHookConfig {
    /// Shell command to execute.
    pub command: String,
    /// Optional shell interpreter.
    #[serde(default)]
    pub shell: Option<HookShellType>,
    /// Timeout in seconds.
    #[serde(default)]
    pub timeout: Option<u64>,
    /// Permission-rule syntax condition.
    #[serde(default, rename = "if")]
    pub if_condition: Option<String>,
    /// Custom status message.
    #[serde(default)]
    pub status_message: Option<String>,
    /// Run only once per session.
    #[serde(default)]
    pub once: bool,
    /// Run asynchronously.
    #[serde(default)]
    pub r#async: bool,
    /// Async with re-wake on exit code 2.
    #[serde(default)]
    pub async_rewake: bool,
}

/// Prompt hook configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptHookConfig {
    /// Prompt text.
    pub prompt: String,
    /// Model to use.
    #[serde(default)]
    pub model: Option<String>,
    /// Timeout in seconds.
    #[serde(default)]
    pub timeout: Option<u64>,
    /// Permission-rule syntax condition.
    #[serde(default, rename = "if")]
    pub if_condition: Option<String>,
    /// Custom status message.
    #[serde(default)]
    pub status_message: Option<String>,
    /// Run only once.
    #[serde(default)]
    pub once: bool,
}

/// Agent verifier hook configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentHookConfig {
    /// Prompt describing what to verify.
    pub prompt: String,
    /// Model to use.
    #[serde(default)]
    pub model: Option<String>,
    /// Timeout in seconds.
    #[serde(default)]
    pub timeout: Option<u64>,
    /// Permission-rule syntax condition.
    #[serde(default, rename = "if")]
    pub if_condition: Option<String>,
    /// Custom status message.
    #[serde(default)]
    pub status_message: Option<String>,
    /// Run only once.
    #[serde(default)]
    pub once: bool,
}

/// HTTP hook configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpHookConfig {
    /// URL to POST hook input JSON to.
    pub url: String,
    /// Additional headers.
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Allowed env vars for interpolation.
    #[serde(default)]
    pub allowed_env_vars: Vec<String>,
    /// Timeout in seconds.
    #[serde(default)]
    pub timeout: Option<u64>,
    /// Permission-rule syntax condition.
    #[serde(default, rename = "if")]
    pub if_condition: Option<String>,
    /// Custom status message.
    #[serde(default)]
    pub status_message: Option<String>,
    /// Run only once.
    #[serde(default)]
    pub once: bool,
}

/// Hook matcher configuration — groups a pattern with its associated hooks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookMatcherConfig {
    /// Optional tool-name pattern (e.g. `"Write"` or `"Bash|Edit"`).
    #[serde(default)]
    pub matcher: Option<String>,
    /// Hooks to run when the matcher fires.
    #[serde(default)]
    pub hooks: Vec<HookCommandConfig>,
}

/// Hook settings configuration — supports all 26 hook events.
///
/// Uses a flexible map-based approach where each key is an event name
/// and the value is a list of matcher configurations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookSettings {
    /// Hooks indexed by event name (PascalCase, e.g. "PreToolUse").
    #[serde(flatten)]
    pub events: HashMap<String, Vec<HookMatcherConfig>>,
}

/// Legacy hook entry for backward compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookEntry {
    /// The hook command to execute.
    pub command: String,
    /// Optional matcher for tool names.
    pub matcher: Option<String>,
    /// Optional timeout in milliseconds.
    pub timeout: Option<u64>,
    /// Whether to run the hook in the background.
    #[serde(default)]
    pub background: bool,
    /// Environment variables for the hook.
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// All 26 standard hook event names.
pub const HOOK_EVENT_NAMES: &[&str] = &[
    "PreToolUse",
    "PostToolUse",
    "PostToolUseFailure",
    "Notification",
    "UserPromptSubmit",
    "SessionStart",
    "SessionEnd",
    "Stop",
    "StopFailure",
    "SubagentStart",
    "SubagentStop",
    "PreCompact",
    "PostCompact",
    "PermissionRequest",
    "PermissionDenied",
    "Setup",
    "TeammateIdle",
    "TaskCreated",
    "TaskCompleted",
    "Elicitation",
    "ElicitationResult",
    "ConfigChange",
    "WorktreeCreate",
    "WorktreeRemove",
    "InstructionsLoaded",
    "CwdChanged",
    "FileChanged",
];

impl HookSettings {
    /// Get hook matchers for a specific event type.
    #[must_use]
    pub fn get_hooks(&self, event: &str) -> &[HookMatcherConfig] {
        match self.events.get(event) {
            Some(hooks) => hooks,
            None => &[],
        }
    }

    /// Check if any hooks are configured.
    #[must_use]
    pub fn has_hooks(&self) -> bool {
        self.events.values().any(|h| !h.is_empty())
    }

    /// Check if hooks are configured for a specific event.
    #[must_use]
    pub fn has_hooks_for_event(&self, event: &str) -> bool {
        self.events.get(event).is_some_and(|h| !h.is_empty())
    }

    /// Get all configured event names.
    #[must_use]
    pub fn configured_events(&self) -> Vec<&str> {
        self.events
            .keys()
            .filter(|k| HOOK_EVENT_NAMES.contains(&k.as_str()))
            .map(String::as_str)
            .collect()
    }

    /// Total number of hook matchers across all events.
    #[must_use]
    pub fn total_matcher_count(&self) -> usize {
        self.events.values().map(Vec::len).sum()
    }

    /// Total number of individual hooks across all events and matchers.
    #[must_use]
    pub fn total_hook_count(&self) -> usize {
        self.events
            .values()
            .flat_map(|matchers| matchers.iter())
            .map(|m| m.hooks.len())
            .sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_command_hook(cmd: &str) -> HookCommandConfig {
        HookCommandConfig::Command(BashCommandHookConfig {
            command: cmd.to_string(),
            shell: None,
            timeout: None,
            if_condition: None,
            status_message: None,
            once: false,
            r#async: false,
            async_rewake: false,
        })
    }

    fn make_prompt_hook(prompt: &str) -> HookCommandConfig {
        HookCommandConfig::Prompt(PromptHookConfig {
            prompt: prompt.to_string(),
            model: None,
            timeout: None,
            if_condition: None,
            status_message: None,
            once: false,
        })
    }

    // ── Basic tests ──────────────────────────────────────────────────────

    #[test]
    fn default_is_empty() {
        let h = HookSettings::default();
        assert!(!h.has_hooks());
    }

    #[test]
    fn has_hooks_with_pre_tool() {
        let mut events = HashMap::new();
        events.insert(
            "PreToolUse".to_string(),
            vec![HookMatcherConfig {
                matcher: None,
                hooks: vec![make_command_hook("echo test")],
            }],
        );
        let h = HookSettings { events };
        assert!(h.has_hooks());
    }

    #[test]
    fn get_hooks_by_event() {
        let mut events = HashMap::new();
        events.insert(
            "Stop".to_string(),
            vec![HookMatcherConfig {
                matcher: None,
                hooks: vec![make_command_hook("cleanup")],
            }],
        );
        let h = HookSettings { events };
        assert_eq!(h.get_hooks("Stop").len(), 1);
        assert!(h.get_hooks("PreToolUse").is_empty());
        assert!(h.get_hooks("unknown").is_empty());
    }

    #[test]
    fn has_hooks_for_event() {
        let mut events = HashMap::new();
        events.insert(
            "PreToolUse".to_string(),
            vec![HookMatcherConfig {
                matcher: None,
                hooks: vec![make_command_hook("lint")],
            }],
        );
        let h = HookSettings { events };
        assert!(h.has_hooks_for_event("PreToolUse"));
        assert!(!h.has_hooks_for_event("PostToolUse"));
    }

    // ── Serialization tests ──────────────────────────────────────────────

    #[test]
    fn command_hook_config_serialization() {
        let hook = make_command_hook("test.sh");
        let json = serde_json::to_string(&hook).expect("serialize");
        assert!(json.contains("\"type\":\"command\""));
        assert!(json.contains("test.sh"));
    }

    #[test]
    fn prompt_hook_config_serialization() {
        let hook = make_prompt_hook("review code");
        let json = serde_json::to_string(&hook).expect("serialize");
        assert!(json.contains("\"type\":\"prompt\""));
        assert!(json.contains("review code"));
    }

    #[test]
    fn hook_settings_serialization() {
        let mut events = HashMap::new();
        events.insert(
            "PreToolUse".to_string(),
            vec![HookMatcherConfig {
                matcher: Some("Bash".to_string()),
                hooks: vec![make_command_hook("lint.sh")],
            }],
        );
        let h = HookSettings { events };
        let json = serde_json::to_string(&h).expect("serialize");
        assert!(json.contains("PreToolUse"));
        assert!(json.contains("Bash"));
        assert!(json.contains("lint.sh"));
    }

    #[test]
    fn hook_settings_deserialization() {
        let json = r#"{"PreToolUse":[{"matcher":"Bash","hooks":[{"type":"command","command":"lint.sh"}]}]}"#;
        let h: HookSettings = serde_json::from_str(json).expect("deserialize");
        assert!(h.has_hooks_for_event("PreToolUse"));
        assert_eq!(h.get_hooks("PreToolUse").len(), 1);
    }

    // ── All 26 events tests ──────────────────────────────────────────────

    #[test]
    fn all_26_events_recognized() {
        assert_eq!(HOOK_EVENT_NAMES.len(), 27);
    }

    #[test]
    fn configure_multiple_events() {
        let mut events = HashMap::new();
        for &name in HOOK_EVENT_NAMES {
            events.insert(
                name.to_string(),
                vec![HookMatcherConfig {
                    matcher: None,
                    hooks: vec![make_command_hook("generic.sh")],
                }],
            );
        }
        let h = HookSettings { events };
        assert!(h.has_hooks());
        assert_eq!(h.configured_events().len(), 27);
    }

    #[test]
    fn total_matcher_count() {
        let mut events = HashMap::new();
        events.insert(
            "PreToolUse".to_string(),
            vec![
                HookMatcherConfig {
                    matcher: Some("Bash".to_string()),
                    hooks: vec![make_command_hook("a.sh")],
                },
                HookMatcherConfig {
                    matcher: Some("Write".to_string()),
                    hooks: vec![make_command_hook("b.sh")],
                },
            ],
        );
        let h = HookSettings { events };
        assert_eq!(h.total_matcher_count(), 2);
    }

    #[test]
    fn total_hook_count() {
        let mut events = HashMap::new();
        events.insert(
            "PreToolUse".to_string(),
            vec![HookMatcherConfig {
                matcher: None,
                hooks: vec![make_command_hook("a.sh"), make_command_hook("b.sh")],
            }],
        );
        let h = HookSettings { events };
        assert_eq!(h.total_hook_count(), 2);
    }

    // ── HookCommandConfig tests ──────────────────────────────────────────

    #[test]
    fn bash_command_hook_with_all_fields() {
        let json = r#"{
            "type": "command",
            "command": "echo hello",
            "shell": "bash",
            "timeout": 30,
            "if": "Bash(git *)",
            "statusMessage": "Running git hook",
            "once": true,
            "async": false,
            "asyncRewake": false
        }"#;
        let hook: HookCommandConfig = serde_json::from_str(json).expect("deserialize");
        match hook {
            HookCommandConfig::Command(cmd) => {
                assert_eq!(cmd.command, "echo hello");
                assert_eq!(cmd.shell, Some(HookShellType::Bash));
                assert_eq!(cmd.timeout, Some(30));
                assert_eq!(cmd.if_condition.as_deref(), Some("Bash(git *)"));
                assert!(cmd.once);
            }
            _ => panic!("expected Command variant"),
        }
    }

    #[test]
    fn prompt_hook_deserialization() {
        let json = r#"{
            "type": "prompt",
            "prompt": "Check code quality",
            "model": "claude-sonnet-4-6",
            "timeout": 60
        }"#;
        let hook: HookCommandConfig = serde_json::from_str(json).expect("deserialize");
        match hook {
            HookCommandConfig::Prompt(p) => {
                assert_eq!(p.prompt, "Check code quality");
                assert_eq!(p.model.as_deref(), Some("claude-sonnet-4-6"));
            }
            _ => panic!("expected Prompt variant"),
        }
    }

    #[test]
    fn http_hook_deserialization() {
        let json = r#"{
            "type": "http",
            "url": "https://example.com/hook",
            "headers": {"Authorization": "Bearer $TOKEN"},
            "allowedEnvVars": ["TOKEN"],
            "timeout": 10
        }"#;
        let hook: HookCommandConfig = serde_json::from_str(json).expect("deserialize");
        match hook {
            HookCommandConfig::Http(h) => {
                assert_eq!(h.url, "https://example.com/hook");
                assert!(h.headers.contains_key("Authorization"));
            }
            _ => panic!("expected Http variant"),
        }
    }

    // ── Backward compatibility tests ─────────────────────────────────────

    #[test]
    fn legacy_hook_entry_serialization() {
        let entry = HookEntry {
            command: "test.sh".to_string(),
            matcher: Some("Bash".to_string()),
            timeout: Some(5000),
            background: true,
            env: HashMap::new(),
        };
        let json = serde_json::to_string(&entry).expect("serialize");
        assert!(json.contains("test.sh"));
        assert!(json.contains("Bash"));
        assert!(json.contains("5000"));
    }

    #[test]
    fn shell_type_platform_default() {
        let shell = HookShellType::platform_default();
        if cfg!(windows) {
            assert_eq!(shell, HookShellType::PowerShell);
        } else {
            assert_eq!(shell, HookShellType::Bash);
        }
    }
}

//! Core settings types.
//!
//! Corresponds to `src/utils/settings/types.ts` (SettingsSchema, lines 284–1109).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::attribution::AttributionSettings;
use crate::hooks::HookSettings;
use crate::mcp::{AllowedMcpServerEntry, DeniedMcpServerEntry};
use crate::permissions::PermissionSettings;
use crate::provider::ProviderConfig;
use crate::sandbox::SandboxSettings;
use crate::worktree::WorktreeSettings;

/// Customization surfaces that can be locked by enterprise policy.
pub const CUSTOMIZATION_SURFACES: &[&str] = &["skills", "hooks", "mcp"];

/// Spinner verb mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SpinnerVerbMode {
    Append,
    Replace,
}

/// Spinner configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpinnerConfig {
    pub mode: SpinnerVerbMode,
    pub verbs: Vec<String>,
}

/// Spinner tips override configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpinnerTipsOverride {
    #[serde(default)]
    pub exclude_default: bool,
    pub tips: Vec<String>,
}

/// Customization surfaces for strict plugin-only customization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CustomizationSurface {
    Skills,
    Hooks,
    Mcp,
}

/// The main settings structure.
///
/// Corresponds to `SettingsSchema` in the TypeScript source (lines 284–1109).
/// Fields use `Option<T>` to represent optional settings — absent values
/// use defaults defined in the application logic.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    // ── Authentication ──────────────────────────────────────────
    /// Path to a script that outputs authentication values.
    pub api_key_helper: Option<String>,
    /// Path to a script that exports AWS credentials.
    pub aws_credential_export: Option<String>,
    /// Path to a script that refreshes AWS authentication.
    pub aws_auth_refresh: Option<String>,
    /// Command to refresh GCP authentication.
    pub gcp_auth_refresh: Option<String>,

    // ── Model & Provider ────────────────────────────────────────
    /// Override the default model.
    pub model: Option<String>,
    /// Select the active model provider by ID.
    pub provider: Option<String>,
    /// Provider configurations keyed by provider ID.
    pub providers: Option<HashMap<String, ProviderConfig>>,
    /// Allowlist of models that users can select.
    pub available_models: Option<Vec<String>>,
    /// Override mapping from Anthropic model ID to provider-specific model ID.
    pub model_overrides: Option<HashMap<String, String>>,
    /// Advisor model for the server-side advisor tool.
    pub advisor_model: Option<String>,
    /// Name of an agent to use for the main thread.
    pub agent: Option<String>,

    // ── Effort & Performance ────────────────────────────────────
    /// Persisted effort level for supported models.
    pub effort_level: Option<String>,
    /// Whether fast mode is enabled.
    pub fast_mode: Option<bool>,
    /// Whether fast mode does not persist across sessions.
    pub fast_mode_per_session_opt_in: Option<bool>,
    /// Whether thinking is always enabled.
    pub always_thinking_enabled: Option<bool>,

    // ── Permissions ─────────────────────────────────────────────
    /// Tool usage permissions configuration.
    pub permissions: Option<PermissionSettings>,

    // ── MCP Servers ─────────────────────────────────────────────
    /// Whether to automatically approve all MCP servers in the project.
    pub enable_all_project_mcp_servers: Option<bool>,
    /// List of approved MCP servers from .mcp.json.
    pub enabled_mcpjson_servers: Option<Vec<String>>,
    /// List of rejected MCP servers from .mcp.json.
    pub disabled_mcpjson_servers: Option<Vec<String>>,
    /// Enterprise allowlist of MCP servers.
    pub allowed_mcp_servers: Option<Vec<AllowedMcpServerEntry>>,
    /// Enterprise denylist of MCP servers.
    pub denied_mcp_servers: Option<Vec<DeniedMcpServerEntry>>,

    // ── Hooks ───────────────────────────────────────────────────
    /// Custom commands to run before/after tool executions.
    pub hooks: Option<HookSettings>,
    /// Whether to disable all hooks and statusLine execution.
    pub disable_all_hooks: Option<bool>,
    /// Only run hooks defined in managed settings.
    pub allow_managed_hooks_only: Option<bool>,
    /// Allowlist of URL patterns HTTP hooks may target.
    pub allowed_http_hook_urls: Option<Vec<String>>,
    /// Allowlist of env var names HTTP hooks may interpolate.
    pub http_hook_allowed_env_vars: Option<Vec<String>>,

    // ── Permissions Policy ──────────────────────────────────────
    /// Only use permission rules from managed settings.
    pub allow_managed_permission_rules_only: Option<bool>,
    /// Only read MCP allowlist from managed settings.
    pub allow_managed_mcp_servers_only: Option<bool>,

    // ── Plugins & Marketplace ───────────────────────────────────
    /// Enabled plugins using marketplace-first format.
    pub enabled_plugins: Option<HashMap<String, serde_json::Value>>,
    /// Strict plugin-only customization policy.
    pub strict_plugin_only_customization: Option<serde_json::Value>,
    /// Strict list of allowed marketplace sources.
    pub strict_known_marketplaces: Option<Vec<serde_json::Value>>,
    /// Blocklist of marketplace sources.
    pub blocked_marketplaces: Option<Vec<serde_json::Value>>,
    /// Per-plugin configuration.
    pub plugin_configs: Option<HashMap<String, serde_json::Value>>,

    // ── Attribution ─────────────────────────────────────────────
    /// Attribution settings for commits and PRs.
    pub attribution: Option<AttributionSettings>,
    /// Whether to include co-authored-by attribution (deprecated).
    pub include_co_authored_by: Option<bool>,
    /// Whether to include git instructions in system prompt.
    pub include_git_instructions: Option<bool>,

    // ── File & Session ──────────────────────────────────────────
    /// Environment variables for sessions.
    pub env: Option<HashMap<String, String>>,
    /// Whether file picker should respect .gitignore.
    pub respect_gitignore: Option<bool>,
    /// Number of days to retain chat transcripts.
    pub cleanup_period_days: Option<u32>,
    /// Custom file suggestion configuration.
    pub file_suggestion: Option<serde_json::Value>,
    /// File checkpointing enabled (default true).
    pub file_checkpointing_enabled: Option<bool>,

    // ── UI & Display ────────────────────────────────────────────
    /// Output style for assistant responses.
    pub output_style: Option<String>,
    /// Preferred language for responses.
    pub language: Option<String>,
    /// Whether to show tips in the spinner.
    pub spinner_tips_enabled: Option<bool>,
    /// Custom spinner verbs.
    pub spinner_verbs: Option<SpinnerConfig>,
    /// Override spinner tips.
    pub spinner_tips_override: Option<SpinnerTipsOverride>,
    /// Whether to disable syntax highlighting in diffs.
    pub syntax_highlighting_disabled: Option<bool>,
    /// Whether /rename updates terminal tab title.
    pub terminal_title_from_rename: Option<bool>,
    /// Whether prompt suggestions are enabled.
    pub prompt_suggestion_enabled: Option<bool>,

    // ── Login & Auth ────────────────────────────────────────────
    /// Force a specific login method.
    pub force_login_method: Option<String>,
    /// Organization UUID for OAuth login.
    pub force_login_org_uuid: Option<String>,

    // ── Worktree ────────────────────────────────────────────────
    /// Git worktree configuration.
    pub worktree: Option<WorktreeSettings>,

    // ── Remote ──────────────────────────────────────────────────
    /// Remote session configuration.
    pub remote: Option<serde_json::Value>,

    // ── Sandbox ─────────────────────────────────────────────────
    /// Sandbox settings.
    pub sandbox: Option<SandboxSettings>,

    // ── Updates ─────────────────────────────────────────────────
    /// Release channel for auto-updates.
    pub auto_updates_channel: Option<String>,
    /// Minimum version to stay on.
    pub minimum_version: Option<String>,

    // ── Misc ────────────────────────────────────────────────────
    /// Default shell for input-box ! commands.
    pub default_shell: Option<String>,
    /// Skip WebFetch blocklist check.
    pub skip_web_fetch_preflight: Option<bool>,
    /// Path to OpenTelemetry headers script.
    pub otel_headers_helper: Option<String>,
    /// Feedback survey rate (0–1).
    pub feedback_survey_rate: Option<f64>,
    /// Show clear context on plan accept.
    pub show_clear_context_on_plan_accept: Option<bool>,
    /// Company announcements.
    pub company_announcements: Option<Vec<String>>,
    /// Custom directory for plan files.
    pub plans_directory: Option<String>,
    /// Whether to enable AI-based classification for Bash permission rules.
    pub classifier_permissions_enabled: Option<bool>,
}

impl Settings {
    /// Create empty settings with all fields set to None.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a specific permission rule is allowed.
    #[must_use]
    pub fn is_permission_allowed(&self, rule: &str) -> bool {
        self.permissions
            .as_ref()
            .and_then(|p| p.allow.as_ref())
            .map_or(false, |rules| rules.iter().any(|r| r == rule))
    }

    /// Check if a specific permission rule is denied.
    #[must_use]
    pub fn is_permission_denied(&self, rule: &str) -> bool {
        self.permissions
            .as_ref()
            .and_then(|p| p.deny.as_ref())
            .map_or(false, |rules| rules.iter().any(|r| r == rule))
    }

    /// Get the effective model, falling back to the provided default.
    #[must_use]
    pub fn effective_model<'a>(&'a self, default: &'a str) -> &'a str {
        match &self.model {
            Some(m) if !m.is_empty() => m.as_str(),
            _ => default,
        }
    }

    /// Check if file checkpointing is enabled (defaults to true).
    #[must_use]
    pub fn is_file_checkpointing_enabled(&self) -> bool {
        self.file_checkpointing_enabled.unwrap_or(true)
    }

    /// Check if hooks are enabled.
    #[must_use]
    pub fn hooks_enabled(&self) -> bool {
        !self.disable_all_hooks.unwrap_or(false)
    }

    /// Check if prompt suggestions are enabled (defaults to true).
    #[must_use]
    pub fn prompt_suggestions_enabled(&self) -> bool {
        self.prompt_suggestion_enabled.unwrap_or(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_settings_is_all_none() {
        let s = Settings::new();
        assert!(s.model.is_none());
        assert!(s.permissions.is_none());
        assert!(s.hooks.is_none());
        assert!(s.providers.is_none());
    }

    #[test]
    fn effective_model_with_override() {
        let mut s = Settings::new();
        assert_eq!(s.effective_model("default-model"), "default-model");
        s.model = Some("claude-opus-4".to_string());
        assert_eq!(s.effective_model("default-model"), "claude-opus-4");
    }

    #[test]
    fn file_checkpointing_defaults_true() {
        let s = Settings::new();
        assert!(s.is_file_checkpointing_enabled());
    }

    #[test]
    fn file_checkpointing_can_disable() {
        let mut s = Settings::new();
        s.file_checkpointing_enabled = Some(false);
        assert!(!s.is_file_checkpointing_enabled());
    }

    #[test]
    fn hooks_enabled_by_default() {
        let s = Settings::new();
        assert!(s.hooks_enabled());
    }

    #[test]
    fn hooks_can_be_disabled() {
        let mut s = Settings::new();
        s.disable_all_hooks = Some(true);
        assert!(!s.hooks_enabled());
    }

    #[test]
    fn prompt_suggestions_default_true() {
        let s = Settings::new();
        assert!(s.prompt_suggestions_enabled());
    }

    #[test]
    fn permission_check() {
        let mut s = Settings::new();
        assert!(!s.is_permission_allowed("Bash(*)"));
        assert!(!s.is_permission_denied("Bash(*)"));

        s.permissions = Some(PermissionSettings {
            allow: Some(vec!["Bash(*)".to_string()]),
            deny: Some(vec!["Edit(/etc/*)".to_string()]),
            ..Default::default()
        });

        assert!(s.is_permission_allowed("Bash(*)"));
        assert!(!s.is_permission_allowed("Edit(/etc/*)"));
        assert!(s.is_permission_denied("Edit(/etc/*)"));
    }

    #[test]
    fn settings_serializes_to_json() {
        let mut s = Settings::new();
        s.model = Some("claude-sonnet-4".to_string());
        s.fast_mode = Some(true);

        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains("claude-sonnet-4"));
        assert!(json.contains("fastMode"));

        let deserialized: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.model.as_deref(), Some("claude-sonnet-4"));
        assert_eq!(deserialized.fast_mode, Some(true));
    }

    #[test]
    fn settings_deserialize_partial() {
        let json = r#"{"model":"test-model","cleanupPeriodDays":14}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s.model.as_deref(), Some("test-model"));
        assert_eq!(s.cleanup_period_days, Some(14));
        assert!(s.permissions.is_none());
    }

    #[test]
    fn customization_surfaces_list() {
        assert!(CUSTOMIZATION_SURFACES.contains(&"skills"));
        assert!(CUSTOMIZATION_SURFACES.contains(&"hooks"));
        assert!(CUSTOMIZATION_SURFACES.contains(&"mcp"));
    }
}

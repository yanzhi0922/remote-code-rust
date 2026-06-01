//! Codex-specific configuration types and config-building helpers.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Context;
use serde::{Deserialize, Serialize};
use toml::Value as TomlValue;

use codex_app_server_protocol::AskForApproval;
use codex_protocol::models::PermissionProfile as CorePermissionProfile;

pub(super) const REMOTE_CODE_PROJECT_QUALIFIER: &str = "com";
pub(super) const REMOTE_CODE_PROJECT_ORGANIZATION: &str = "RemoteCode";
pub(super) const REMOTE_CODE_PROJECT_APPLICATION: &str = "remote-code";

/// Runtime options used when starting the in-process Codex app-server.
///
/// The adapter treats these as in-memory overrides and delegates final
/// validation to Codex's native `ConfigBuilder` and app-server request handlers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CodexAdapterOptions {
    pub cwd: PathBuf,
    pub model: Option<String>,
    pub model_provider: Option<String>,
    #[serde(skip_serializing)]
    pub api_key: Option<String>,
    pub base_url: Option<String>,
    /// Wire protocol for the provider. When set to "anthropic_messages", an
    /// in-process protocol translator proxy is started automatically.
    pub wire_api: Option<String>,
    /// The *real* upstream URL that the protocol translator forwards to.
    /// Only used when `wire_api = "anthropic_messages"`.
    pub upstream_url: Option<String>,
    pub approval_policy: Option<String>,
    pub sandbox_mode: Option<String>,
    pub permission_profile: Option<serde_json::Value>,
    pub service_tier: Option<serde_json::Value>,
    pub persist_extended_history: bool,
    pub ephemeral: Option<bool>,
    pub memories_enabled: Option<bool>,
    pub thread_store_endpoint: Option<String>,
    pub config_overrides: HashMap<String, String>,
    #[serde(skip)]
    pub cli_overrides: Vec<(String, TomlValue)>,
    pub mcp_servers: HashMap<String, serde_json::Value>,
    pub enable_codex_api_key_env: bool,
    pub client_name: Option<String>,
    pub client_version: Option<String>,
    pub exec_server_url: Option<String>,
    pub channel_capacity: Option<usize>,
    #[serde(default = "default_true")]
    pub feedback_capture_enabled: bool,
}

fn default_true() -> bool {
    true
}

impl Default for CodexAdapterOptions {
    fn default() -> Self {
        Self {
            cwd: Default::default(),
            model: Default::default(),
            model_provider: Default::default(),
            api_key: Default::default(),
            base_url: Default::default(),
            wire_api: Default::default(),
            upstream_url: Default::default(),
            approval_policy: Default::default(),
            sandbox_mode: Default::default(),
            permission_profile: Default::default(),
            service_tier: Default::default(),
            persist_extended_history: Default::default(),
            ephemeral: Default::default(),
            memories_enabled: Default::default(),
            thread_store_endpoint: Default::default(),
            config_overrides: Default::default(),
            cli_overrides: Default::default(),
            mcp_servers: Default::default(),
            enable_codex_api_key_env: Default::default(),
            client_name: Default::default(),
            client_version: Default::default(),
            exec_server_url: Default::default(),
            channel_capacity: Default::default(),
            feedback_capture_enabled: default_true(),
        }
    }
}

/// Resolve the Codex home used by Remote Code's embedded Codex runtime.
///
/// This intentionally does not use Codex's default `~/.codex` directory. It is
/// shared by the GUI process entry point and the adapter so helper binaries,
/// `.env`, thread stores, config, memories, and runtime state all live under the
/// same app-scoped directory.
pub fn isolated_codex_home() -> anyhow::Result<PathBuf> {
    let project_dirs = directories::ProjectDirs::from(
        REMOTE_CODE_PROJECT_QUALIFIER,
        REMOTE_CODE_PROJECT_ORGANIZATION,
        REMOTE_CODE_PROJECT_APPLICATION,
    )
    .ok_or_else(|| anyhow::anyhow!("Cannot determine OS data directory"))?;
    let codex_home = project_dirs.data_dir().join("codex");
    std::fs::create_dir_all(&codex_home)
        .with_context(|| format!("Failed to create isolated codex home at {:?}", codex_home))?;
    Ok(codex_home)
}

pub(super) fn build_harness_overrides(
    options: &CodexAdapterOptions,
    cwd: &std::path::Path,
) -> anyhow::Result<codex_core::config::ConfigOverrides> {
    Ok(codex_core::config::ConfigOverrides {
        model: options.model.clone(),
        cwd: Some(cwd.to_path_buf()),
        approval_policy: options
            .approval_policy
            .as_deref()
            .map(parse_approval_policy_core)
            .transpose()?,
        sandbox_mode: options
            .sandbox_mode
            .as_deref()
            .map(parse_sandbox_mode_core)
            .transpose()?,
        permission_profile: options
            .permission_profile
            .clone()
            .map(serde_json::from_value::<CorePermissionProfile>)
            .transpose()
            .context("invalid Codex permission profile")?
            .map(CorePermissionProfile::from),
        model_provider: options.model_provider.clone(),
        service_tier: options
            .service_tier
            .clone()
            .map(serde_json::from_value)
            .transpose()
            .context("invalid Codex service tier")?,
        ephemeral: options.ephemeral,
        ..Default::default()
    })
}

pub(super) fn build_cli_overrides(
    options: &CodexAdapterOptions,
) -> anyhow::Result<Vec<(String, TomlValue)>> {
    let mut overrides = Vec::new();

    if let Some(endpoint) = trim_opt(options.thread_store_endpoint.clone()) {
        overrides.push((
            "experimental_thread_store".to_string(),
            toml::from_str::<TomlValue>(&format!(
                "{{ type = \"remote\", endpoint = {} }}",
                toml_string(&endpoint)
            ))
            .context("failed to build thread store override")?,
        ));
    } else {
        overrides.push((
            "experimental_thread_store".to_string(),
            toml::from_str::<TomlValue>("{ type = \"local\" }")
                .context("failed to build local thread store override")?,
        ));
    }

    if let Some(enabled) = options.memories_enabled {
        overrides.push((
            "memories.generate_memories".to_string(),
            TomlValue::Boolean(enabled),
        ));
        overrides.push((
            "memories.use_memories".to_string(),
            TomlValue::Boolean(enabled),
        ));
    }

    if let Some(provider_id) = trim_opt(options.model_provider.clone()) {
        let provider_prefix = format!("model_providers.{provider_id}");
        overrides.push((
            "model_provider".to_string(),
            TomlValue::String(provider_id.clone()),
        ));
        overrides.push((
            format!("{provider_prefix}.name"),
            TomlValue::String(provider_id.clone()),
        ));
        let wire_api_value = options
            .wire_api
            .as_deref()
            .unwrap_or("responses")
            .to_string();
        overrides.push((
            format!("{provider_prefix}.wire_api"),
            TomlValue::String(wire_api_value),
        ));
        if let Some(base_url) = trim_opt(options.base_url.clone()) {
            overrides.push((
                format!("{provider_prefix}.base_url"),
                TomlValue::String(base_url),
            ));
        }
        if let Some(api_key) = trim_opt(options.api_key.clone()) {
            overrides.push((
                format!("{provider_prefix}.experimental_bearer_token"),
                TomlValue::String(api_key),
            ));
        }
    } else if let Some(base_url) = trim_opt(options.base_url.clone()) {
        overrides.push(("openai_base_url".to_string(), TomlValue::String(base_url)));
    }

    for (name, value) in &options.mcp_servers {
        overrides.push((format!("mcp_servers.{name}"), json_to_toml(value.clone())?));
    }

    for (key, raw) in &options.config_overrides {
        // Security: validate config override keys to prevent TOML path injection.
        // Keys must consist of alphanumeric segments separated by dots.
        let valid = key.split('.').all(|segment| {
            !segment.is_empty()
                && segment
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        });
        if !valid {
            anyhow::bail!(
                "invalid config override key `{key}`: \
                 must be dot-separated alphanumeric segments"
            );
        }
        overrides.push((key.clone(), parse_toml_scalar(raw)));
    }

    Ok(overrides)
}

pub(super) fn build_thread_config_overrides_json(
    options: &CodexAdapterOptions,
) -> HashMap<String, serde_json::Value> {
    let mut config = HashMap::new();
    if let Some(enabled) = options.memories_enabled {
        config.insert(
            "memories".to_string(),
            serde_json::json!({
                "generate_memories": enabled,
                "use_memories": enabled,
            }),
        );
    }
    config
}

pub(super) fn parse_approval_policy(value: &str) -> anyhow::Result<AskForApproval> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "untrusted" | "unless-trusted" | "unless_trusted" => Ok(AskForApproval::UnlessTrusted),
        "on-failure" | "on_failure" | "onfailure" => Ok(AskForApproval::OnFailure),
        "on-request" | "on_request" | "onrequest" => Ok(AskForApproval::OnRequest),
        "never" => Ok(AskForApproval::Never),
        other => Err(anyhow::anyhow!(
            "unsupported Codex approval policy `{other}`"
        )),
    }
}

fn parse_approval_policy_core(
    value: &str,
) -> anyhow::Result<codex_protocol::protocol::AskForApproval> {
    Ok(parse_approval_policy(value)?.to_core())
}

pub(super) fn parse_sandbox_mode(
    value: &str,
) -> anyhow::Result<codex_app_server_protocol::SandboxMode> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "read-only" | "readonly" | "read_only" => {
            Ok(codex_app_server_protocol::SandboxMode::ReadOnly)
        }
        "workspace-write" | "workspace_write" | "workspacewrite" => {
            Ok(codex_app_server_protocol::SandboxMode::WorkspaceWrite)
        }
        "danger-full-access" | "danger_full_access" | "dangerfullaccess" | "none" => {
            Ok(codex_app_server_protocol::SandboxMode::DangerFullAccess)
        }
        other => Err(anyhow::anyhow!("unsupported Codex sandbox mode `{other}`")),
    }
}

fn parse_sandbox_mode_core(
    value: &str,
) -> anyhow::Result<codex_protocol::config_types::SandboxMode> {
    Ok(parse_sandbox_mode(value)?.to_core())
}

pub(super) fn trim_opt(value: Option<String>) -> Option<String> {
    value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
}

pub(super) fn parse_toml_scalar(raw: &str) -> TomlValue {
    // Security: wrap the raw value as a quoted TOML string to prevent TOML
    // injection. The previous approach of `format!("_x_ = {raw}")` allowed a
    // crafted value like `x = 1\n_y_ = 2` to inject extra TOML keys/values.
    let quoted = toml::to_string(&raw).unwrap_or_else(|_| {
        // Fallback: escape any embedded quotes and newlines.
        let escaped = raw
            .replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r");
        format!("\"{escaped}\"")
    });
    let wrapped = format!("_x_ = {quoted}");
    toml::from_str::<toml::Table>(&wrapped)
        .ok()
        .and_then(|table| table.get("_x_").cloned())
        .unwrap_or_else(|| TomlValue::String(raw.trim_matches(['"', '\'']).to_string()))
}

fn toml_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

pub(super) fn json_to_toml(value: serde_json::Value) -> anyhow::Result<TomlValue> {
    match value {
        serde_json::Value::Null => Ok(TomlValue::String(String::new())),
        serde_json::Value::Bool(v) => Ok(TomlValue::Boolean(v)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(TomlValue::Integer(i))
            } else if let Some(f) = n.as_f64() {
                Ok(TomlValue::Float(f))
            } else {
                Err(anyhow::anyhow!(
                    "unsupported JSON number for TOML conversion"
                ))
            }
        }
        serde_json::Value::String(v) => Ok(TomlValue::String(v)),
        serde_json::Value::Array(values) => values
            .into_iter()
            .map(json_to_toml)
            .collect::<anyhow::Result<Vec<_>>>()
            .map(TomlValue::Array),
        serde_json::Value::Object(values) => {
            let mut table = toml::map::Map::new();
            for (key, value) in values {
                if !value.is_null() {
                    table.insert(key, json_to_toml(value)?);
                }
            }
            Ok(TomlValue::Table(table))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // -- parse_approval_policy tests --

    #[test]
    fn parse_approval_policy_untrusted_aliases() {
        assert_eq!(
            parse_approval_policy("untrusted").unwrap(),
            AskForApproval::UnlessTrusted
        );
        assert_eq!(
            parse_approval_policy("unless-trusted").unwrap(),
            AskForApproval::UnlessTrusted
        );
        assert_eq!(
            parse_approval_policy("unless_trusted").unwrap(),
            AskForApproval::UnlessTrusted
        );
    }

    #[test]
    fn parse_approval_policy_on_failure_aliases() {
        assert_eq!(
            parse_approval_policy("on-failure").unwrap(),
            AskForApproval::OnFailure
        );
        assert_eq!(
            parse_approval_policy("on_failure").unwrap(),
            AskForApproval::OnFailure
        );
    }

    #[test]
    fn parse_approval_policy_on_request_aliases() {
        assert_eq!(
            parse_approval_policy("on-request").unwrap(),
            AskForApproval::OnRequest
        );
    }

    #[test]
    fn parse_approval_policy_never() {
        assert_eq!(
            parse_approval_policy("never").unwrap(),
            AskForApproval::Never
        );
    }

    #[test]
    fn parse_approval_policy_unsupported_returns_error() {
        assert!(parse_approval_policy("always").is_err());
        assert!(parse_approval_policy("").is_err());
    }

    #[test]
    fn parse_approval_policy_trims_whitespace() {
        assert_eq!(
            parse_approval_policy("  untrusted  ").unwrap(),
            AskForApproval::UnlessTrusted
        );
    }

    // -- parse_sandbox_mode tests --

    #[test]
    fn parse_sandbox_mode_read_only_aliases() {
        assert_eq!(
            parse_sandbox_mode("read-only").unwrap(),
            codex_app_server_protocol::SandboxMode::ReadOnly
        );
        assert_eq!(
            parse_sandbox_mode("readonly").unwrap(),
            codex_app_server_protocol::SandboxMode::ReadOnly
        );
    }

    #[test]
    fn parse_sandbox_mode_workspace_write_aliases() {
        assert_eq!(
            parse_sandbox_mode("workspace-write").unwrap(),
            codex_app_server_protocol::SandboxMode::WorkspaceWrite
        );
    }

    #[test]
    fn parse_sandbox_mode_danger_full_access_aliases() {
        assert_eq!(
            parse_sandbox_mode("danger-full-access").unwrap(),
            codex_app_server_protocol::SandboxMode::DangerFullAccess
        );
        assert_eq!(
            parse_sandbox_mode("none").unwrap(),
            codex_app_server_protocol::SandboxMode::DangerFullAccess
        );
    }

    #[test]
    fn parse_sandbox_mode_unsupported_returns_error() {
        assert!(parse_sandbox_mode("jail").is_err());
    }

    // -- trim_opt tests --

    #[test]
    fn trim_opt_non_empty_after_trim() {
        assert_eq!(trim_opt(Some("  hello  ".into())), Some("hello".into()));
    }

    #[test]
    fn trim_opt_empty_after_trim_returns_none() {
        assert_eq!(trim_opt(Some("   ".into())), None);
    }

    #[test]
    fn trim_opt_empty_string_returns_none() {
        assert_eq!(trim_opt(Some("".into())), None);
    }

    #[test]
    fn trim_opt_none_returns_none() {
        assert_eq!(trim_opt(None), None);
    }

    // -- json_to_toml tests --

    #[test]
    fn json_to_toml_primitives() {
        assert_eq!(json_to_toml(json!(42)).unwrap(), TomlValue::Integer(42));
        assert_eq!(json_to_toml(json!(true)).unwrap(), TomlValue::Boolean(true));
        assert_eq!(
            json_to_toml(json!("hello")).unwrap(),
            TomlValue::String("hello".into())
        );
        assert_eq!(json_to_toml(json!(3.14)).unwrap(), TomlValue::Float(3.14));
    }

    #[test]
    fn json_to_toml_null_becomes_empty_string() {
        assert_eq!(
            json_to_toml(json!(null)).unwrap(),
            TomlValue::String(String::new())
        );
    }

    #[test]
    fn json_to_toml_array() {
        let result = json_to_toml(json!([1, 2, 3])).unwrap();
        assert!(matches!(result, TomlValue::Array(arr) if arr.len() == 3));
    }

    #[test]
    fn json_to_toml_object_skips_null_values() {
        let result = json_to_toml(json!({"a": 1, "b": null, "c": "x"})).unwrap();
        match result {
            TomlValue::Table(table) => {
                assert_eq!(table.len(), 2);
                assert!(table.contains_key("a"));
                assert!(table.contains_key("c"));
                assert!(!table.contains_key("b"));
            }
            other => panic!("expected table, got {other:?}"),
        }
    }

    // -- parse_toml_scalar tests --

    #[test]
    fn parse_toml_scalar_simple_string() {
        let result = parse_toml_scalar("hello");
        assert!(matches!(result, TomlValue::String(s) if s == "hello"));
    }

    #[test]
    fn parse_toml_scalar_toml_injection_prevention() {
        // Attempt to inject an extra key via newline
        let result = parse_toml_scalar("x = 1\n_y_ = 2");
        // Should be treated as a simple string, not parsed as TOML
        assert!(matches!(result, TomlValue::String(_)));
    }

    // -- CodexAdapterOptions serde tests --

    #[test]
    fn codex_adapter_options_serde_roundtrip() {
        let opts = CodexAdapterOptions {
            cwd: "/tmp".into(),
            model: Some("gpt-4".into()),
            approval_policy: Some("on-failure".into()),
            ..Default::default()
        };
        let json = serde_json::to_string(&opts).unwrap();
        let back: CodexAdapterOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(back.cwd, PathBuf::from("/tmp"));
        assert_eq!(back.model.as_deref(), Some("gpt-4"));
        assert_eq!(back.approval_policy.as_deref(), Some("on-failure"));
        assert!(back.feedback_capture_enabled);
    }

    #[test]
    fn codex_adapter_options_defaults() {
        let opts = CodexAdapterOptions::default();
        assert!(opts.model.is_none());
        assert!(opts.api_key.is_none());
        assert!(opts.config_overrides.is_empty());
        assert!(opts.feedback_capture_enabled);
    }

    #[test]
    fn codex_adapter_options_serde_default_matches_programmatic_default() {
        // Serialize minimal JSON (no feedback_capture_enabled field) and verify
        // serde default matches programmatic default.
        let json = r#"{"cwd":"/tmp"}"#;
        let from_json: CodexAdapterOptions = serde_json::from_str(json).unwrap();
        let from_default = CodexAdapterOptions::default();
        assert_eq!(
            from_json.feedback_capture_enabled, from_default.feedback_capture_enabled,
            "serde default and programmatic default must agree"
        );
    }
}

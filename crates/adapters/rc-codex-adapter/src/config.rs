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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CodexAdapterOptions {
    pub cwd: PathBuf,
    pub model: Option<String>,
    pub model_provider: Option<String>,
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
            .map(serde_json::from_value::<codex_app_server_protocol::PermissionProfile>)
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

pub(super) fn parse_sandbox_mode(value: &str) -> anyhow::Result<codex_app_server_protocol::SandboxMode> {
    let normalized = value.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "read-only" | "readonly" | "read_only" => Ok(codex_app_server_protocol::SandboxMode::ReadOnly),
        "workspace-write" | "workspace_write" | "workspacewrite" => Ok(codex_app_server_protocol::SandboxMode::WorkspaceWrite),
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

fn parse_toml_scalar(raw: &str) -> TomlValue {
    let wrapped = format!("_x_ = {raw}");
    toml::from_str::<toml::Table>(&wrapped)
        .ok()
        .and_then(|table| table.get("_x_").cloned())
        .unwrap_or_else(|| TomlValue::String(raw.trim_matches(['"', '\'']).to_string()))
}

fn toml_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn json_to_toml(value: serde_json::Value) -> anyhow::Result<TomlValue> {
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
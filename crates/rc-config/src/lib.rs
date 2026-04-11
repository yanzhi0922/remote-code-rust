use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use directories::BaseDirs;
use rc_core::{
    DEFAULT_PROFILE_DIR_NAME, HookEvent, HookMatcher, InputFormat, LEGACY_PROFILE_DIR_NAME,
    OutputFormat, PermissionMode, ProviderProtocol,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const RUNTIME_VERSION: &str = env!("CARGO_PKG_VERSION");

const RESERVED_PROVIDER_HEADER_NAMES: &[&str] = &[
    "accept",
    "anthropic-beta",
    "anthropic-version",
    "authorization",
    "content-length",
    "content-type",
    "host",
    "user-agent",
    "x-api-key",
    "x-app",
    "x-anthropic-additional-protection",
    "x-claude-code-session-id",
    "x-claude-remote-container-id",
    "x-claude-remote-session-id",
    "x-client-app",
];

const fn default_provider_max_retries() -> u32 {
    2
}

const fn default_provider_retry_initial_backoff_ms() -> u64 {
    500
}

const fn default_provider_retry_max_backoff_ms() -> u64 {
    5_000
}

const fn default_provider_respect_retry_after() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppPaths {
    pub profile_dir: PathBuf,
    pub state_db_path: PathBuf,
    pub sessions_dir: PathBuf,
    pub artifacts_dir: PathBuf,
    pub logs_dir: PathBuf,
    pub profiles_dir: PathBuf,
    pub skills_dir: PathBuf,
    pub plugins_dir: PathBuf,
}

impl AppPaths {
    /// # Errors
    /// Returns an error if the user home directory cannot be located.
    pub fn discover(profile_override: Option<PathBuf>) -> Result<Self> {
        let profile_dir = if let Some(path) = profile_override {
            path
        } else {
            let base_dirs = BaseDirs::new()
                .ok_or_else(|| anyhow!("failed to locate the user home directory"))?;
            base_dirs.home_dir().join(DEFAULT_PROFILE_DIR_NAME)
        };

        Ok(Self {
            state_db_path: profile_dir.join("state.db"),
            sessions_dir: profile_dir.join("sessions"),
            artifacts_dir: profile_dir.join("artifacts"),
            logs_dir: profile_dir.join("logs"),
            profiles_dir: profile_dir.join("profiles"),
            skills_dir: profile_dir.join("skills"),
            plugins_dir: profile_dir.join("plugins"),
            profile_dir,
        })
    }

    /// # Errors
    /// Returns an error if any directory cannot be created.
    pub fn ensure_exists(&self) -> Result<()> {
        for directory in [
            &self.profile_dir,
            &self.sessions_dir,
            &self.artifacts_dir,
            &self.logs_dir,
            &self.profiles_dir,
            &self.skills_dir,
            &self.plugins_dir,
        ] {
            fs::create_dir_all(directory)
                .with_context(|| format!("failed to create {}", directory.display()))?;
        }
        Ok(())
    }

    /// # Errors
    /// Returns an error if the user home directory cannot be located.
    pub fn legacy_profile_dir() -> Result<PathBuf> {
        let base_dirs =
            BaseDirs::new().ok_or_else(|| anyhow!("failed to locate the user home directory"))?;
        Ok(base_dirs.home_dir().join(LEGACY_PROFILE_DIR_NAME))
    }
}

#[derive(Debug, Clone, Default)]
pub struct ProviderOverrides {
    pub provider: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub protocol: Option<ProviderProtocol>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub protocol: ProviderProtocol,
    pub timeout_ms: u64,
    pub max_output_tokens: u32,
    #[serde(default = "default_provider_max_retries")]
    pub max_retries: u32,
    #[serde(default = "default_provider_retry_initial_backoff_ms")]
    pub retry_initial_backoff_ms: u64,
    #[serde(default = "default_provider_retry_max_backoff_ms")]
    pub retry_max_backoff_ms: u64,
    #[serde(default = "default_provider_respect_retry_after")]
    pub respect_retry_after: bool,
    #[serde(default)]
    pub request_header_overrides: BTreeMap<String, String>,
}

/// Configuration for provider failover / load-balancing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverConfig {
    /// Ordered list of provider configurations to try.
    pub providers: Vec<ProviderConfig>,
    /// Maximum number of providers to attempt before giving up.
    #[serde(default = "default_failover_max_attempts")]
    pub max_failover_attempts: usize,
    /// HTTP status codes that should trigger a failover to the next provider.
    #[serde(default)]
    pub failover_on_status: Vec<u16>,
    /// Whether a timeout error should trigger failover.
    #[serde(default = "default_true")]
    pub failover_on_timeout: bool,
}

const fn default_failover_max_attempts() -> usize {
    3
}

const fn default_true() -> bool {
    true
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub cwd: PathBuf,
    pub session_id: Uuid,
    pub permission_mode: PermissionMode,
    pub input_format: InputFormat,
    pub output_format: OutputFormat,
    pub print_mode: bool,
    pub verbose: bool,
    pub replay_user_messages: bool,
    pub include_partial_messages: bool,
    pub max_turns: usize,
    pub provider: ProviderConfig,
    pub paths: AppPaths,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DoctorReport {
    pub ok: bool,
    pub issues: Vec<String>,
}

#[allow(clippy::fn_params_excessive_bools)]
#[allow(clippy::too_many_arguments)]
/// # Errors
/// Returns an error if configuration loading fails.
pub fn load_runtime_config(
    cwd_override: Option<PathBuf>,
    profile_dir_override: Option<PathBuf>,
    session_id_override: Option<Uuid>,
    permission_mode: PermissionMode,
    input_format: InputFormat,
    output_format: OutputFormat,
    print_mode: bool,
    verbose: bool,
    replay_user_messages: bool,
    include_partial_messages: bool,
    max_turns: usize,
    overrides: ProviderOverrides,
) -> Result<RuntimeConfig> {
    let cwd = match cwd_override {
        Some(cwd) => cwd,
        None => env::current_dir().context("failed to discover the current working directory")?,
    };
    let paths = AppPaths::discover(profile_dir_override)?;
    paths.ensure_exists()?;

    let provider = load_provider_config(overrides, session_id_override)?;
    Ok(RuntimeConfig {
        cwd,
        session_id: session_id_override.unwrap_or_else(Uuid::new_v4),
        permission_mode,
        input_format,
        output_format,
        print_mode,
        verbose,
        replay_user_messages,
        include_partial_messages,
        max_turns: max_turns.max(1),
        provider,
        paths,
    })
}

/// # Errors
/// Returns an error if provider configuration is invalid.
pub fn load_provider_config(
    overrides: ProviderOverrides,
    session_id: Option<Uuid>,
) -> Result<ProviderConfig> {
    let provider_name = overrides
        .provider
        .or_else(|| read_env_first(&["REMOTE_CODE_PROVIDER"]))
        .unwrap_or_else(|| "custom".to_owned());
    let base_url = overrides.base_url.or_else(|| {
        read_env_first(&[
            "REMOTE_CODE_BASE_URL",
            "OPENAI_BASE_URL",
            "ANTHROPIC_BASE_URL",
        ])
    });
    let explicit_protocol = overrides.protocol.or_else(|| {
        read_env_first(&["REMOTE_CODE_PROTOCOL", "REMOTE_CODE_PROVIDER_PROTOCOL"]).and_then(|raw| {
            match raw.to_ascii_lowercase().as_str() {
                "openai" => Some(ProviderProtocol::OpenAi),
                "anthropic" => Some(ProviderProtocol::Anthropic),
                "bedrock" => Some(ProviderProtocol::Bedrock),
                "vertex" => Some(ProviderProtocol::Vertex),
                _ => None,
            }
        })
    });
    let protocol = normalize_protocol(base_url.as_deref(), explicit_protocol);
    let normalized_base_url = normalize_base_url(base_url, protocol);
    let timeout_ms = read_env_first(&["REMOTE_CODE_API_TIMEOUT_MS", "API_TIMEOUT_MS"])
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(600_000)
        .max(1_000);
    let max_output_tokens = read_env_first(&["REMOTE_CODE_MAX_OUTPUT_TOKENS"])
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(4_096)
        .max(256);
    let max_retries = read_env_first(&["REMOTE_CODE_PROVIDER_MAX_RETRIES"])
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(default_provider_max_retries());
    let retry_initial_backoff_ms =
        read_env_first(&["REMOTE_CODE_PROVIDER_RETRY_INITIAL_BACKOFF_MS"])
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(default_provider_retry_initial_backoff_ms())
            .max(50);
    let retry_max_backoff_ms = read_env_first(&["REMOTE_CODE_PROVIDER_RETRY_MAX_BACKOFF_MS"])
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(default_provider_retry_max_backoff_ms())
        .max(retry_initial_backoff_ms);
    let respect_retry_after = read_env_first(&["REMOTE_CODE_PROVIDER_RESPECT_RETRY_AFTER"])
        .map_or(default_provider_respect_retry_after(), |value| {
            !matches!(value.to_ascii_lowercase().as_str(), "0" | "false" | "no")
        });
    let request_header_overrides = build_request_header_overrides(session_id);

    Ok(ProviderConfig {
        name: provider_name,
        base_url: normalized_base_url,
        api_key: overrides.api_key.or_else(|| {
            read_env_first(&["REMOTE_CODE_API_KEY", "OPENAI_API_KEY", "ANTHROPIC_API_KEY"])
        }),
        model: overrides
            .model
            .or_else(|| read_env_first(&["REMOTE_CODE_MODEL", "OPENAI_MODEL", "ANTHROPIC_MODEL"])),
        protocol,
        timeout_ms,
        max_output_tokens,
        max_retries,
        retry_initial_backoff_ms,
        retry_max_backoff_ms,
        respect_retry_after,
        request_header_overrides,
    })
}

#[must_use]
pub fn validate_provider_config(provider: &ProviderConfig) -> DoctorReport {
    let mut issues = Vec::new();
    if provider.base_url.is_none() {
        issues.push("Missing REMOTE_CODE_BASE_URL (or provider-compatible base URL).".to_owned());
    }
    if provider.model.is_none() {
        issues.push("Missing REMOTE_CODE_MODEL.".to_owned());
    }
    if provider.api_key.is_none() && provider.name != "mock" {
        issues.push("Missing REMOTE_CODE_API_KEY.".to_owned());
    }
    DoctorReport {
        ok: issues.is_empty(),
        issues,
    }
}

#[must_use]
pub fn normalize_protocol(
    base_url: Option<&str>,
    explicit_protocol: Option<ProviderProtocol>,
) -> ProviderProtocol {
    if let Some(protocol) = explicit_protocol {
        return protocol;
    }
    let Some(base_url) = base_url else {
        return ProviderProtocol::OpenAi;
    };
    let normalized = base_url.to_ascii_lowercase();
    if normalized.ends_with("/messages")
        || normalized.contains("/anthropic")
        || normalized.contains("compat=anthropic")
    {
        ProviderProtocol::Anthropic
    } else {
        ProviderProtocol::OpenAi
    }
}

#[must_use]
pub fn normalize_base_url(base_url: Option<String>, protocol: ProviderProtocol) -> Option<String> {
    let raw = base_url?;
    let trimmed = raw.trim().trim_end_matches('/').to_owned();
    let normalized = match protocol {
        ProviderProtocol::Anthropic => {
            if trimmed.ends_with("/messages") {
                trimmed
            } else if trimmed.rsplit('/').next().is_some_and(|segment| {
                segment.starts_with('v') && segment[1..].chars().all(|ch| ch.is_ascii_digit())
            }) {
                format!("{trimmed}/messages")
            } else {
                format!("{trimmed}/v1/messages")
            }
        }
        ProviderProtocol::OpenAi => {
            if trimmed.ends_with("/chat/completions") {
                trimmed
            } else {
                format!("{trimmed}/chat/completions")
            }
        }
        // Bedrock and Vertex use OpenAI-compatible endpoints for now.
        ProviderProtocol::Bedrock | ProviderProtocol::Vertex => trimmed,
    };
    Some(normalized)
}

fn build_request_header_overrides(session_id: Option<Uuid>) -> BTreeMap<String, String> {
    let mut merged = BTreeMap::new();
    if let Some(raw) = read_env_first(&["ANTHROPIC_CUSTOM_HEADERS"]) {
        for line in raw.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let Some((name, value)) = trimmed.split_once(':') else {
                continue;
            };
            let name = name.trim();
            let value = value.trim();
            if !name.is_empty() && !value.is_empty() {
                merged.insert(name.to_owned(), value.to_owned());
            }
        }
    }

    if let Some(raw) = read_env_first(&["REMOTE_CODE_REQUEST_HEADERS_JSON"])
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw)
        && let Some(object) = value.as_object()
    {
        for (name, raw_value) in object {
            let normalized = match raw_value {
                serde_json::Value::String(value) => Some(value.trim().to_owned()),
                serde_json::Value::Number(value) => Some(value.to_string()),
                serde_json::Value::Bool(value) => Some(value.to_string()),
                _ => None,
            };
            if let Some(value) = normalized
                && !value.is_empty()
            {
                merged.insert(name.trim().to_owned(), value);
            }
        }
    }

    let session = session_id
        .map(|value| value.to_string())
        .unwrap_or_default();
    let mut filtered = BTreeMap::new();
    for (name, value) in merged {
        if RESERVED_PROVIDER_HEADER_NAMES
            .iter()
            .any(|reserved| reserved.eq_ignore_ascii_case(name.as_str()))
        {
            continue;
        }
        let resolved = value
            .replace("${REMOTE_CODE_SESSION_ID}", &session)
            .replace("${REMOTE_CODE_VERSION}", RUNTIME_VERSION);
        filtered.insert(name, resolved);
    }
    filtered
}

fn read_env_first(keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        env::var(key).ok().and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_owned())
            }
        })
    })
}

/// Discover provider configurations from well-known environment variables.
///
/// Checks for the following keys and creates a [`ProviderConfig`] for each one
/// that is present:
///
/// | Env var              | Provider name | Protocol  | Base URL                                              | Model          |
/// |----------------------|---------------|-----------|-------------------------------------------------------|----------------|
/// | `GLM_API_KEY`        | `glm`         | `openai`  | `https://open.bigmodel.cn/api/paas/v4`                | `glm-4-plus`   |
/// | `ANTHROPIC_API_KEY`  | `anthropic`   | `anthropic`| *(default)*                                           | *(default)*    |
/// | `OPENAI_API_KEY`     | `openai`      | `openai`  | *(default)*                                           | *(default)*    |
///
/// The returned list only contains entries for keys that are actually set.
/// This function is intended to be called **before** the main
/// [`load_provider_config`] so that the discovered providers can be merged or
/// offered as fallbacks.
pub fn discover_env_providers() -> Vec<ProviderConfig> {
    let mut providers = Vec::new();

    // GLM / ZhipuAI — OpenAI-compatible endpoint
    if let Some(api_key) = read_env_first(&["GLM_API_KEY"]) {
        let base_url = normalize_base_url(
            Some("https://open.bigmodel.cn/api/paas/v4".to_owned()),
            ProviderProtocol::OpenAi,
        );
        providers.push(ProviderConfig {
            name: "glm".to_owned(),
            base_url,
            api_key: Some(api_key),
            model: Some("glm-4-plus".to_owned()),
            protocol: ProviderProtocol::OpenAi,
            timeout_ms: 600_000,
            max_output_tokens: 4_096,
            max_retries: default_provider_max_retries(),
            retry_initial_backoff_ms: default_provider_retry_initial_backoff_ms(),
            retry_max_backoff_ms: default_provider_retry_max_backoff_ms(),
            respect_retry_after: default_provider_respect_retry_after(),
            request_header_overrides: BTreeMap::new(),
        });
    }

    // Anthropic
    if let Some(api_key) = read_env_first(&["ANTHROPIC_API_KEY"]) {
        let base_url = normalize_base_url(
            read_env_first(&["ANTHROPIC_BASE_URL"]),
            ProviderProtocol::Anthropic,
        );
        providers.push(ProviderConfig {
            name: "anthropic".to_owned(),
            base_url,
            api_key: Some(api_key),
            model: read_env_first(&["ANTHROPIC_MODEL"]),
            protocol: ProviderProtocol::Anthropic,
            timeout_ms: 600_000,
            max_output_tokens: 4_096,
            max_retries: default_provider_max_retries(),
            retry_initial_backoff_ms: default_provider_retry_initial_backoff_ms(),
            retry_max_backoff_ms: default_provider_retry_max_backoff_ms(),
            respect_retry_after: default_provider_respect_retry_after(),
            request_header_overrides: BTreeMap::new(),
        });
    }

    // OpenAI
    if let Some(api_key) = read_env_first(&["OPENAI_API_KEY"]) {
        let base_url = normalize_base_url(
            read_env_first(&["OPENAI_BASE_URL"]),
            ProviderProtocol::OpenAi,
        );
        providers.push(ProviderConfig {
            name: "openai".to_owned(),
            base_url,
            api_key: Some(api_key),
            model: read_env_first(&["OPENAI_MODEL"]),
            protocol: ProviderProtocol::OpenAi,
            timeout_ms: 600_000,
            max_output_tokens: 4_096,
            max_retries: default_provider_max_retries(),
            retry_initial_backoff_ms: default_provider_retry_initial_backoff_ms(),
            retry_max_backoff_ms: default_provider_retry_max_backoff_ms(),
            respect_retry_after: default_provider_respect_retry_after(),
            request_header_overrides: BTreeMap::new(),
        });
    }

    // AWS Bedrock — discovered via AWS credentials
    if read_env_first(&["AWS_ACCESS_KEY_ID"]).is_some()
        && read_env_first(&["AWS_SECRET_ACCESS_KEY"]).is_some()
    {
        let region =
            read_env_first(&["AWS_REGION"]).unwrap_or_else(|| "us-east-1".to_owned());
        let base_url = Some(format!(
            "https://bedrock-runtime.{region}.amazonaws.com"
        ));
        providers.push(ProviderConfig {
            name: "bedrock".to_owned(),
            base_url,
            api_key: None, // Bedrock uses SigV4, not Bearer tokens
            model: read_env_first(&["BEDROCK_MODEL"]),
            protocol: ProviderProtocol::Bedrock,
            timeout_ms: 600_000,
            max_output_tokens: 4_096,
            max_retries: default_provider_max_retries(),
            retry_initial_backoff_ms: default_provider_retry_initial_backoff_ms(),
            retry_max_backoff_ms: default_provider_retry_max_backoff_ms(),
            respect_retry_after: default_provider_respect_retry_after(),
            request_header_overrides: BTreeMap::new(),
        });
    }

    // Google Vertex AI — discovered via GCP credentials or project env
    if read_env_first(&["GOOGLE_APPLICATION_CREDENTIALS"]).is_some()
        || read_env_first(&["VERTEX_PROJECT"]).is_some()
    {
        let project =
            read_env_first(&["VERTEX_PROJECT"]).unwrap_or_else(|| "default".to_owned());
        let region =
            read_env_first(&["VERTEX_REGION"]).unwrap_or_else(|| "us-central1".to_owned());
        let base_url = Some(format!(
            "https://{region}-aiplatform.googleapis.com/v1/projects/{project}/locations/{region}/endpoints/openapi"
        ));
        providers.push(ProviderConfig {
            name: "vertex".to_owned(),
            base_url,
            api_key: None, // Vertex uses OAuth2, not Bearer tokens
            model: read_env_first(&["VERTEX_MODEL"]),
            protocol: ProviderProtocol::Vertex,
            timeout_ms: 600_000,
            max_output_tokens: 4_096,
            max_retries: default_provider_max_retries(),
            retry_initial_backoff_ms: default_provider_retry_initial_backoff_ms(),
            retry_max_backoff_ms: default_provider_retry_max_backoff_ms(),
            respect_retry_after: default_provider_respect_retry_after(),
            request_header_overrides: BTreeMap::new(),
        });
    }

    providers
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyImportSummary {
    pub source_dir: PathBuf,
    pub destination_dir: PathBuf,
    pub copied_files: usize,
    pub skipped_files: usize,
    pub imported_paths: Vec<PathBuf>,
}

/// # Errors
/// Returns an error if the legacy profile cannot be imported.
pub fn import_legacy_profile(
    source_dir: Option<PathBuf>,
    destination: &AppPaths,
) -> Result<LegacyImportSummary> {
    let source_dir = match source_dir {
        Some(path) => path,
        None => AppPaths::legacy_profile_dir()?,
    };
    let destination_dir = destination.profiles_dir.join("legacy-import");
    fs::create_dir_all(&destination_dir)
        .with_context(|| format!("failed to create {}", destination_dir.display()))?;

    let mut copied_files = 0usize;
    let mut skipped_files = 0usize;
    let mut imported_paths = Vec::new();
    for relative in [
        Path::new("feature-flags.json"),
        Path::new("settings.json"),
        Path::new("history.json"),
        Path::new("history.ndjson"),
        Path::new("sessions"),
        Path::new("skills"),
        Path::new("plugins"),
    ] {
        let source_path = source_dir.join(relative);
        if !source_path.exists() {
            continue;
        }
        let target_path = destination_dir.join(relative);
        if source_path.is_dir() {
            copy_directory(
                &source_path,
                &target_path,
                &mut copied_files,
                &mut skipped_files,
            )?;
            imported_paths.push(target_path);
        } else {
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }
            if target_path.exists() {
                skipped_files += 1;
            } else {
                fs::copy(&source_path, &target_path)?;
                copied_files += 1;
                imported_paths.push(target_path);
            }
        }
    }

    Ok(LegacyImportSummary {
        source_dir,
        destination_dir,
        copied_files,
        skipped_files,
        imported_paths,
    })
}

/// # Errors
/// Returns an error if the hooks file cannot be read or parsed.
pub fn load_hooks_file(path: impl AsRef<Path>) -> Result<BTreeMap<HookEvent, Vec<HookMatcher>>> {
    let path = path.as_ref();
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&content)
        .with_context(|| format!("failed to parse hooks file {}", path.display()))
}

/// # Errors
/// Returns an error if the settings file cannot be read or parsed.
pub fn load_settings_hooks(
    path: impl AsRef<Path>,
) -> Result<BTreeMap<HookEvent, Vec<HookMatcher>>> {
    let path = path.as_ref();
    let content =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse settings file {}", path.display()))?;
    let Some(hooks) = value.get("hooks") else {
        return Ok(BTreeMap::new());
    };
    serde_json::from_value(hooks.clone())
        .with_context(|| format!("failed to decode hooks from settings {}", path.display()))
}

fn copy_directory(
    source: &Path,
    destination: &Path,
    copied_files: &mut usize,
    skipped_files: &mut usize,
) -> Result<()> {
    for entry in walkdir::WalkDir::new(source) {
        let entry = entry?;
        let Ok(relative) = entry.path().strip_prefix(source) else {
            continue;
        };
        let target = destination.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&target)?;
            continue;
        }
        if target.exists() {
            *skipped_files += 1;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(entry.path(), &target)?;
        *copied_files += 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;

    use super::{load_hooks_file, load_settings_hooks, normalize_base_url, normalize_protocol};
    use rc_core::HookEvent;
    use rc_core::ProviderProtocol;
    use tempfile::tempdir;

    #[test]
    fn anthropic_base_url_is_normalized() {
        let normalized = normalize_base_url(
            Some("https://example.com/anthropic".to_owned()),
            ProviderProtocol::Anthropic,
        );
        assert_eq!(
            normalized.as_deref(),
            Some("https://example.com/anthropic/v1/messages")
        );
    }

    #[test]
    fn openai_base_url_is_normalized() {
        let normalized = normalize_base_url(
            Some("https://example.com/v1".to_owned()),
            ProviderProtocol::OpenAi,
        );
        assert_eq!(
            normalized.as_deref(),
            Some("https://example.com/v1/chat/completions")
        );
    }

    #[test]
    fn protocol_is_detected_from_base_url() {
        let protocol = normalize_protocol(Some("https://example.com/anthropic"), None);
        assert_eq!(protocol, ProviderProtocol::Anthropic);
    }

    #[test]
    fn loads_hooks_file_using_upstream_shape() {
        let temp = tempdir().expect("tempdir should work");
        let path = temp.path().join("hooks.json");
        fs::write(
            &path,
            r#"{
                "SessionStart": [
                    {
                        "matcher": "startup",
                        "hooks": [{"type": "command", "command": "echo session"}]
                    }
                ]
            }"#,
        )
        .expect("write should work");

        let hooks = load_hooks_file(&path).expect("hooks file should load");
        assert_eq!(hooks.len(), 1);
        assert_eq!(
            hooks
                .get(&HookEvent::SessionStart)
                .expect("session start hook should exist")
                .len(),
            1
        );
    }

    #[test]
    fn loads_hooks_from_settings_file() {
        let temp = tempdir().expect("tempdir should work");
        let path = temp.path().join("settings.json");
        fs::write(
            &path,
            r#"{
                "model": "test-model",
                "hooks": {
                    "PreToolUse": [
                        {
                            "matcher": "Bash",
                            "hooks": [{"type": "command", "command": "echo before"}]
                        }
                    ]
                }
            }"#,
        )
        .expect("write should work");

        let hooks = load_settings_hooks(&path).expect("settings hooks should load");
        assert_eq!(hooks.len(), 1);
        assert_eq!(
            hooks
                .get(&HookEvent::PreToolUse)
                .expect("pre tool use hook should exist")[0]
                .matcher
                .as_deref(),
            Some("Bash")
        );
    }

    #[test]
    fn settings_without_hooks_returns_empty_map() {
        let temp = tempdir().expect("tempdir should work");
        let path = temp.path().join("settings.json");
        fs::write(&path, r#"{"model": "test-model"}"#).expect("write should work");

        let hooks = load_settings_hooks(&path).expect("settings hooks should load");
        assert_eq!(hooks, BTreeMap::new());
    }
}

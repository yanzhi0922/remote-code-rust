use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rc_core::ProviderProtocol;
use serde::{Deserialize, Serialize};

use crate::tool_filters::{merge_tool_filters, normalize_tool_filters};

/// Allowed startup setting sources, mirroring the upstream user/project/local split.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingSource {
    User,
    Project,
    Local,
}

impl SettingSource {
    #[must_use]
    pub const fn all() -> [Self; 3] {
        [Self::User, Self::Project, Self::Local]
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Project => "project",
            Self::Local => "local",
        }
    }
}

/// Runtime-only overrides layered on top of environment variables and settings files.
#[derive(Debug, Clone, Default)]
pub struct RuntimeOverrides {
    pub session_name: Option<String>,
    pub system_prompt: Option<String>,
    pub append_system_prompt: Option<String>,
    pub settings_files: Vec<PathBuf>,
    pub show_setting_sources: bool,
    pub allowed_setting_sources: Option<Vec<SettingSource>>,
    pub allowed_tools: Vec<String>,
    pub disallowed_tools: Vec<String>,
    pub effort: Option<String>,
    pub fallback_model: Option<String>,
    pub output_style: Option<String>,
    pub language: Option<String>,
    pub brief_enabled: Option<bool>,
    pub proactive_active: Option<bool>,
}

/// Settings materialized from one or more settings files.
#[derive(Debug, Clone, Default)]
pub struct ResolvedRuntimeSettings {
    pub provider_name: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub api_key_helper: Option<String>,
    pub model: Option<String>,
    pub protocol: Option<ProviderProtocol>,
    pub timeout_ms: Option<u64>,
    pub max_output_tokens: Option<u32>,
    pub thinking_budget: Option<u32>,
    pub fast_mode: Option<bool>,
    pub fast_mode_per_session_opt_in: Option<bool>,
    pub session_name: Option<String>,
    pub allowed_tools: Vec<String>,
    pub disallowed_tools: Vec<String>,
    pub effort: Option<String>,
    pub fallback_model: Option<String>,
    pub output_style: Option<String>,
    pub language: Option<String>,
    pub auto_memory_enabled: Option<bool>,
    pub auto_memory_directory: Option<String>,
    pub setting_sources: Vec<String>,
    pub auth_source: Option<String>,
}

/// A standard runtime settings source discovered from the local filesystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeSettingsSource {
    pub kind: &'static str,
    pub path: PathBuf,
}

#[derive(Debug, Default, Deserialize)]
struct SettingsDocument {
    #[serde(default)]
    provider: Option<SettingsProvider>,
    #[serde(default)]
    #[serde(alias = "apiKeyHelper")]
    api_key_helper: Option<String>,
    #[serde(default)]
    session_name: Option<String>,
    #[serde(default)]
    #[serde(alias = "fastMode")]
    fast_mode: Option<bool>,
    #[serde(default)]
    #[serde(alias = "fastModePerSessionOptIn")]
    fast_mode_per_session_opt_in: Option<bool>,
    #[serde(default)]
    allowed_tools: Option<Vec<String>>,
    #[serde(default)]
    disallowed_tools: Option<Vec<String>>,
    #[serde(default)]
    effort: Option<String>,
    #[serde(default)]
    fallback_model: Option<String>,
    #[serde(default)]
    output_style: Option<String>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    #[serde(alias = "autoMemoryEnabled")]
    auto_memory_enabled: Option<bool>,
    #[serde(default)]
    #[serde(alias = "autoMemoryDirectory")]
    auto_memory_directory: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct SettingsProvider {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    base_url: Option<String>,
    #[serde(default)]
    api_key: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    protocol: Option<ProviderProtocol>,
    #[serde(default)]
    timeout_ms: Option<u64>,
    #[serde(default)]
    max_output_tokens: Option<u32>,
    #[serde(default)]
    thinking_budget: Option<u32>,
}

/// Discover the standard runtime settings files in low-to-high precedence order.
#[must_use]
pub fn discover_runtime_settings_sources(
    cwd: &Path,
    profile_dir: &Path,
    profiles_dir: &Path,
    allowed_sources: &[SettingSource],
) -> Vec<RuntimeSettingsSource> {
    [
        (
            "legacy-import",
            profiles_dir.join("legacy-import").join("settings.json"),
        ),
        ("profile", profile_dir.join("settings.json")),
        ("project", cwd.join(".remote-code").join("settings.json")),
        (
            "local",
            cwd.join(".remote-code").join("settings.local.json"),
        ),
    ]
    .into_iter()
    .filter_map(|(kind, path)| {
        (path.exists() && is_runtime_settings_source_enabled(kind, allowed_sources))
            .then_some(RuntimeSettingsSource { kind, path })
    })
    .collect()
}

/// Resolve the effective runtime settings file list.
///
/// Explicit CLI files fully override auto-discovery. When no explicit files are
/// provided, the standard runtime settings sources are discovered in
/// low-to-high precedence order.
#[must_use]
pub fn resolve_runtime_settings_files(
    cwd: &Path,
    profile_dir: &Path,
    profiles_dir: &Path,
    explicit_files: &[PathBuf],
    allowed_sources: &[SettingSource],
) -> Vec<PathBuf> {
    if !explicit_files.is_empty() {
        let mut seen = BTreeSet::new();
        return explicit_files
            .iter()
            .filter_map(|path| {
                let normalized = path.clone();
                seen.insert(normalized.clone()).then_some(normalized)
            })
            .collect();
    }

    discover_runtime_settings_sources(cwd, profile_dir, profiles_dir, allowed_sources)
        .into_iter()
        .map(|source| source.path)
        .collect()
}

#[must_use]
pub fn is_setting_source_enabled(
    allowed_sources: &[SettingSource],
    candidate: SettingSource,
) -> bool {
    allowed_sources.contains(&candidate)
}

fn is_runtime_settings_source_enabled(kind: &str, allowed_sources: &[SettingSource]) -> bool {
    let source = match kind {
        "legacy-import" | "profile" => SettingSource::User,
        "project" => SettingSource::Project,
        "local" => SettingSource::Local,
        _ => return false,
    };
    is_setting_source_enabled(allowed_sources, source)
}

/// Load and merge runtime settings files from lowest priority to highest priority.
///
/// Later files override earlier scalar values while list-based tool filters are merged.
///
/// # Errors
/// Returns an error if any requested settings file cannot be read or parsed.
pub fn load_runtime_settings(paths: &[PathBuf]) -> Result<ResolvedRuntimeSettings> {
    let mut resolved = ResolvedRuntimeSettings::default();
    for path in paths {
        let document = load_settings_document(path)?;
        resolved
            .setting_sources
            .push(format!("settings:{}", path.display()));
        if let Some(provider) = document.provider {
            if let Some(name) = provider.name {
                resolved.provider_name = Some(name);
            }
            if let Some(base_url) = provider.base_url {
                resolved.base_url = Some(base_url);
            }
            if let Some(api_key) = provider.api_key {
                resolved.api_key = Some(api_key);
                resolved.auth_source = Some(format!("settings:{}", path.display()));
            }
            if let Some(model) = provider.model {
                resolved.model = Some(model);
            }
            if let Some(protocol) = provider.protocol {
                resolved.protocol = Some(protocol);
            }
            if let Some(timeout_ms) = provider.timeout_ms {
                resolved.timeout_ms = Some(timeout_ms);
            }
            if let Some(max_output_tokens) = provider.max_output_tokens {
                resolved.max_output_tokens = Some(max_output_tokens);
            }
            if let Some(thinking_budget) = provider.thinking_budget {
                resolved.thinking_budget = Some(thinking_budget);
            }
        }
        if let Some(session_name) = document.session_name {
            resolved.session_name = normalize_optional_string(Some(session_name));
        }
        if let Some(api_key_helper) = document.api_key_helper {
            resolved.api_key_helper = normalize_optional_string(Some(api_key_helper));
        }
        if let Some(fast_mode) = document.fast_mode {
            resolved.fast_mode = Some(fast_mode);
        }
        if let Some(fast_mode_per_session_opt_in) = document.fast_mode_per_session_opt_in {
            resolved.fast_mode_per_session_opt_in = Some(fast_mode_per_session_opt_in);
        }
        if let Some(allowed_tools) = document.allowed_tools {
            resolved.allowed_tools = merge_tool_filters(&resolved.allowed_tools, &allowed_tools);
        }
        if let Some(disallowed_tools) = document.disallowed_tools {
            resolved.disallowed_tools =
                merge_tool_filters(&resolved.disallowed_tools, &disallowed_tools);
        }
        if let Some(effort) = document.effort {
            resolved.effort = normalize_optional_string(Some(effort));
        }
        if let Some(fallback_model) = document.fallback_model {
            resolved.fallback_model = normalize_optional_string(Some(fallback_model));
        }
        if let Some(output_style) = document.output_style {
            resolved.output_style = normalize_optional_string(Some(output_style));
        }
        if let Some(language) = document.language {
            resolved.language = normalize_optional_string(Some(language));
        }
        if let Some(auto_memory_enabled) = document.auto_memory_enabled {
            resolved.auto_memory_enabled = Some(auto_memory_enabled);
        }
        if let Some(auto_memory_directory) = document.auto_memory_directory {
            resolved.auto_memory_directory = normalize_optional_string(Some(auto_memory_directory));
        }
    }
    resolved.allowed_tools = normalize_tool_filters(&resolved.allowed_tools);
    resolved.disallowed_tools = normalize_tool_filters(&resolved.disallowed_tools);
    Ok(resolved)
}

fn load_settings_document(path: &Path) -> Result<SettingsDocument> {
    let raw = fs::read_to_string(path)
        .with_context(|| format!("failed to read settings file {}", path.display()))?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if extension == "json" {
        return serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse JSON settings file {}", path.display()));
    }
    if extension == "toml" {
        return toml::from_str(&raw)
            .with_context(|| format!("failed to parse TOML settings file {}", path.display()));
    }

    toml::from_str(&raw)
        .or_else(|toml_error| {
            serde_json::from_str(&raw).map_err(|json_error| {
                anyhow::anyhow!(
                    "failed to parse settings file {} as TOML ({toml_error}) or JSON ({json_error})",
                    path.display()
                )
            })
        })
        .with_context(|| format!("failed to parse settings file {}", path.display()))
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_owned())
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{
        RuntimeOverrides, SettingSource, discover_runtime_settings_sources, load_runtime_settings,
        resolve_runtime_settings_files,
    };
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn runtime_overrides_default_is_empty() {
        let overrides = RuntimeOverrides::default();
        assert!(overrides.settings_files.is_empty());
        assert!(overrides.allowed_tools.is_empty());
        assert!(overrides.disallowed_tools.is_empty());
    }

    #[test]
    fn load_runtime_settings_merges_toml_files() {
        let tempdir = tempdir().expect("tempdir");
        let first = tempdir.path().join("first.toml");
        let second = tempdir.path().join("second.toml");
        fs::write(
            &first,
            r#"
session_name = "alpha"
allowed_tools = ["read_file"]
[provider]
name = "mock"
model = "gpt-4o-mini"
"#,
        )
        .expect("write first");
        fs::write(
            &second,
            r#"
disallowed_tools = ["bash_command"]
fallback_model = "gpt-4.1-mini"
[provider]
base_url = "https://example.com/v1"
"#,
        )
        .expect("write second");

        let resolved = load_runtime_settings(&[first, second]).expect("load settings");
        assert_eq!(resolved.session_name.as_deref(), Some("alpha"));
        assert_eq!(resolved.provider_name.as_deref(), Some("mock"));
        assert_eq!(resolved.model.as_deref(), Some("gpt-4o-mini"));
        assert_eq!(resolved.base_url.as_deref(), Some("https://example.com/v1"));
        assert_eq!(resolved.fallback_model.as_deref(), Some("gpt-4.1-mini"));
        assert_eq!(resolved.allowed_tools, vec!["read_file".to_owned()]);
        assert_eq!(resolved.disallowed_tools, vec!["bash_command".to_owned()]);
        assert_eq!(resolved.setting_sources.len(), 2);
    }

    #[test]
    fn load_runtime_settings_supports_json() {
        let tempdir = tempdir().expect("tempdir");
        let settings = tempdir.path().join("settings.json");
        fs::write(
            &settings,
            r#"{
  "session_name": "json session",
  "allowed_tools": ["read_file", "glob"],
  "provider": {
    "name": "json-provider",
    "api_key": "secret"
  }
}"#,
        )
        .expect("write settings");

        let resolved = load_runtime_settings(&[settings]).expect("load settings");
        assert_eq!(resolved.session_name.as_deref(), Some("json session"));
        assert_eq!(resolved.provider_name.as_deref(), Some("json-provider"));
        assert!(
            resolved
                .auth_source
                .as_deref()
                .is_some_and(|source| source.starts_with("settings:"))
        );
        assert!(resolved.allowed_tools.contains(&"glob".to_owned()));
    }

    #[test]
    fn load_runtime_settings_supports_auto_memory_enabled_alias() {
        let tempdir = tempdir().expect("tempdir");
        let settings = tempdir.path().join("settings.json");
        fs::write(&settings, r#"{ "autoMemoryEnabled": false }"#).expect("write settings");

        let resolved = load_runtime_settings(&[settings]).expect("load settings");
        assert_eq!(resolved.auto_memory_enabled, Some(false));
    }

    #[test]
    fn load_runtime_settings_supports_auto_memory_directory_alias() {
        let tempdir = tempdir().expect("tempdir");
        let settings = tempdir.path().join("settings.json");
        fs::write(
            &settings,
            r#"{ "autoMemoryDirectory": "C:/tmp/auto-memory" }"#,
        )
        .expect("write settings");

        let resolved = load_runtime_settings(&[settings]).expect("load settings");
        assert_eq!(
            resolved.auto_memory_directory.as_deref(),
            Some("C:/tmp/auto-memory")
        );
    }

    #[test]
    fn discover_runtime_settings_files_returns_explicit_files_verbatim() {
        let tempdir = tempdir().expect("tempdir");
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join("profile");
        let profiles = profile.join("profiles");
        fs::create_dir_all(cwd.join(".remote-code")).expect("workspace dir");
        fs::create_dir_all(&profiles).expect("profiles dir");
        let explicit = tempdir.path().join("custom.json");
        fs::write(cwd.join(".remote-code").join("settings.json"), "{}").expect("workspace");
        fs::write(&explicit, "{}").expect("explicit");

        let resolved = resolve_runtime_settings_files(
            &cwd,
            &profile,
            &profiles,
            std::slice::from_ref(&explicit),
            &SettingSource::all(),
        );
        assert_eq!(resolved, vec![explicit]);
    }

    #[test]
    fn discover_runtime_settings_files_discovers_sources_in_override_order() {
        let tempdir = tempdir().expect("tempdir");
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join("profile");
        let profiles = profile.join("profiles");
        fs::create_dir_all(cwd.join(".remote-code")).expect("workspace dir");
        fs::create_dir_all(profiles.join("legacy-import")).expect("legacy dir");
        fs::create_dir_all(&profile).expect("profile dir");

        let legacy = profiles.join("legacy-import").join("settings.json");
        let user = profile.join("settings.json");
        let project = cwd.join(".remote-code").join("settings.json");
        let local = cwd.join(".remote-code").join("settings.local.json");
        fs::write(&legacy, "{}").expect("legacy");
        fs::write(&user, "{}").expect("user");
        fs::write(&project, "{}").expect("project");
        fs::write(&local, "{}").expect("local");

        let discovered =
            discover_runtime_settings_sources(&cwd, &profile, &profiles, &SettingSource::all());
        assert_eq!(
            discovered
                .iter()
                .map(|source| source.kind)
                .collect::<Vec<_>>(),
            vec!["legacy-import", "profile", "project", "local"]
        );
        assert_eq!(
            discovered
                .into_iter()
                .map(|source| source.path)
                .collect::<Vec<_>>(),
            vec![legacy, user, project, local]
        );
    }

    #[test]
    fn discover_runtime_settings_files_skips_missing_files() {
        let tempdir = tempdir().expect("tempdir");
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join("profile");
        let profiles = profile.join("profiles");
        fs::create_dir_all(cwd.join(".remote-code")).expect("workspace dir");
        fs::create_dir_all(profiles.join("legacy-import")).expect("legacy dir");
        fs::create_dir_all(&profile).expect("profile dir");
        let project = cwd.join(".remote-code").join("settings.json");
        fs::write(&project, "{}").expect("project");

        let resolved =
            resolve_runtime_settings_files(&cwd, &profile, &profiles, &[], &SettingSource::all());
        assert_eq!(resolved, vec![project]);
    }

    #[test]
    fn discover_runtime_settings_files_can_limit_to_local_scope() {
        let tempdir = tempdir().expect("tempdir");
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join("profile");
        let profiles = profile.join("profiles");
        fs::create_dir_all(cwd.join(".remote-code")).expect("workspace dir");
        fs::create_dir_all(profiles.join("legacy-import")).expect("legacy dir");
        fs::create_dir_all(&profile).expect("profile dir");

        let legacy = profiles.join("legacy-import").join("settings.json");
        let user = profile.join("settings.json");
        let project = cwd.join(".remote-code").join("settings.json");
        let local = cwd.join(".remote-code").join("settings.local.json");
        fs::write(&legacy, "{}").expect("legacy");
        fs::write(&user, "{}").expect("user");
        fs::write(&project, "{}").expect("project");
        fs::write(&local, "{}").expect("local");

        let resolved =
            resolve_runtime_settings_files(&cwd, &profile, &profiles, &[], &[SettingSource::Local]);

        assert_eq!(resolved, vec![local]);
    }
}

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use rc_core::ProviderProtocol;
use serde::Deserialize;

use crate::tool_filters::{merge_tool_filters, normalize_tool_filters};

/// Runtime-only overrides layered on top of environment variables and settings files.
#[derive(Debug, Clone, Default)]
pub struct RuntimeOverrides {
    pub session_name: Option<String>,
    pub settings_files: Vec<PathBuf>,
    pub show_setting_sources: bool,
    pub allowed_tools: Vec<String>,
    pub disallowed_tools: Vec<String>,
    pub effort: Option<String>,
    pub fallback_model: Option<String>,
}

/// Settings materialized from one or more settings files.
#[derive(Debug, Clone, Default)]
pub struct ResolvedRuntimeSettings {
    pub provider_name: Option<String>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub model: Option<String>,
    pub protocol: Option<ProviderProtocol>,
    pub timeout_ms: Option<u64>,
    pub max_output_tokens: Option<u32>,
    pub thinking_budget: Option<u32>,
    pub session_name: Option<String>,
    pub allowed_tools: Vec<String>,
    pub disallowed_tools: Vec<String>,
    pub effort: Option<String>,
    pub fallback_model: Option<String>,
    pub setting_sources: Vec<String>,
    pub auth_source: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct SettingsDocument {
    #[serde(default)]
    provider: Option<SettingsProvider>,
    #[serde(default)]
    session_name: Option<String>,
    #[serde(default)]
    allowed_tools: Option<Vec<String>>,
    #[serde(default)]
    disallowed_tools: Option<Vec<String>>,
    #[serde(default)]
    effort: Option<String>,
    #[serde(default)]
    fallback_model: Option<String>,
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
    use super::{RuntimeOverrides, load_runtime_settings};
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
}

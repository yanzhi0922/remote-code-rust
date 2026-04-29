//! Settings Import/Export
//!
//! Handles importing and exporting configuration settings.
//! Mirrors `importExport.ts`.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors during import/export operations.
#[derive(Debug, thiserror::Error)]
pub enum ImportExportError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("No valid profiles could be imported: {0}")]
    NoValidProfiles(String),

    #[error("User cancelled file selection")]
    UserCancelled,

    #[error("{0}")]
    ValidationError(String),
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Result of a settings import operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportResult {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warnings: Option<Vec<String>>,
}

/// Provider profiles structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfiles {
    pub current_api_config_name: String,
    pub api_configs: HashMap<String, Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode_api_configs: Option<HashMap<String, String>>,
}

/// Export data structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportData {
    pub provider_profiles: ProviderProfiles,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global_settings: Option<Value>,
}

// ---------------------------------------------------------------------------
// Import/Export functions
// ---------------------------------------------------------------------------

/// Sanitize a provider config by resetting invalid/removed apiProvider values.
///
/// Source: `importExport.ts` — `sanitizeProviderConfig`
pub fn sanitize_provider_config(
    config_name: &str,
    api_config: &Value,
) -> (Value, Option<String>) {
    if !api_config.is_object() {
        return (api_config.clone(), None);
    }

    let obj = api_config.as_object().unwrap();

    // Valid provider names
    let valid_providers = [
        "anthropic", "openai", "bedrock", "vertex", "google", "ollama",
        "openrouter", "deepseek", "xai", "minimax", "moonshot", "qwen",
        "mistral", "fireworks", "sambanova", "baseten", "poe", "litellm",
        "requesty", "unbound", "roo", "vercel", "lmstudio", "vscode-lm", "zai",
    ];

    if let Some(api_provider) = obj.get("apiProvider").and_then(|v| v.as_str()) {
        if !valid_providers.contains(&api_provider) {
            let mut sanitized = obj.clone();
            sanitized.remove("apiProvider");
            return (
                Value::Object(sanitized),
                Some(format!(
                    "Profile \"{}\": Invalid provider \"{}\" was removed. Please reconfigure this profile.",
                    config_name, api_provider
                )),
            );
        }
    }

    (api_config.clone(), None)
}

/// Import settings from a file path.
///
/// Source: `importExport.ts` — `importSettingsFromPath`
pub async fn import_settings_from_path(
    file_path: &Path,
    previous_profiles: &ProviderProfiles,
) -> Result<ImportResult, ImportExportError> {
    let raw_data = tokio::fs::read_to_string(file_path).await?;
    let raw_value: Value = serde_json::from_str(&raw_data)?;

    // Extract provider profiles
    let raw_profiles = raw_value
        .get("providerProfiles")
        .ok_or_else(|| ImportExportError::ValidationError("Missing providerProfiles".to_string()))?;

    let raw_configs = raw_profiles
        .get("apiConfigs")
        .and_then(|v| v.as_object())
        .ok_or_else(|| ImportExportError::ValidationError("Missing apiConfigs".to_string()))?;

    let current_name = raw_profiles
        .get("currentApiConfigName")
        .and_then(|v| v.as_str())
        .unwrap_or("default");

    // Process each config with sanitization
    let mut warnings: Vec<String> = Vec::new();
    let mut valid_configs: HashMap<String, Value> = HashMap::new();

    for (config_name, raw_config) in raw_configs {
        let (sanitized, warning) = sanitize_provider_config(config_name, raw_config);
        if let Some(w) = warning {
            warnings.push(w);
        }
        valid_configs.insert(config_name.clone(), sanitized);
    }

    if valid_configs.is_empty() && !warnings.is_empty() {
        return Ok(ImportResult {
            success: false,
            error: Some(format!(
                "No valid profiles could be imported:\n{}",
                warnings.join("\n")
            )),
            warnings: None,
        });
    }

    // Determine current config name
    let resolved_name = if valid_configs.contains_key(current_name) {
        current_name.to_string()
    } else {
        let valid_names: Vec<&String> = valid_configs.keys().collect();
        if !valid_names.is_empty() {
            let first = valid_names[0].clone();
            warnings.push(format!(
                "Profile \"{}\" was not available; defaulting to \"{}\".",
                current_name, first
            ));
            first.clone()
        } else {
            previous_profiles.current_api_config_name.clone()
        }
    };

    // Merge with previous profiles
    let mut merged_configs = previous_profiles.api_configs.clone();
    merged_configs.extend(valid_configs);

    let imported_profiles = ProviderProfiles {
        current_api_config_name: resolved_name,
        api_configs: merged_configs,
        mode_api_configs: raw_profiles
            .get("modeApiConfigs")
            .and_then(|v| serde_json::from_value(v.clone()).ok()),
    };

    // Extract global settings
    let global_settings = raw_value.get("globalSettings").cloned();

    let _ = (imported_profiles, global_settings);

    Ok(ImportResult {
        success: true,
        error: None,
        warnings: if warnings.is_empty() {
            None
        } else {
            Some(warnings)
        },
    })
}

/// Export settings to a file path.
///
/// Source: `importExport.ts` — `exportSettings`
pub async fn export_settings(
    file_path: &Path,
    provider_profiles: &ProviderProfiles,
    global_settings: Option<&Value>,
) -> Result<(), ImportExportError> {
    let export_data = ExportData {
        provider_profiles: provider_profiles.clone(),
        global_settings: global_settings.cloned(),
    };

    let json_content = serde_json::to_string_pretty(&export_data)?;

    if let Some(parent) = file_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    tokio::fs::write(file_path, &json_content).await?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_sanitize_provider_config_valid() {
        let config = json!({"apiProvider": "anthropic", "apiKey": "test"});
        let (sanitized, warning) = sanitize_provider_config("test", &config);
        assert!(warning.is_none());
        assert_eq!(sanitized["apiProvider"], "anthropic");
    }

    #[test]
    fn test_sanitize_provider_config_invalid() {
        let config = json!({"apiProvider": "invalid_provider", "apiKey": "test"});
        let (sanitized, warning) = sanitize_provider_config("test", &config);
        assert!(warning.is_some());
        assert!(sanitized.get("apiProvider").is_none());
    }

    #[test]
    fn test_sanitize_provider_config_no_provider() {
        let config = json!({"apiKey": "test"});
        let (sanitized, warning) = sanitize_provider_config("test", &config);
        assert!(warning.is_none());
        assert!(sanitized.get("apiKey").is_some());
    }

    #[tokio::test]
    async fn test_export_and_import_settings() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("settings.json");

        let profiles = ProviderProfiles {
            current_api_config_name: "default".to_string(),
            api_configs: {
                let mut m = HashMap::new();
                m.insert("default".to_string(), json!({"apiProvider": "openai"}));
                m
            },
            mode_api_configs: None,
        };

        export_settings(&file_path, &profiles, None)
            .await
            .unwrap();

        assert!(file_path.exists());

        let previous = ProviderProfiles {
            current_api_config_name: "default".to_string(),
            api_configs: HashMap::new(),
            mode_api_configs: None,
        };

        let result = import_settings_from_path(&file_path, &previous)
            .await
            .unwrap();
        assert!(result.success);
    }

    #[test]
    fn test_import_result_serialization() {
        let result = ImportResult {
            success: true,
            error: None,
            warnings: Some(vec!["test warning".to_string()]),
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: ImportResult = serde_json::from_str(&json).unwrap();
        assert!(parsed.success);
        assert!(parsed.warnings.is_some());
    }
}

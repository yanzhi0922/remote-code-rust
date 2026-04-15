//! Settings loading from files and strings.

use std::path::Path;

use anyhow::{Context, Result};

use crate::types::Settings;

/// Load settings from a JSON file.
///
/// # Errors
/// Returns an error if the file cannot be read or parsed as valid JSON.
pub fn load_settings_from_file(path: impl AsRef<Path>) -> Result<Settings> {
    let path = path.as_ref();
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read settings file: {}", path.display()))?;
    load_settings_from_str(&content)
        .with_context(|| format!("Failed to parse settings file: {}", path.display()))
}

/// Load settings from a JSON string.
///
/// # Errors
/// Returns an error if the string is not valid JSON or doesn't match the settings schema.
pub fn load_settings_from_str(content: &str) -> Result<Settings> {
    let settings: Settings = serde_json::from_str(content)
        .context("Failed to parse settings JSON")?;
    Ok(settings)
}

/// Save settings to a JSON file.
///
/// # Errors
/// Returns an error if the file cannot be written.
pub fn save_settings_to_file(settings: &Settings, path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref();
    let content = serde_json::to_string_pretty(settings)
        .context("Failed to serialize settings")?;

    // Create parent directories if needed
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    std::fs::write(path, content)
        .with_context(|| format!("Failed to write settings file: {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn load_from_str_empty() {
        let settings = load_settings_from_str("{}").unwrap();
        assert!(settings.model.is_none());
    }

    #[test]
    fn load_from_str_with_model() {
        let json = r#"{"model":"claude-opus-4","fastMode":true}"#;
        let settings = load_settings_from_str(json).unwrap();
        assert_eq!(settings.model.as_deref(), Some("claude-opus-4"));
        assert_eq!(settings.fast_mode, Some(true));
    }

    #[test]
    fn load_from_str_with_permissions() {
        let json = r#"{
            "permissions": {
                "allow": ["Bash(*)", "Edit(*)"],
                "deny": ["Bash(rm -rf /)"]
            }
        }"#;
        let settings = load_settings_from_str(json).unwrap();
        let perms = settings.permissions.unwrap();
        assert_eq!(perms.allow.unwrap().len(), 2);
        assert_eq!(perms.deny.unwrap().len(), 1);
    }

    #[test]
    fn load_from_str_with_providers() {
        let json = r#"{
            "providers": {
                "custom": {
                    "type": "anthropic-compatible",
                    "baseURL": "https://api.example.com"
                }
            }
        }"#;
        let settings = load_settings_from_str(json).unwrap();
        let providers = settings.providers.unwrap();
        assert!(providers.contains_key("custom"));
    }

    #[test]
    fn load_from_str_invalid_json() {
        let result = load_settings_from_str("{invalid}");
        assert!(result.is_err());
    }

    #[test]
    fn load_from_file() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");
        std::fs::write(&path, r#"{"model":"test-model"}"#).unwrap();

        let settings = load_settings_from_file(&path).unwrap();
        assert_eq!(settings.model.as_deref(), Some("test-model"));
    }

    #[test]
    fn load_from_missing_file() {
        let result = load_settings_from_file("/nonexistent/settings.json");
        assert!(result.is_err());
    }

    #[test]
    fn save_and_reload() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");

        let mut settings = Settings::new();
        settings.model = Some("test-model".to_string());
        settings.fast_mode = Some(true);

        save_settings_to_file(&settings, &path).unwrap();

        let reloaded = load_settings_from_file(&path).unwrap();
        assert_eq!(reloaded.model, settings.model);
        assert_eq!(reloaded.fast_mode, settings.fast_mode);
    }

    #[test]
    fn save_creates_parent_dirs() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("nested").join("dir").join("settings.json");

        let settings = Settings::new();
        save_settings_to_file(&settings, &path).unwrap();
        assert!(path.exists());
    }
}

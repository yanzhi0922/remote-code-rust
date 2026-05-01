//! Provider Settings Manager
//!
//! Manages provider configuration profiles with CRUD operations and migrations.
//! Mirrors `ProviderSettingsManager.ts`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors from provider settings management.
#[derive(Debug, thiserror::Error)]
pub enum ProviderSettingsError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Config not found: {0}")]
    NotFound(String),

    #[error("Config already exists: {0}")]
    AlreadyExists(String),

    #[error("Lock error: {0}")]
    LockError(String),
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Model migrations mapping.
const MODEL_MIGRATIONS: &[(&str, &str, &str)] = &[
    ("roo", "roo/code-supernova", "roo/code-supernova-1-million"),
];

/// Provider settings with an ID.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSettingsWithId {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

/// Provider profiles structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderProfiles {
    pub current_api_config_name: String,
    pub api_configs: HashMap<String, ProviderSettingsWithId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode_api_configs: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub migrations: Option<MigrationState>,
}

/// Migration state tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MigrationState {
    pub rate_limit_seconds_migrated: Option<bool>,
    pub open_ai_headers_migrated: Option<bool>,
    pub consecutive_mistake_limit_migrated: Option<bool>,
    pub todo_list_enabled_migrated: Option<bool>,
    pub claude_code_legacy_settings_migrated: Option<bool>,
}

impl Default for MigrationState {
    fn default() -> Self {
        Self {
            rate_limit_seconds_migrated: Some(true),
            open_ai_headers_migrated: Some(true),
            consecutive_mistake_limit_migrated: Some(true),
            todo_list_enabled_migrated: Some(true),
            claude_code_legacy_settings_migrated: Some(true),
        }
    }
}

// ---------------------------------------------------------------------------
// ProviderSettingsManager
// ---------------------------------------------------------------------------

/// Manages provider configuration profiles.
///
/// Source: `.research/Roo-Code/src/core/config/ProviderSettingsManager.ts`
pub struct ProviderSettingsManager {
    settings_path: PathBuf,
    profiles: ProviderProfiles,
}

impl ProviderSettingsManager {
    /// Create a new manager with the given settings directory.
    pub async fn new(settings_dir: &Path) -> Result<Self, ProviderSettingsError> {
        let settings_path = settings_dir.join("provider_profiles.json");
        let profiles = Self::load_profiles(&settings_path).await?;

        Ok(Self {
            settings_path,
            profiles,
        })
    }

    /// Get the current profiles.
    pub fn profiles(&self) -> &ProviderProfiles {
        &self.profiles
    }

    /// Get the current API config name.
    pub fn current_config_name(&self) -> &str {
        &self.profiles.current_api_config_name
    }

    /// Set the current API config name.
    pub fn set_current_config_name(&mut self, name: &str) {
        self.profiles.current_api_config_name = name.to_string();
    }

    /// List all config names.
    pub fn list_configs(&self) -> Vec<String> {
        self.profiles.api_configs.keys().cloned().collect()
    }

    /// Get a specific config by name.
    pub fn get_config(&self, name: &str) -> Option<&ProviderSettingsWithId> {
        self.profiles.api_configs.get(name)
    }

    /// Add or update a config.
    pub fn upsert_config(
        &mut self,
        name: &str,
        config: ProviderSettingsWithId,
    ) {
        self.profiles.api_configs.insert(name.to_string(), config);
    }

    /// Delete a config by name.
    pub fn delete_config(&mut self, name: &str) -> Option<ProviderSettingsWithId> {
        self.profiles.api_configs.remove(name)
    }

    /// Export all profiles.
    pub fn export_profiles(&self) -> &ProviderProfiles {
        &self.profiles
    }

    /// Import profiles, merging with existing.
    pub async fn import_profiles(
        &mut self,
        new_profiles: ProviderProfiles,
    ) -> Result<(), ProviderSettingsError> {
        // Merge api_configs
        self.profiles
            .api_configs
            .extend(new_profiles.api_configs);

        // Update mode_api_configs if provided
        if let Some(new_modes) = new_profiles.mode_api_configs {
            let modes = self.profiles.mode_api_configs.get_or_insert_with(HashMap::new);
            modes.extend(new_modes);
        }

        // Update current config name
        self.profiles.current_api_config_name = new_profiles.current_api_config_name;

        // Apply model migrations
        self.apply_model_migrations();

        // Save
        self.save_profiles().await
    }

    /// Save profiles to disk.
    pub async fn save_profiles(&self) -> Result<(), ProviderSettingsError> {
        if let Some(parent) = self.settings_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let json = serde_json::to_string_pretty(&self.profiles)?;
        tokio::fs::write(&self.settings_path, &json).await?;
        Ok(())
    }

    /// Generate a random ID.
    pub fn generate_id() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        format!("{:013}", now.as_nanos() % 1_000_000_000_000_0)
    }

    // -- Private helpers -------------------------------------------------------

    async fn load_profiles(
        path: &Path,
    ) -> Result<ProviderProfiles, ProviderSettingsError> {
        if !path.exists() {
            let default_id = Self::generate_id();
            return Ok(ProviderProfiles {
                current_api_config_name: "default".to_string(),
                api_configs: {
                    let mut m = HashMap::new();
                    m.insert(
                        "default".to_string(),
                        ProviderSettingsWithId {
                            id: Some(default_id),
                            api_provider: None,
                            model_id: None,
                            extra: serde_json::Map::new(),
                        },
                    );
                    m
                },
                mode_api_configs: None,
                migrations: Some(MigrationState::default()),
            });
        }

        let content = tokio::fs::read_to_string(path).await?;
        let profiles: ProviderProfiles = serde_json::from_str(&content)?;
        Ok(profiles)
    }

    fn apply_model_migrations(&mut self) {
        for (_name, config) in &mut self.profiles.api_configs {
            let should_migrate = config.model_id.as_deref().map_or(false, |model_id| {
                MODEL_MIGRATIONS.iter().any(|(provider, old_model, _)| {
                    config.api_provider.as_deref() == Some(*provider) && model_id == *old_model
                })
            });

            if should_migrate {
                let old_id = config.model_id.clone().unwrap();
                for (provider, old_model, new_model) in MODEL_MIGRATIONS {
                    if config.api_provider.as_deref() == Some(*provider) && old_id == *old_model {
                        config.model_id = Some(new_model.to_string());
                        break;
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_id() {
        let id = ProviderSettingsManager::generate_id();
        assert!(!id.is_empty());
    }

    #[test]
    fn test_migration_state_default() {
        let state = MigrationState::default();
        assert_eq!(state.rate_limit_seconds_migrated, Some(true));
        assert_eq!(state.open_ai_headers_migrated, Some(true));
    }

    #[tokio::test]
    async fn test_new_manager_creates_default() {
        let tmp = tempfile::tempdir().unwrap();
        let manager = ProviderSettingsManager::new(tmp.path()).await.unwrap();
        assert_eq!(manager.current_config_name(), "default");
        assert!(manager.get_config("default").is_some());
    }

    #[tokio::test]
    async fn test_upsert_and_get_config() {
        let tmp = tempfile::tempdir().unwrap();
        let mut manager = ProviderSettingsManager::new(tmp.path()).await.unwrap();

        let config = ProviderSettingsWithId {
            id: Some("test-id".to_string()),
            api_provider: Some("anthropic".to_string()),
            model_id: Some("claude-3".to_string()),
            extra: serde_json::Map::new(),
        };

        manager.upsert_config("my-profile", config);
        assert!(manager.get_config("my-profile").is_some());
        assert_eq!(
            manager.get_config("my-profile").unwrap().api_provider,
            Some("anthropic".to_string())
        );
    }

    #[tokio::test]
    async fn test_delete_config() {
        let tmp = tempfile::tempdir().unwrap();
        let mut manager = ProviderSettingsManager::new(tmp.path()).await.unwrap();

        let config = ProviderSettingsWithId {
            id: Some("test-id".to_string()),
            api_provider: None,
            model_id: None,
            extra: serde_json::Map::new(),
        };

        manager.upsert_config("to-delete", config);
        assert!(manager.delete_config("to-delete").is_some());
        assert!(manager.get_config("to-delete").is_none());
    }

    #[tokio::test]
    async fn test_save_and_reload() {
        let tmp = tempfile::tempdir().unwrap();
        let mut manager = ProviderSettingsManager::new(tmp.path()).await.unwrap();

        manager.set_current_config_name("updated");
        manager.save_profiles().await.unwrap();

        let reloaded = ProviderSettingsManager::new(tmp.path()).await.unwrap();
        assert_eq!(reloaded.current_config_name(), "updated");
    }
}

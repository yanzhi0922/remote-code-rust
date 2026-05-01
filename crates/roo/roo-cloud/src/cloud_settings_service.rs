/// Cloud settings service for fetching and managing organization/user settings.
/// Mirrors packages/cloud/src/CloudSettingsService.ts

use crate::config::get_roo_code_api_url;
use crate::types::{CloudError, CloudSettingsConfig, OrganizationSettingsData, UserFeatures, UserSettingsData};
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Callback type for settings change notifications.
pub type SettingsChangeCallback = Box<dyn Fn(&str) + Send + Sync>;

/// Cloud settings service that fetches and caches organization and user settings.
pub struct CloudSettingsService {
    org_settings: Arc<RwLock<Option<OrganizationSettingsData>>>,
    user_settings: Arc<RwLock<Option<UserSettingsData>>>,
    session_token: Arc<RwLock<Option<String>>>,
}

impl CloudSettingsService {
    /// Create a new CloudSettingsService.
    pub fn new() -> Self {
        Self {
            org_settings: Arc::new(RwLock::new(None)),
            user_settings: Arc::new(RwLock::new(None)),
            session_token: Arc::new(RwLock::new(None)),
        }
    }

    /// Set the session token for authentication.
    pub async fn set_session_token(&self, token: Option<String>) {
        let mut guard = self.session_token.write().await;
        *guard = token;
    }

    /// Fetch settings from the cloud API.
    /// Returns true on success, false on failure.
    pub async fn fetch_settings(&self) -> bool {
        let token = self.session_token.read().await;
        let token = match token.as_ref() {
            Some(t) => t.clone(),
            None => return false,
        };
        drop(token);

        let url = format!("{}/api/extension-settings", get_roo_code_api_url());

        let client = reqwest::Client::new();
        let token_guard = self.session_token.read().await;
        let token_val = match token_guard.as_ref() {
            Some(t) => t.as_str(),
            None => return false,
        };

        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token_val))
            .send()
            .await;

        match response {
            Ok(resp) => {
                if !resp.status().is_success() {
                    return false;
                }

                match resp.json::<Value>().await {
                    Ok(data) => {
                        let _ = self.parse_and_update_settings(data).await;
                        true
                    }
                    Err(_) => false,
                }
            }
            Err(_) => false,
        }
    }

    /// Parse the extension settings response and update cached settings.
    async fn parse_and_update_settings(&self, data: Value) -> Result<bool, CloudError> {
        let org_data = &data["organization"];
        let user_data = &data["user"];

        // Parse organization settings
        let new_org = OrganizationSettingsData {
            version: org_data["version"].as_u64().unwrap_or(0),
            allow_list: org_data["allowList"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default(),
            mcps: org_data["mcps"]
                .as_array()
                .cloned()
                .unwrap_or_default(),
            cloud_settings: self.parse_cloud_settings(&org_data["cloudSettings"]),
        };

        // Parse user settings
        let new_user = UserSettingsData {
            version: user_data["version"].as_u64().unwrap_or(0),
            cloud_settings: self.parse_cloud_settings(&user_data["cloudSettings"]),
            features: self.parse_user_features(&user_data["features"]),
        };

        // Check for changes
        let mut org_guard = self.org_settings.write().await;
        let mut user_guard = self.user_settings.write().await;

        let org_changed = org_guard
            .as_ref()
            .map_or(true, |existing| existing.version != new_org.version);

        let user_changed = user_guard
            .as_ref()
            .map_or(true, |existing| existing.version != new_user.version);

        if org_changed {
            *org_guard = Some(new_org);
        }

        if user_changed {
            *user_guard = Some(new_user);
        }

        Ok(org_changed || user_changed)
    }

    fn parse_cloud_settings(&self, value: &Value) -> Option<CloudSettingsConfig> {
        if value.is_null() {
            return None;
        }

        Some(CloudSettingsConfig {
            enable_task_sharing: value["enableTaskSharing"].as_bool().unwrap_or(false),
            allow_public_task_sharing: value["allowPublicTaskSharing"].as_bool().unwrap_or(false),
        })
    }

    fn parse_user_features(&self, value: &Value) -> Option<UserFeatures> {
        if value.is_null() {
            return None;
        }

        Some(UserFeatures {
            task_sync: value["taskSync"].as_bool().unwrap_or(false),
            task_sharing: value["taskSharing"].as_bool().unwrap_or(false),
            code_indexing: value["codeIndexing"].as_bool().unwrap_or(false),
        })
    }

    /// Get the current organization settings.
    pub async fn get_org_settings(&self) -> Option<OrganizationSettingsData> {
        self.org_settings.read().await.clone()
    }

    /// Get the current user settings.
    pub async fn get_user_settings(&self) -> Option<UserSettingsData> {
        self.user_settings.read().await.clone()
    }

    /// Check if task sharing is enabled.
    pub async fn is_task_sharing_enabled(&self) -> bool {
        let user = self.user_settings.read().await;
        user.as_ref()
            .and_then(|s| s.cloud_settings.as_ref())
            .map(|c| c.enable_task_sharing)
            .unwrap_or(false)
    }

    /// Check if public task sharing is allowed.
    pub async fn is_public_sharing_allowed(&self) -> bool {
        let user = self.user_settings.read().await;
        user.as_ref()
            .and_then(|s| s.cloud_settings.as_ref())
            .map(|c| c.enable_task_sharing && c.allow_public_task_sharing)
            .unwrap_or(false)
    }

    /// Clear all cached settings (e.g., on logout).
    pub async fn clear_settings(&self) {
        *self.org_settings.write().await = None;
        *self.user_settings.write().await = None;
    }
}

impl Default for CloudSettingsService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_new_service_has_no_settings() {
        let service = CloudSettingsService::new();
        assert!(service.get_org_settings().await.is_none());
        assert!(service.get_user_settings().await.is_none());
    }

    #[tokio::test]
    async fn test_clear_settings() {
        let service = CloudSettingsService::new();
        service.clear_settings().await;
        assert!(service.get_org_settings().await.is_none());
        assert!(service.get_user_settings().await.is_none());
    }

    #[tokio::test]
    async fn test_is_task_sharing_enabled_default() {
        let service = CloudSettingsService::new();
        assert!(!service.is_task_sharing_enabled().await);
    }

    #[tokio::test]
    async fn test_is_public_sharing_allowed_default() {
        let service = CloudSettingsService::new();
        assert!(!service.is_public_sharing_allowed().await);
    }

    #[tokio::test]
    async fn test_parse_and_update_settings() {
        let service = CloudSettingsService::new();

        let data = serde_json::json!({
            "organization": {
                "version": 1,
                "allowList": ["openai"],
                "mcps": [],
                "cloudSettings": {
                    "enableTaskSharing": true,
                    "allowPublicTaskSharing": false
                }
            },
            "user": {
                "version": 2,
                "cloudSettings": {
                    "enableTaskSharing": true,
                    "allowPublicTaskSharing": true
                },
                "features": {
                    "taskSync": true,
                    "taskSharing": true,
                    "codeIndexing": false
                }
            }
        });

        let result = service.parse_and_update_settings(data).await;
        assert!(result.unwrap());

        let org = service.get_org_settings().await.unwrap();
        assert_eq!(1, org.version);
        assert_eq!(vec!["openai".to_string()], org.allow_list);

        let user = service.get_user_settings().await.unwrap();
        assert_eq!(2, user.version);
        assert!(service.is_task_sharing_enabled().await);
        assert!(service.is_public_sharing_allowed().await);
    }

    #[tokio::test]
    async fn test_set_session_token() {
        let service = CloudSettingsService::new();
        service.set_session_token(Some("test-token".to_string())).await;

        let token = service.session_token.read().await;
        assert_eq!(Some("test-token".to_string()), token.clone());
    }
}

/// Static settings service for non-cloud environments.
/// Mirrors packages/cloud/src/StaticSettingsService.ts

use crate::types::{OrganizationSettingsData, UserSettingsData};

/// Static settings service that returns fixed/default settings.
/// Used when cloud connectivity is not available.
pub struct StaticSettingsService {
    org_settings: Option<OrganizationSettingsData>,
    user_settings: Option<UserSettingsData>,
}

impl StaticSettingsService {
    /// Create a new StaticSettingsService with default settings.
    pub fn new() -> Self {
        Self {
            org_settings: Some(OrganizationSettingsData {
                version: 0,
                allow_list: vec![],
                mcps: vec![],
                cloud_settings: None,
            }),
            user_settings: Some(UserSettingsData {
                version: 0,
                cloud_settings: None,
                features: None,
            }),
        }
    }

    /// Create with custom settings.
    pub fn with_settings(
        org: Option<OrganizationSettingsData>,
        user: Option<UserSettingsData>,
    ) -> Self {
        Self {
            org_settings: org,
            user_settings: user,
        }
    }

    /// Get organization settings.
    pub fn get_org_settings(&self) -> Option<&OrganizationSettingsData> {
        self.org_settings.as_ref()
    }

    /// Get user settings.
    pub fn get_user_settings(&self) -> Option<&UserSettingsData> {
        self.user_settings.as_ref()
    }

    /// Check if task sharing is enabled (always false for static).
    pub fn is_task_sharing_enabled(&self) -> bool {
        self.user_settings
            .as_ref()
            .and_then(|s| s.cloud_settings.as_ref())
            .map(|c| c.enable_task_sharing)
            .unwrap_or(false)
    }

    /// Check if public sharing is allowed (always false for static).
    pub fn is_public_sharing_allowed(&self) -> bool {
        self.user_settings
            .as_ref()
            .and_then(|s| s.cloud_settings.as_ref())
            .map(|c| c.enable_task_sharing && c.allow_public_task_sharing)
            .unwrap_or(false)
    }
}

impl Default for StaticSettingsService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CloudSettingsConfig, UserFeatures};

    #[test]
    fn test_new_has_default_settings() {
        let service = StaticSettingsService::new();
        assert!(service.get_org_settings().is_some());
        assert!(service.get_user_settings().is_some());
    }

    #[test]
    fn test_task_sharing_disabled_by_default() {
        let service = StaticSettingsService::new();
        assert!(!service.is_task_sharing_enabled());
        assert!(!service.is_public_sharing_allowed());
    }

    #[test]
    fn test_with_custom_settings() {
        let user = UserSettingsData {
            version: 1,
            cloud_settings: Some(CloudSettingsConfig {
                enable_task_sharing: true,
                allow_public_task_sharing: true,
            }),
            features: Some(UserFeatures {
                task_sync: true,
                task_sharing: true,
                code_indexing: false,
            }),
        };

        let service = StaticSettingsService::with_settings(None, Some(user));
        assert!(service.is_task_sharing_enabled());
        assert!(service.is_public_sharing_allowed());
    }
}

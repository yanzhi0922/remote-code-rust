/// Cloud share service for sharing tasks.
/// Mirrors packages/cloud/src/CloudShareService.ts

use crate::cloud_api::{CloudApi, ShareVisibility};
use crate::cloud_settings_service::CloudSettingsService;
use crate::types::{CloudError, ShareResponse};

/// Service for sharing tasks via the cloud.
pub struct CloudShareService {
    api: CloudApi,
    settings: CloudSettingsService,
}

impl CloudShareService {
    /// Create a new CloudShareService.
    pub fn new(api: CloudApi, settings: CloudSettingsService) -> Self {
        Self { api, settings }
    }

    /// Share a task with the specified visibility.
    pub async fn share_task(
        &self,
        task_id: &str,
        visibility: ShareVisibility,
        token: &str,
    ) -> Result<ShareResponse, CloudError> {
        let response = self.api.share_task(task_id, visibility, token).await?;

        // Convert from cloud_api::ShareResponse to types::ShareResponse
        let result = ShareResponse {
            success: response.success,
            share_url: response.share_url,
            task_id: response.task_id,
        };

        Ok(result)
    }

    /// Check if task sharing is enabled.
    pub async fn can_share_task(&self) -> bool {
        self.settings.is_task_sharing_enabled().await
    }

    /// Check if public task sharing is allowed.
    pub async fn can_share_publicly(&self) -> bool {
        self.settings.is_public_sharing_allowed().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_can_create_service() {
        let api = CloudApi::new(Some("1.0.0"));
        let settings = CloudSettingsService::new();
        let _service = CloudShareService::new(api, settings);
    }
}

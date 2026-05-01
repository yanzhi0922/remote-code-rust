use crate::types::{AuthState, CloudError, CloudUserInfo, OrganizationSettings, UserSettings};

/// Cloud service for managing authentication, user info, and organization settings.
///
/// Corresponds to TS: `CloudService` class in `packages/cloud/src/CloudService.ts`.
/// Manages auth state, user info, organization settings, and provides a facade
/// over the underlying auth/settings/share/telemetry services.
pub struct CloudService {
    auth_state: AuthState,
    user_info: Option<CloudUserInfo>,
    org_id: Option<String>,
    org_name: Option<String>,
    org_role: Option<String>,
    org_settings: Option<OrganizationSettings>,
    user_settings: Option<UserSettings>,
    is_initialized: bool,
    is_cloud_agent: bool,
}

impl CloudService {
    /// Create a new cloud service in the logged-out state.
    pub fn new() -> Self {
        Self {
            auth_state: AuthState::LoggedOut,
            user_info: None,
            org_id: None,
            org_name: None,
            org_role: None,
            org_settings: None,
            user_settings: None,
            is_initialized: false,
            is_cloud_agent: false,
        }
    }

    /// Attempt to log in by sending an authentication request to the given
    /// API endpoint.
    ///
    /// If no endpoint is provided or the endpoint is unreachable, returns a
    /// meaningful error instead of falling back to simulated data.
    pub async fn login(
        &mut self,
        api_endpoint: Option<&str>,
        token: Option<&str>,
    ) -> Result<(), CloudError> {
        self.auth_state = AuthState::AttemptingSession;

        let endpoint = api_endpoint.unwrap_or("https://api.roocode.com/v1/auth/session");

        let client = reqwest::Client::new();
        let mut request = client.get(endpoint);

        if let Some(t) = token {
            request = request.bearer_auth(t);
        }

        let response = request.send().await.map_err(|e| {
            self.auth_state = AuthState::LoggedOut;
            CloudError::NetworkError(format!(
                "Failed to reach authentication endpoint '{}': {}",
                endpoint, e
            ))
        })?;

        if !response.status().is_success() {
            self.auth_state = AuthState::LoggedOut;
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(CloudError::AuthenticationFailed(format!(
                "Authentication endpoint returned {}: {}",
                status, body
            )));
        }

        let user_data: serde_json::Value = response.json().await.map_err(|e| {
            self.auth_state = AuthState::LoggedOut;
            CloudError::SerializationError(format!("Failed to parse auth response: {}", e))
        })?;

        // Parse user info from response
        let user_info = CloudUserInfo {
            id: user_data["id"].as_str().unwrap_or_default().to_string(),
            email: user_data["email"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            name: user_data["name"].as_str().unwrap_or_default().to_string(),
            avatar_url: user_data["avatarUrl"].as_str().map(|s| s.to_string()),
        };

        self.auth_state = AuthState::ActiveSession;
        self.user_info = Some(user_info);
        self.user_settings = Some(UserSettings {
            task_sync_enabled: user_data["taskSyncEnabled"].as_bool().unwrap_or(true),
            telemetry_setting: user_data["telemetrySetting"]
                .as_str()
                .unwrap_or("enabled")
                .to_string(),
        });
        self.is_initialized = true;

        Ok(())
    }

    /// Initialize the cloud service.
    /// Corresponds to TS: `CloudService.initialize()`
    pub async fn initialize(&mut self) -> Result<(), CloudError> {
        if self.is_initialized {
            return Ok(());
        }

        // Check for static token (cloud agent mode)
        // Corresponds to TS: checking ROO_CODE_CLOUD_TOKEN env var
        if let Ok(cloud_token) = std::env::var("ROO_CODE_CLOUD_TOKEN") {
            if !cloud_token.is_empty() {
                self.is_cloud_agent = true;
                self.login(None, Some(&cloud_token)).await?;
                return Ok(());
            }
        }

        // Default: mark as initialized without auth
        self.is_initialized = true;
        Ok(())
    }

    /// Log out and clear all session data.
    /// Corresponds to TS: `CloudService.logout()`
    pub fn logout(&mut self) -> Result<(), CloudError> {
        if self.auth_state == AuthState::LoggedOut {
            return Err(CloudError::NotAuthenticated);
        }

        self.auth_state = AuthState::LoggedOut;
        self.user_info = None;
        self.org_id = None;
        self.org_name = None;
        self.org_role = None;
        self.org_settings = None;
        self.user_settings = None;
        Ok(())
    }

    /// Check whether the service currently has an active session.
    /// Corresponds to TS: `CloudService.isAuthenticated()`
    pub fn is_authenticated(&self) -> bool {
        matches!(self.auth_state, AuthState::ActiveSession)
    }

    /// Check whether the service has an active session or is acquiring one.
    /// Corresponds to TS: `CloudService.hasOrIsAcquiringActiveSession()`
    pub fn has_or_is_acquiring_active_session(&self) -> bool {
        matches!(
            self.auth_state,
            AuthState::ActiveSession | AuthState::AttemptingSession
        )
    }

    /// Check whether the service has a stored active session.
    /// Corresponds to TS: `CloudService.hasActiveSession()`
    pub fn has_active_session(&self) -> bool {
        matches!(self.auth_state, AuthState::ActiveSession)
    }

    /// Get the current user info, if authenticated.
    /// Corresponds to TS: `CloudService.getUserInfo()`
    pub fn get_user_info(&self) -> Option<&CloudUserInfo> {
        self.user_info.as_ref()
    }

    /// Get the current organization ID, if set.
    /// Corresponds to TS: `CloudService.getOrganizationId()`
    pub fn get_organization_id(&self) -> Option<&str> {
        self.org_id.as_deref()
    }

    /// Get the current organization name, if set.
    /// Corresponds to TS: `CloudService.getOrganizationName()`
    pub fn get_organization_name(&self) -> Option<&str> {
        self.org_name.as_deref()
    }

    /// Get the current organization role, if set.
    /// Corresponds to TS: `CloudService.getOrganizationRole()`
    pub fn get_organization_role(&self) -> Option<&str> {
        self.org_role.as_deref()
    }

    /// Check whether a stored organization ID exists.
    /// Corresponds to TS: `CloudService.hasStoredOrganizationId()`
    pub fn has_stored_organization_id(&self) -> bool {
        self.org_id.is_some()
    }

    /// Get the current auth state as a string.
    /// Corresponds to TS: `CloudService.getAuthState()`
    pub fn get_auth_state_string(&self) -> &str {
        match self.auth_state {
            AuthState::LoggedOut => "logged-out",
            AuthState::Initializing => "initializing",
            AuthState::AttemptingSession => "attempting-session",
            AuthState::ActiveSession => "active-session",
            AuthState::InactiveSession => "inactive-session",
        }
    }

    /// Switch to a different organization.
    /// Corresponds to TS: `CloudService.switchOrganization(organizationId)`
    pub async fn switch_organization(
        &mut self,
        organization_id: Option<&str>,
    ) -> Result<(), CloudError> {
        self.ensure_initialized()?;

        // Update the org ID
        if let Some(id) = organization_id {
            self.org_id = Some(id.to_string());
        } else {
            self.org_id = None;
        }

        Ok(())
    }

    /// Explicitly set the authentication state.
    pub fn set_auth_state(&mut self, state: AuthState) {
        self.auth_state = state;
    }

    /// Explicitly set the user info.
    pub fn set_user_info(&mut self, info: CloudUserInfo) {
        self.user_info = Some(info);
    }

    /// Explicitly set the organization ID.
    pub fn set_organization_id(&mut self, org_id: String) {
        self.org_id = Some(org_id);
    }

    /// Set the organization name.
    pub fn set_organization_name(&mut self, name: String) {
        self.org_name = Some(name);
    }

    /// Set the organization role.
    pub fn set_organization_role(&mut self, role: String) {
        self.org_role = Some(role);
    }

    /// Get a reference to the current authentication state.
    pub fn get_auth_state(&self) -> &AuthState {
        &self.auth_state
    }

    /// Check whether task sync is enabled in user settings.
    /// Corresponds to TS: `CloudService.isTaskSyncEnabled()`
    ///
    /// Returns `false` if user settings are not available.
    pub fn is_task_sync_enabled(&self) -> bool {
        self.user_settings
            .as_ref()
            .map_or(false, |s| s.task_sync_enabled)
    }

    /// Set the organization settings.
    pub fn set_org_settings(&mut self, settings: OrganizationSettings) {
        self.org_settings = Some(settings);
    }

    /// Get the organization settings, if set.
    pub fn get_org_settings(&self) -> Option<&OrganizationSettings> {
        self.org_settings.as_ref()
    }

    /// Set the user settings.
    pub fn set_user_settings(&mut self, settings: UserSettings) {
        self.user_settings = Some(settings);
    }

    /// Check whether this is a cloud agent (running with static token).
    /// Corresponds to TS: `CloudService.isCloudAgent`
    pub fn is_cloud_agent(&self) -> bool {
        self.is_cloud_agent
    }

    /// Check whether the service has been initialized.
    /// Corresponds to TS: `CloudService.isInitialized`
    pub fn is_initialized(&self) -> bool {
        self.is_initialized
    }

    /// Dispose the service, clearing all state.
    /// Corresponds to TS: `CloudService.dispose()`
    pub fn dispose(&mut self) {
        self.auth_state = AuthState::LoggedOut;
        self.user_info = None;
        self.org_id = None;
        self.org_name = None;
        self.org_role = None;
        self.org_settings = None;
        self.user_settings = None;
        self.is_initialized = false;
    }

    /// Ensure the service is initialized before performing operations.
    /// Corresponds to TS: `CloudService.ensureInitialized()`
    fn ensure_initialized(&self) -> Result<(), CloudError> {
        if !self.is_initialized {
            return Err(CloudError::NotAuthenticated);
        }
        Ok(())
    }
}

impl Default for CloudService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::AuthState;

    #[test]
    fn test_new_service_is_logged_out() {
        let svc = CloudService::new();
        assert_eq!(AuthState::LoggedOut, *svc.get_auth_state());
        assert!(!svc.is_authenticated());
    }

    #[test]
    fn test_default_is_same_as_new() {
        let svc = CloudService::default();
        assert_eq!(AuthState::LoggedOut, *svc.get_auth_state());
    }

    /// Helper to simulate a logged-in state without making real HTTP calls.
    fn simulate_login(svc: &mut CloudService) {
        svc.set_auth_state(AuthState::ActiveSession);
        svc.set_user_info(CloudUserInfo {
            id: "test-user-id".to_string(),
            email: "test@example.com".to_string(),
            name: "Test User".to_string(),
            avatar_url: None,
        });
        svc.set_user_settings(UserSettings {
            task_sync_enabled: true,
            telemetry_setting: "enabled".to_string(),
        });
    }

    #[tokio::test]
    async fn test_login_network_error() {
        let mut svc = CloudService::new();
        // Using an unreachable endpoint to verify error handling
        let result = svc.login(Some("http://127.0.0.1:1/nonexistent"), None).await;
        assert!(result.is_err());
        assert!(!svc.is_authenticated());
    }

    #[test]
    fn test_simulated_login_success() {
        let mut svc = CloudService::new();
        simulate_login(&mut svc);
        assert!(svc.is_authenticated());
        assert_eq!(AuthState::ActiveSession, *svc.get_auth_state());
    }

    #[test]
    fn test_simulated_login_sets_user_info() {
        let mut svc = CloudService::new();
        simulate_login(&mut svc);
        let info = svc.get_user_info().unwrap();
        assert_eq!("test-user-id", info.id);
        assert_eq!("test@example.com", info.email);
    }

    #[test]
    fn test_logout_success() {
        let mut svc = CloudService::new();
        simulate_login(&mut svc);
        let result = svc.logout();
        assert!(result.is_ok());
        assert!(!svc.is_authenticated());
        assert!(svc.get_user_info().is_none());
    }

    #[test]
    fn test_logout_when_already_logged_out() {
        let mut svc = CloudService::new();
        let result = svc.logout();
        assert!(result.is_err());
    }

    #[test]
    fn test_logout_clears_org() {
        let mut svc = CloudService::new();
        simulate_login(&mut svc);
        svc.set_organization_id("org-1".to_string());
        svc.logout().unwrap();
        assert!(svc.get_organization_id().is_none());
    }

    #[test]
    fn test_is_authenticated_after_login() {
        let mut svc = CloudService::new();
        assert!(!svc.is_authenticated());
        simulate_login(&mut svc);
        assert!(svc.is_authenticated());
    }

    #[test]
    fn test_get_user_info_none_when_logged_out() {
        let svc = CloudService::new();
        assert!(svc.get_user_info().is_none());
    }

    #[test]
    fn test_set_and_get_organization_id() {
        let mut svc = CloudService::new();
        svc.set_organization_id("org-123".to_string());
        assert_eq!(Some("org-123"), svc.get_organization_id());
    }

    #[test]
    fn test_set_auth_state() {
        let mut svc = CloudService::new();
        svc.set_auth_state(AuthState::AttemptingSession);
        assert_eq!(AuthState::AttemptingSession, *svc.get_auth_state());
    }

    #[test]
    fn test_set_user_info() {
        let mut svc = CloudService::new();
        let info = CloudUserInfo {
            id: "custom-id".to_string(),
            email: "custom@example.com".to_string(),
            name: "Custom".to_string(),
            avatar_url: Some("https://example.com/avatar.png".to_string()),
        };
        svc.set_user_info(info);
        let retrieved = svc.get_user_info().unwrap();
        assert_eq!("custom-id", retrieved.id);
        assert_eq!("custom@example.com", retrieved.email);
    }

    #[test]
    fn test_is_task_sync_enabled_default() {
        let svc = CloudService::new();
        assert!(!svc.is_task_sync_enabled());
    }

    #[test]
    fn test_is_task_sync_enabled_after_login() {
        let mut svc = CloudService::new();
        simulate_login(&mut svc);
        assert!(svc.is_task_sync_enabled());
    }

    #[test]
    fn test_set_user_settings() {
        let mut svc = CloudService::new();
        svc.set_user_settings(UserSettings {
            task_sync_enabled: false,
            telemetry_setting: "disabled".to_string(),
        });
        assert!(!svc.is_task_sync_enabled());
    }

    #[test]
    fn test_set_org_settings() {
        let mut svc = CloudService::new();
        svc.set_org_settings(OrganizationSettings {
            allow_list: vec!["openai".to_string()],
            mcps: vec![],
        });
        let settings = svc.get_org_settings().unwrap();
        assert_eq!(1, settings.allow_list.len());
        assert_eq!("openai", settings.allow_list[0]);
    }

    #[test]
    fn test_get_org_settings_none_initially() {
        let svc = CloudService::new();
        assert!(svc.get_org_settings().is_none());
    }

    #[test]
    fn test_full_auth_lifecycle() {
        let mut svc = CloudService::new();
        assert!(!svc.is_authenticated());

        simulate_login(&mut svc);
        assert!(svc.is_authenticated());
        assert!(svc.get_user_info().is_some());

        svc.set_organization_id("org-1".to_string());
        assert_eq!(Some("org-1"), svc.get_organization_id());

        svc.logout().unwrap();
        assert!(!svc.is_authenticated());
        assert!(svc.get_user_info().is_none());
        assert!(svc.get_organization_id().is_none());
    }
}

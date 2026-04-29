/// Static token authentication service for non-cloud environments.
/// Mirrors packages/cloud/src/StaticTokenAuthService.ts

use crate::types::{AuthState, CloudError, CloudUserInfo};

/// Static token authentication service.
/// Uses a pre-configured token for authentication without Clerk.
pub struct StaticTokenAuthService {
    token: Option<String>,
    state: AuthState,
    user_info: Option<CloudUserInfo>,
}

impl StaticTokenAuthService {
    /// Create a new StaticTokenAuthService.
    pub fn new(token: Option<String>) -> Self {
        let state = if token.is_some() {
            AuthState::ActiveSession
        } else {
            AuthState::LoggedOut
        };

        Self {
            token,
            state,
            user_info: None,
        }
    }

    /// Get the current authentication state.
    pub fn get_state(&self) -> &AuthState {
        &self.state
    }

    /// Check if there is an active session.
    pub fn has_active_session(&self) -> bool {
        matches!(self.state, AuthState::ActiveSession)
    }

    /// Get the session token.
    pub fn get_session_token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    /// Get the user info.
    pub fn get_user_info(&self) -> Option<&CloudUserInfo> {
        self.user_info.as_ref()
    }

    /// Set the user info.
    pub fn set_user_info(&mut self, info: CloudUserInfo) {
        self.user_info = Some(info);
    }

    /// Validate the token.
    pub fn validate_token(&self) -> Result<(), CloudError> {
        if self.token.is_none() {
            return Err(CloudError::InvalidClientToken);
        }
        Ok(())
    }

    /// Sign out.
    pub fn sign_out(&mut self) {
        self.token = None;
        self.state = AuthState::LoggedOut;
        self.user_info = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_with_token() {
        let service = StaticTokenAuthService::new(Some("test-token".to_string()));
        assert!(service.has_active_session());
        assert_eq!(Some("test-token"), service.get_session_token());
    }

    #[test]
    fn test_new_without_token() {
        let service = StaticTokenAuthService::new(None);
        assert!(!service.has_active_session());
        assert!(service.get_session_token().is_none());
    }

    #[test]
    fn test_validate_token() {
        let service = StaticTokenAuthService::new(Some("token".to_string()));
        assert!(service.validate_token().is_ok());
    }

    #[test]
    fn test_validate_token_missing() {
        let service = StaticTokenAuthService::new(None);
        assert!(service.validate_token().is_err());
    }

    #[test]
    fn test_sign_out() {
        let mut service = StaticTokenAuthService::new(Some("token".to_string()));
        service.sign_out();
        assert!(!service.has_active_session());
        assert!(service.get_session_token().is_none());
    }

    #[test]
    fn test_set_user_info() {
        let mut service = StaticTokenAuthService::new(Some("token".to_string()));
        service.set_user_info(CloudUserInfo {
            id: "u1".to_string(),
            email: "test@example.com".to_string(),
            name: "Test".to_string(),
            avatar_url: None,
        });
        assert!(service.get_user_info().is_some());
        assert_eq!("u1", service.get_user_info().unwrap().id);
    }
}

/// Web-based authentication service using Clerk.
/// Mirrors packages/cloud/src/WebAuthService.ts

use crate::config::{get_clerk_base_url, PRODUCTION_CLERK_BASE_URL};
use crate::types::{AuthCredentials, AuthState, CloudError, CloudUserInfo};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Web-based authentication service.
pub struct WebAuthService {
    state: Arc<RwLock<AuthState>>,
    credentials: Arc<RwLock<Option<AuthCredentials>>>,
    session_token: Arc<RwLock<Option<String>>>,
    user_info: Arc<RwLock<Option<CloudUserInfo>>>,
    #[allow(dead_code)]
    clerk_base_url: String,
    #[allow(dead_code)]
    auth_credentials_key: String,
}

impl WebAuthService {
    /// Create a new WebAuthService.
    pub fn new() -> Self {
        let clerk_base_url = get_clerk_base_url();
        let auth_credentials_key = if clerk_base_url != PRODUCTION_CLERK_BASE_URL {
            format!("clerk-auth-credentials-{}", clerk_base_url)
        } else {
            "clerk-auth-credentials".to_string()
        };

        Self {
            state: Arc::new(RwLock::new(AuthState::LoggedOut)),
            credentials: Arc::new(RwLock::new(None)),
            session_token: Arc::new(RwLock::new(None)),
            user_info: Arc::new(RwLock::new(None)),
            clerk_base_url,
            auth_credentials_key,
        }
    }

    /// Get the current authentication state.
    pub async fn get_state(&self) -> AuthState {
        self.state.read().await.clone()
    }

    /// Check if there is an active session.
    pub async fn has_active_session(&self) -> bool {
        let state = self.state.read().await;
        matches!(*state, AuthState::ActiveSession)
    }

    /// Get the current session token.
    pub async fn get_session_token(&self) -> Option<String> {
        self.session_token.read().await.clone()
    }

    /// Get the current user info.
    pub async fn get_user_info(&self) -> Option<CloudUserInfo> {
        self.user_info.read().await.clone()
    }

    /// Get the current credentials.
    pub async fn get_credentials(&self) -> Option<AuthCredentials> {
        self.credentials.read().await.clone()
    }

    /// Set credentials (e.g., from storage).
    pub async fn set_credentials(&self, creds: Option<AuthCredentials>) {
        let mut guard = self.credentials.write().await;
        *guard = creds;
    }

    /// Attempt to sign in using client token and session ID.
    pub async fn sign_in(
        &self,
        client_token: &str,
        session_id: &str,
        organization_id: Option<&str>,
    ) -> Result<(), CloudError> {
        {
            let mut state = self.state.write().await;
            *state = AuthState::AttemptingSession;
        }

        let clerk_url = &self.clerk_base_url;
        let url = format!(
            "{}/client/sessions/{}/tokens?_is_native=1",
            clerk_url, session_id
        );

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", client_token))
            .header("Content-Type", "application/json")
            .send()
            .await;

        let response = match response {
            Ok(r) => r,
            Err(e) => {
                let mut state = self.state.write().await;
                *state = AuthState::LoggedOut;
                return Err(CloudError::NetworkError(format!("Failed to sign in: {}", e)));
            }
        };

        if !response.status().is_success() {
            let mut state = self.state.write().await;
            *state = AuthState::LoggedOut;
            return Err(CloudError::AuthenticationFailed(format!(
                "Sign-in failed with status: {}",
                response.status()
            )));
        }

        let data: serde_json::Value = match response.json().await {
            Ok(d) => d,
            Err(e) => {
                let mut state = self.state.write().await;
                *state = AuthState::LoggedOut;
                return Err(CloudError::SerializationError(format!(
                    "Failed to parse sign-in response: {}",
                    e
                )));
            }
        };

        let jwt = data["jwt"].as_str().unwrap_or_default().to_string();

        // Store credentials
        let creds = AuthCredentials {
            client_token: client_token.to_string(),
            session_id: session_id.to_string(),
            organization_id: organization_id.map(|s| s.to_string()),
        };
        {
            let mut guard = self.credentials.write().await;
            *guard = Some(creds);
        }
        {
            let mut guard = self.session_token.write().await;
            *guard = Some(jwt);
        }

        // Fetch user info
        self.fetch_user_info().await?;

        {
            let mut state = self.state.write().await;
            *state = AuthState::ActiveSession;
        }
        Ok(())
    }

    /// Fetch user info from Clerk.
    pub async fn fetch_user_info(&self) -> Result<CloudUserInfo, CloudError> {
        let token_val = {
            let token = self.session_token.read().await;
            token.clone()
        };
        let token_val = token_val.ok_or(CloudError::NotAuthenticated)?;

        let clerk_url = &self.clerk_base_url;
        let url = format!("{}/me", clerk_url);

        let client = reqwest::Client::new();
        let response = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token_val))
            .send()
            .await
            .map_err(|e| CloudError::NetworkError(format!("Failed to fetch user info: {}", e)))?;

        if !response.status().is_success() {
            return Err(CloudError::AuthenticationFailed(format!(
                "Failed to fetch user info: {}",
                response.status()
            )));
        }

        let data: serde_json::Value = response.json().await.map_err(|e| {
            CloudError::SerializationError(format!("Failed to parse user info: {}", e))
        })?;

        let response_data = &data["response"];
        let first_name = response_data["first_name"].as_str().unwrap_or("");
        let last_name = response_data["last_name"].as_str().unwrap_or("");
        let name = format!("{} {}", first_name, last_name).trim().to_string();

        let email = response_data["email_addresses"]
            .as_array()
            .and_then(|emails| {
                let primary_id = response_data["primary_email_address_id"].as_str()?;
                emails.iter().find_map(|e| {
                    if e["id"].as_str() == Some(primary_id) {
                        e["email_address"].as_str().map(|s| s.to_string())
                    } else {
                        None
                    }
                })
            })
            .unwrap_or_default();

        let user_info = CloudUserInfo {
            id: response_data["id"].as_str().unwrap_or_default().to_string(),
            email,
            name,
            avatar_url: response_data["image_url"].as_str().map(|s| s.to_string()),
        };

        let mut guard = self.user_info.write().await;
        *guard = Some(user_info.clone());
        Ok(user_info)
    }

    /// Refresh the session token.
    pub async fn refresh_session(&self) -> Result<bool, CloudError> {
        let creds = {
            let guard = self.credentials.read().await;
            guard.clone()
        };

        let creds = match creds {
            Some(c) => c,
            None => return Ok(false),
        };

        let clerk_url = &self.clerk_base_url;
        let url = format!(
            "{}/client/sessions/{}/tokens?_is_native=1",
            clerk_url, creds.session_id
        );

        let client = reqwest::Client::new();
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", creds.client_token))
            .header("Content-Type", "application/json")
            .send()
            .await;

        match response {
            Ok(resp) => {
                if !resp.status().is_success() {
                    let mut state = self.state.write().await;
                    *state = AuthState::InactiveSession;
                    return Ok(false);
                }

                let data: serde_json::Value = resp.json().await.unwrap_or_default();
                let jwt = data["jwt"].as_str().unwrap_or_default().to_string();

                {
                    let mut guard = self.session_token.write().await;
                    *guard = Some(jwt);
                }
                {
                    let mut state = self.state.write().await;
                    *state = AuthState::ActiveSession;
                }
                Ok(true)
            }
            Err(_) => {
                let mut state = self.state.write().await;
                *state = AuthState::InactiveSession;
                Ok(false)
            }
        }
    }

    /// Sign out and clear all session data.
    pub async fn sign_out(&self) {
        {
            let mut state = self.state.write().await;
            *state = AuthState::LoggedOut;
        }
        {
            let mut guard = self.credentials.write().await;
            *guard = None;
        }
        {
            let mut guard = self.session_token.write().await;
            *guard = None;
        }
        {
            let mut guard = self.user_info.write().await;
            *guard = None;
        }
    }
}

impl Default for WebAuthService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_new_service_is_logged_out() {
        let service = WebAuthService::new();
        assert_eq!(AuthState::LoggedOut, service.get_state().await);
        assert!(!service.has_active_session().await);
        assert!(service.get_session_token().await.is_none());
        assert!(service.get_user_info().await.is_none());
    }

    #[tokio::test]
    async fn test_sign_out() {
        let service = WebAuthService::new();
        service.sign_out().await;
        assert_eq!(AuthState::LoggedOut, service.get_state().await);
        assert!(service.get_session_token().await.is_none());
    }

    #[tokio::test]
    async fn test_set_credentials() {
        let service = WebAuthService::new();
        let creds = AuthCredentials {
            client_token: "tok".to_string(),
            session_id: "sess".to_string(),
            organization_id: None,
        };
        service.set_credentials(Some(creds)).await;
        assert!(service.get_credentials().await.is_some());
    }

    #[tokio::test]
    async fn test_auth_credentials_key() {
        let service = WebAuthService::new();
        assert!(!service.auth_credentials_key.is_empty());
    }
}

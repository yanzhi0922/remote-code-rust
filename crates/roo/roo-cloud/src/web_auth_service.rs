/// Web-based authentication service using Clerk.
/// Mirrors packages/cloud/src/WebAuthService.ts
use crate::config::{PRODUCTION_CLERK_BASE_URL, get_clerk_base_url};
use crate::types::{AuthCredentials, AuthState, CloudError, CloudUserInfo};
use std::sync::Arc;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::sync::RwLock;

/// Shared HTTP client reused across web auth requests.
static SHARED_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn shared_http_client() -> &'static reqwest::Client {
    SHARED_HTTP_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new())
    })
}

/// Consolidated authentication state, protected by a single RwLock.
struct WebAuthState {
    state: AuthState,
    credentials: Option<AuthCredentials>,
    session_token: Option<String>,
    user_info: Option<CloudUserInfo>,
}

impl Default for WebAuthState {
    fn default() -> Self {
        Self {
            state: AuthState::LoggedOut,
            credentials: None,
            session_token: None,
            user_info: None,
        }
    }
}

/// Web-based authentication service.
///
/// ## Concurrency note
/// This service uses four separate `RwLock` guards (`state`, `credentials`,
/// `session_token`, `user_info`).  Operations like `sign_out` are **not atomic**
/// — between clearing the state and clearing the session token, a concurrent
/// task may observe a partially-cleared session.  If atomic sign-out is needed
/// in the future, these four fields should be consolidated into a single
/// `RwLock<WebAuthState>` struct.
pub struct WebAuthService {
    inner: Arc<RwLock<WebAuthState>>,
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
            inner: Arc::new(RwLock::new(WebAuthState::default())),
            clerk_base_url,
            auth_credentials_key,
        }
    }

    /// Get the current authentication state.
    pub async fn get_state(&self) -> AuthState {
        self.inner.read().await.state.clone()
    }

    /// Check if there is an active session.
    pub async fn has_active_session(&self) -> bool {
        matches!(self.inner.read().await.state, AuthState::ActiveSession)
    }

    /// Get the current session token.
    pub async fn get_session_token(&self) -> Option<String> {
        self.inner.read().await.session_token.clone()
    }

    /// Get the current user info.
    pub async fn get_user_info(&self) -> Option<CloudUserInfo> {
        self.inner.read().await.user_info.clone()
    }

    /// Get the current credentials.
    pub async fn get_credentials(&self) -> Option<AuthCredentials> {
        self.inner.read().await.credentials.clone()
    }

    /// Set credentials (e.g., from storage).
    pub async fn set_credentials(&self, creds: Option<AuthCredentials>) {
        self.inner.write().await.credentials = creds;
    }

    /// Attempt to sign in using client token and session ID.
    pub async fn sign_in(
        &self,
        client_token: &str,
        session_id: &str,
        organization_id: Option<&str>,
    ) -> Result<(), CloudError> {
        {
            self.inner.write().await.state = AuthState::AttemptingSession;
        }

        let clerk_url = &self.clerk_base_url;
        let url = format!(
            "{}/client/sessions/{}/tokens?_is_native=1",
            clerk_url, session_id
        );

        let client = shared_http_client();
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", client_token))
            .header("Content-Type", "application/json")
            .send()
            .await;

        let response = match response {
            Ok(r) => r,
            Err(e) => {
                self.inner.write().await.state = AuthState::LoggedOut;
                return Err(CloudError::NetworkError(format!(
                    "Failed to sign in: {}",
                    e
                )));
            }
        };

        if !response.status().is_success() {
            self.inner.write().await.state = AuthState::LoggedOut;
            return Err(CloudError::AuthenticationFailed(format!(
                "Sign-in failed with status: {}",
                response.status()
            )));
        }

        let data: serde_json::Value = match response.json().await {
            Ok(d) => d,
            Err(e) => {
                self.inner.write().await.state = AuthState::LoggedOut;
                return Err(CloudError::SerializationError(format!(
                    "Failed to parse sign-in response: {}",
                    e
                )));
            }
        };

        let jwt = data["jwt"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                CloudError::SerializationError(
                    "Sign-in response missing non-empty jwt field".to_string(),
                )
            })?
            .to_string();

        // Store credentials and token atomically under a single lock
        let creds = AuthCredentials {
            client_token: client_token.to_string(),
            session_id: session_id.to_string(),
            organization_id: organization_id.map(|s| s.to_string()),
        };
        {
            let mut guard = self.inner.write().await;
            guard.credentials = Some(creds);
            guard.session_token = Some(jwt);
        }

        // Fetch user info
        self.fetch_user_info().await?;

        {
            self.inner.write().await.state = AuthState::ActiveSession;
        }
        Ok(())
    }

    /// Fetch user info from Clerk.
    pub async fn fetch_user_info(&self) -> Result<CloudUserInfo, CloudError> {
        let token_val = {
            let guard = self.inner.read().await;
            guard.session_token.clone()
        };
        let token_val = token_val.ok_or(CloudError::NotAuthenticated)?;

        let clerk_url = &self.clerk_base_url;
        let url = format!("{}/me", clerk_url);

        let client = shared_http_client();
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

        self.inner.write().await.user_info = Some(user_info.clone());
        Ok(user_info)
    }

    /// Refresh the session token.
    pub async fn refresh_session(&self) -> Result<bool, CloudError> {
        let creds = {
            let guard = self.inner.read().await;
            guard.credentials.clone()
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

        let client = shared_http_client();
        let response = client
            .post(&url)
            .header("Authorization", format!("Bearer {}", creds.client_token))
            .header("Content-Type", "application/json")
            .send()
            .await;

        match response {
            Ok(resp) => {
                if !resp.status().is_success() {
                    self.inner.write().await.state = AuthState::InactiveSession;
                    return Ok(false);
                }

                let data: serde_json::Value = resp.json().await.unwrap_or_default();
                let jwt = data["jwt"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .unwrap_or_default()
                    .to_string();

                {
                    let mut guard = self.inner.write().await;
                    guard.session_token = Some(jwt);
                    guard.state = AuthState::ActiveSession;
                }
                Ok(true)
            }
            Err(_) => {
                self.inner.write().await.state = AuthState::InactiveSession;
                Ok(false)
            }
        }
    }

    /// Sign out and clear all session data atomically under a single lock.
    pub async fn sign_out(&self) {
        let mut guard = self.inner.write().await;
        guard.state = AuthState::LoggedOut;
        guard.credentials = None;
        guard.session_token = None;
        guard.user_info = None;
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

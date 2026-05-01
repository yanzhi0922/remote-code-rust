//! Google Cloud Vertex AI OAuth2 authentication via service account credentials.
//!
//! Implements the JWT-based OAuth2 flow for Google Cloud service accounts:
//! 1. Parse service account JSON credentials (client_email, private_key, etc.)
//! 2. Create a signed JWT with RS256 algorithm
//! 3. Exchange the JWT for an access token via Google's OAuth2 token endpoint
//! 4. Cache the token until it nears expiration
//!
//! Reference: <https://developers.google.com/identity/protocols/oauth2/service-account>

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur during Vertex AI authentication.
#[derive(Debug, thiserror::Error)]
pub enum VertexAuthError {
    /// The credentials JSON is invalid or missing required fields.
    #[error("Invalid credentials: {0}")]
    InvalidCredentials(String),

    /// Failed to create or sign the JWT.
    #[error("JWT error: {0}")]
    JwtError(String),

    /// Failed to send the token request to Google's OAuth2 endpoint.
    #[error("Token request failed: {0}")]
    TokenRequestError(String),

    /// The token endpoint returned an error response.
    #[error("Token response error: {0}")]
    TokenResponseError(String),
}

// ---------------------------------------------------------------------------
// Service account credentials
// ---------------------------------------------------------------------------

/// Parsed Google Cloud service account credentials.
///
/// Expected JSON format (from the Google Cloud Console):
/// ```json
/// {
///   "type": "service_account",
///   "client_email": "...@...iam.gserviceaccount.com",
///   "private_key": "-----BEGIN PRIVATE KEY-----\n...\n-----END PRIVATE KEY-----\n",
///   "project_id": "my-project",
///   "token_uri": "https://oauth2.googleapis.com/token"
/// }
/// ```
#[derive(Debug, Clone, Deserialize)]
pub struct ServiceAccountCredentials {
    /// The service account email address (used as JWT `iss`).
    pub client_email: String,
    /// PEM-encoded RSA private key for signing the JWT.
    pub private_key: String,
    /// Google Cloud project ID.
    #[serde(default)]
    pub project_id: String,
    /// OAuth2 token endpoint URI. Defaults to Google's standard endpoint.
    #[serde(default)]
    pub token_uri: Option<String>,
}

// ---------------------------------------------------------------------------
// JWT claims
// ---------------------------------------------------------------------------

/// JWT claims for the Google OAuth2 service account flow.
///
/// See: <https://developers.google.com/identity/protocols/oauth2/service-account#authorizingrequests>
#[derive(Debug, Serialize, Deserialize)]
struct JwtClaims {
    /// Issuer — the service account email.
    iss: String,
    /// OAuth2 scope for Vertex AI.
    scope: String,
    /// Audience — the token endpoint URL.
    aud: String,
    /// Issued-at time (seconds since epoch).
    iat: u64,
    /// Expiration time (seconds since epoch).
    exp: u64,
}

// ---------------------------------------------------------------------------
// Token response
// ---------------------------------------------------------------------------

/// Response from Google's OAuth2 token endpoint.
#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    expires_in: Option<u64>,
    #[allow(dead_code)]
    token_type: String,
}

// ---------------------------------------------------------------------------
// Cached token
// ---------------------------------------------------------------------------

/// A cached OAuth2 access token with its expiration time.
#[derive(Debug, Clone)]
struct CachedToken {
    access_token: String,
    expires_at: Instant,
}

// ---------------------------------------------------------------------------
// VertexTokenProvider
// ---------------------------------------------------------------------------

/// Provider for Google Cloud OAuth2 access tokens using service account credentials.
///
/// Usage:
/// ```ignore
/// let provider = VertexTokenProvider::new(credentials_json)?;
/// let token = provider.get_access_token().await?;
/// // Use `token` as Bearer token in Authorization header
/// ```
#[derive(Debug)]
pub struct VertexTokenProvider {
    credentials: ServiceAccountCredentials,
    cached_token: Arc<Mutex<Option<CachedToken>>>,
    http_client: reqwest::Client,
}

impl VertexTokenProvider {
    /// OAuth2 scope for Vertex AI / Cloud Platform.
    const SCOPE: &'static str = "https://www.googleapis.com/auth/cloud-platform";

    /// Default Google OAuth2 token endpoint.
    const DEFAULT_TOKEN_URL: &'static str = "https://oauth2.googleapis.com/token";

    /// How many seconds before expiration to refresh the token.
    const REFRESH_MARGIN_SECS: u64 = 300; // 5 minutes

    /// JWT validity duration in seconds.
    const JWT_DURATION_SECS: u64 = 3600; // 1 hour

    /// Create a new token provider from a service account JSON string.
    ///
    /// Returns `Err(VertexAuthError::InvalidCredentials)` if the JSON cannot be
    /// parsed or required fields (`client_email`, `private_key`) are missing/empty.
    pub fn new(credentials_json: &str) -> Result<Self, VertexAuthError> {
        let credentials: ServiceAccountCredentials =
            serde_json::from_str(credentials_json).map_err(|e| {
                VertexAuthError::InvalidCredentials(format!("Failed to parse JSON: {e}"))
            })?;

        if credentials.client_email.is_empty() {
            return Err(VertexAuthError::InvalidCredentials(
                "client_email is empty".to_string(),
            ));
        }
        if credentials.private_key.is_empty() {
            return Err(VertexAuthError::InvalidCredentials(
                "private_key is empty".to_string(),
            ));
        }

        Ok(Self {
            credentials,
            cached_token: Arc::new(Mutex::new(None)),
            http_client: reqwest::Client::new(),
        })
    }

    /// Get a valid OAuth2 access token, refreshing from cache or fetching a new one.
    ///
    /// This method is safe to call concurrently — the cache is protected by a Mutex.
    pub async fn get_access_token(&self) -> Result<String, VertexAuthError> {
        // Check the cache first
        {
            let cached = self.cached_token.lock().await;
            if let Some(token) = cached.as_ref() {
                if token.expires_at > Instant::now() + Duration::from_secs(Self::REFRESH_MARGIN_SECS)
                {
                    return Ok(token.access_token.clone());
                }
            }
        }

        // Cache miss or expired — fetch a new token
        let new_token = self.fetch_new_token().await?;
        let access_token = new_token.access_token.clone();

        *self.cached_token.lock().await = Some(new_token);

        Ok(access_token)
    }

    /// Fetch a fresh access token from Google's OAuth2 endpoint.
    async fn fetch_new_token(&self) -> Result<CachedToken, VertexAuthError> {
        // Build JWT
        let jwt = self.create_jwt()?;

        // Determine token endpoint
        let token_url = self
            .credentials
            .token_uri
            .as_deref()
            .unwrap_or(Self::DEFAULT_TOKEN_URL);

        // Exchange JWT for access token
        let response = self
            .http_client
            .post(token_url)
            .form(&[
                ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
                ("assertion", jwt.as_str()),
            ])
            .send()
            .await
            .map_err(|e| VertexAuthError::TokenRequestError(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(VertexAuthError::TokenResponseError(format!(
                "HTTP {status}: {body}"
            )));
        }

        let token_response: TokenResponse = response
            .json()
            .await
            .map_err(|e| VertexAuthError::TokenResponseError(format!(
                "Failed to parse token response: {e}"
            )))?;

        let expires_in = token_response.expires_in.unwrap_or(Self::JWT_DURATION_SECS);

        Ok(CachedToken {
            access_token: token_response.access_token,
            expires_at: Instant::now() + Duration::from_secs(expires_in),
        })
    }

    /// Create a signed JWT for the OAuth2 service account flow.
    fn create_jwt(&self) -> Result<String, VertexAuthError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let token_uri = self
            .credentials
            .token_uri
            .as_deref()
            .unwrap_or(Self::DEFAULT_TOKEN_URL);

        let claims = JwtClaims {
            iss: self.credentials.client_email.clone(),
            scope: Self::SCOPE.to_string(),
            aud: token_uri.to_string(),
            iat: now,
            exp: now + Self::JWT_DURATION_SECS,
        };

        let header = jsonwebtoken::Header::new(jsonwebtoken::Algorithm::RS256);
        let encoding_key = jsonwebtoken::EncodingKey::from_rsa_pem(
            self.credentials.private_key.as_bytes(),
        )
        .map_err(|e| VertexAuthError::JwtError(format!("Invalid private key: {e}")))?;

        jsonwebtoken::encode(&header, &claims, &encoding_key)
            .map_err(|e| VertexAuthError::JwtError(format!("JWT encoding failed: {e}")))
    }

    /// Get the project ID from the service account credentials.
    pub fn project_id(&self) -> &str {
        &self.credentials.project_id
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_credentials() {
        let json = r#"{
            "type": "service_account",
            "client_email": "test@project.iam.gserviceaccount.com",
            "private_key": "-----BEGIN PRIVATE KEY-----\nMIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQC7VJTUt9Us8cKj\n-----END PRIVATE KEY-----\n",
            "project_id": "my-project",
            "token_uri": "https://oauth2.googleapis.com/token"
        }"#;
        let provider = VertexTokenProvider::new(json);
        assert!(provider.is_ok());
        let provider = provider.unwrap();
        assert_eq!(provider.project_id(), "my-project");
    }

    #[test]
    fn test_parse_invalid_json() {
        let json = "not json";
        let provider = VertexTokenProvider::new(json);
        assert!(provider.is_err());
        match provider.unwrap_err() {
            VertexAuthError::InvalidCredentials(msg) => {
                assert!(msg.contains("Failed to parse JSON"));
            }
            _ => panic!("Expected InvalidCredentials error"),
        }
    }

    #[test]
    fn test_parse_empty_client_email() {
        let json = r#"{
            "client_email": "",
            "private_key": "some-key",
            "project_id": "test"
        }"#;
        let provider = VertexTokenProvider::new(json);
        assert!(provider.is_err());
        match provider.unwrap_err() {
            VertexAuthError::InvalidCredentials(msg) => {
                assert!(msg.contains("client_email"));
            }
            _ => panic!("Expected InvalidCredentials error"),
        }
    }

    #[test]
    fn test_parse_empty_private_key() {
        let json = r#"{
            "client_email": "test@test.com",
            "private_key": "",
            "project_id": "test"
        }"#;
        let provider = VertexTokenProvider::new(json);
        assert!(provider.is_err());
        match provider.unwrap_err() {
            VertexAuthError::InvalidCredentials(msg) => {
                assert!(msg.contains("private_key"));
            }
            _ => panic!("Expected InvalidCredentials error"),
        }
    }

    #[test]
    fn test_parse_missing_optional_fields() {
        let json = r#"{
            "client_email": "test@project.iam.gserviceaccount.com",
            "private_key": "some-key-data"
        }"#;
        let provider = VertexTokenProvider::new(json);
        assert!(provider.is_ok());
        let provider = provider.unwrap();
        assert_eq!(provider.project_id(), ""); // default empty string
        assert!(provider.credentials.token_uri.is_none());
    }

    #[test]
    fn test_non_json_string_fails_gracefully() {
        // A plain access token string should fail to parse as service account
        let plain_token = "ya29.a0AfH6SMBx...";
        let provider = VertexTokenProvider::new(plain_token);
        assert!(provider.is_err());
    }

    #[test]
    fn test_scope_constant() {
        assert_eq!(
            VertexTokenProvider::SCOPE,
            "https://www.googleapis.com/auth/cloud-platform"
        );
    }

    #[test]
    fn test_default_token_url() {
        assert_eq!(
            VertexTokenProvider::DEFAULT_TOKEN_URL,
            "https://oauth2.googleapis.com/token"
        );
    }
}

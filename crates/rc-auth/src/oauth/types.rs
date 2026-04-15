//! OAuth type definitions.
//!
//! Mirrors the TypeScript types from `services/oauth/types.ts` and the
//! token-exchange response shape used by the Anthropic OAuth endpoints.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// OAuth token set returned by the token endpoint or stored locally.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthTokens {
    /// Bearer token for API calls.
    pub access_token: String,
    /// Long-lived token used to obtain fresh access tokens.
    pub refresh_token: Option<String>,
    /// Unix-millis epoch at which the access token expires.
    pub expires_at: Option<i64>,
    /// Space-separated OAuth scopes granted.
    pub scope: Option<String>,
    /// Subscription tier resolved from the profile endpoint.
    pub subscription_type: Option<String>,
    /// Rate-limit tier from the profile endpoint.
    pub rate_limit_tier: Option<String>,
}

impl OAuthTokens {
    /// Returns `true` when the access token is within 5 minutes of expiry
    /// (or already expired).
    pub fn is_expired(&self) -> bool {
        match self.expires_at {
            Some(expires_at) => {
                let buffer_ms = 5 * 60 * 1000;
                chrono::Utc::now().timestamp_millis() + buffer_ms >= expires_at
            }
            None => false,
        }
    }

    /// Parse the scope string into individual scope tokens.
    pub fn scopes(&self) -> Vec<String> {
        self.scope
            .as_deref()
            .map(|s| s.split(' ').map(str::to_owned).collect::<Vec<_>>())
            .expect("scope string is always valid UTF-8")
    }
}

/// Response body from the OAuth token exchange endpoint.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OAuthTokenExchangeResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    pub expires_in: u64,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub token_type: Option<String>,
    #[serde(default)]
    pub account: Option<TokenAccount>,
    #[serde(default)]
    pub organization: Option<TokenOrganization>,
}

/// Account info embedded in a token-exchange response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TokenAccount {
    pub uuid: String,
    pub email_address: String,
}

/// Organization info embedded in a token-exchange response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct TokenOrganization {
    pub uuid: String,
}

/// OAuth profile response from `/api/oauth/profile`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OAuthProfileResponse {
    #[serde(default)]
    pub account: Option<OAuthProfileAccount>,
    #[serde(default)]
    pub organization: Option<OAuthProfileOrganization>,
}

/// Account section of the profile response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OAuthProfileAccount {
    pub uuid: String,
    pub email: String,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub created_at: Option<String>,
}

/// Organization section of the profile response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct OAuthProfileOrganization {
    pub uuid: String,
    #[serde(default)]
    pub organization_type: Option<String>,
    #[serde(default)]
    pub rate_limit_tier: Option<String>,
    #[serde(default)]
    pub has_extra_usage_enabled: Option<bool>,
    #[serde(default)]
    pub billing_type: Option<String>,
    #[serde(default)]
    pub subscription_created_at: Option<String>,
}

/// Configuration for an OAuth client (endpoints, client ID, etc.).
#[derive(Debug, Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub authorize_url: String,
    pub console_authorize_url: String,
    pub token_url: String,
    pub manual_redirect_url: String,
    pub claudeai_success_url: String,
    pub console_success_url: String,
    pub profile_url: String,
    pub api_key_url: String,
    pub roles_url: String,
}

impl Default for OAuthConfig {
    fn default() -> Self {
        Self {
            client_id: String::new(),
            authorize_url: String::new(),
            console_authorize_url: String::new(),
            token_url: String::new(),
            manual_redirect_url: String::new(),
            claudeai_success_url: String::new(),
            console_success_url: String::new(),
            profile_url: String::new(),
            api_key_url: String::new(),
            roles_url: String::new(),
        }
    }
}

/// Parameters for building the authorization URL.
#[derive(Debug, Clone)]
pub struct BuildAuthUrlParams {
    pub code_challenge: String,
    pub state: String,
    pub port: u16,
    pub is_manual: bool,
    pub login_with_claude_ai: bool,
    pub inference_only: bool,
    pub org_uuid: Option<String>,
    pub login_hint: Option<String>,
    pub login_method: Option<String>,
}

/// Result of a completed OAuth flow.
#[derive(Debug, Clone)]
pub struct OAuthFlowResult {
    pub tokens: OAuthTokens,
    pub profile: Option<OAuthProfileResponse>,
    pub token_account: Option<TokenAccountInfo>,
}

/// Token-derived account info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenAccountInfo {
    pub uuid: String,
    pub email_address: String,
    pub organization_uuid: Option<String>,
}

/// Timestamp helper — convert `DateTime<Utc>` to epoch millis.
pub fn datetime_to_epoch_millis(dt: &DateTime<Utc>) -> i64 {
    dt.timestamp_millis()
}

//! Claude.ai proxy server support.
//!
//! Provides types for building authenticated requests to a Claude.ai
//! proxy endpoint. The proxy acts as an intermediary for MCP server
//! communication, forwarding requests with proper authentication headers.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::error::McpRuntimeError;

// ── Proxy configuration ─────────────────────────────────────────────────────

/// Configuration for connecting to a Claude.ai proxy server.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ClaudeAiProxyConfig {
    /// Proxy server URL.
    pub url: String,
    /// Proxy identifier (used in request paths).
    pub id: String,
    /// OAuth access token for authentication.
    #[serde(default)]
    pub access_token: Option<String>,
}

// ── Proxy request ───────────────────────────────────────────────────────────

/// An HTTP request prepared for the Claude.ai proxy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyRequest {
    /// Target URL.
    pub url: String,
    /// HTTP method (GET, POST, etc.).
    pub method: String,
    /// HTTP headers including authentication.
    pub headers: HashMap<String, String>,
    /// Optional request body as JSON.
    #[serde(default)]
    pub body: Option<serde_json::Value>,
}

// ── Proxy fetch ─────────────────────────────────────────────────────────────

/// HTTP fetch wrapper for Claude.ai proxy with authentication.
///
/// Builds authenticated requests that include the access token in
/// the `Authorization` header and the proxy identifier in the URL path.
#[derive(Debug, Clone)]
pub struct ClaudeAiProxyFetch {
    proxy_config: ClaudeAiProxyConfig,
}

impl ClaudeAiProxyFetch {
    /// Create a new proxy fetch wrapper.
    #[must_use]
    pub fn new(config: ClaudeAiProxyConfig) -> Self {
        Self {
            proxy_config: config,
        }
    }

    /// Get a reference to the proxy configuration.
    #[must_use]
    pub fn config(&self) -> &ClaudeAiProxyConfig {
        &self.proxy_config
    }

    /// Build an authenticated request for the proxy.
    ///
    /// Constructs a [`ProxyRequest`] with:
    /// - URL: `<proxy_url>/proxy/<id>/<path>`
    /// - Authorization header: `Bearer <access_token>`
    /// - Content-Type: `application/json`
    pub fn build_authenticated_request(
        &self,
        path: &str,
        method: &str,
        body: Option<serde_json::Value>,
    ) -> Result<ProxyRequest, McpRuntimeError> {
        let access_token =
            self.proxy_config
                .access_token
                .as_deref()
                .ok_or_else(|| McpRuntimeError::Proxy {
                    message: "no access token configured for proxy".to_owned(),
                })?;

        let url = format!(
            "{}/proxy/{}/{}",
            self.proxy_config.url, self.proxy_config.id, path
        );

        let mut headers = HashMap::new();
        headers.insert("Authorization".to_owned(), format!("Bearer {access_token}"));
        headers.insert("Content-Type".to_owned(), "application/json".to_owned());

        Ok(ProxyRequest {
            url,
            method: method.to_owned(),
            headers,
            body,
        })
    }

    /// Update the access token (e.g., after a refresh).
    pub fn set_access_token(&mut self, token: String) {
        self.proxy_config.access_token = Some(token);
    }

    /// Clear the access token.
    pub fn clear_access_token(&mut self) {
        self.proxy_config.access_token = None;
    }

    /// Check if an access token is configured.
    #[must_use]
    pub fn has_token(&self) -> bool {
        self.proxy_config.access_token.is_some()
    }

    /// Refresh the access token.
    ///
    /// This is a placeholder for the actual token refresh logic,
    /// which would typically involve calling an OAuth endpoint.
    pub async fn refresh_token(&mut self) -> Result<(), McpRuntimeError> {
        // In a real implementation, this would:
        // 1. Use a refresh token to get a new access token
        // 2. Update self.proxy_config.access_token
        // For now, return an error indicating the feature needs configuration
        Err(McpRuntimeError::Proxy {
            message: "token refresh not configured".to_owned(),
        })
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> ClaudeAiProxyConfig {
        ClaudeAiProxyConfig {
            url: "https://api.claude.ai".to_owned(),
            id: "proxy-123".to_owned(),
            access_token: Some("test-token-abc".to_owned()),
        }
    }

    #[test]
    fn proxy_config_serde_roundtrip() {
        let config = test_config();
        let json = serde_json::to_string(&config).expect("serialize");
        let back: ClaudeAiProxyConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, config);
    }

    #[test]
    fn proxy_config_deserializes_without_token() {
        let json = r#"{"url":"https://api.claude.ai","id":"proxy-1"}"#;
        let config: ClaudeAiProxyConfig = serde_json::from_str(json).expect("deserialize");
        assert_eq!(config.url, "https://api.claude.ai");
        assert_eq!(config.id, "proxy-1");
        assert!(config.access_token.is_none());
    }

    #[test]
    fn proxy_fetch_new() {
        let fetch = ClaudeAiProxyFetch::new(test_config());
        assert!(fetch.has_token());
        assert_eq!(fetch.config().id, "proxy-123");
    }

    #[test]
    fn build_authenticated_request_get() {
        let fetch = ClaudeAiProxyFetch::new(test_config());
        let req = fetch
            .build_authenticated_request("tools/list", "GET", None)
            .expect("build request");

        assert_eq!(req.method, "GET");
        assert_eq!(req.url, "https://api.claude.ai/proxy/proxy-123/tools/list");
        assert_eq!(
            req.headers.get("Authorization").map(|s| s.as_str()),
            Some("Bearer test-token-abc")
        );
        assert_eq!(
            req.headers.get("Content-Type").map(|s| s.as_str()),
            Some("application/json")
        );
        assert!(req.body.is_none());
    }

    #[test]
    fn build_authenticated_request_post_with_body() {
        let fetch = ClaudeAiProxyFetch::new(test_config());
        let body = serde_json::json!({"tool": "search", "query": "test"});
        let req = fetch
            .build_authenticated_request("tools/call", "POST", Some(body.clone()))
            .expect("build request");

        assert_eq!(req.method, "POST");
        assert_eq!(req.body.as_ref(), Some(&body));
    }

    #[test]
    fn build_request_fails_without_token() {
        let config = ClaudeAiProxyConfig {
            url: "https://api.claude.ai".to_owned(),
            id: "proxy-1".to_owned(),
            access_token: None,
        };
        let fetch = ClaudeAiProxyFetch::new(config);
        let result = fetch.build_authenticated_request("test", "GET", None);
        assert!(result.is_err());
    }

    #[test]
    fn set_and_clear_access_token() {
        let mut fetch = ClaudeAiProxyFetch::new(ClaudeAiProxyConfig {
            url: "https://api.claude.ai".to_owned(),
            id: "p1".to_owned(),
            access_token: None,
        });
        assert!(!fetch.has_token());

        fetch.set_access_token("new-token".to_owned());
        assert!(fetch.has_token());

        fetch.clear_access_token();
        assert!(!fetch.has_token());
    }

    #[test]
    fn proxy_request_serde_roundtrip() {
        let req = ProxyRequest {
            url: "https://example.com/test".to_owned(),
            method: "POST".to_owned(),
            headers: {
                let mut h = HashMap::new();
                h.insert("Authorization".to_owned(), "Bearer tok".to_owned());
                h
            },
            body: Some(serde_json::json!({"key": "value"})),
        };
        let json = serde_json::to_string(&req).expect("serialize");
        let back: ProxyRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.url, req.url);
        assert_eq!(back.method, req.method);
        assert_eq!(back.headers, req.headers);
    }
}

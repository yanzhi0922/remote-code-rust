//! Network Proxy Configuration
//!
//! Provides proxy configuration for all outbound HTTP/HTTPS requests.
//! Supports HTTP, HTTPS, and SOCKS proxies with optional TLS verification bypass.
//!
//! Source: `.research/Roo-Code/src/utils/networkProxy.ts`

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Proxy configuration
// ---------------------------------------------------------------------------

/// Proxy configuration state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    /// Whether the debug proxy is enabled.
    pub enabled: bool,
    /// The proxy server URL (e.g., `http://127.0.0.1:8888`).
    pub server_url: String,
    /// Accept self-signed/insecure TLS certificates from the proxy.
    pub tls_insecure: bool,
    /// Whether running in debug/development mode.
    pub is_debug_mode: bool,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            server_url: "http://127.0.0.1:8888".to_string(),
            tls_insecure: false,
            is_debug_mode: false,
        }
    }
}

/// Supported proxy protocols.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProxyProtocol {
    Http,
    Https,
    Socks5,
}

// ---------------------------------------------------------------------------
// Network proxy manager
// ---------------------------------------------------------------------------

/// Manages network proxy configuration for outbound requests.
///
/// Source: `.research/Roo-Code/src/utils/networkProxy.ts`
pub struct NetworkProxy {
    config: ProxyConfig,
}

impl NetworkProxy {
    /// Create a new `NetworkProxy` with the given configuration.
    pub fn new(config: ProxyConfig) -> Self {
        Self { config }
    }

    /// Create with default configuration.
    pub fn with_defaults() -> Self {
        Self {
            config: ProxyConfig::default(),
        }
    }

    /// Get the current proxy configuration.
    pub fn config(&self) -> &ProxyConfig {
        &self.config
    }

    /// Update the proxy configuration.
    pub fn set_config(&mut self, config: ProxyConfig) {
        self.config = config;
    }

    /// Check if proxy is enabled and should be used.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled && self.config.is_debug_mode
    }

    /// Detect the proxy protocol from the server URL.
    pub fn detect_protocol(&self) -> ProxyProtocol {
        let url = self.config.server_url.to_lowercase();
        if url.starts_with("socks5://") || url.starts_with("socks://") {
            ProxyProtocol::Socks5
        } else if url.starts_with("https://") {
            ProxyProtocol::Https
        } else {
            ProxyProtocol::Http
        }
    }

    /// Get the proxy URL for reqwest.
    pub fn proxy_url(&self) -> Option<String> {
        if self.is_enabled() {
            Some(self.config.server_url.clone())
        } else {
            None
        }
    }

    /// Build a reqwest client with the proxy configured.
    pub fn build_client(&self) -> Result<reqwest::Client, reqwest::Error> {
        let mut builder = reqwest::Client::builder();

        if let Some(proxy_url) = self.proxy_url() {
            match self.detect_protocol() {
                ProxyProtocol::Http | ProxyProtocol::Https => {
                    let proxy = reqwest::Proxy::all(&proxy_url)?;
                    builder = builder.proxy(proxy);
                }
                ProxyProtocol::Socks5 => {
                    let proxy = reqwest::Proxy::all(&proxy_url)?;
                    builder = builder.proxy(proxy);
                }
            }

            if self.config.tls_insecure {
                builder = builder.danger_accept_invalid_certs(true);
            }
        }

        builder.build()
    }
}

/// Redact credentials from a proxy URL for safe logging.
pub fn redact_proxy_url(proxy_url: &str) -> String {
    match url::Url::parse(proxy_url) {
        Ok(mut parsed) => {
            let _ = parsed.set_username("");
            let _ = parsed.set_password(None);
            parsed.to_string()
        }
        Err(_) => {
            // Fallback: redact basic auth if present
            regex::Regex::new(r"//[^@/]+@")
                .and_then(|re| Ok(re.replace(proxy_url, "//REDACTED@").to_string()))
                .unwrap_or_else(|_| "(invalid url)".to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_proxy_config() {
        let config = ProxyConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.server_url, "http://127.0.0.1:8888");
        assert!(!config.tls_insecure);
        assert!(!config.is_debug_mode);
    }

    #[test]
    fn test_proxy_disabled_by_default() {
        let proxy = NetworkProxy::with_defaults();
        assert!(!proxy.is_enabled());
        assert!(proxy.proxy_url().is_none());
    }

    #[test]
    fn test_proxy_enabled_in_debug_mode() {
        let config = ProxyConfig {
            enabled: true,
            server_url: "http://127.0.0.1:8888".to_string(),
            tls_insecure: false,
            is_debug_mode: true,
        };
        let proxy = NetworkProxy::new(config);
        assert!(proxy.is_enabled());
        assert_eq!(proxy.proxy_url(), Some("http://127.0.0.1:8888".to_string()));
    }

    #[test]
    fn test_detect_protocol_http() {
        let config = ProxyConfig {
            server_url: "http://proxy.example.com:8080".to_string(),
            ..Default::default()
        };
        let proxy = NetworkProxy::new(config);
        assert_eq!(proxy.detect_protocol(), ProxyProtocol::Http);
    }

    #[test]
    fn test_detect_protocol_https() {
        let config = ProxyConfig {
            server_url: "https://proxy.example.com:8080".to_string(),
            ..Default::default()
        };
        let proxy = NetworkProxy::new(config);
        assert_eq!(proxy.detect_protocol(), ProxyProtocol::Https);
    }

    #[test]
    fn test_detect_protocol_socks5() {
        let config = ProxyConfig {
            server_url: "socks5://proxy.example.com:1080".to_string(),
            ..Default::default()
        };
        let proxy = NetworkProxy::new(config);
        assert_eq!(proxy.detect_protocol(), ProxyProtocol::Socks5);
    }

    #[test]
    fn test_redact_proxy_url_no_auth() {
        let result = redact_proxy_url("http://127.0.0.1:8888");
        assert_eq!(result, "http://127.0.0.1:8888/");
    }

    #[test]
    fn test_redact_proxy_url_with_auth() {
        let result = redact_proxy_url("http://user:pass@proxy.example.com:8080");
        assert!(result.contains("REDACTED") || !result.contains("user:pass"));
    }
}

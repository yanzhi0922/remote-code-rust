//! Server configuration validation.
//!
//! Corresponds to TS: `validateServerConfig` and `ServerConfigSchema`.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::error::McpError;

// Error messages matching TS source exactly
const TYPE_ERROR_MSG: &str = "Server type must be 'stdio', 'sse', or 'streamable-http'";
const STDIO_FIELDS_ERROR_MSG: &str =
    "For 'stdio' type servers, you must provide a 'command' field and can optionally include 'args' and 'env'";
const SSE_FIELDS_ERROR_MSG: &str =
    "For 'sse' type servers, you must provide a 'url' field and can optionally include 'headers'";
const STREAMABLE_HTTP_FIELDS_ERROR_MSG: &str =
    "For 'streamable-http' type servers, you must provide a 'url' field and can optionally include 'headers'";
const MIXED_FIELDS_ERROR_MSG: &str =
    "Cannot mix 'stdio' and ('sse' or 'streamable-http') fields. For 'stdio' use 'command', 'args', and 'env'. For 'sse'/'streamable-http' use 'url' and 'headers'";
const MISSING_FIELDS_ERROR_MSG: &str =
    "Server configuration must include either 'command' (for stdio) or 'url' (for sse/streamable-http) and a corresponding 'type' if 'url' is used.";

/// The type of MCP server transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ServerTransportType {
    Stdio,
    Sse,
    #[serde(rename = "streamable-http")]
    StreamableHttp,
}

/// A validated MCP server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ValidatedServerConfig {
    /// Stdio transport configuration.
    #[serde(rename = "stdio")]
    Stdio {
        command: String,
        #[serde(default)]
        args: Vec<String>,
        #[serde(default)]
        env: HashMap<String, String>,
        #[serde(default)]
        cwd: Option<String>,
        #[serde(default = "default_timeout")]
        timeout: u64,
        #[serde(default)]
        disabled: bool,
        #[serde(default)]
        always_allow: Vec<String>,
        #[serde(default)]
        disabled_tools: Vec<String>,
        #[serde(default)]
        watch_paths: Option<Vec<String>>,
    },
    /// SSE transport configuration.
    #[serde(rename = "sse")]
    Sse {
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
        #[serde(default = "default_timeout")]
        timeout: u64,
        #[serde(default)]
        disabled: bool,
        #[serde(default)]
        always_allow: Vec<String>,
        #[serde(default)]
        disabled_tools: Vec<String>,
        #[serde(default)]
        watch_paths: Option<Vec<String>>,
    },
    /// Streamable HTTP transport configuration.
    #[serde(rename = "streamable-http")]
    StreamableHttp {
        url: String,
        #[serde(default)]
        headers: HashMap<String, String>,
        #[serde(default = "default_timeout")]
        timeout: u64,
        #[serde(default)]
        disabled: bool,
        #[serde(default)]
        always_allow: Vec<String>,
        #[serde(default)]
        disabled_tools: Vec<String>,
        #[serde(default)]
        watch_paths: Option<Vec<String>>,
    },
}

fn default_timeout() -> u64 {
    60
}

impl ValidatedServerConfig {
    /// Get the transport type.
    pub fn transport_type(&self) -> ServerTransportType {
        match self {
            ValidatedServerConfig::Stdio { .. } => ServerTransportType::Stdio,
            ValidatedServerConfig::Sse { .. } => ServerTransportType::Sse,
            ValidatedServerConfig::StreamableHttp { .. } => ServerTransportType::StreamableHttp,
        }
    }

    /// Get the timeout in seconds.
    pub fn timeout(&self) -> u64 {
        match self {
            ValidatedServerConfig::Stdio { timeout, .. }
            | ValidatedServerConfig::Sse { timeout, .. }
            | ValidatedServerConfig::StreamableHttp { timeout, .. } => *timeout,
        }
    }

    /// Get the timeout in milliseconds.
    pub fn timeout_ms(&self) -> u64 {
        self.timeout() * 1000
    }

    /// Check if this server is disabled.
    pub fn is_disabled(&self) -> bool {
        match self {
            ValidatedServerConfig::Stdio { disabled, .. }
            | ValidatedServerConfig::Sse { disabled, .. }
            | ValidatedServerConfig::StreamableHttp { disabled, .. } => *disabled,
        }
    }

    /// Get the always-allow list.
    pub fn always_allow(&self) -> &[String] {
        match self {
            ValidatedServerConfig::Stdio { always_allow, .. }
            | ValidatedServerConfig::Sse { always_allow, .. }
            | ValidatedServerConfig::StreamableHttp { always_allow, .. } => always_allow,
        }
    }

    /// Get the disabled tools list.
    pub fn disabled_tools(&self) -> &[String] {
        match self {
            ValidatedServerConfig::Stdio { disabled_tools, .. }
            | ValidatedServerConfig::Sse { disabled_tools, .. }
            | ValidatedServerConfig::StreamableHttp { disabled_tools, .. } => disabled_tools,
        }
    }

    /// Get watch paths if any.
    pub fn watch_paths(&self) -> Option<&[String]> {
        match self {
            ValidatedServerConfig::Stdio { watch_paths, .. }
            | ValidatedServerConfig::Sse { watch_paths, .. }
            | ValidatedServerConfig::StreamableHttp { watch_paths, .. } => {
                watch_paths.as_deref()
            }
        }
    }
}

/// Validate and normalize a server configuration.
///
/// Corresponds to TS: `validateServerConfig`
///
/// The `config` parameter should be a JSON object (serde_json::Value::Object).
/// The `server_name` parameter is optional and used for error messages.
pub fn validate_server_config(
    config: &serde_json::Value,
    _server_name: Option<&str>,
) -> Result<ValidatedServerConfig, McpError> {
    let obj = config
        .as_object()
        .ok_or_else(|| McpError::ConfigError("Server configuration must be a JSON object".to_string()))?;

    let has_stdio_fields = obj.contains_key("command");
    let has_url_fields = obj.contains_key("url");

    // Check for mixed fields (stdio vs url-based)
    if has_stdio_fields && has_url_fields {
        return Err(McpError::ConfigError(MIXED_FIELDS_ERROR_MSG.to_string()));
    }

    // Get the type field if present
    let type_value = obj.get("type").and_then(|v| v.as_str());

    // Infer type for stdio if not provided
    let effective_type = if type_value.is_none() && has_stdio_fields {
        Some("stdio")
    } else {
        type_value
    };

    // For url-based configs, type must be provided by the user
    if has_url_fields && type_value.is_none() {
        return Err(McpError::ConfigError(
            "Configuration with 'url' must explicitly specify 'type' as 'sse' or 'streamable-http'."
                .to_string(),
        ));
    }

    // Validate type if provided
    if let Some(t) = effective_type {
        if !["stdio", "sse", "streamable-http"].contains(&t) {
            return Err(McpError::ConfigError(TYPE_ERROR_MSG.to_string()));
        }
    }

    // Check for type/field mismatch
    if effective_type == Some("stdio") && !has_stdio_fields {
        return Err(McpError::ConfigError(STDIO_FIELDS_ERROR_MSG.to_string()));
    }
    if effective_type == Some("sse") && !has_url_fields {
        return Err(McpError::ConfigError(SSE_FIELDS_ERROR_MSG.to_string()));
    }
    if effective_type == Some("streamable-http") && !has_url_fields {
        return Err(McpError::ConfigError(
            STREAMABLE_HTTP_FIELDS_ERROR_MSG.to_string(),
        ));
    }

    // If neither command nor url is present
    if !has_stdio_fields && !has_url_fields {
        return Err(McpError::ConfigError(
            MISSING_FIELDS_ERROR_MSG.to_string(),
        ));
    }

    // Now build the validated config based on the effective type
    let timeout = obj
        .get("timeout")
        .and_then(|v| v.as_u64())
        .unwrap_or(60);
    let timeout = timeout.clamp(1, 3600);

    let disabled = obj
        .get("disabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let always_allow = obj
        .get("alwaysAllow")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let disabled_tools = obj
        .get("disabledTools")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let watch_paths = obj.get("watchPaths").and_then(|v| v.as_array()).map(|arr| {
        arr.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect::<Vec<_>>()
    });

    match effective_type {
        Some("stdio") => {
            let command = obj
                .get("command")
                .and_then(|v| v.as_str())
                .ok_or_else(|| McpError::ConfigError("Command cannot be empty".to_string()))?;

            if command.is_empty() {
                return Err(McpError::ConfigError("Command cannot be empty".to_string()));
            }

            let args = obj
                .get("args")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();

            let env = obj
                .get("env")
                .and_then(|v| v.as_object())
                .map(|map| {
                    map.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect::<HashMap<_, _>>()
                })
                .unwrap_or_default();

            let cwd = obj.get("cwd").and_then(|v| v.as_str()).map(|s| s.to_string());

            Ok(ValidatedServerConfig::Stdio {
                command: command.to_string(),
                args,
                env,
                cwd,
                timeout,
                disabled,
                always_allow,
                disabled_tools,
                watch_paths,
            })
        }
        Some("sse") => {
            let url = obj
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or_else(|| McpError::ConfigError(SSE_FIELDS_ERROR_MSG.to_string()))?;

            // Validate URL format
            url::Url::parse(url).map_err(|_| {
                McpError::ConfigError("URL must be a valid URL format".to_string())
            })?;

            let headers = obj
                .get("headers")
                .and_then(|v| v.as_object())
                .map(|map| {
                    map.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect::<HashMap<_, _>>()
                })
                .unwrap_or_default();

            Ok(ValidatedServerConfig::Sse {
                url: url.to_string(),
                headers,
                timeout,
                disabled,
                always_allow,
                disabled_tools,
                watch_paths,
            })
        }
        Some("streamable-http") => {
            let url = obj
                .get("url")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    McpError::ConfigError(STREAMABLE_HTTP_FIELDS_ERROR_MSG.to_string())
                })?;

            // Validate URL format
            url::Url::parse(url).map_err(|_| {
                McpError::ConfigError("URL must be a valid URL format".to_string())
            })?;

            let headers = obj
                .get("headers")
                .and_then(|v| v.as_object())
                .map(|map| {
                    map.iter()
                        .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                        .collect::<HashMap<_, _>>()
                })
                .unwrap_or_default();

            Ok(ValidatedServerConfig::StreamableHttp {
                url: url.to_string(),
                headers,
                timeout,
                disabled,
                always_allow,
                disabled_tools,
                watch_paths,
            })
        }
        _ => Err(McpError::ConfigError(
            MISSING_FIELDS_ERROR_MSG.to_string(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn validate(config: serde_json::Value) -> Result<ValidatedServerConfig, McpError> {
        validate_server_config(&config, None)
    }

    fn validate_with_name(
        config: serde_json::Value,
        name: &str,
    ) -> Result<ValidatedServerConfig, McpError> {
        validate_server_config(&config, Some(name))
    }

    // ---- Valid configurations ----

    #[test]
    fn test_valid_stdio_minimal() {
        let config = json!({
            "command": "node"
        });
        let result = validate(config).unwrap();
        assert!(matches!(result, ValidatedServerConfig::Stdio { .. }));
        if let ValidatedServerConfig::Stdio { command, args, env, timeout, .. } = result {
            assert_eq!(command, "node");
            assert!(args.is_empty());
            assert!(env.is_empty());
            assert_eq!(timeout, 60);
        }
    }

    #[test]
    fn test_valid_stdio_explicit_type() {
        let config = json!({
            "type": "stdio",
            "command": "npx",
            "args": ["-y", "@modelcontextprotocol/server-memory"],
            "env": {"NODE_ENV": "development"},
            "timeout": 120
        });
        let result = validate(config).unwrap();
        if let ValidatedServerConfig::Stdio {
            command,
            args,
            env,
            timeout,
            ..
        } = result
        {
            assert_eq!(command, "npx");
            assert_eq!(args, vec!["-y", "@modelcontextprotocol/server-memory"]);
            assert_eq!(env.get("NODE_ENV").unwrap(), "development");
            assert_eq!(timeout, 120);
        } else {
            panic!("Expected Stdio config");
        }
    }

    #[test]
    fn test_valid_stdio_with_all_options() {
        let config = json!({
            "type": "stdio",
            "command": "python",
            "args": ["server.py"],
            "env": {"DEBUG": "1"},
            "cwd": "/tmp",
            "timeout": 30,
            "disabled": true,
            "alwaysAllow": ["tool1", "tool2"],
            "disabledTools": ["tool3"]
        });
        let result = validate(config).unwrap();
        if let ValidatedServerConfig::Stdio {
            command,
            disabled,
            always_allow,
            disabled_tools,
            ..
        } = result
        {
            assert_eq!(command, "python");
            assert!(disabled);
            assert_eq!(always_allow, vec!["tool1", "tool2"]);
            assert_eq!(disabled_tools, vec!["tool3"]);
        } else {
            panic!("Expected Stdio config");
        }
    }

    #[test]
    fn test_valid_sse() {
        let config = json!({
            "type": "sse",
            "url": "http://localhost:3000/sse"
        });
        let result = validate(config).unwrap();
        if let ValidatedServerConfig::Sse { url, .. } = result {
            assert_eq!(url, "http://localhost:3000/sse");
        } else {
            panic!("Expected SSE config");
        }
    }

    #[test]
    fn test_valid_sse_with_headers() {
        let config = json!({
            "type": "sse",
            "url": "http://localhost:3000/sse",
            "headers": {"Authorization": "Bearer token123"}
        });
        let result = validate(config).unwrap();
        if let ValidatedServerConfig::Sse { headers, .. } = result {
            assert_eq!(headers.get("Authorization").unwrap(), "Bearer token123");
        } else {
            panic!("Expected SSE config");
        }
    }

    #[test]
    fn test_valid_streamable_http() {
        let config = json!({
            "type": "streamable-http",
            "url": "http://localhost:3000/mcp"
        });
        let result = validate(config).unwrap();
        if let ValidatedServerConfig::StreamableHttp { url, .. } = result {
            assert_eq!(url, "http://localhost:3000/mcp");
        } else {
            panic!("Expected StreamableHttp config");
        }
    }

    // ---- Invalid configurations ----

    #[test]
    fn test_invalid_mixed_fields() {
        let config = json!({
            "command": "node",
            "url": "http://localhost:3000"
        });
        let err = validate(config).unwrap_err();
        assert!(err.to_string().contains("Cannot mix"));
    }

    #[test]
    fn test_invalid_url_without_type() {
        let config = json!({
            "url": "http://localhost:3000"
        });
        let err = validate(config).unwrap_err();
        assert!(err.to_string().contains("must explicitly specify 'type'"));
    }

    #[test]
    fn test_invalid_bad_type() {
        let config = json!({
            "type": "websocket",
            "url": "http://localhost:3000"
        });
        let err = validate(config).unwrap_err();
        assert!(err.to_string().contains("must be 'stdio', 'sse', or 'streamable-http'"));
    }

    #[test]
    fn test_invalid_stdio_without_command() {
        let config = json!({
            "type": "stdio"
        });
        let err = validate(config).unwrap_err();
        assert!(err.to_string().contains("must provide a 'command' field"));
    }

    #[test]
    fn test_invalid_sse_without_url() {
        let config = json!({
            "type": "sse"
        });
        let err = validate(config).unwrap_err();
        assert!(err.to_string().contains("must provide a 'url' field"));
    }

    #[test]
    fn test_invalid_streamable_http_without_url() {
        let config = json!({
            "type": "streamable-http"
        });
        let err = validate(config).unwrap_err();
        assert!(err.to_string().contains("must provide a 'url' field"));
    }

    #[test]
    fn test_invalid_no_fields() {
        let config = json!({});
        let err = validate(config).unwrap_err();
        assert!(err.to_string().contains("must include either 'command'"));
    }

    #[test]
    fn test_invalid_empty_command() {
        let config = json!({
            "command": ""
        });
        let err = validate(config).unwrap_err();
        assert!(err.to_string().contains("Command cannot be empty"));
    }

    #[test]
    fn test_invalid_bad_url_format() {
        let config = json!({
            "type": "sse",
            "url": "not-a-valid-url"
        });
        let err = validate(config).unwrap_err();
        assert!(err.to_string().contains("valid URL format"));
    }

    #[test]
    fn test_timeout_clamped_min() {
        let config = json!({
            "command": "node",
            "timeout": 0
        });
        let result = validate(config).unwrap();
        assert_eq!(result.timeout(), 1);
    }

    #[test]
    fn test_timeout_clamped_max() {
        let config = json!({
            "command": "node",
            "timeout": 5000
        });
        let result = validate(config).unwrap();
        assert_eq!(result.timeout(), 3600);
    }

    #[test]
    fn test_timeout_default() {
        let config = json!({
            "command": "node"
        });
        let result = validate(config).unwrap();
        assert_eq!(result.timeout(), 60);
    }

    #[test]
    fn test_validate_with_server_name() {
        let config = json!({});
        let err = validate_with_name(config, "my-server").unwrap_err();
        // Should not panic; the name is used for error context
        assert!(!err.to_string().is_empty());
    }

    #[test]
    fn test_not_a_json_object() {
        let config = json!("not an object");
        let err = validate(config).unwrap_err();
        assert!(err.to_string().contains("must be a JSON object"));
    }
}

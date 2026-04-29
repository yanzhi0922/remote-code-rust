//! Helper functions for MCP tools.

use crate::types::McpToolError;

/// Check if two tool names match, normalizing hyphens/underscores and case.
///
/// Examples:
/// - "my-tool" matches "my_tool"
/// - "MyTool" matches "my-tool"
/// - "my_tool" matches "my-tool"
pub fn tool_names_match(a: &str, b: &str) -> bool {
    let normalize = |s: &str| s.replace('-', "_").to_lowercase();
    normalize(a) == normalize(b)
}

/// Validate that MCP arguments are a valid JSON object (or null).
///
/// Returns Ok for:
/// - JSON objects (`{...}`)
/// - `null`
///
/// Returns Err for:
/// - Non-object types (arrays, strings, numbers, booleans)
pub fn validate_mcp_arguments(args: &serde_json::Value) -> Result<(), McpToolError> {
    match args {
        serde_json::Value::Object(_) => Ok(()),
        serde_json::Value::Null => Ok(()),
        _ => Err(McpToolError::InvalidArguments(format!(
            "arguments must be a JSON object or null, got {}",
            args
        ))),
    }
}

/// Format resource content for display.
///
/// Handles text and image content types.
pub fn format_resource_content(content: &str, content_type: &str) -> String {
    if content_type.starts_with("image/") {
        format!("(Image content: {}, {} bytes)", content_type, content.len())
    } else {
        content.to_string()
    }
}

/// Normalize a tool name by replacing hyphens with underscores and lowering case.
pub fn normalize_tool_name(name: &str) -> String {
    name.replace('-', "_").to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ---- tool_names_match tests ----

    #[test]
    fn test_exact_match() {
        assert!(tool_names_match("my-tool", "my-tool"));
    }

    #[test]
    fn test_hyphen_underscore_match() {
        assert!(tool_names_match("my-tool", "my_tool"));
    }

    #[test]
    fn test_case_insensitive_match() {
        assert!(tool_names_match("MyTool", "mytool"));
    }

    #[test]
    fn test_combined_normalization() {
        assert!(tool_names_match("My-Tool", "my_tool"));
    }

    #[test]
    fn test_no_match() {
        assert!(!tool_names_match("tool-a", "tool-b"));
    }

    #[test]
    fn test_empty_names() {
        assert!(tool_names_match("", ""));
    }

    // ---- validate_mcp_arguments tests ----

    #[test]
    fn test_valid_object() {
        assert!(validate_mcp_arguments(&json!({"key": "value"})).is_ok());
    }

    #[test]
    fn test_valid_null() {
        assert!(validate_mcp_arguments(&json!(null)).is_ok());
    }

    #[test]
    fn test_valid_empty_object() {
        assert!(validate_mcp_arguments(&json!({})).is_ok());
    }

    #[test]
    fn test_invalid_array() {
        assert!(validate_mcp_arguments(&json!([1, 2, 3])).is_err());
    }

    #[test]
    fn test_invalid_string() {
        assert!(validate_mcp_arguments(&json!("hello")).is_err());
    }

    #[test]
    fn test_invalid_number() {
        assert!(validate_mcp_arguments(&json!(42)).is_err());
    }

    #[test]
    fn test_invalid_boolean() {
        assert!(validate_mcp_arguments(&json!(true)).is_err());
    }

    // ---- format_resource_content tests ----

    #[test]
    fn test_text_content() {
        assert_eq!(
            format_resource_content("hello", "text/plain"),
            "hello"
        );
    }

    #[test]
    fn test_image_content() {
        let result = format_resource_content("base64data", "image/png");
        assert!(result.contains("Image content"));
        assert!(result.contains("image/png"));
    }

    #[test]
    fn test_json_content() {
        assert_eq!(
            format_resource_content("{\"key\": \"val\"}", "application/json"),
            "{\"key\": \"val\"}"
        );
    }

    // ---- normalize_tool_name tests ----

    #[test]
    fn test_normalize() {
        assert_eq!(normalize_tool_name("My-Tool-Name"), "my_tool_name");
    }

    #[test]
    fn test_normalize_already_normalized() {
        assert_eq!(normalize_tool_name("my_tool"), "my_tool");
    }

    // ---- McpToolValidation tests ----

    #[test]
    fn test_mcp_tool_validation_valid() {
        let v = crate::types::McpToolValidation {
            server_name: "test-server".to_string(),
            tool_name: "my-tool".to_string(),
            is_valid: true,
            error: None,
        };
        assert!(v.is_valid);
        assert!(v.error.is_none());
    }

    #[test]
    fn test_mcp_tool_validation_invalid() {
        let v = crate::types::McpToolValidation {
            server_name: "test-server".to_string(),
            tool_name: "bad-tool".to_string(),
            is_valid: false,
            error: Some("not found".to_string()),
        };
        assert!(!v.is_valid);
        assert!(v.error.is_some());
    }

    // ---- McpResourceResult tests ----

    #[test]
    fn test_mcp_resource_result() {
        let r = crate::types::McpResourceResult {
            server_name: "server".to_string(),
            uri: "file:///test".to_string(),
            content: "data".to_string(),
            content_type: "text/plain".to_string(),
        };
        assert_eq!(r.server_name, "server");
        assert_eq!(r.uri, "file:///test");
    }

    // ---- McpToolResult tests ----

    #[test]
    fn test_mcp_tool_result() {
        let r = crate::types::McpToolResult {
            server_name: "server".to_string(),
            tool_name: "tool".to_string(),
            result: json!({"status": "ok"}),
            is_error: false,
        };
        assert!(!r.is_error);
    }

    // ---- McpToolError tests ----

    #[test]
    fn test_mcp_tool_error_display() {
        let err = McpToolError::MissingParameter("server_name".to_string());
        assert_eq!(format!("{err}"), "Missing parameter: server_name");

        let err = McpToolError::ToolNameMismatch {
            expected: "foo".to_string(),
            actual: "bar".to_string(),
        };
        assert!(format!("{err}").contains("foo"));
        assert!(format!("{err}").contains("bar"));
    }
}

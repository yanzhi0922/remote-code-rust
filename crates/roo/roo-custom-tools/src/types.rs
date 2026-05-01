use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The type of handler for a custom tool.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandlerType {
    /// A built-in handler.
    Builtin,
    /// A script-based handler (e.g., shell, python).
    Script,
    /// An HTTP-based handler.
    Http,
}

/// Definition of a custom tool that can be registered with the tool registry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomToolDefinition {
    /// The unique name of the tool.
    pub name: String,
    /// A human-readable description of what the tool does.
    pub description: String,
    /// JSON Schema describing the tool's parameters.
    pub parameters_schema: CustomToolParametersSchema,
    /// The type of handler for this tool.
    pub handler_type: HandlerType,
}

/// The parameters schema for a custom tool.
/// Can be any JSON value (typically a JSON Schema object).
pub type CustomToolParametersSchema = Value;

/// Result of loading tools from a directory.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoadResult {
    /// Names of tools that were successfully loaded.
    pub loaded: Vec<String>,
    /// Error messages for tools that failed to load.
    pub errors: Vec<String>,
}

/// Error type for custom tool operations.
#[derive(Debug, thiserror::Error)]
pub enum CustomToolError {
    #[error("Tool not found: {0}")]
    NotFound(String),

    #[error("Invalid tool definition: {0}")]
    InvalidDefinition(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_handler_type_serialization() {
        let builtin = HandlerType::Builtin;
        let json = serde_json::to_string(&builtin).unwrap();
        assert_eq!(json, "\"builtin\"");

        let script = HandlerType::Script;
        let json = serde_json::to_string(&script).unwrap();
        assert_eq!(json, "\"script\"");

        let http = HandlerType::Http;
        let json = serde_json::to_string(&http).unwrap();
        assert_eq!(json, "\"http\"");
    }

    #[test]
    fn test_custom_tool_definition_serialization() {
        let def = CustomToolDefinition {
            name: "my_tool".to_string(),
            description: "A test tool".to_string(),
            parameters_schema: json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string" }
                }
            }),
            handler_type: HandlerType::Builtin,
        };
        let json_str = serde_json::to_string(&def).unwrap();
        let deserialized: CustomToolDefinition = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.name, "my_tool");
        assert_eq!(deserialized.description, "A test tool");
        assert_eq!(deserialized.handler_type, HandlerType::Builtin);
    }

    #[test]
    fn test_custom_tool_definition_clone() {
        let def = CustomToolDefinition {
            name: "my_tool".to_string(),
            description: "A test tool".to_string(),
            parameters_schema: json!(null),
            handler_type: HandlerType::Script,
        };
        let cloned = def.clone();
        assert_eq!(cloned.name, "my_tool");
    }

    #[test]
    fn test_load_result_default() {
        let result = LoadResult::default();
        assert!(result.loaded.is_empty());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_load_result_with_data() {
        let result = LoadResult {
            loaded: vec!["tool1".to_string(), "tool2".to_string()],
            errors: vec!["error1".to_string()],
        };
        assert_eq!(result.loaded.len(), 2);
        assert_eq!(result.errors.len(), 1);
    }

    #[test]
    fn test_load_result_serialization() {
        let result = LoadResult {
            loaded: vec!["tool1".to_string()],
            errors: vec![],
        };
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: LoadResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.loaded.len(), 1);
        assert!(deserialized.errors.is_empty());
    }

    #[test]
    fn test_custom_tool_error_not_found() {
        let err = CustomToolError::NotFound("test_tool".to_string());
        assert!(err.to_string().contains("test_tool"));
    }

    #[test]
    fn test_custom_tool_error_invalid_definition() {
        let err = CustomToolError::InvalidDefinition("missing name".to_string());
        assert!(err.to_string().contains("missing name"));
    }

    #[test]
    fn test_handler_type_equality() {
        assert_eq!(HandlerType::Builtin, HandlerType::Builtin);
        assert_ne!(HandlerType::Builtin, HandlerType::Script);
    }

    #[test]
    fn test_parameters_schema_json() {
        let schema = json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search query" },
                "limit": { "type": "integer", "minimum": 1, "maximum": 100 }
            },
            "required": ["query"]
        });
        let def = CustomToolDefinition {
            name: "search".to_string(),
            description: "Search tool".to_string(),
            parameters_schema: schema,
            handler_type: HandlerType::Http,
        };
        assert!(def.parameters_schema.is_object());
    }
}

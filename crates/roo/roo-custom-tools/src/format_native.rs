//! Format Custom Tools for OpenAI Function Calling
//!
//! Converts a custom tool definition into the OpenAI function calling format.
//! Mirrors `formatNative.ts`.

use serde_json::{Value, json};

use crate::types::CustomToolDefinition;

/// Converts a custom tool definition to OpenAI function calling format.
///
/// This function:
/// 1. Removes `$schema` from the parameters object
/// 2. Adds `required: []` if the `required` field is missing
/// 3. Sets `type: "object"` if the parameters object is empty
/// 4. Returns `{ "type": "function", "function": { ...tool, strict: true, parameters } }`
///
/// Source: `.research/Roo-Code/packages/core/src/tools/customTools/formatNative.ts`
pub fn format_native(tool: &CustomToolDefinition) -> Value {
    let mut parameters = tool.parameters_schema.clone();

    // Ensure parameters is an object
    let params_obj = match parameters {
        Value::Object(ref mut map) => {
            // Remove $schema if present
            map.remove("$schema");

            // Add required: [] if missing
            if !map.contains_key("required") {
                map.insert("required".to_string(), json!([]));
            }

            // Ensure type is set
            if !map.contains_key("type") {
                map.insert("type".to_string(), json!("object"));
            }

            map.clone()
        }
        Value::Null => {
            // Empty parameters: create minimal object
            let mut map = serde_json::Map::new();
            map.insert("type".to_string(), json!("object"));
            map.insert("required".to_string(), json!([]));
            map
        }
        _ => {
            // Non-object value: wrap in a proper object
            let mut map = serde_json::Map::new();
            map.insert("type".to_string(), json!("object"));
            map.insert("required".to_string(), json!([]));
            map
        }
    };

    json!({
        "type": "function",
        "function": {
            "name": tool.name,
            "description": tool.description,
            "strict": true,
            "parameters": params_obj,
        }
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::HandlerType;
    use serde_json::json;

    fn make_tool(name: &str, description: &str, parameters: Value) -> CustomToolDefinition {
        CustomToolDefinition {
            name: name.to_string(),
            description: description.to_string(),
            parameters_schema: parameters,
            handler_type: HandlerType::Builtin,
            path: None,
            url: None,
        }
    }

    #[test]
    fn test_format_native_basic() {
        let tool = make_tool(
            "my_tool",
            "A test tool",
            json!({
                "type": "object",
                "properties": {
                    "input": { "type": "string" }
                },
                "required": ["input"]
            }),
        );
        let result = format_native(&tool);

        assert_eq!(result["type"], "function");
        assert_eq!(result["function"]["name"], "my_tool");
        assert_eq!(result["function"]["description"], "A test tool");
        assert_eq!(result["function"]["strict"], true);
        assert_eq!(result["function"]["parameters"]["type"], "object");
        assert_eq!(
            result["function"]["parameters"]["required"],
            json!(["input"])
        );
    }

    #[test]
    fn test_format_native_removes_schema() {
        let tool = make_tool(
            "test_tool",
            "Test",
            json!({
                "$schema": "https://json-schema.org/draft/2020-12/schema",
                "type": "object",
                "properties": {}
            }),
        );
        let result = format_native(&tool);

        assert!(result["function"]["parameters"].get("$schema").is_none());
    }

    #[test]
    fn test_format_native_adds_required_when_missing() {
        let tool = make_tool(
            "test_tool",
            "Test",
            json!({
                "type": "object",
                "properties": {
                    "x": { "type": "number" }
                }
            }),
        );
        let result = format_native(&tool);

        assert_eq!(result["function"]["parameters"]["required"], json!([]));
    }

    #[test]
    fn test_format_native_empty_parameters() {
        let tool = make_tool("test_tool", "Test", json!(null));
        let result = format_native(&tool);

        assert_eq!(result["function"]["parameters"]["type"], "object");
        assert_eq!(result["function"]["parameters"]["required"], json!([]));
    }

    #[test]
    fn test_format_native_preserves_existing_required() {
        let tool = make_tool(
            "test_tool",
            "Test",
            json!({
                "type": "object",
                "properties": {
                    "a": { "type": "string" }
                },
                "required": ["a"]
            }),
        );
        let result = format_native(&tool);

        assert_eq!(result["function"]["parameters"]["required"], json!(["a"]));
    }

    #[test]
    fn test_format_native_structure() {
        let tool = make_tool(
            "search",
            "Search tool",
            json!({
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" }
                },
                "required": ["query"]
            }),
        );
        let result = format_native(&tool);

        // Verify top-level structure
        assert_eq!(result["type"], "function");
        assert!(result["function"].is_object());

        // Verify function fields
        let func = &result["function"];
        assert_eq!(func["name"], "search");
        assert_eq!(func["description"], "Search tool");
        assert_eq!(func["strict"], true);
        assert!(func["parameters"].is_object());
    }
}

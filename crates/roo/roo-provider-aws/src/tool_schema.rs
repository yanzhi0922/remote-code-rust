//! Tool schema normalization for Bedrock tool specs.
//!
//! Bedrock has stricter JSON Schema requirements than OpenAI:
//! 1. `additionalProperties: false` is required on all object types.
//! 2. `type: ["T", "null"]` must be converted to `anyOf` format.
//! 3. Unsupported `format` values must be stripped.

use serde_json::Value;

/// Normalize a JSON Schema for Bedrock tool specs.
///
/// 1. Sets `additionalProperties: false` on all object types
/// 2. Converts `type: ["T", "null"]` to anyOf format
/// 3. Strips unsupported format values
pub fn normalize_tool_schema(schema: &mut Value) {
    let valid_formats = [
        "date-time",
        "time",
        "date",
        "duration",
        "email",
        "hostname",
        "ipv4",
        "ipv6",
        "uuid",
    ];

    match schema {
        Value::Object(map) => {
            // Convert type array to anyOf
            if let Some(Value::Array(types)) = map.get("type") {
                if types.len() == 2 {
                    let non_null: Vec<_> = types
                        .iter()
                        .filter(|t| t.as_str() != Some("null"))
                        .cloned()
                        .collect();
                    let has_null = types.iter().any(|t| t.as_str() == Some("null"));
                    if has_null && non_null.len() == 1 {
                        let non_null_val = non_null.into_iter().next().unwrap();
                        map.insert("type".to_string(), non_null_val.clone());
                        map.entry("anyOf".to_string()).or_insert_with(|| {
                            vec![non_null_val, Value::String("null".to_string())].into()
                        });
                    }
                }
            }

            // Set additionalProperties: false on objects
            if map.get("type").and_then(|t| t.as_str()) == Some("object") {
                map.entry("additionalProperties".to_string())
                    .or_insert(Value::Bool(false));
            }

            // Strip unsupported format values
            if let Some(Value::String(fmt)) = map.get("format") {
                if !valid_formats.contains(&fmt.as_str()) {
                    map.remove("format");
                }
            }

            // Recurse into nested schemas
            for (_key, value) in map.iter_mut() {
                normalize_tool_schema(value);
            }
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                normalize_tool_schema(item);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_normalize_adds_additional_properties_false() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            }
        });
        normalize_tool_schema(&mut schema);
        assert_eq!(schema.get("additionalProperties"), Some(&json!(false)));
    }

    #[test]
    fn test_normalize_does_not_overwrite_additional_properties() {
        let mut schema = json!({
            "type": "object",
            "additionalProperties": true,
            "properties": {}
        });
        normalize_tool_schema(&mut schema);
        assert_eq!(schema.get("additionalProperties"), Some(&json!(true)));
    }

    #[test]
    fn test_normalize_converts_nullable_type_array() {
        let mut schema = json!({
            "type": ["string", "null"]
        });
        normalize_tool_schema(&mut schema);
        assert_eq!(schema.get("type"), Some(&json!("string")));
    }

    #[test]
    fn test_normalize_strips_unsupported_format() {
        let mut schema = json!({
            "type": "string",
            "format": "binary"
        });
        normalize_tool_schema(&mut schema);
        assert!(schema.get("format").is_none());
    }

    #[test]
    fn test_normalize_keeps_supported_format() {
        let mut schema = json!({
            "type": "string",
            "format": "date-time"
        });
        normalize_tool_schema(&mut schema);
        assert_eq!(schema.get("format"), Some(&json!("date-time")));
    }

    #[test]
    fn test_normalize_recurse_into_properties() {
        let mut schema = json!({
            "type": "object",
            "properties": {
                "nested": {
                    "type": "object",
                    "properties": {
                        "deep": { "type": "string", "format": "binary" }
                    }
                }
            }
        });
        normalize_tool_schema(&mut schema);
        // Top-level should have additionalProperties: false
        assert_eq!(schema.get("additionalProperties"), Some(&json!(false)));
        // Nested object should also have additionalProperties: false
        let nested = schema.pointer("/properties/nested").unwrap();
        assert_eq!(nested.get("additionalProperties"), Some(&json!(false)));
        // Unsupported format should be stripped from deep nesting
        let deep = schema
            .pointer("/properties/nested/properties/deep")
            .unwrap();
        assert!(deep.get("format").is_none());
    }
}

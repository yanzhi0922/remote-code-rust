//! Base provider with common functionality.
//!
//! Derived from `src/api/providers/base-provider.ts`.

use serde_json::{json, Value};

use roo_types::api::{ContentBlock, ProviderName};
use roo_types::model::ModelInfo;

/// Checks if a tool name indicates an MCP tool (uses 'mcp__' prefix).
fn is_mcp_tool(name: &str) -> bool {
    name.starts_with("mcp__")
}

/// Converts an array of tools to be compatible with OpenAI's strict mode.
///
/// Source: `src/api/providers/base-provider.ts` — `convertToolsForOpenAI`
pub fn convert_tools_for_openai(tools: Option<&Vec<Value>>) -> Option<Vec<Value>> {
    let tools = tools?;

    Some(
        tools
            .iter()
            .map(|tool| {
                let tool_type = tool.get("type").and_then(|t| t.as_str()).unwrap_or("");
                if tool_type != "function" {
                    return tool.clone();
                }

                let function = tool.get("function").cloned().unwrap_or(json!({}));
                let func_name = function.get("name").and_then(|n| n.as_str()).unwrap_or("");
                let is_mcp = is_mcp_tool(func_name);

                let parameters = if is_mcp {
                    function.get("parameters").cloned()
                } else {
                    function.get("parameters").map(|p| convert_tool_schema_for_openai(p))
                };

                json!({
                    "type": tool_type,
                    "function": {
                        "name": func_name,
                        "strict": !is_mcp,
                        "parameters": parameters.unwrap_or(json!({})),
                    }
                })
            })
            .collect(),
    )
}

/// Converts tool schemas to be compatible with OpenAI's strict mode.
///
/// Source: `src/api/providers/base-provider.ts` — `convertToolSchemaForOpenAI`
pub fn convert_tool_schema_for_openai(schema: &Value) -> Value {
    if schema.is_null() || !schema.is_object() {
        return schema.clone();
    }

    let schema_type = schema.get("type").and_then(|t| t.as_str()).unwrap_or("");
    if schema_type != "object" {
        return schema.clone();
    }

    let mut result = schema.clone();

    // OpenAI Responses API requires additionalProperties: false
    if result.get("additionalProperties").and_then(|v| v.as_bool()) != Some(false) {
        result["additionalProperties"] = json!(false);
    }

    if let Some(properties) = result.get("properties").and_then(|p| p.as_object()).cloned() {
        let all_keys: Vec<String> = properties.keys().cloned().collect();

        // OpenAI strict mode requires ALL properties in required array
        result["required"] = Value::Array(
            all_keys.iter().map(|k| Value::String(k.clone())).collect(),
        );

        let mut new_props = serde_json::Map::new();

        for key in &all_keys {
            let mut prop = properties[key].clone();

            // Handle nullable types by removing null
            if let Some(types) = prop.get("type").and_then(|t| t.as_array()) {
                let non_null: Vec<Value> = types
                    .iter()
                    .filter(|t| t.as_str() != Some("null"))
                    .cloned()
                    .collect();
                if non_null.len() == 1 {
                    prop["type"] = non_null.into_iter().next().unwrap();
                } else if !non_null.is_empty() {
                    prop["type"] = Value::Array(non_null);
                }
            }

            // Recursively process nested objects
            let prop_type = prop.get("type").and_then(|t| t.as_str()).unwrap_or("");
            if prop_type == "object" {
                prop = convert_tool_schema_for_openai(&prop);
            } else if prop_type == "array" {
                if let Some(items) = prop.get("items") {
                    let items_type = items.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    if items_type == "object" {
                        prop["items"] = convert_tool_schema_for_openai(items);
                    }
                }
            }

            new_props.insert(key.clone(), prop);
        }

        result["properties"] = Value::Object(new_props);
    }

    result
}

/// Base provider struct with common functionality.
///
/// Source: `src/api/providers/base-provider.ts` — `BaseProvider`
pub struct BaseProvider {
    pub model_id: String,
    pub model_info: ModelInfo,
    pub provider_name_value: ProviderName,
}

impl BaseProvider {
    pub fn new(model_id: String, model_info: ModelInfo, provider_name: ProviderName) -> Self {
        Self {
            model_id,
            model_info,
            provider_name_value: provider_name,
        }
    }

    pub fn get_model(&self) -> (String, ModelInfo) {
        (self.model_id.clone(), self.model_info.clone())
    }

    /// Count tokens for content blocks using a BPE-style heuristic.
    ///
    /// For text blocks, uses ~4 chars/token as a reasonable approximation
    /// of the tiktoken o200k_base encoding. For images, estimates based on
    /// base64 data length or a conservative default.
    pub async fn count_tokens(&self, content: &[ContentBlock]) -> u64 {
        if content.is_empty() {
            return 0;
        }

        let mut total: u64 = 0;
        for block in content {
            total += match block {
                ContentBlock::Text { text } => {
                    (text.len() as u64).div_ceil(4)
                }
                ContentBlock::ToolUse { name, input, .. } => {
                    let name_tokens = (name.len() as u64).div_ceil(4);
                    let input_str = serde_json::to_string(input).unwrap_or_default();
                    let input_tokens = (input_str.len() as u64).div_ceil(4);
                    name_tokens + input_tokens
                }
                ContentBlock::ToolResult { content: inner, .. } => {
                    inner
                        .iter()
                        .map(|c| match c {
                            roo_types::api::ToolResultContent::Text { text } => {
                                (text.len() as u64).div_ceil(4)
                            }
                            roo_types::api::ToolResultContent::Image { .. } => 256,
                        })
                        .sum()
                }
                ContentBlock::Image { source } => match source {
                    roo_types::api::ImageSource::Base64 { data, .. } => {
                        let len = data.len() as f64;
                        (len.sqrt().ceil() as u64).max(1)
                    }
                    roo_types::api::ImageSource::Url { .. } => 300,
                },
                ContentBlock::Thinking { thinking, .. } => {
                    (thinking.len() as u64).div_ceil(4)
                }
                ContentBlock::RedactedThinking { data } => {
                    (data.len() as f64 / 4.0).ceil() as u64
                }
            };
        }

        total
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_tool_schema_for_openai() {
        let schema = json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "age": { "type": ["integer", "null"] }
            },
            "required": ["name"]
        });

        let result = convert_tool_schema_for_openai(&schema);
        assert_eq!(result["additionalProperties"], json!(false));
        assert_eq!(result["required"].as_array().unwrap().len(), 2);
        assert_eq!(result["properties"]["age"]["type"], json!("integer"));
    }

    #[test]
    fn test_convert_tools_for_openai_mcp() {
        let tools = vec![json!({
            "type": "function",
            "function": {
                "name": "mcp__server__tool",
                "parameters": { "type": "object", "properties": {} }
            }
        })];

        let result = convert_tools_for_openai(Some(&tools)).unwrap();
        assert_eq!(result[0]["function"]["strict"], json!(false));
    }

    #[test]
    fn test_convert_tools_for_openai_regular() {
        let tools = vec![json!({
            "type": "function",
            "function": {
                "name": "read_file",
                "parameters": {
                    "type": "object",
                    "properties": { "path": { "type": "string" } }
                }
            }
        })];

        let result = convert_tools_for_openai(Some(&tools)).unwrap();
        assert_eq!(result[0]["function"]["strict"], json!(true));
    }

    #[test]
    fn test_is_mcp_tool() {
        assert!(is_mcp_tool("mcp__server__tool"));
        assert!(!is_mcp_tool("read_file"));
    }
}

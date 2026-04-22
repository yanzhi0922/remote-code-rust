//! MCP (Model Context Protocol) tool implementations.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Result, anyhow};
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use rc_core::ToolResult;
use rc_mcp::{
    McpResourceContent, McpToolCallContent, McpToolCallResponse, McpToolCallResult,
    normalization::normalize_name_for_mcp,
};
use serde_json::{Map, Value, json};
use uuid::Uuid;

use super::{
    ToolExecutionContext, ToolResultSizePolicy, current_runtime_agent_prompt_context,
    current_tool_runtime_policy,
};
use crate::mcp_output_storage::{
    McpResultFormat, get_binary_blob_saved_message, get_format_description,
    get_large_output_instructions, max_mcp_output_chars, max_mcp_output_tokens,
    persist_binary_content,
};
use crate::mcp_runtime::resolve_runtime_policy_mcp_server;
use crate::tool_result_storage::{persist_tool_result_text, process_tool_result_text};

const MCP_RESOURCE_TOOL_MAX_RESULT_SIZE_CHARS: usize = 100_000;

pub(crate) fn mcp_auth_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let server = input["server"]
        .as_str()
        .ok_or_else(|| anyhow!("server is required"))?;
    let action = input["action"]
        .as_str()
        .ok_or_else(|| anyhow!("action is required (login, logout, or status)"))?;

    let auth_dir = context.cwd.join(".remote-code-rust").join("mcp-auth");
    std::fs::create_dir_all(&auth_dir)?;
    let auth_file = auth_dir.join(format!("{server}.json"));

    match action {
        "login" => {
            let entry = json!({
                "server": server,
                "status": "authenticated",
                "timestamp": SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|duration| duration.as_secs())
                    .unwrap_or(0),
            });
            let content = serde_json::to_string_pretty(&entry)?;
            std::fs::write(&auth_file, content)?;
            Ok(format!("Logged in to MCP server '{server}'."))
        }
        "logout" => {
            if auth_file.exists() {
                std::fs::remove_file(&auth_file)?;
                Ok(format!("Logged out from MCP server '{server}'."))
            } else {
                Ok(format!("No active session for MCP server '{server}'."))
            }
        }
        "status" => {
            if auth_file.exists() {
                let content = std::fs::read_to_string(&auth_file)?;
                Ok(content)
            } else {
                Ok(json!({
                    "server": server,
                    "status": "not_authenticated",
                })
                .to_string())
            }
        }
        _ => Err(anyhow!("action must be 'login', 'logout', or 'status'")),
    }
}

pub(crate) async fn list_mcp_resources_tool(
    input: &Value,
    _context: &ToolExecutionContext,
) -> Result<String> {
    let runtime_policy = current_tool_runtime_policy();
    let target_server = input
        .get("server")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|server| !server.is_empty());
    let servers = if let Some(server_name) = target_server {
        match resolve_runtime_policy_mcp_server(&runtime_policy, server_name) {
            Ok(entry) => vec![(server_name.to_owned(), entry.server)],
            Err(_) => {
                let available = runtime_policy
                    .mcp_servers
                    .iter()
                    .map(|entry| entry.server.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(anyhow!(
                    "Server \"{}\" not found. Available servers: {}",
                    server_name,
                    available
                ));
            }
        }
    } else {
        runtime_policy
            .mcp_servers
            .iter()
            .map(|entry| (entry.server.name.clone(), entry.server.clone()))
            .collect::<Vec<_>>()
    };

    let client_info = rc_mcp::McpClientInfo::new("remote-code-rust", env!("CARGO_PKG_VERSION"));
    let mut resource_list = Vec::new();
    for (server_name, server) in servers {
        if let Ok(resources) = rc_mcp::list_resources(&server, &client_info).await {
            resource_list.extend(resources.iter().map(|resource| {
                json!({
                    "uri": resource.uri,
                    "name": resource.name,
                    "mimeType": resource.mime_type,
                    "description": resource.description,
                    "server": server_name,
                })
            }));
        }
    }
    Ok(json!(resource_list).to_string())
}

pub(crate) async fn read_mcp_resource_tool(
    tool_use_id: &str,
    input: &Value,
    context: &ToolExecutionContext,
) -> Result<ToolResult> {
    let server_name = input["server"]
        .as_str()
        .ok_or_else(|| anyhow!("server is required"))?;
    let uri = input["uri"]
        .as_str()
        .ok_or_else(|| anyhow!("uri is required"))?;

    let runtime_policy = current_tool_runtime_policy();
    let server = resolve_runtime_policy_mcp_server(&runtime_policy, server_name)?.server;

    let client_info = rc_mcp::McpClientInfo::new("remote-code-rust", env!("CARGO_PKG_VERSION"));
    match rc_mcp::read_resource(&server, &client_info, uri).await {
        Ok(contents) => {
            let tool_results_dir = runtime_tool_results_dir(context);
            let payload = transform_read_mcp_resource_contents(
                &contents,
                server_name,
                tool_results_dir.as_deref(),
            );
            let content = json!({
                "contents": payload,
            })
            .to_string();
            let processed = process_tool_result_text(
                &content,
                tool_use_id,
                tool_results_dir.as_deref(),
                ToolResultSizePolicy::finite(MCP_RESOURCE_TOOL_MAX_RESULT_SIZE_CHARS),
            )?;
            Ok(tool_result_from_text(processed, false))
        }
        Err(error) => Err(error.into()),
    }
}

/// Call a tool on an MCP server directly.
pub(crate) async fn mcp_call_tool(
    input: &Value,
    context: &ToolExecutionContext,
) -> Result<ToolResult> {
    let server_name = input["server"]
        .as_str()
        .ok_or_else(|| anyhow!("server is required"))?;
    let tool_name = input["tool"]
        .as_str()
        .ok_or_else(|| anyhow!("tool is required"))?;
    let arguments = input.get("arguments").cloned().unwrap_or(json!({}));

    let runtime_policy = current_tool_runtime_policy();
    let server = resolve_runtime_policy_mcp_server(&runtime_policy, server_name)?.server;

    let client_info = rc_mcp::McpClientInfo::new("remote-code-rust", env!("CARGO_PKG_VERSION"));
    let response = rc_mcp::call_tool(&server, &client_info, tool_name, arguments).await?;
    transform_mcp_tool_response(&response, context)
}

pub(crate) fn transform_mcp_tool_response(
    response: &McpToolCallResponse,
    context: &ToolExecutionContext,
) -> Result<ToolResult> {
    transform_mcp_tool_result(
        &response.result,
        &response.server_name,
        &response.tool_name,
        runtime_tool_results_dir(context).as_deref(),
    )
}

pub(crate) fn transform_mcp_tool_result(
    result: &McpToolCallResult,
    server_name: &str,
    tool_name: &str,
    tool_results_dir: Option<&Path>,
) -> Result<ToolResult> {
    if let Some(tool_result) = &result.tool_result {
        let content = legacy_tool_result_to_text(tool_result);
        return handle_large_mcp_output(McpLargeOutput {
            is_error: result.is_error,
            server_name,
            tool_name,
            format: McpResultFormat::ToolResult,
            schema: None,
            content: &content,
            content_blocks: Vec::new(),
            tool_results_dir,
        });
    }

    if let Some(structured_content) = &result.structured_content {
        let content = serde_json::to_string(structured_content)?;
        return handle_large_mcp_output(McpLargeOutput {
            is_error: result.is_error,
            server_name,
            tool_name,
            format: McpResultFormat::StructuredContent,
            schema: Some(infer_compact_schema(structured_content, 2)),
            content: &content,
            content_blocks: Vec::new(),
            tool_results_dir,
        });
    }

    if !result.content.is_empty() {
        let blocks = result
            .content
            .iter()
            .flat_map(|content| transform_mcp_content_block(content, server_name, tool_results_dir))
            .collect::<Vec<_>>();
        let content = flatten_text_blocks(&blocks);
        return handle_large_mcp_output(McpLargeOutput {
            is_error: result.is_error,
            server_name,
            tool_name,
            format: McpResultFormat::ContentArray,
            schema: Some(infer_compact_schema(&Value::Array(blocks.clone()), 2)),
            content: &content,
            content_blocks: blocks,
            tool_results_dir,
        });
    }

    Err(anyhow!(
        "MCP server \"{server_name}\" tool \"{tool_name}\": unexpected response format"
    ))
}

pub(crate) fn transform_read_mcp_resource_contents(
    contents: &[McpResourceContent],
    server_name: &str,
    tool_results_dir: Option<&Path>,
) -> Vec<Value> {
    contents
        .iter()
        .enumerate()
        .map(|(index, content)| {
            if let Some(text) = &content.text {
                return json!({
                    "uri": content.uri,
                    "mimeType": content.mime_type,
                    "text": text,
                });
            }

            if let Some(blob) = &content.blob {
                let source_description =
                    format!("[Resource from {server_name} at {}] ", content.uri);
                return match persist_mcp_blob(
                    blob,
                    content.mime_type.as_deref(),
                    &format!(
                        "mcp-resource-{}-{index}",
                        normalize_name_for_mcp(server_name)
                    ),
                    &source_description,
                    tool_results_dir,
                ) {
                    PersistedMcpBlob::Saved { path, message } => json!({
                        "uri": content.uri,
                        "mimeType": content.mime_type,
                        "blobSavedTo": path,
                        "text": message,
                    }),
                    PersistedMcpBlob::Error { message } => json!({
                        "uri": content.uri,
                        "mimeType": content.mime_type,
                        "text": message,
                    }),
                };
            }

            json!({
                "uri": content.uri,
                "mimeType": content.mime_type,
            })
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PersistedMcpBlob {
    Saved { path: String, message: String },
    Error { message: String },
}

fn runtime_tool_results_dir(context: &ToolExecutionContext) -> Option<PathBuf> {
    current_runtime_agent_prompt_context()
        .and_then(|prompt_context| prompt_context.tool_results_dir)
        .or_else(|| current_tool_runtime_policy().tool_results_dir)
        .or_else(|| Some(context.cwd.join(".remote-code-rust").join("tool-results")))
}

struct McpLargeOutput<'a> {
    is_error: bool,
    server_name: &'a str,
    tool_name: &'a str,
    format: McpResultFormat,
    schema: Option<String>,
    content: &'a str,
    content_blocks: Vec<Value>,
    tool_results_dir: Option<&'a Path>,
}

fn handle_large_mcp_output(input: McpLargeOutput<'_>) -> Result<ToolResult> {
    let size_estimate = mcp_result_size_estimate(input.content, &input.content_blocks);
    if size_estimate <= max_mcp_output_chars() {
        return Ok(ToolResult {
            content: input.content.to_owned(),
            is_error: input.is_error,
            content_blocks: input.content_blocks,
            follow_up_user_blocks: Vec::new(),
        });
    }

    let truncated = should_truncate_large_mcp_output(&input.content_blocks);
    if truncated {
        return Ok(ToolResult {
            content: truncate_mcp_string(input.content),
            is_error: input.is_error,
            content_blocks: truncate_mcp_content_blocks(&input.content_blocks),
            follow_up_user_blocks: Vec::new(),
        });
    }

    let Some(tool_results_dir) = input.tool_results_dir else {
        return Ok(ToolResult {
            content: truncate_mcp_string(input.content),
            is_error: input.is_error,
            content_blocks: truncate_mcp_content_blocks(&input.content_blocks),
            follow_up_user_blocks: Vec::new(),
        });
    };

    let raw_output = if input.content_blocks.is_empty() {
        input.content.to_owned()
    } else {
        serde_json::to_string_pretty(&input.content_blocks)?
    };
    let persist_id = format!(
        "mcp-{}-{}-{}",
        normalize_name_for_mcp(input.server_name),
        normalize_name_for_mcp(input.tool_name),
        timestamp_fragment(),
    );
    let persisted = match persist_tool_result_text(&raw_output, &persist_id, tool_results_dir) {
        Ok(persisted) => persisted,
        Err(error) => {
            return Ok(ToolResult {
                content: large_output_persist_failure_message(raw_output.chars().count(), &error),
                is_error: input.is_error,
                content_blocks: Vec::new(),
                follow_up_user_blocks: Vec::new(),
            });
        }
    };
    let instructions = get_large_output_instructions(
        &persisted.filepath,
        raw_output.chars().count(),
        &get_format_description(input.format, input.schema.as_deref()),
        None,
    );
    Ok(ToolResult {
        content: instructions,
        is_error: input.is_error,
        content_blocks: Vec::new(),
        follow_up_user_blocks: Vec::new(),
    })
}

fn should_truncate_large_mcp_output(content_blocks: &[Value]) -> bool {
    env_disables_mcp_large_output_files() || content_blocks_contain_images(content_blocks)
}

fn env_disables_mcp_large_output_files() -> bool {
    std::env::var("ENABLE_MCP_LARGE_OUTPUT_FILES")
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "no" | "off"
            )
        })
        .unwrap_or(false)
}

fn transform_mcp_content_block(
    content: &McpToolCallContent,
    server_name: &str,
    tool_results_dir: Option<&Path>,
) -> Vec<Value> {
    match content.kind.as_str() {
        "text" => value_string(&content.fields, &["text"])
            .map(|text| vec![text_block(text.to_owned())])
            .unwrap_or_default(),
        "audio" => {
            let source_description = format!("[Audio from {server_name}] ");
            persist_mcp_blob(
                value_string(&content.fields, &["data"]).unwrap_or_default(),
                value_string(&content.fields, &["mimeType", "mime_type"]),
                &format!("mcp-{}-audio", normalize_name_for_mcp(server_name)),
                &source_description,
                tool_results_dir,
            )
            .into_text_blocks()
        }
        "image" => transform_image_content_block(content),
        "resource" => transform_resource_content_block(content, server_name, tool_results_dir),
        "resource_link" => {
            let uri = value_string(&content.fields, &["uri"]).unwrap_or_default();
            let name = value_string(&content.fields, &["name"]).unwrap_or("resource");
            let description = value_string(&content.fields, &["description"]);
            let mut text = format!("[Resource link: {name}] {uri}");
            if let Some(description) = description {
                text.push_str(&format!(" ({description})"));
            }
            vec![text_block(text)]
        }
        _ => Vec::new(),
    }
}

fn transform_image_content_block(content: &McpToolCallContent) -> Vec<Value> {
    if let Some(source) = content.fields.get("source").and_then(Value::as_object)
        && let (Some(data), Some(media_type)) = (
            source.get("data").and_then(Value::as_str),
            source
                .get("media_type")
                .or_else(|| source.get("mimeType"))
                .and_then(Value::as_str),
        )
    {
        return vec![image_block(data, media_type)];
    }

    let Some(data) = value_string(&content.fields, &["data"]) else {
        return Vec::new();
    };
    let Some(mime_type) = value_string(
        &content.fields,
        &["mimeType", "mime_type", "mediaType", "media_type"],
    ) else {
        return Vec::new();
    };
    vec![image_block(data, mime_type)]
}

fn transform_resource_content_block(
    content: &McpToolCallContent,
    server_name: &str,
    tool_results_dir: Option<&Path>,
) -> Vec<Value> {
    let Some(resource) = content.fields.get("resource").and_then(Value::as_object) else {
        return Vec::new();
    };

    let uri = object_value_string(resource, &["uri"]).unwrap_or_default();
    let prefix = format!("[Resource from {server_name} at {uri}] ");

    if let Some(text) = object_value_string(resource, &["text"]) {
        return vec![text_block(format!("{prefix}{text}"))];
    }

    let Some(blob) = object_value_string(resource, &["blob"]) else {
        return Vec::new();
    };
    let mime_type = object_value_string(resource, &["mimeType", "mime_type"]);
    if is_image_mime_type(mime_type) {
        let mut blocks = Vec::new();
        if !prefix.trim().is_empty() {
            blocks.push(text_block(prefix));
        }
        blocks.push(image_block(blob, mime_type.unwrap_or("image/png")));
        return blocks;
    }

    persist_mcp_blob(
        blob,
        mime_type,
        &format!("mcp-{}-resource", normalize_name_for_mcp(server_name)),
        &prefix,
        tool_results_dir,
    )
    .into_text_blocks()
}

fn persist_mcp_blob(
    base64_data: &str,
    mime_type: Option<&str>,
    persist_prefix: &str,
    source_description: &str,
    tool_results_dir: Option<&Path>,
) -> PersistedMcpBlob {
    let Some(tool_results_dir) = tool_results_dir else {
        return PersistedMcpBlob::Error {
            message: format!(
                "{source_description}Binary content could not be saved to disk: tool results directory is not configured"
            ),
        };
    };

    let bytes = match BASE64_STANDARD.decode(base64_data) {
        Ok(bytes) => bytes,
        Err(error) => {
            return PersistedMcpBlob::Error {
                message: format!(
                    "{source_description}Binary content could not be decoded from base64: {error}"
                ),
            };
        }
    };

    let persist_id = format!("{persist_prefix}-{}", unique_id_fragment());
    match persist_binary_content(&bytes, mime_type, &persist_id, tool_results_dir) {
        Ok(persisted) => PersistedMcpBlob::Saved {
            path: persisted.filepath.display().to_string(),
            message: get_binary_blob_saved_message(
                &persisted.filepath,
                mime_type,
                persisted.size,
                source_description,
            ),
        },
        Err(error) => PersistedMcpBlob::Error {
            message: format!(
                "{source_description}Binary content could not be saved to disk: {error}"
            ),
        },
    }
}

fn mcp_result_size_estimate(content: &str, content_blocks: &[Value]) -> usize {
    if content_blocks.is_empty() {
        return content.chars().count();
    }
    content_blocks
        .iter()
        .map(|block| {
            if let Some(text) = block_text(block) {
                return text.chars().count();
            }
            if block_image_data(block).is_some() {
                return image_token_estimate_chars();
            }
            block.to_string().len()
        })
        .sum()
}

fn truncate_mcp_string(content: &str) -> String {
    let truncated = char_boundary_prefix(content, max_mcp_output_chars());
    format!("{truncated}{}", mcp_truncation_message())
}

fn truncate_mcp_content_blocks(blocks: &[Value]) -> Vec<Value> {
    if blocks.is_empty() {
        return Vec::new();
    }
    let mut result = Vec::new();
    let mut current_chars = 0usize;
    let max_chars = max_mcp_output_chars();

    for block in blocks {
        if let Some(text) = block_text(block) {
            if current_chars >= max_chars {
                break;
            }
            let remaining = max_chars - current_chars;
            let truncated = char_boundary_prefix(text, remaining);
            if !truncated.is_empty() {
                result.push(text_block(truncated.to_owned()));
                current_chars += truncated.chars().count();
            }
            if truncated.chars().count() < text.chars().count() {
                break;
            }
            continue;
        }

        let image_size = if block_image_data(block).is_some() {
            image_token_estimate_chars()
        } else {
            0
        };
        if current_chars + image_size > max_chars {
            break;
        }
        result.push(block.clone());
        current_chars += image_size;
    }

    result.push(text_block(mcp_truncation_message()));
    result
}

fn mcp_truncation_message() -> String {
    format!(
        "\n\n[OUTPUT TRUNCATED - exceeded {} token limit]\n\nThe tool output was truncated. If this MCP server provides pagination or filtering tools, use them to retrieve specific portions of the data. If pagination is not available, inform the user that you are working with truncated output and results may be incomplete.",
        max_mcp_output_tokens()
    )
}

fn large_output_persist_failure_message(content_length: usize, error: &anyhow::Error) -> String {
    format!(
        "Error: result ({} characters) exceeds maximum allowed tokens. Failed to save output to file: {}. If this MCP server provides pagination or filtering tools, use them to retrieve specific portions of the data.",
        format_number_with_commas(content_length),
        error
    )
}

fn legacy_tool_result_to_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Null => "null".to_owned(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number.to_string(),
        Value::Array(items) => items
            .iter()
            .map(legacy_tool_result_to_text)
            .collect::<Vec<_>>()
            .join(","),
        Value::Object(_) => "[object Object]".to_owned(),
    }
}

fn format_number_with_commas(value: usize) -> String {
    let digits = value.to_string();
    let mut formatted = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, ch) in digits.chars().rev().enumerate() {
        if index != 0 && index % 3 == 0 {
            formatted.push(',');
        }
        formatted.push(ch);
    }
    formatted.chars().rev().collect()
}

fn infer_compact_schema(value: &Value, depth: usize) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Bool(_) => "boolean".to_owned(),
        Value::Number(_) => "number".to_owned(),
        Value::String(_) => "string".to_owned(),
        Value::Array(values) => values
            .first()
            .map(|first| format!("[{}]", infer_compact_schema(first, depth.saturating_sub(1))))
            .unwrap_or_else(|| "[]".to_owned()),
        Value::Object(object) => {
            if depth == 0 {
                return "{...}".to_owned();
            }
            let mut entries = object
                .iter()
                .take(10)
                .map(|(key, value)| {
                    format!(
                        "{key}: {}",
                        infer_compact_schema(value, depth.saturating_sub(1))
                    )
                })
                .collect::<Vec<_>>();
            if object.len() > 10 {
                entries.push("...".to_owned());
            }
            format!("{{{}}}", entries.join(", "))
        }
    }
}

fn text_block(text: String) -> Value {
    json!({
        "type": "text",
        "text": text,
    })
}

fn image_block(data: &str, mime_type: &str) -> Value {
    json!({
        "type": "image",
        "source": {
            "type": "base64",
            "media_type": mime_type,
            "data": data,
        }
    })
}

fn flatten_text_blocks(blocks: &[Value]) -> String {
    blocks
        .iter()
        .filter_map(block_text)
        .filter(|text| !text.trim().is_empty())
        .map(str::to_owned)
        .collect::<Vec<_>>()
        .join("\n")
}

fn content_blocks_contain_images(blocks: &[Value]) -> bool {
    blocks
        .iter()
        .any(|block| block.get("type").and_then(Value::as_str) == Some("image"))
}

fn block_text(block: &Value) -> Option<&str> {
    (block.get("type").and_then(Value::as_str) == Some("text"))
        .then(|| block.get("text").and_then(Value::as_str))
        .flatten()
}

fn block_image_data(block: &Value) -> Option<&str> {
    if block.get("type").and_then(Value::as_str) != Some("image") {
        return None;
    }
    block
        .get("source")
        .and_then(Value::as_object)
        .and_then(|source| source.get("data"))
        .and_then(Value::as_str)
}

fn image_token_estimate_chars() -> usize {
    1_600 * 4
}

fn char_boundary_prefix(content: &str, max_chars: usize) -> &str {
    if content.chars().count() <= max_chars {
        return content;
    }
    for (count, (index, _)) in content.char_indices().enumerate() {
        if count == max_chars {
            return &content[..index];
        }
    }
    content
}

fn value_string<'a>(fields: &'a BTreeMap<String, Value>, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| fields.get(*key).and_then(Value::as_str))
}

fn object_value_string<'a>(object: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(Value::as_str))
}

fn is_image_mime_type(mime_type: Option<&str>) -> bool {
    matches!(
        mime_type.unwrap_or_default(),
        "image/jpeg" | "image/png" | "image/gif" | "image/webp"
    )
}

fn timestamp_fragment() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0)
}

fn unique_id_fragment() -> String {
    let uuid = Uuid::new_v4().simple().to_string();
    uuid[..8].to_owned()
}

fn tool_result_from_text(content: String, is_error: bool) -> ToolResult {
    ToolResult {
        content,
        is_error,
        content_blocks: Vec::new(),
        follow_up_user_blocks: Vec::new(),
    }
}

impl PersistedMcpBlob {
    fn into_text_blocks(self) -> Vec<Value> {
        match self {
            Self::Saved { message, .. } | Self::Error { message } => vec![text_block(message)],
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use base64::Engine;
    use tempfile::tempdir;

    use super::{
        MCP_RESOURCE_TOOL_MAX_RESULT_SIZE_CHARS, ToolResultSizePolicy, flatten_text_blocks,
        process_tool_result_text, runtime_tool_results_dir, text_block, transform_mcp_tool_result,
        transform_read_mcp_resource_contents,
    };
    use crate::{
        ToolExecutionContext, ToolRuntimePolicy, configure_tool_runtime_policy,
        current_tool_runtime_policy,
    };
    use rc_mcp::{McpResourceContent, McpToolCallContent, McpToolCallResult};
    use serde_json::{Value, json};

    #[test]
    fn read_mcp_resource_binary_blob_is_persisted_to_disk() {
        let temp = tempdir().expect("tempdir");
        let contents = vec![McpResourceContent {
            uri: "test://resource".to_owned(),
            mime_type: Some("application/pdf".to_owned()),
            text: None,
            blob: Some(base64::prelude::BASE64_STANDARD.encode("%PDF-1.7")),
        }];

        let payload = transform_read_mcp_resource_contents(&contents, "demo", Some(temp.path()));

        assert_eq!(payload.len(), 1);
        let first = payload[0].as_object().expect("object");
        assert!(first.get("blobSavedTo").is_some());
        let saved_path = first["blobSavedTo"].as_str().expect("saved path");
        assert!(saved_path.ends_with(".pdf"));
        assert!(std::path::Path::new(saved_path).exists());
        assert!(
            first["text"]
                .as_str()
                .expect("saved text")
                .contains("Binary content")
        );
    }

    #[test]
    fn read_mcp_resource_large_text_is_persisted_like_builtin_tool_result() {
        let temp = tempdir().expect("tempdir");
        let contents = vec![McpResourceContent {
            uri: "test://large".to_owned(),
            mime_type: Some("text/plain".to_owned()),
            text: Some("x".repeat(120_000)),
            blob: None,
        }];
        let content = json!({
            "contents": transform_read_mcp_resource_contents(&contents, "demo", Some(temp.path())),
        })
        .to_string();

        let processed = process_tool_result_text(
            &content,
            "call-large-resource",
            Some(temp.path()),
            ToolResultSizePolicy::finite(MCP_RESOURCE_TOOL_MAX_RESULT_SIZE_CHARS),
        )
        .expect("process");

        assert!(processed.starts_with("<persisted-output>"));
        assert!(processed.contains("Full output saved to:"));
        assert!(temp.path().join("call-large-resource.txt").exists());
    }

    #[test]
    fn transform_mcp_tool_result_persists_large_structured_content_with_instructions() {
        let temp = tempdir().expect("tempdir");
        let large = "x".repeat(120_000);
        let result = McpToolCallResult {
            tool_result: None,
            content: Vec::new(),
            structured_content: Some(json!({"payload": large})),
            is_error: false,
        };

        let processed =
            transform_mcp_tool_result(&result, "demo", "search", Some(temp.path())).expect("ok");

        assert!(processed.content.contains("Output has been saved to"));
        assert!(
            processed
                .content
                .contains("REQUIREMENTS FOR SUMMARIZATION/ANALYSIS/REVIEW")
        );
        assert!(processed.content_blocks.is_empty());
    }

    #[test]
    fn transform_mcp_tool_result_preserves_image_blocks() {
        let temp = tempdir().expect("tempdir");
        let result = McpToolCallResult {
            tool_result: None,
            content: vec![McpToolCallContent {
                kind: "image".to_owned(),
                fields: BTreeMap::from([
                    ("data".to_owned(), Value::String("ZmFrZQ==".to_owned())),
                    ("mimeType".to_owned(), Value::String("image/png".to_owned())),
                ]),
            }],
            structured_content: None,
            is_error: false,
        };

        let processed =
            transform_mcp_tool_result(&result, "demo", "render", Some(temp.path())).expect("ok");

        assert_eq!(processed.content_blocks.len(), 1);
        assert_eq!(
            processed.content_blocks[0]
                .get("type")
                .and_then(Value::as_str),
            Some("image")
        );
    }

    #[test]
    fn flatten_text_blocks_joins_text_content() {
        let blocks = vec![text_block("one".to_owned()), text_block("two".to_owned())];
        assert_eq!(flatten_text_blocks(&blocks), "one\ntwo");
    }

    #[test]
    fn transform_mcp_tool_result_prefers_legacy_tool_result_field() {
        let temp = tempdir().expect("tempdir");
        let result = McpToolCallResult {
            tool_result: Some(json!("legacy-result")),
            content: vec![McpToolCallContent {
                kind: "text".to_owned(),
                fields: BTreeMap::from([("text".to_owned(), Value::String("ignored".to_owned()))]),
            }],
            structured_content: Some(json!({"library": "ignored"})),
            is_error: false,
        };

        let processed =
            transform_mcp_tool_result(&result, "demo", "resolve", Some(temp.path())).expect("ok");

        assert_eq!(processed.content, "legacy-result");
        assert!(processed.content_blocks.is_empty());
    }

    #[test]
    fn transform_mcp_tool_result_drops_unknown_content_blocks_like_research() {
        let temp = tempdir().expect("tempdir");
        let result = McpToolCallResult {
            tool_result: None,
            content: vec![McpToolCallContent {
                kind: "unknown_future_block".to_owned(),
                fields: BTreeMap::from([(
                    "payload".to_owned(),
                    Value::String("must not leak into context".to_owned()),
                )]),
            }],
            structured_content: None,
            is_error: false,
        };

        let processed =
            transform_mcp_tool_result(&result, "demo", "future", Some(temp.path())).expect("ok");

        assert!(processed.content.is_empty());
        assert!(processed.content_blocks.is_empty());
    }

    #[test]
    fn structured_mcp_content_uses_compact_json_stringify() {
        let temp = tempdir().expect("tempdir");
        let result = McpToolCallResult {
            tool_result: None,
            content: Vec::new(),
            structured_content: Some(json!({"payload": {"id": 1, "name": "demo"}})),
            is_error: false,
        };

        let processed = transform_mcp_tool_result(&result, "demo", "structured", Some(temp.path()))
            .expect("ok");

        assert_eq!(processed.content, r#"{"payload":{"id":1,"name":"demo"}}"#);
        assert!(processed.content_blocks.is_empty());
    }

    #[test]
    fn transform_mcp_tool_result_falls_back_to_message_when_large_output_persist_fails() {
        let temp = tempdir().expect("tempdir");
        let invalid_tool_results_dir = temp.path().join("not-a-dir");
        std::fs::write(&invalid_tool_results_dir, "file").expect("write blocking file");
        let large = "x".repeat(120_000);
        let result = McpToolCallResult {
            tool_result: None,
            content: Vec::new(),
            structured_content: Some(json!({"payload": large})),
            is_error: false,
        };

        let processed =
            transform_mcp_tool_result(&result, "demo", "search", Some(&invalid_tool_results_dir))
                .expect("ok");

        assert!(processed.content.contains("Failed to save output to file"));
        assert!(processed.content.contains("pagination or filtering tools"));
        assert!(processed.content_blocks.is_empty());
    }

    #[test]
    fn runtime_tool_results_dir_falls_back_to_workspace_when_runtime_unset() {
        let temp = tempdir().expect("tempdir");
        let context = ToolExecutionContext {
            cwd: temp.path().to_path_buf(),
            original_cwd: temp.path().to_path_buf(),
            active_worktree_session: None,
            timeout_ms: 1_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Default::default(),
        };
        let original_policy = current_tool_runtime_policy();
        configure_tool_runtime_policy(ToolRuntimePolicy {
            allowed_tools: Vec::new(),
            disallowed_tools: Vec::new(),
            task_output_dir: None,
            tasks_dir: None,
            tool_results_dir: None,
            mcp_servers: Vec::new(),
            shell_policy: Default::default(),
        })
        .expect("set runtime policy");

        let resolved = runtime_tool_results_dir(&context).expect("resolved");

        configure_tool_runtime_policy(original_policy).expect("restore runtime policy");
        assert!(resolved.ends_with(".remote-code-rust\\tool-results"));
    }
}

use std::collections::BTreeMap;

use anyhow::{Result, anyhow};
use once_cell::sync::Lazy;
use rc_mcp::normalization::{build_mcp_tool_name, normalize_name_for_mcp};
use rc_mcp::{
    McpClientInfo, McpServerConfig, McpServerInspection, McpToolCallResult, inspect_server,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::{
    RuntimeMcpServerPolicyEntry, ToolSpec, current_tool_runtime_policy, tool_allowed_by_policy,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeMcpClientDescriptor {
    pub server_name: String,
    pub normalized_server_name: String,
    pub instructions: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeMcpToolDescriptor {
    pub tool_spec: ToolSpec,
    pub server_name: String,
    pub normalized_server_name: String,
    pub tool_name: String,
    pub normalized_tool_name: String,
    pub server_config: McpServerConfig,
    pub annotations: Value,
}

impl RuntimeMcpToolDescriptor {
    #[must_use]
    pub fn qualified_name(&self) -> &str {
        &self.tool_spec.name
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeMcpCatalog {
    pub clients: Vec<RuntimeMcpClientDescriptor>,
    pub tools: Vec<RuntimeMcpToolDescriptor>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
struct CachedInspection {
    server_config: McpServerConfig,
    inspection: McpServerInspection,
}

static RUNTIME_MCP_INSPECTION_CACHE: Lazy<Mutex<BTreeMap<String, CachedInspection>>> =
    Lazy::new(|| Mutex::new(BTreeMap::new()));

fn inspection_cache_key(entry: &RuntimeMcpServerPolicyEntry) -> String {
    format!("{}::{}", entry.config_path.display(), entry.server.name)
}

fn annotation_hint_is_true(annotations: &Value, key: &str) -> bool {
    annotations
        .get(key)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn build_mcp_tool_spec(
    entry: &RuntimeMcpServerPolicyEntry,
    tool: &rc_mcp::McpToolDescriptor,
) -> RuntimeMcpToolDescriptor {
    let qualified_name = build_mcp_tool_name(&entry.server.name, &tool.name);
    let description = tool.description.clone().unwrap_or_else(|| {
        format!(
            "Call the `{}` tool from the `{}` MCP server.",
            tool.name, entry.server.name
        )
    });
    let requires_permission = !annotation_hint_is_true(&tool.annotations, "readOnlyHint");

    RuntimeMcpToolDescriptor {
        tool_spec: ToolSpec {
            name: qualified_name,
            protocol_name: build_mcp_tool_name(&entry.server.name, &tool.name),
            permission_tool_name: build_mcp_tool_name(&entry.server.name, &tool.name),
            description,
            requires_permission,
            input_schema: tool.input_schema.clone(),
        },
        server_name: entry.server.name.clone(),
        normalized_server_name: normalize_name_for_mcp(&entry.server.name),
        tool_name: tool.name.clone(),
        normalized_tool_name: normalize_name_for_mcp(&tool.name),
        server_config: entry.server.clone(),
        annotations: tool.annotations.clone(),
    }
}

async fn inspect_runtime_mcp_server(
    entry: &RuntimeMcpServerPolicyEntry,
) -> Result<McpServerInspection> {
    let cache_key = inspection_cache_key(entry);
    {
        let cache = RUNTIME_MCP_INSPECTION_CACHE.lock().await;
        if let Some(cached) = cache.get(&cache_key)
            && cached.server_config == entry.server
        {
            return Ok(cached.inspection.clone());
        }
    }

    let inspection = inspect_server(&entry.server, &McpClientInfo::default()).await?;
    let mut cache = RUNTIME_MCP_INSPECTION_CACHE.lock().await;
    cache.insert(
        cache_key,
        CachedInspection {
            server_config: entry.server.clone(),
            inspection: inspection.clone(),
        },
    );
    Ok(inspection)
}

pub async fn runtime_mcp_catalog() -> RuntimeMcpCatalog {
    let policy = current_tool_runtime_policy();
    let mut catalog = RuntimeMcpCatalog::default();
    let mut tool_map = BTreeMap::<String, RuntimeMcpToolDescriptor>::new();

    for entry in &policy.mcp_servers {
        if !entry.server.enabled {
            continue;
        }

        match inspect_runtime_mcp_server(entry).await {
            Ok(inspection) => {
                catalog.clients.push(RuntimeMcpClientDescriptor {
                    server_name: entry.server.name.clone(),
                    normalized_server_name: normalize_name_for_mcp(&entry.server.name),
                    instructions: inspection.instructions.clone(),
                });

                for tool in &inspection.tools {
                    let descriptor = build_mcp_tool_spec(entry, tool);
                    if !tool_allowed_by_policy(descriptor.qualified_name(), &policy) {
                        continue;
                    }

                    if let Some(existing) =
                        tool_map.insert(descriptor.qualified_name().to_owned(), descriptor.clone())
                    {
                        catalog.warnings.push(format!(
                            "Normalized MCP tool name collision for {} between {}:{} and {}:{}; keeping the later definition",
                            existing.qualified_name(),
                            existing.server_name,
                            existing.tool_name,
                            descriptor.server_name,
                            descriptor.tool_name
                        ));
                    }
                }
            }
            Err(error) => catalog.warnings.push(format!(
                "Failed to inspect MCP server {} from {}: {error}",
                entry.server.name,
                entry.config_path.display()
            )),
        }
    }

    catalog.clients.sort_by(|left, right| {
        left.server_name.cmp(&right.server_name).then_with(|| {
            left.normalized_server_name
                .cmp(&right.normalized_server_name)
        })
    });
    catalog.tools = tool_map.into_values().collect();
    catalog.tools.sort_by(|left, right| {
        left.qualified_name()
            .cmp(right.qualified_name())
            .then_with(|| left.server_name.cmp(&right.server_name))
            .then_with(|| left.tool_name.cmp(&right.tool_name))
    });
    catalog
}

#[must_use]
pub async fn runtime_mcp_tool_specs() -> Vec<ToolSpec> {
    runtime_mcp_catalog()
        .await
        .tools
        .into_iter()
        .map(|tool| tool.tool_spec)
        .collect()
}

pub async fn resolve_runtime_mcp_tool(name: &str) -> Result<RuntimeMcpToolDescriptor> {
    runtime_mcp_catalog()
        .await
        .tools
        .into_iter()
        .find(|tool| tool.qualified_name() == name)
        .ok_or_else(|| anyhow!("MCP tool '{name}' is not available in the current runtime catalog"))
}

fn format_mcp_tool_result(result: &McpToolCallResult) -> Result<String> {
    let text_blocks = result
        .content
        .iter()
        .filter(|content| content.kind == "text")
        .filter_map(|content| content.fields.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>();
    if !text_blocks.is_empty() {
        return Ok(text_blocks.join("\n"));
    }
    if let Some(structured) = &result.structured_content {
        return serde_json::to_string_pretty(structured)
            .map_err(|error| anyhow!("failed to serialize MCP structured content: {error}"));
    }
    serde_json::to_string_pretty(&result.content)
        .map_err(|error| anyhow!("failed to serialize MCP content blocks: {error}"))
}

pub async fn execute_runtime_mcp_tool(name: &str, input: &Value) -> Result<String> {
    let descriptor = resolve_runtime_mcp_tool(name).await?;
    let response = rc_mcp::call_tool(
        &descriptor.server_config,
        &McpClientInfo::default(),
        &descriptor.tool_name,
        input.clone(),
    )
    .await?;

    let formatted = format_mcp_tool_result(&response.result)?;
    if response.result.is_error {
        return Err(anyhow!(formatted));
    }
    Ok(formatted)
}

pub async fn clear_runtime_mcp_catalog_cache() {
    RUNTIME_MCP_INSPECTION_CACHE.lock().await.clear();
}

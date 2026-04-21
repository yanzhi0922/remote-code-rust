use std::collections::BTreeMap;

use anyhow::{Result, anyhow};
use once_cell::sync::Lazy;
use rc_mcp::{
    McpClientInfo, McpListChangedSurface, McpServerConfig, McpServerInspection, inspect_server,
    normalization::{build_mcp_tool_name, normalize_name_for_mcp},
};
use rc_ui_bridge::UiRuntimeMcpServerStatus;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::Mutex;

use crate::{
    RuntimeMcpServerPolicyEntry, ToolSpec, current_runtime_mcp_observation,
    current_tool_runtime_policy, mcp_runtime::RuntimeMcpServerObservation, tool_allowed_by_policy,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeMcpClientDescriptor {
    pub server_name: String,
    pub normalized_server_name: String,
    pub instructions: Option<String>,
    pub supports_resources: bool,
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

fn observation_entry_matches_policy_entry(
    observation: &RuntimeMcpServerObservation,
    entry: &RuntimeMcpServerPolicyEntry,
) -> bool {
    observation.entry.origin_kind == entry.origin_kind
        && observation.entry.origin_name == entry.origin_name
        && observation.entry.config_path == entry.config_path
        && observation.entry.server == entry.server
}

fn snapshot_inspection_for_entry(
    entry: &RuntimeMcpServerPolicyEntry,
) -> Option<Result<McpServerInspection>> {
    let observation = current_runtime_mcp_observation()?;
    let server = observation
        .servers
        .iter()
        .find(|server| observation_entry_matches_policy_entry(server, entry))?;
    if let Some(inspection) = &server.inspection {
        return Some(Ok(inspection.clone()));
    }
    if server.status == UiRuntimeMcpServerStatus::Failed {
        let message = server
            .error
            .clone()
            .unwrap_or_else(|| "runtime MCP observation recorded a failed connection".to_owned());
        return Some(Err(anyhow!(message)));
    }
    None
}

fn annotation_hint_is_true(annotations: &Value, key: &str) -> bool {
    annotations
        .get(key)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn inspection_supports_resources(
    entry: &RuntimeMcpServerPolicyEntry,
    inspection: &McpServerInspection,
) -> bool {
    entry.server.capabilities.supports_resources
        || inspection.capabilities.get("resources").is_some()
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
    if let Some(snapshot_result) = snapshot_inspection_for_entry(entry) {
        return snapshot_result;
    }

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
                    supports_resources: inspection_supports_resources(entry, &inspection),
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

pub async fn execute_runtime_mcp_tool(
    name: &str,
    input: &Value,
    context: &crate::ToolExecutionContext,
) -> Result<rc_core::ToolResult> {
    let descriptor = resolve_runtime_mcp_tool(name).await?;
    let response = rc_mcp::call_tool(
        &descriptor.server_config,
        &McpClientInfo::default(),
        &descriptor.tool_name,
        input.clone(),
    )
    .await?;

    crate::mcp_tools::transform_mcp_tool_response(&response, context)
}

pub async fn clear_runtime_mcp_catalog_cache() {
    RUNTIME_MCP_INSPECTION_CACHE.lock().await.clear();
}

pub async fn invalidate_runtime_mcp_catalog_server(server_name: &str) {
    let mut cache = RUNTIME_MCP_INSPECTION_CACHE.lock().await;
    cache.retain(|_, cached| cached.inspection.server_name != server_name);
}

pub async fn handle_runtime_mcp_list_changed(server_name: &str, surface: McpListChangedSurface) {
    match surface {
        McpListChangedSurface::Tools | McpListChangedSurface::Prompts => {
            invalidate_runtime_mcp_catalog_server(server_name).await;
        }
        McpListChangedSurface::Resources => {
            // Resource listing is fetched by rc_mcp::list_resources on demand.
            // Keep tool/prompt cache intact, matching Claude Code's split
            // invalidation where resources/list_changed does not evict tools.
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use rc_mcp::{
        McpCapabilityMatrix, McpListChangedSurface, McpPeerInfo, McpServerConfig,
        McpServerInspection, McpToolDescriptor, McpTransportConfig,
    };
    use serde_json::json;

    use super::{
        CachedInspection, RUNTIME_MCP_INSPECTION_CACHE, handle_runtime_mcp_list_changed,
        inspection_cache_key, invalidate_runtime_mcp_catalog_server,
    };
    use crate::RuntimeMcpServerPolicyEntry;

    fn policy_entry(server_name: &str, config_path: &str) -> RuntimeMcpServerPolicyEntry {
        RuntimeMcpServerPolicyEntry {
            origin_kind: "cwd".to_owned(),
            origin_name: "workspace".to_owned(),
            config_path: PathBuf::from(config_path),
            server: McpServerConfig {
                name: server_name.to_owned(),
                enabled: true,
                transport: McpTransportConfig::Stdio {
                    command: "python".to_owned(),
                    args: Vec::new(),
                    cwd: None,
                    env: BTreeMap::new(),
                },
                capabilities: McpCapabilityMatrix::default(),
                startup_timeout_secs: None,
                request_timeout_secs: None,
                metadata: BTreeMap::new(),
            },
        }
    }

    fn cached_inspection(server_name: &str) -> CachedInspection {
        CachedInspection {
            server_config: policy_entry(server_name, "ignored").server,
            inspection: McpServerInspection {
                server_name: server_name.to_owned(),
                protocol_version: "2025-03-26".to_owned(),
                server_info: Some(McpPeerInfo {
                    name: server_name.to_owned(),
                    title: None,
                    version: None,
                }),
                capabilities: json!({"tools": {"listChanged": true}}),
                instructions: Some("instructions".to_owned()),
                tools: vec![McpToolDescriptor {
                    name: "search".to_owned(),
                    title: None,
                    description: None,
                    input_schema: json!({}),
                    annotations: json!({}),
                }],
            },
        }
    }

    #[tokio::test]
    async fn invalidate_runtime_mcp_catalog_server_removes_only_matching_server() {
        let first = policy_entry("alpha", "alpha.toml");
        let second = policy_entry("beta", "beta.toml");
        {
            let mut cache = RUNTIME_MCP_INSPECTION_CACHE.lock().await;
            cache.clear();
            cache.insert(inspection_cache_key(&first), cached_inspection("alpha"));
            cache.insert(inspection_cache_key(&second), cached_inspection("beta"));
        }

        invalidate_runtime_mcp_catalog_server("alpha").await;

        let cache = RUNTIME_MCP_INSPECTION_CACHE.lock().await;
        assert!(!cache.contains_key(&inspection_cache_key(&first)));
        assert!(cache.contains_key(&inspection_cache_key(&second)));
    }

    #[tokio::test]
    async fn list_changed_invalidates_tools_and_prompts_but_not_resources() {
        let entry = policy_entry("alpha", "alpha.toml");
        {
            let mut cache = RUNTIME_MCP_INSPECTION_CACHE.lock().await;
            cache.clear();
            cache.insert(inspection_cache_key(&entry), cached_inspection("alpha"));
        }

        handle_runtime_mcp_list_changed("alpha", McpListChangedSurface::Resources).await;
        assert!(
            RUNTIME_MCP_INSPECTION_CACHE
                .lock()
                .await
                .contains_key(&inspection_cache_key(&entry))
        );

        handle_runtime_mcp_list_changed("alpha", McpListChangedSurface::Prompts).await;
        assert!(
            !RUNTIME_MCP_INSPECTION_CACHE
                .lock()
                .await
                .contains_key(&inspection_cache_key(&entry))
        );
    }
}

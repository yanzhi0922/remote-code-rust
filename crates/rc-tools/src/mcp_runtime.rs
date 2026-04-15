use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use rc_config::{RuntimeConfig, SettingSource};

use crate::{RuntimeMcpServerPolicyEntry, ToolRuntimePolicy};

#[derive(Debug, Clone)]
pub struct RuntimeMcpServerEntry {
    pub origin_kind: &'static str,
    pub origin_name: String,
    pub config_path: PathBuf,
    pub server: rc_mcp::McpServerConfig,
}

#[derive(Debug, Clone, Default)]
pub struct RuntimeMcpDiscovery {
    pub servers: Vec<RuntimeMcpServerEntry>,
    pub warnings: Vec<String>,
}

impl RuntimeMcpDiscovery {
    pub fn enabled_server_names(&self) -> Vec<String> {
        self.servers
            .iter()
            .filter(|entry| entry.server.enabled)
            .map(|entry| entry.server.name.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn disabled_server_names(&self) -> Vec<String> {
        self.servers
            .iter()
            .filter(|entry| !entry.server.enabled)
            .map(|entry| entry.server.name.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn into_policy_entries(self) -> Vec<RuntimeMcpServerPolicyEntry> {
        self.servers
            .into_iter()
            .map(|entry| RuntimeMcpServerPolicyEntry {
                origin_kind: entry.origin_kind.to_owned(),
                origin_name: entry.origin_name,
                config_path: entry.config_path,
                server: entry.server,
            })
            .collect()
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeMcpResolution {
    pub entry: RuntimeMcpServerEntry,
    pub warnings: Vec<String>,
}

pub fn runtime_mcp_policy_entries(
    config: &RuntimeConfig,
    extra_config_paths: &[PathBuf],
) -> Vec<RuntimeMcpServerPolicyEntry> {
    discover_runtime_mcp_servers(config, extra_config_paths).into_policy_entries()
}

pub fn resolve_runtime_policy_mcp_server(
    policy: &ToolRuntimePolicy,
    server_name: &str,
) -> Result<RuntimeMcpServerPolicyEntry> {
    if policy.mcp_servers.is_empty() {
        return Err(anyhow!(
            "MCP runtime inventory is not configured for the current process"
        ));
    }

    let matches = policy
        .mcp_servers
        .iter()
        .filter(|entry| entry.server.name == server_name)
        .cloned()
        .collect::<Vec<_>>();
    match matches.len() {
        0 => Err(anyhow!(
            "MCP server '{server_name}' is not available in the current runtime inventory"
        )),
        1 => {
            let entry = matches.into_iter().next().expect("single match");
            if !entry.server.enabled {
                return Err(anyhow!(
                    "MCP server '{server_name}' is disabled by the current runtime inventory"
                ));
            }
            Ok(entry)
        }
        _ => {
            let candidates = matches
                .into_iter()
                .map(|entry| {
                    format!(
                        "{}:{} ({})",
                        entry.origin_kind,
                        entry.origin_name,
                        entry.config_path.display()
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            Err(anyhow!(
                "MCP server '{server_name}' is ambiguous across: {candidates}"
            ))
        }
    }
}

pub fn resolve_runtime_mcp_server(
    config: &RuntimeConfig,
    server_name: &str,
    extra_config_paths: &[PathBuf],
) -> Result<RuntimeMcpResolution> {
    let mut discovery = discover_runtime_mcp_servers(config, extra_config_paths);
    let mut matches = discovery
        .servers
        .iter()
        .filter(|entry| entry.server.name == server_name)
        .cloned()
        .collect::<Vec<_>>();

    match matches.len() {
        0 => Err(anyhow!("No MCP server named `{server_name}` was found")),
        1 => Ok(RuntimeMcpResolution {
            entry: matches.pop().expect("single server match must exist"),
            warnings: discovery.warnings,
        }),
        _ => {
            let candidates = matches
                .into_iter()
                .map(|entry| {
                    format!(
                        "{}:{} ({})",
                        entry.origin_kind,
                        entry.origin_name,
                        entry.config_path.display()
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            discovery.warnings.push(format!(
                "Multiple MCP servers named `{server_name}` were discovered; use a unique config layout"
            ));
            Err(anyhow!(
                "MCP server `{server_name}` is ambiguous across: {candidates}"
            ))
        }
    }
}

pub fn discover_runtime_mcp_servers(
    config: &RuntimeConfig,
    extra_config_paths: &[PathBuf],
) -> RuntimeMcpDiscovery {
    let mut discovery = RuntimeMcpDiscovery::default();
    let mut loaded_paths = BTreeSet::new();
    if setting_source_enabled(config, SettingSource::Project) {
        load_runtime_mcp_file(
            &mut discovery,
            &mut loaded_paths,
            "cwd",
            &config.cwd.display().to_string(),
            &config.cwd.join(rc_mcp::DEFAULT_MCP_CONFIG_FILE),
        );
    }
    if setting_source_enabled(config, SettingSource::User) {
        load_runtime_mcp_file(
            &mut discovery,
            &mut loaded_paths,
            "profile",
            &config.paths.profile_dir.display().to_string(),
            &config
                .paths
                .profile_dir
                .join(rc_mcp::DEFAULT_MCP_CONFIG_FILE),
        );
    }
    for path in extra_config_paths {
        let candidate = if path.is_dir() {
            path.join(rc_mcp::DEFAULT_MCP_CONFIG_FILE)
        } else {
            path.clone()
        };
        load_runtime_mcp_file(
            &mut discovery,
            &mut loaded_paths,
            "explicit",
            &path.display().to_string(),
            &candidate,
        );
    }

    if setting_source_enabled(config, SettingSource::User) && config.paths.plugins_dir.exists() {
        match rc_plugins::discover_plugins(&config.paths.plugins_dir) {
            Ok(plugins) => {
                for plugin in plugins {
                    if let Some(path) = plugin.mcp_config_path() {
                        if !loaded_paths.insert(path.clone()) {
                            continue;
                        }
                        match rc_mcp::McpConfig::load(&path) {
                            Ok(config) => push_runtime_mcp_servers(
                                &mut discovery.servers,
                                "plugin",
                                &plugin.manifest.name,
                                &path,
                                config,
                            ),
                            Err(error) => discovery.warnings.push(format!(
                                "Failed to load plugin MCP config for {}: {error}",
                                plugin.manifest.name
                            )),
                        }
                    }
                }
            }
            Err(error) => discovery.warnings.push(format!(
                "Failed to discover plugins for MCP inspection: {error}"
            )),
        }
    }

    discovery.servers.sort_by(|left, right| {
        left.server
            .name
            .cmp(&right.server.name)
            .then_with(|| left.origin_kind.cmp(right.origin_kind))
            .then_with(|| left.origin_name.cmp(&right.origin_name))
    });
    discovery
}

fn setting_source_enabled(config: &RuntimeConfig, source: SettingSource) -> bool {
    config.allowed_setting_sources.contains(&source)
}

fn load_runtime_mcp_file(
    discovery: &mut RuntimeMcpDiscovery,
    loaded_paths: &mut BTreeSet<PathBuf>,
    origin_kind: &'static str,
    origin_name: &str,
    path: &Path,
) {
    if !path.exists() {
        if origin_kind == "explicit" {
            discovery.warnings.push(format!(
                "Explicit MCP config {} was not found",
                path.display()
            ));
        }
        return;
    }
    if !loaded_paths.insert(path.to_path_buf()) {
        return;
    }
    match rc_mcp::McpConfig::load(path) {
        Ok(config) => push_runtime_mcp_servers(
            &mut discovery.servers,
            origin_kind,
            origin_name,
            path,
            config,
        ),
        Err(error) => discovery.warnings.push(format!(
            "Failed to load MCP config {}: {error}",
            path.display()
        )),
    }
}

fn push_runtime_mcp_servers(
    servers: &mut Vec<RuntimeMcpServerEntry>,
    origin_kind: &'static str,
    origin_name: &str,
    config_path: &Path,
    config: rc_mcp::McpConfig,
) {
    for server in config.servers.into_values() {
        servers.push(RuntimeMcpServerEntry {
            origin_kind,
            origin_name: origin_name.to_string(),
            config_path: config_path.to_path_buf(),
            server,
        });
    }
}

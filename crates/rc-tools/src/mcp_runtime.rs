use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use rc_config::{RuntimeConfig, SettingSource};
use rc_ui_bridge::{UiRuntimeMcpInventorySummary, UiRuntimeMcpOriginCounts};

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

    #[must_use]
    pub fn inventory_summary(&self) -> UiRuntimeMcpInventorySummary {
        let unique_server_names = self
            .servers
            .iter()
            .map(|entry| entry.server.name.as_str())
            .collect::<BTreeSet<_>>();
        let ambiguous_server_names = unique_server_names
            .iter()
            .filter(|name| {
                self.servers
                    .iter()
                    .filter(|entry| entry.server.name == **name)
                    .nth(1)
                    .is_some()
            })
            .count();
        let mut origins = UiRuntimeMcpOriginCounts::default();
        for entry in &self.servers {
            match entry.origin_kind {
                "cwd" => origins.cwd += 1,
                "profile" => origins.profile += 1,
                "explicit" => origins.explicit += 1,
                "plugin" => origins.plugin += 1,
                _ => {}
            }
        }

        UiRuntimeMcpInventorySummary {
            total_servers: self.servers.len(),
            enabled_servers: self
                .servers
                .iter()
                .filter(|entry| entry.server.enabled)
                .count(),
            disabled_servers: self
                .servers
                .iter()
                .filter(|entry| !entry.server.enabled)
                .count(),
            unique_server_names: unique_server_names.len(),
            ambiguous_server_names,
            warning_count: self.warnings.len(),
            origins,
        }
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

#[must_use]
pub fn runtime_mcp_inventory_summary(
    config: &RuntimeConfig,
    extra_config_paths: &[PathBuf],
) -> UiRuntimeMcpInventorySummary {
    discover_runtime_mcp_servers(config, extra_config_paths).inventory_summary()
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

#[cfg(test)]
mod tests {
    use super::runtime_mcp_inventory_summary;
    use rc_config::{ProviderOverrides, RuntimeOverrides, SettingSource, load_runtime_config};
    use rc_core::{InputFormat, OutputFormat, PermissionMode};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn runtime_mcp_inventory_summary_counts_origins_and_duplicates() {
        let tempdir = tempdir().expect("tempdir");
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join(".remote-code-rust");
        let plugin_root = profile.join("plugins").join("sample");
        fs::create_dir_all(&cwd).expect("cwd");
        fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("plugin dir");
        fs::write(
            plugin_root.join(".codex-plugin").join("plugin.json"),
            r#"{"name":"sample","version":"0.1.0","mcp":"./mcp.toml"}"#,
        )
        .expect("plugin manifest");

        fs::write(
            cwd.join(rc_mcp::DEFAULT_MCP_CONFIG_FILE),
            concat!(
                "[mcp_servers.shared]\ncommand = \"python\"\n",
                "[mcp_servers.disabled]\ncommand = \"python\"\nenabled = false\n"
            ),
        )
        .expect("cwd mcp");
        fs::write(
            profile.join(rc_mcp::DEFAULT_MCP_CONFIG_FILE),
            "[mcp_servers.shared]\ncommand = \"python\"\n",
        )
        .expect("profile mcp");
        fs::write(
            plugin_root.join(rc_mcp::DEFAULT_MCP_CONFIG_FILE),
            "[mcp_servers.plugin]\ncommand = \"python\"\n",
        )
        .expect("plugin mcp");

        let config = load_runtime_config(
            Some(cwd),
            Some(profile),
            None,
            PermissionMode::Default,
            InputFormat::Text,
            OutputFormat::Text,
            false,
            false,
            false,
            false,
            4,
            ProviderOverrides::default(),
            RuntimeOverrides::default(),
        )
        .expect("config");

        let summary = runtime_mcp_inventory_summary(&config, &[]);
        assert_eq!(summary.total_servers, 4);
        assert_eq!(summary.enabled_servers, 3);
        assert_eq!(summary.disabled_servers, 1);
        assert_eq!(summary.unique_server_names, 3);
        assert_eq!(summary.ambiguous_server_names, 1);
        assert_eq!(summary.origins.cwd, 2);
        assert_eq!(summary.origins.profile, 1);
        assert_eq!(summary.origins.plugin, 1);
    }

    #[test]
    fn runtime_mcp_inventory_summary_obeys_allowed_setting_sources() {
        let tempdir = tempdir().expect("tempdir");
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join(".remote-code-rust");
        let plugin_root = profile.join("plugins").join("sample");
        fs::create_dir_all(&cwd).expect("cwd");
        fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("plugin dir");
        fs::write(
            plugin_root.join(".codex-plugin").join("plugin.json"),
            r#"{"name":"sample","version":"0.1.0","mcp":"./mcp.toml"}"#,
        )
        .expect("plugin manifest");
        fs::write(
            cwd.join(rc_mcp::DEFAULT_MCP_CONFIG_FILE),
            "[mcp_servers.project]\ncommand = \"python\"\n",
        )
        .expect("cwd mcp");
        fs::write(
            profile.join(rc_mcp::DEFAULT_MCP_CONFIG_FILE),
            "[mcp_servers.profile]\ncommand = \"python\"\n",
        )
        .expect("profile mcp");
        fs::write(
            plugin_root.join(rc_mcp::DEFAULT_MCP_CONFIG_FILE),
            "[mcp_servers.plugin]\ncommand = \"python\"\n",
        )
        .expect("plugin mcp");

        let mut config = load_runtime_config(
            Some(cwd),
            Some(profile),
            None,
            PermissionMode::Default,
            InputFormat::Text,
            OutputFormat::Text,
            false,
            false,
            false,
            false,
            4,
            ProviderOverrides::default(),
            RuntimeOverrides::default(),
        )
        .expect("config");
        config.allowed_setting_sources = vec![SettingSource::Project];

        let summary = runtime_mcp_inventory_summary(&config, &[]);
        assert_eq!(summary.total_servers, 1);
        assert_eq!(summary.enabled_servers, 1);
        assert_eq!(summary.disabled_servers, 0);
        assert_eq!(summary.origins.cwd, 1);
        assert_eq!(summary.origins.profile, 0);
        assert_eq!(summary.origins.plugin, 0);
    }
}

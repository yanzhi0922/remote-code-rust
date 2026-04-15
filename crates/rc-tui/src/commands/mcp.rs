use std::path::{Path, PathBuf};

use rc_config::{RuntimeConfig, SettingSource};

#[derive(Debug, Clone)]
struct McpServerView {
    name: String,
    enabled: bool,
    transport: rc_mcp::McpTransport,
    origin_kind: &'static str,
    origin_name: String,
    config_path: PathBuf,
    startup_timeout_secs: Option<u64>,
    request_timeout_secs: Option<u64>,
    metadata_keys: Vec<String>,
}

pub fn dispatch(input: &str, config: &RuntimeConfig) {
    let remainder = input.trim().strip_prefix("/mcp").unwrap_or_default().trim();
    if remainder.is_empty() || remainder == "list" {
        render(config);
        return;
    }

    let tokens = remainder.split_whitespace().collect::<Vec<_>>();
    match tokens.first().copied().unwrap_or_default() {
        "show" | "get" => {
            let Some(name) = tokens.get(1).copied() else {
                println!(
                    "Usage: /mcp [list|show <server>|enable <server> [project]|disable <server> [project]|reset [project]]"
                );
                return;
            };
            render_server(config, name);
        }
        "enable" | "disable" => {
            let Some(name) = tokens.get(1).copied() else {
                println!(
                    "Usage: /mcp {} <server> [project]",
                    tokens.first().copied().unwrap_or("enable")
                );
                return;
            };
            let scope_token = tokens.get(2).copied();
            set_server_enabled(
                config,
                name,
                scope_token == Some("project"),
                tokens[0] == "enable",
            );
        }
        "reset" => reset_managed_config(config, tokens.get(1).copied() == Some("project")),
        other => {
            println!("Unknown /mcp subcommand '{other}'.");
            println!(
                "Usage: /mcp [list|show <server>|enable <server> [project]|disable <server> [project]|reset [project]]"
            );
        }
    }
}

pub fn render(config: &RuntimeConfig) {
    let (warnings, mut servers) = discover_mcp_servers(config);
    if servers.is_empty() {
        println!("MCP servers: none discovered.");
        for warning in warnings {
            println!("  - {warning}");
        }
        return;
    }

    servers.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.origin_kind.cmp(right.origin_kind))
            .then_with(|| left.origin_name.cmp(&right.origin_name))
    });

    println!("MCP servers ({}):", servers.len());
    for warning in &warnings {
        println!("  warning: {warning}");
    }
    for server in &servers {
        println!(
            "  {}  {}  {}  {}",
            server.name,
            if server.enabled {
                "enabled"
            } else {
                "disabled"
            },
            transport_label(server.transport),
            source_label(server.origin_kind, &server.origin_name, &server.config_path)
        );
    }
    println!("Tip: /mcp show <server>");
}

fn render_server(config: &RuntimeConfig, server_name: &str) {
    let (warnings, servers) = discover_mcp_servers(config);
    let matches = servers
        .into_iter()
        .filter(|server| server.name == server_name)
        .collect::<Vec<_>>();

    if matches.is_empty() {
        println!("No MCP server named `{server_name}` was found.");
        for warning in warnings {
            println!("  warning: {warning}");
        }
        return;
    }
    if matches.len() > 1 {
        println!("MCP server `{server_name}` is ambiguous across:");
        for server in matches {
            println!(
                "  - {}",
                source_label(server.origin_kind, &server.origin_name, &server.config_path)
            );
        }
        return;
    }

    let server = &matches[0];
    println!("MCP server: {}", server.name);
    println!("  enabled: {}", server.enabled);
    println!("  transport: {}", transport_label(server.transport));
    println!(
        "  source: {}",
        source_label(server.origin_kind, &server.origin_name, &server.config_path)
    );
    println!(
        "  startup timeout: {}",
        server
            .startup_timeout_secs
            .map_or_else(|| "default".to_owned(), |value| format!("{value}s"))
    );
    println!(
        "  request timeout: {}",
        server
            .request_timeout_secs
            .map_or_else(|| "default".to_owned(), |value| format!("{value}s"))
    );
    if server.metadata_keys.is_empty() {
        println!("  metadata: none");
    } else {
        println!("  metadata keys: {}", server.metadata_keys.join(", "));
    }
    for warning in warnings {
        println!("  warning: {warning}");
    }
}

fn set_server_enabled(config: &RuntimeConfig, server_name: &str, project: bool, enabled: bool) {
    let path = managed_mcp_config_path(config, project);
    let mut mcp_config = match load_managed_mcp_config(&path) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("Failed to load MCP config {}: {error}", path.display());
            return;
        }
    };
    let Some(server) = mcp_config.servers.get_mut(server_name) else {
        println!(
            "No managed MCP server named `{server_name}` exists in {}.",
            path.display()
        );
        return;
    };
    if server.enabled == enabled {
        println!(
            "MCP server {} already {} in {}.",
            server_name,
            if enabled { "enabled" } else { "disabled" },
            path.display()
        );
        return;
    }
    server.enabled = enabled;
    if let Err(error) = mcp_config.save(&path) {
        eprintln!("Failed to save MCP config {}: {error}", path.display());
        return;
    }
    println!(
        "MCP server {} {} in {}.",
        server_name,
        if enabled { "enabled" } else { "disabled" },
        path.display()
    );
}

fn reset_managed_config(config: &RuntimeConfig, project: bool) {
    let path = managed_mcp_config_path(config, project);
    if !path.exists() {
        println!("Managed MCP config already absent at {}.", path.display());
        return;
    }
    match std::fs::remove_file(&path) {
        Ok(()) => println!("Managed MCP config reset at {}.", path.display()),
        Err(error) => eprintln!("Failed to reset MCP config {}: {error}", path.display()),
    }
}

fn discover_mcp_servers(config: &RuntimeConfig) -> (Vec<String>, Vec<McpServerView>) {
    let mut warnings = Vec::new();
    let mut servers = Vec::new();

    if setting_source_enabled(config, SettingSource::Project) {
        load_mcp_servers_from_path(
            &mut warnings,
            &mut servers,
            "cwd",
            &config.cwd.display().to_string(),
            &config.cwd.join(rc_mcp::DEFAULT_MCP_CONFIG_FILE),
        );
    }
    if setting_source_enabled(config, SettingSource::User) {
        load_mcp_servers_from_path(
            &mut warnings,
            &mut servers,
            "profile",
            &config.paths.profile_dir.display().to_string(),
            &config
                .paths
                .profile_dir
                .join(rc_mcp::DEFAULT_MCP_CONFIG_FILE),
        );
    }

    if setting_source_enabled(config, SettingSource::User) && config.paths.plugins_dir.exists() {
        match rc_plugins::discover_plugins(&config.paths.plugins_dir) {
            Ok(plugins) => {
                for plugin in plugins {
                    if let Some(path) = plugin.mcp_config_path() {
                        load_mcp_servers_from_path(
                            &mut warnings,
                            &mut servers,
                            "plugin",
                            &plugin.manifest.name,
                            &path,
                        );
                    }
                }
            }
            Err(error) => warnings.push(format!(
                "Failed to discover plugins in {}: {error}",
                config.paths.plugins_dir.display()
            )),
        }
    }

    (warnings, servers)
}

fn setting_source_enabled(config: &RuntimeConfig, source: SettingSource) -> bool {
    config.allowed_setting_sources.contains(&source)
}

pub(crate) fn discovered_server_count(config: &RuntimeConfig) -> usize {
    discover_mcp_servers(config).1.len()
}

fn load_mcp_servers_from_path(
    warnings: &mut Vec<String>,
    servers: &mut Vec<McpServerView>,
    origin_kind: &'static str,
    origin_name: &str,
    path: &Path,
) {
    if !path.exists() {
        return;
    }
    match rc_mcp::McpConfig::load(path) {
        Ok(config) => {
            for server in config.servers.into_values() {
                let mut metadata_keys = server.metadata.keys().cloned().collect::<Vec<_>>();
                metadata_keys.sort();
                servers.push(McpServerView {
                    name: server.name,
                    enabled: server.enabled,
                    transport: server.transport.kind(),
                    origin_kind,
                    origin_name: origin_name.to_owned(),
                    config_path: path.to_path_buf(),
                    startup_timeout_secs: server.startup_timeout_secs,
                    request_timeout_secs: server.request_timeout_secs,
                    metadata_keys,
                });
            }
        }
        Err(error) => warnings.push(format!(
            "Failed to load MCP config {}: {error}",
            path.display()
        )),
    }
}

fn managed_mcp_config_path(config: &RuntimeConfig, project: bool) -> PathBuf {
    if project {
        config.cwd.join(rc_mcp::DEFAULT_MCP_CONFIG_FILE)
    } else {
        config
            .paths
            .profile_dir
            .join(rc_mcp::DEFAULT_MCP_CONFIG_FILE)
    }
}

fn load_managed_mcp_config(path: &Path) -> anyhow::Result<rc_mcp::McpConfig> {
    if path.exists() {
        Ok(rc_mcp::McpConfig::load(path)?)
    } else {
        Ok(rc_mcp::McpConfig::default())
    }
}

fn transport_label(transport: rc_mcp::McpTransport) -> &'static str {
    match transport {
        rc_mcp::McpTransport::Stdio => "stdio",
        rc_mcp::McpTransport::Http => "http",
        rc_mcp::McpTransport::WebSocket => "websocket",
    }
}

fn source_label(origin_kind: &str, origin_name: &str, path: &Path) -> String {
    match origin_kind {
        "plugin" => format!("plugin:{origin_name} ({})", path.display()),
        _ => format!("{origin_kind}:{origin_name} ({})", path.display()),
    }
}

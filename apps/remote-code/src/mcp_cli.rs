use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use rc_config::{RUNTIME_VERSION, RuntimeConfig};

use crate::cli::{McpAddArgs, McpCallArgs, McpCommand, McpGetArgs, McpListArgs, McpRemoveArgs};

#[derive(Debug, Clone)]
pub(crate) struct RuntimeMcpServerEntry {
    pub(crate) origin_kind: &'static str,
    pub(crate) origin_name: String,
    pub(crate) config_path: PathBuf,
    pub(crate) server: rc_mcp::McpServerConfig,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RuntimeMcpDiscovery {
    pub(crate) servers: Vec<RuntimeMcpServerEntry>,
    pub(crate) warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimeMcpResolution {
    pub(crate) entry: RuntimeMcpServerEntry,
    pub(crate) warnings: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct McpListOutput {
    pub(crate) warnings: Vec<String>,
    pub(crate) servers: Vec<McpServerRecord>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct McpServerRecord {
    pub(crate) name: String,
    pub(crate) enabled: bool,
    pub(crate) transport: rc_mcp::McpTransport,
    pub(crate) origin_kind: String,
    pub(crate) origin_name: String,
    pub(crate) config_path: PathBuf,
    pub(crate) live: Option<McpLiveRecord>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct McpLiveRecord {
    pub(crate) status: String,
    pub(crate) protocol_version: Option<String>,
    pub(crate) server_info: Option<rc_mcp::McpPeerInfo>,
    pub(crate) tool_count: usize,
    pub(crate) tools: Vec<rc_mcp::McpToolDescriptor>,
    pub(crate) error: Option<String>,
}

impl McpLiveRecord {
    pub(crate) fn from_inspection(inspection: rc_mcp::McpServerInspection) -> Self {
        Self {
            status: "ok".to_owned(),
            protocol_version: Some(inspection.protocol_version),
            server_info: inspection.server_info,
            tool_count: inspection.tools.len(),
            tools: inspection.tools,
            error: None,
        }
    }

    pub(crate) fn skipped(reason: impl Into<String>) -> Self {
        Self {
            status: "skipped".to_owned(),
            protocol_version: None,
            server_info: None,
            tool_count: 0,
            tools: Vec::new(),
            error: Some(reason.into()),
        }
    }

    pub(crate) fn failed(error: &impl ToString) -> Self {
        Self {
            status: "error".to_owned(),
            protocol_version: None,
            server_info: None,
            tool_count: 0,
            tools: Vec::new(),
            error: Some(error.to_string()),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct McpCallOutput {
    pub(crate) warnings: Vec<String>,
    pub(crate) server: McpCallServerRecord,
    pub(crate) arguments: serde_json::Value,
    pub(crate) response: rc_mcp::McpToolCallResponse,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct McpCallServerRecord {
    pub(crate) name: String,
    pub(crate) enabled: bool,
    pub(crate) origin_kind: String,
    pub(crate) origin_name: String,
    pub(crate) config_path: PathBuf,
}

pub(crate) async fn run_mcp(config: &RuntimeConfig, command: McpCommand) -> Result<()> {
    match command {
        McpCommand::List(args) => run_mcp_list(config, args).await,
        McpCommand::Get(args) => run_mcp_get(config, args).await,
        McpCommand::Add(args) => run_mcp_add(config, args),
        McpCommand::Remove(args) => run_mcp_remove(config, args),
        McpCommand::Call(args) => run_mcp_call(config, args).await,
    }
}

async fn run_mcp_list(config: &RuntimeConfig, args: McpListArgs) -> Result<()> {
    let output = build_mcp_list_output(config, &args).await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    if output.servers.is_empty() {
        println!("No MCP servers found.");
        for warning in output.warnings {
            println!("  - {warning}");
        }
        return Ok(());
    }

    for warning in &output.warnings {
        println!("warning: {warning}");
    }
    for server in &output.servers {
        println!(
            "{}  {}  {}  {}",
            server.name,
            if server.enabled {
                "enabled"
            } else {
                "disabled"
            },
            format_mcp_transport(server.transport),
            format_mcp_source(server)
        );
        if let Some(live) = &server.live {
            match live.status.as_str() {
                "ok" => {
                    let peer = live.server_info.as_ref().map_or_else(
                        || "unknown-server".to_owned(),
                        |info| match &info.version {
                            Some(version) => format!("{} {}", info.name, version),
                            None => info.name.clone(),
                        },
                    );
                    println!(
                        "  connect: ok  protocol={}  tools={}  peer={peer}",
                        live.protocol_version.as_deref().unwrap_or("unknown"),
                        live.tool_count
                    );
                    for tool in &live.tools {
                        match &tool.description {
                            Some(description) => println!("    - {}: {description}", tool.name),
                            None => println!("    - {}", tool.name),
                        }
                    }
                }
                "skipped" => {
                    println!(
                        "  connect: skipped  {}",
                        live.error.as_deref().unwrap_or("inspection not attempted")
                    );
                }
                _ => {
                    println!(
                        "  connect: error  {}",
                        live.error
                            .as_deref()
                            .unwrap_or("inspection failed without details")
                    );
                }
            }
        }
    }
    Ok(())
}

async fn run_mcp_get(config: &RuntimeConfig, args: McpGetArgs) -> Result<()> {
    let output = build_mcp_list_output(
        config,
        &McpListArgs {
            connect: args.connect,
            json: args.json,
            servers: vec![args.server.clone()],
            include_disabled: args.include_disabled,
            config_paths: args.config_paths.clone(),
        },
    )
    .await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    let Some(server) = output.servers.first() else {
        return Err(anyhow!("No MCP server named `{}` was found", args.server));
    };
    for warning in &output.warnings {
        println!("warning: {warning}");
    }
    println!("server: {}", server.name);
    println!("enabled: {}", server.enabled);
    println!("transport: {}", format_mcp_transport(server.transport));
    println!("source: {}", format_mcp_source(server));
    if let Some(live) = &server.live {
        println!("connect: {}", live.status);
        if let Some(error) = &live.error {
            println!("connect detail: {error}");
        }
        if !live.tools.is_empty() {
            println!("tools:");
            for tool in &live.tools {
                println!("  - {}", tool.name);
            }
        }
    }
    Ok(())
}

fn run_mcp_add(config: &RuntimeConfig, args: McpAddArgs) -> Result<()> {
    let config_path = managed_mcp_config_path(config, args.config_path.as_ref(), args.project);
    let mut mcp_config = load_managed_mcp_config(&config_path)?;
    let existed = mcp_config.servers.contains_key(&args.name);
    let transport = match (&args.command, &args.url) {
        (Some(command), None) => rc_mcp::McpTransportConfig::Stdio {
            command: command.clone(),
            args: args.args.clone(),
            cwd: args.cwd.clone(),
            env: parse_string_map("--env", &args.env)?,
        },
        (None, Some(url)) => {
            let headers = parse_string_map("--meta", &args.metadata)?;
            if url.starts_with("ws://") || url.starts_with("wss://") {
                rc_mcp::McpTransportConfig::WebSocket {
                    url: url.clone(),
                    headers,
                }
            } else {
                rc_mcp::McpTransportConfig::Http {
                    url: url.clone(),
                    headers,
                }
            }
        }
        (Some(_), Some(_)) => {
            return Err(anyhow!("Pass either --command or --url, not both"));
        }
        (None, None) => {
            return Err(anyhow!("Either --command or --url is required"));
        }
    };

        let metadata = if matches!(transport, rc_mcp::McpTransportConfig::Stdio { .. }) {
            parse_string_map("--meta", &args.metadata)?
        } else {
            BTreeMap::new()
        };
    mcp_config.servers.insert(
        args.name.clone(),
        rc_mcp::McpServerConfig {
            name: args.name.clone(),
            enabled: !args.disabled,
            transport,
            capabilities: rc_mcp::McpCapabilityMatrix::default(),
            startup_timeout_secs: args.startup_timeout_secs,
            request_timeout_secs: args.request_timeout_secs,
            metadata,
        },
    );
    mcp_config.save(&config_path)?;

    let output = serde_json::json!({
        "status": if existed { "updated" } else { "created" },
        "name": args.name,
        "config_path": config_path,
    });
    if args.json {
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!(
            "MCP server {} at {}.",
            output["status"].as_str().unwrap_or("saved"),
            output["config_path"].as_str().unwrap_or_default()
        );
    }
    Ok(())
}

fn run_mcp_remove(config: &RuntimeConfig, args: McpRemoveArgs) -> Result<()> {
    let config_path = managed_mcp_config_path(config, args.config_path.as_ref(), args.project);
    let mut mcp_config = load_managed_mcp_config(&config_path)?;
    let removed = mcp_config.servers.remove(&args.name);
    if removed.is_none() && !args.if_exists {
        return Err(anyhow!(
            "No MCP server named `{}` exists in {}",
            args.name,
            config_path.display()
        ));
    }
    mcp_config.save(&config_path)?;
    let output = serde_json::json!({
        "status": if removed.is_some() { "removed" } else { "noop" },
        "name": args.name,
        "config_path": config_path,
    });
    if args.json {
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else {
        println!(
            "MCP server {} in {}.",
            output["status"].as_str().unwrap_or("saved"),
            output["config_path"].as_str().unwrap_or_default()
        );
    }
    Ok(())
}

async fn run_mcp_call(config: &RuntimeConfig, args: McpCallArgs) -> Result<()> {
    let output = build_mcp_call_output(config, &args).await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    for warning in &output.warnings {
        println!("warning: {warning}");
    }
    println!(
        "server: {}  {}",
        output.server.name,
        format_mcp_call_source(&output.server)
    );
    println!("tool: {}", output.response.tool_name);
    println!(
        "status: {}",
        if output.response.result.is_error {
            "error"
        } else {
            "ok"
        }
    );
    println!("protocol: {}", output.response.protocol_version);
    if let Some(server_info) = &output.response.server_info {
        match &server_info.version {
            Some(version) => println!("peer: {} {}", server_info.name, version),
            None => println!("peer: {}", server_info.name),
        }
    }

    if !output.response.result.content.is_empty() {
        println!("content:");
        for block in &output.response.result.content {
            if block.kind == "text"
                && let Some(text) = block.fields.get("text").and_then(serde_json::Value::as_str)
            {
                for line in text.lines() {
                    println!("  {line}");
                }
            } else {
                println!("  {}", serde_json::to_string_pretty(block)?);
            }
        }
    }

    if let Some(structured) = &output.response.result.structured_content {
        println!("structured:");
        println!("{}", serde_json::to_string_pretty(structured)?);
    }

    Ok(())
}

pub(crate) async fn build_mcp_list_output(
    config: &RuntimeConfig,
    args: &McpListArgs,
) -> Result<McpListOutput> {
    let discovery = discover_runtime_mcp_servers(config, &args.config_paths);
    let filters = args.servers.iter().cloned().collect::<BTreeSet<_>>();
    let mut servers = Vec::new();

    for entry in discovery.servers {
        if !filters.is_empty() && !filters.contains(&entry.server.name) {
            continue;
        }
        let live = if args.connect {
            if !entry.server.enabled && !args.include_disabled {
                Some(McpLiveRecord::skipped(
                    "server is disabled (pass --include-disabled to force inspection)",
                ))
            } else {
                Some(
                    match rc_mcp::inspect_server(
                        &entry.server,
                        &rc_mcp::McpClientInfo::new("remote-code-rust", RUNTIME_VERSION),
                    )
                    .await
                    {
                        Ok(inspection) => McpLiveRecord::from_inspection(inspection),
                        Err(error) => McpLiveRecord::failed(&error),
                    },
                )
            }
        } else {
            None
        };

        servers.push(McpServerRecord {
            name: entry.server.name.clone(),
            enabled: entry.server.enabled,
            transport: entry.server.transport.kind(),
            origin_kind: entry.origin_kind.to_owned(),
            origin_name: entry.origin_name,
            config_path: entry.config_path,
            live,
        });
    }

    if !filters.is_empty() && servers.is_empty() {
        return Err(anyhow!(
            "No matching MCP servers found for: {}",
            filters.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }

    Ok(McpListOutput {
        warnings: discovery.warnings,
        servers,
    })
}

pub(crate) async fn build_mcp_call_output(
    config: &RuntimeConfig,
    args: &McpCallArgs,
) -> Result<McpCallOutput> {
    let resolution = resolve_runtime_mcp_server(config, &args.server, &args.config_paths)?;
    if !resolution.entry.server.enabled && !args.include_disabled {
        return Err(anyhow!(
            "MCP server `{}` is disabled; pass --include-disabled to force a tool call",
            args.server
        ));
    }

    let arguments = parse_mcp_call_arguments(args)?;
    let response = rc_mcp::call_tool(
        &resolution.entry.server,
        &rc_mcp::McpClientInfo::new("remote-code-rust", RUNTIME_VERSION),
        &args.tool,
        arguments.clone(),
    )
    .await?;

    Ok(McpCallOutput {
        warnings: resolution.warnings,
        server: McpCallServerRecord {
            name: resolution.entry.server.name.clone(),
            enabled: resolution.entry.server.enabled,
            origin_kind: resolution.entry.origin_kind.to_owned(),
            origin_name: resolution.entry.origin_name,
            config_path: resolution.entry.config_path,
        },
        arguments,
        response,
    })
}

pub(crate) fn parse_mcp_call_arguments(args: &McpCallArgs) -> Result<serde_json::Value> {
    parse_named_json_object_args("--args-json", args.args_json.as_ref(), &args.args)
}

pub(crate) fn parse_named_json_object_args(
    json_flag_name: &str,
    json_value: Option<&String>,
    args: &[String],
) -> Result<serde_json::Value> {
    let mut object = match json_value {
        Some(raw) => {
            let parsed: serde_json::Value = serde_json::from_str(raw)
                .map_err(|error| anyhow!("failed to parse {json_flag_name} as JSON: {error}"))?;
            match parsed {
                serde_json::Value::Object(map) => map,
                _ => return Err(anyhow!("{json_flag_name} must be a JSON object")),
            }
        }
        None => serde_json::Map::new(),
    };

    for pair in args {
        let (key, raw_value) = pair
            .split_once('=')
            .ok_or_else(|| anyhow!("invalid --arg `{pair}`; expected key=value"))?;
        let key = key.trim();
        if key.is_empty() {
            return Err(anyhow!("invalid --arg `{pair}`; key cannot be empty"));
        }
        let value = match serde_json::from_str::<serde_json::Value>(raw_value.trim()) {
            Ok(parsed) => parsed,
            Err(_) => serde_json::Value::String(raw_value.trim().to_owned()),
        };
        object.insert(key.to_owned(), value);
    }

    Ok(serde_json::Value::Object(object))
}

fn parse_string_map(flag_name: &str, entries: &[String]) -> Result<std::collections::BTreeMap<String, String>> {
    let mut map = std::collections::BTreeMap::new();
    for entry in entries {
        let (key, value) = entry
            .split_once('=')
            .ok_or_else(|| anyhow!("invalid {flag_name} `{entry}`; expected key=value"))?;
        let key = key.trim();
        if key.is_empty() {
            return Err(anyhow!("invalid {flag_name} `{entry}`; key cannot be empty"));
        }
        map.insert(key.to_owned(), value.trim().to_owned());
    }
    Ok(map)
}

fn managed_mcp_config_path(
    config: &RuntimeConfig,
    override_path: Option<&PathBuf>,
    project: bool,
) -> PathBuf {
    override_path
        .cloned()
        .unwrap_or_else(|| {
            if project {
                config.cwd.join(rc_mcp::DEFAULT_MCP_CONFIG_FILE)
            } else {
                config
                    .paths
                    .profile_dir
                    .join(rc_mcp::DEFAULT_MCP_CONFIG_FILE)
            }
        })
}

fn load_managed_mcp_config(path: &Path) -> Result<rc_mcp::McpConfig> {
    if path.exists() {
        Ok(rc_mcp::McpConfig::load(path)?)
    } else {
        Ok(rc_mcp::McpConfig::default())
    }
}

pub(crate) fn resolve_runtime_mcp_server(
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

pub(crate) fn discover_runtime_mcp_servers(
    config: &RuntimeConfig,
    extra_config_paths: &[PathBuf],
) -> RuntimeMcpDiscovery {
    let mut discovery = RuntimeMcpDiscovery::default();
    let mut loaded_paths = BTreeSet::new();
    load_runtime_mcp_file(
        &mut discovery,
        &mut loaded_paths,
        "cwd",
        &config.cwd.display().to_string(),
        &config.cwd.join(rc_mcp::DEFAULT_MCP_CONFIG_FILE),
    );
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

    if config.paths.plugins_dir.exists() {
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

pub(crate) fn format_mcp_transport(transport: rc_mcp::McpTransport) -> &'static str {
    match transport {
        rc_mcp::McpTransport::Stdio => "stdio",
        rc_mcp::McpTransport::Http => "http",
        rc_mcp::McpTransport::WebSocket => "websocket",
    }
}

pub(crate) fn format_mcp_source(server: &McpServerRecord) -> String {
    match server.origin_kind.as_str() {
        "plugin" => format!(
            "plugin:{} ({})",
            server.origin_name,
            server.config_path.display()
        ),
        _ => format!("{} ({})", server.origin_kind, server.config_path.display()),
    }
}

pub(crate) fn format_mcp_call_source(server: &McpCallServerRecord) -> String {
    match server.origin_kind.as_str() {
        "plugin" => format!(
            "plugin:{} ({})",
            server.origin_name,
            server.config_path.display()
        ),
        _ => format!("{} ({})", server.origin_kind, server.config_path.display()),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rc_config::{ProviderOverrides, RuntimeOverrides, load_runtime_config};
    use tempfile::tempdir;

    use super::{load_managed_mcp_config, managed_mcp_config_path, parse_string_map, run_mcp_add, run_mcp_remove};
    use crate::cli::{McpAddArgs, McpRemoveArgs};

    fn test_config() -> (tempfile::TempDir, rc_config::RuntimeConfig) {
        let tempdir = tempdir().expect("tempdir");
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join(".remote-code-rust");
        fs::create_dir_all(&cwd).expect("cwd");
        fs::create_dir_all(&profile).expect("profile");
        let config = load_runtime_config(
            Some(cwd),
            Some(profile),
            None,
            rc_core::PermissionMode::Default,
            rc_core::InputFormat::Text,
            rc_core::OutputFormat::Text,
            false,
            false,
            false,
            false,
            4,
            ProviderOverrides::default(),
            RuntimeOverrides::default(),
        )
        .expect("config");
        (tempdir, config)
    }

    #[test]
    fn parse_string_map_requires_key_value_shape() {
        let parsed = parse_string_map("--env", &["FOO=bar".to_owned()]).expect("parse");
        assert_eq!(parsed.get("FOO").map(String::as_str), Some("bar"));
        assert!(parse_string_map("--env", &["oops".to_owned()]).is_err());
    }

    #[test]
    fn managed_mcp_add_and_remove_round_trip() {
        let (_tempdir, config) = test_config();
        run_mcp_add(
            &config,
            McpAddArgs {
                name: "demo".to_owned(),
                command: Some("python".to_owned()),
                url: None,
                args: vec!["server.py".to_owned()],
                cwd: None,
                env: vec!["TOKEN=secret".to_owned()],
                disabled: false,
                startup_timeout_secs: Some(3),
                request_timeout_secs: Some(5),
                metadata: vec!["scope=local".to_owned()],
                json: false,
                config_path: None,
                project: false,
            },
        )
        .expect("add mcp");

        let path = managed_mcp_config_path(&config, None, false);
        let loaded = load_managed_mcp_config(&path).expect("load config");
        assert!(loaded.servers.contains_key("demo"));

        run_mcp_remove(
            &config,
            McpRemoveArgs {
                name: "demo".to_owned(),
                json: false,
                config_path: None,
                project: false,
                if_exists: false,
            },
        )
        .expect("remove mcp");
        let loaded = load_managed_mcp_config(&path).expect("reload config");
        assert!(!loaded.servers.contains_key("demo"));
    }
}

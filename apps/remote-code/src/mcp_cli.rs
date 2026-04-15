use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Result, anyhow};
use rc_config::{RUNTIME_VERSION, RuntimeConfig};
use rc_tools::mcp_runtime::{
    RuntimeMcpResolution, discover_runtime_mcp_servers, resolve_runtime_mcp_server,
};

use crate::cli::{
    McpAddArgs, McpCallArgs, McpCommand, McpGetArgs, McpListArgs, McpRemoveArgs, McpResetArgs,
    McpServeArgs, McpToggleArgs,
};

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

#[derive(Debug, Clone, serde::Serialize)]
struct McpMutationOutput {
    status: String,
    name: Option<String>,
    enabled: Option<bool>,
    config_path: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize)]
struct McpServeOutput {
    warnings: Vec<String>,
    server: String,
    enabled: bool,
    transport: rc_mcp::McpTransport,
    command: Option<String>,
    args: Vec<String>,
    cwd: Option<PathBuf>,
    env_keys: Vec<String>,
    config_path: PathBuf,
}

pub(crate) async fn run_mcp(config: &RuntimeConfig, command: McpCommand) -> Result<()> {
    match command {
        McpCommand::List(args) => run_mcp_list(config, args).await,
        McpCommand::Get(args) => run_mcp_get(config, args).await,
        McpCommand::Add(args) => run_mcp_add(config, args),
        McpCommand::Remove(args) => run_mcp_remove(config, args),
        McpCommand::Enable(args) => run_mcp_toggle(config, args, true),
        McpCommand::Disable(args) => run_mcp_toggle(config, args, false),
        McpCommand::Reset(args) => run_mcp_reset(config, args),
        McpCommand::Serve(args) => run_mcp_serve(config, args).await,
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

fn run_mcp_toggle(config: &RuntimeConfig, args: McpToggleArgs, enabled: bool) -> Result<()> {
    let config_path = managed_mcp_config_path(config, args.config_path.as_ref(), args.project);
    let mut mcp_config = load_managed_mcp_config(&config_path)?;
    let Some(server) = mcp_config.servers.get_mut(&args.name) else {
        if args.if_exists {
            let output = McpMutationOutput {
                status: "noop".to_owned(),
                name: Some(args.name),
                enabled: Some(enabled),
                config_path,
            };
            if args.json {
                println!("{}", serde_json::to_string_pretty(&output)?);
            } else {
                println!(
                    "MCP server already absent from {}.",
                    output.config_path.display()
                );
            }
            return Ok(());
        }
        return Err(anyhow!(
            "No MCP server named `{}` exists in {}",
            args.name,
            config_path.display()
        ));
    };
    let status = if server.enabled == enabled {
        "noop"
    } else {
        server.enabled = enabled;
        mcp_config.save(&config_path)?;
        if enabled { "enabled" } else { "disabled" }
    };
    let output = McpMutationOutput {
        status: status.to_owned(),
        name: Some(args.name),
        enabled: Some(enabled),
        config_path,
    };
    if args.json {
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else if output.status == "noop" {
        println!(
            "MCP server {} already {} in {}.",
            output.name.as_deref().unwrap_or("unknown"),
            if enabled { "enabled" } else { "disabled" },
            output.config_path.display()
        );
    } else {
        println!(
            "MCP server {} {} in {}.",
            output.name.as_deref().unwrap_or("unknown"),
            output.status,
            output.config_path.display()
        );
    }
    Ok(())
}

fn run_mcp_reset(config: &RuntimeConfig, args: McpResetArgs) -> Result<()> {
    let config_path = managed_mcp_config_path(config, args.config_path.as_ref(), args.project);
    let status = if config_path.exists() {
        std::fs::remove_file(&config_path)?;
        "reset"
    } else if args.if_exists {
        "noop"
    } else {
        return Err(anyhow!(
            "Managed MCP config {} does not exist",
            config_path.display()
        ));
    };

    let output = McpMutationOutput {
        status: status.to_owned(),
        name: None,
        enabled: None,
        config_path,
    };
    if args.json {
        println!("{}", serde_json::to_string_pretty(&output)?);
    } else if output.status == "noop" {
        println!(
            "Managed MCP config already absent at {}.",
            output.config_path.display()
        );
    } else {
        println!(
            "Managed MCP config reset at {}.",
            output.config_path.display()
        );
    }
    Ok(())
}

async fn run_mcp_serve(config: &RuntimeConfig, args: McpServeArgs) -> Result<()> {
    let resolution = resolve_runtime_mcp_server(config, &args.server, &args.config_paths)?;
    if !resolution.entry.server.enabled && !args.include_disabled {
        return Err(anyhow!(
            "MCP server `{}` is disabled; pass --include-disabled to launch it anyway",
            args.server
        ));
    }

    let launch = mcp_serve_output_from_resolution(&resolution);
    if args.json {
        println!("{}", serde_json::to_string_pretty(&launch)?);
        return Ok(());
    }
    for warning in &launch.warnings {
        println!("warning: {warning}");
    }

    let rc_mcp::McpTransportConfig::Stdio {
        command,
        args,
        cwd,
        env,
    } = &resolution.entry.server.transport
    else {
        return Err(anyhow!(
            "MCP server `{}` uses {} transport and cannot be launched locally via `mcp serve`",
            args.server,
            format_mcp_transport(resolution.entry.server.transport.kind())
        ));
    };

    println!(
        "Launching MCP server {} from {}",
        resolution.entry.server.name,
        resolution.entry.config_path.display()
    );
    if let Some(cwd) = cwd {
        println!("cwd: {}", cwd.display());
    }
    let mut command_builder = tokio::process::Command::new(command);
    command_builder
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .kill_on_drop(true);
    if let Some(cwd) = cwd {
        command_builder.current_dir(cwd);
    }
    if !env.is_empty() {
        command_builder.envs(env);
    }
    let mut child = command_builder.spawn().map_err(|error| {
        anyhow!(
            "Failed to launch MCP server `{}` with `{}`: {error}",
            resolution.entry.server.name,
            command
        )
    })?;
    let status = child.wait().await?;
    if !status.success() {
        return Err(anyhow!(
            "MCP server `{}` exited with status {}",
            resolution.entry.server.name,
            status
        ));
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

fn parse_string_map(
    flag_name: &str,
    entries: &[String],
) -> Result<std::collections::BTreeMap<String, String>> {
    let mut map = std::collections::BTreeMap::new();
    for entry in entries {
        let (key, value) = entry
            .split_once('=')
            .ok_or_else(|| anyhow!("invalid {flag_name} `{entry}`; expected key=value"))?;
        let key = key.trim();
        if key.is_empty() {
            return Err(anyhow!(
                "invalid {flag_name} `{entry}`; key cannot be empty"
            ));
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
    override_path.cloned().unwrap_or_else(|| {
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

fn mcp_serve_output_from_resolution(resolution: &RuntimeMcpResolution) -> McpServeOutput {
    let (command, args, cwd, env_keys) = match &resolution.entry.server.transport {
        rc_mcp::McpTransportConfig::Stdio {
            command,
            args,
            cwd,
            env,
        } => {
            let mut env_keys = env.keys().cloned().collect::<Vec<_>>();
            env_keys.sort();
            (Some(command.clone()), args.clone(), cwd.clone(), env_keys)
        }
        _ => (None, Vec::new(), None, Vec::new()),
    };

    McpServeOutput {
        warnings: resolution.warnings.clone(),
        server: resolution.entry.server.name.clone(),
        enabled: resolution.entry.server.enabled,
        transport: resolution.entry.server.transport.kind(),
        command,
        args,
        cwd,
        env_keys,
        config_path: resolution.entry.config_path.clone(),
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

    use rc_config::{ProviderOverrides, RuntimeOverrides, SettingSource, load_runtime_config};
    use tempfile::tempdir;

    use super::{
        load_managed_mcp_config, managed_mcp_config_path, mcp_serve_output_from_resolution,
        parse_string_map, run_mcp_add, run_mcp_remove, run_mcp_reset, run_mcp_toggle,
    };
    use crate::cli::{McpAddArgs, McpRemoveArgs, McpResetArgs, McpToggleArgs};
    use rc_tools::mcp_runtime::{discover_runtime_mcp_servers, resolve_runtime_mcp_server};

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

    #[test]
    fn managed_mcp_toggle_round_trip() {
        let (_tempdir, config) = test_config();
        run_mcp_add(
            &config,
            McpAddArgs {
                name: "demo".to_owned(),
                command: Some("python".to_owned()),
                url: None,
                args: vec!["server.py".to_owned()],
                cwd: None,
                env: Vec::new(),
                disabled: false,
                startup_timeout_secs: None,
                request_timeout_secs: None,
                metadata: Vec::new(),
                json: false,
                config_path: None,
                project: false,
            },
        )
        .expect("add mcp");

        run_mcp_toggle(
            &config,
            McpToggleArgs {
                name: "demo".to_owned(),
                json: false,
                config_path: None,
                project: false,
                if_exists: false,
            },
            false,
        )
        .expect("disable mcp");

        let path = managed_mcp_config_path(&config, None, false);
        let loaded = load_managed_mcp_config(&path).expect("load config");
        assert!(!loaded.servers.get("demo").expect("demo").enabled);

        run_mcp_toggle(
            &config,
            McpToggleArgs {
                name: "demo".to_owned(),
                json: false,
                config_path: None,
                project: false,
                if_exists: false,
            },
            true,
        )
        .expect("enable mcp");
        let loaded = load_managed_mcp_config(&path).expect("reload config");
        assert!(loaded.servers.get("demo").expect("demo").enabled);
    }

    #[test]
    fn managed_mcp_reset_removes_file() {
        let (_tempdir, config) = test_config();
        run_mcp_add(
            &config,
            McpAddArgs {
                name: "demo".to_owned(),
                command: Some("python".to_owned()),
                url: None,
                args: vec!["server.py".to_owned()],
                cwd: None,
                env: Vec::new(),
                disabled: false,
                startup_timeout_secs: None,
                request_timeout_secs: None,
                metadata: Vec::new(),
                json: false,
                config_path: None,
                project: false,
            },
        )
        .expect("add mcp");
        let path = managed_mcp_config_path(&config, None, false);
        assert!(path.exists());

        run_mcp_reset(
            &config,
            McpResetArgs {
                json: false,
                config_path: None,
                project: false,
                if_exists: false,
            },
        )
        .expect("reset mcp");
        assert!(!path.exists());
    }

    #[test]
    fn mcp_serve_output_reports_stdio_launch_plan() {
        let (_tempdir, config) = test_config();
        run_mcp_add(
            &config,
            McpAddArgs {
                name: "demo".to_owned(),
                command: Some("python".to_owned()),
                url: None,
                args: vec!["server.py".to_owned()],
                cwd: Some(config.cwd.clone()),
                env: vec!["TOKEN=secret".to_owned()],
                disabled: false,
                startup_timeout_secs: None,
                request_timeout_secs: None,
                metadata: Vec::new(),
                json: false,
                config_path: None,
                project: false,
            },
        )
        .expect("add mcp");

        let resolution =
            resolve_runtime_mcp_server(&config, "demo", &[]).expect("resolve runtime server");
        let launch = mcp_serve_output_from_resolution(&resolution);
        assert_eq!(launch.server, "demo");
        assert_eq!(launch.command.as_deref(), Some("python"));
        assert_eq!(launch.args, vec!["server.py".to_owned()]);
        assert_eq!(launch.env_keys, vec!["TOKEN".to_owned()]);
        assert_eq!(launch.cwd, Some(config.cwd.clone()));
    }

    #[test]
    fn runtime_mcp_discovery_respects_setting_sources_but_keeps_explicit_paths() {
        let tempdir = tempdir().expect("tempdir");
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join(".remote-code-rust");
        let plugin_root = profile.join("plugins").join("sample");
        let extra_root = tempdir.path().join("external");
        fs::create_dir_all(&cwd).expect("cwd");
        fs::create_dir_all(&profile).expect("profile");
        fs::create_dir_all(plugin_root.join(".codex-plugin")).expect("plugin dir");
        fs::create_dir_all(&extra_root).expect("extra root");
        fs::write(
            cwd.join(rc_mcp::DEFAULT_MCP_CONFIG_FILE),
            r#"[servers.project]
command = "python"
args = ["project.py"]"#,
        )
        .expect("write project mcp");
        fs::write(
            profile.join(rc_mcp::DEFAULT_MCP_CONFIG_FILE),
            r#"[servers.profile]
command = "python"
args = ["profile.py"]"#,
        )
        .expect("write profile mcp");
        fs::write(
            plugin_root.join(".codex-plugin").join("plugin.json"),
            r#"{"name":"sample","version":"0.1.0","mcp":"./mcp.toml"}"#,
        )
        .expect("write plugin manifest");
        fs::write(
            plugin_root.join("mcp.toml"),
            r#"[servers.plugin]
command = "python"
args = ["plugin.py"]"#,
        )
        .expect("write plugin mcp");
        fs::write(
            extra_root.join(rc_mcp::DEFAULT_MCP_CONFIG_FILE),
            r#"[servers.explicit]
command = "python"
args = ["explicit.py"]"#,
        )
        .expect("write explicit mcp");

        let project_only = load_runtime_config(
            Some(cwd.clone()),
            Some(profile.clone()),
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
            RuntimeOverrides {
                allowed_setting_sources: Some(vec![SettingSource::Project]),
                ..RuntimeOverrides::default()
            },
        )
        .expect("project config");
        let discovery = discover_runtime_mcp_servers(&project_only, &[]);
        assert_eq!(
            discovery
                .servers
                .iter()
                .map(|entry| entry.server.name.clone())
                .collect::<Vec<_>>(),
            vec!["project".to_owned()]
        );

        let user_only = load_runtime_config(
            Some(cwd.clone()),
            Some(profile.clone()),
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
            RuntimeOverrides {
                allowed_setting_sources: Some(vec![SettingSource::User]),
                ..RuntimeOverrides::default()
            },
        )
        .expect("user config");
        let discovery = discover_runtime_mcp_servers(&user_only, &[]);
        assert_eq!(
            discovery
                .servers
                .iter()
                .map(|entry| entry.server.name.clone())
                .collect::<Vec<_>>(),
            vec!["plugin".to_owned(), "profile".to_owned()]
        );

        let local_only = load_runtime_config(
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
            RuntimeOverrides {
                allowed_setting_sources: Some(vec![SettingSource::Local]),
                ..RuntimeOverrides::default()
            },
        )
        .expect("local config");
        let discovery = discover_runtime_mcp_servers(&local_only, &[extra_root.clone()]);
        assert_eq!(
            discovery
                .servers
                .iter()
                .map(|entry| entry.server.name.clone())
                .collect::<Vec<_>>(),
            vec!["explicit".to_owned()]
        );
    }

    #[test]
    fn runtime_mcp_resolution_uses_setting_source_filtering() {
        let tempdir = tempdir().expect("tempdir");
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join(".remote-code-rust");
        fs::create_dir_all(&cwd).expect("cwd");
        fs::create_dir_all(&profile).expect("profile");
        fs::write(
            cwd.join(rc_mcp::DEFAULT_MCP_CONFIG_FILE),
            r#"[servers.shared]
command = "python"
args = ["project.py"]"#,
        )
        .expect("write project mcp");
        fs::write(
            profile.join(rc_mcp::DEFAULT_MCP_CONFIG_FILE),
            r#"[servers.shared]
command = "python"
args = ["profile.py"]"#,
        )
        .expect("write profile mcp");

        let filtered = load_runtime_config(
            Some(cwd.clone()),
            Some(profile.clone()),
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
            RuntimeOverrides {
                allowed_setting_sources: Some(vec![SettingSource::Project]),
                ..RuntimeOverrides::default()
            },
        )
        .expect("filtered config");
        let resolution =
            resolve_runtime_mcp_server(&filtered, "shared", &[]).expect("filtered resolve");
        assert_eq!(resolution.entry.origin_kind, "cwd");

        let unfiltered = load_runtime_config(
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
        .expect("unfiltered config");
        assert!(resolve_runtime_mcp_server(&unfiltered, "shared", &[]).is_err());
    }

    #[test]
    fn runtime_mcp_discovery_deduplicates_same_config_path_across_sources() {
        let tempdir = tempdir().expect("tempdir");
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join(".remote-code-rust");
        fs::create_dir_all(&cwd).expect("cwd");
        fs::create_dir_all(&profile).expect("profile");
        fs::write(
            cwd.join(rc_mcp::DEFAULT_MCP_CONFIG_FILE),
            r#"[mcp_servers.project]
command = "python""#,
        )
        .expect("write project mcp");

        let config = load_runtime_config(
            Some(cwd.clone()),
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

        let discovery =
            discover_runtime_mcp_servers(&config, &[cwd.join(rc_mcp::DEFAULT_MCP_CONFIG_FILE)]);
        assert_eq!(discovery.servers.len(), 1);
        assert_eq!(discovery.servers[0].server.name, "project");
        assert!(discovery.warnings.is_empty());
    }

    #[test]
    fn runtime_mcp_discovery_warns_on_missing_or_invalid_explicit_config_but_continues() {
        let tempdir = tempdir().expect("tempdir");
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join(".remote-code-rust");
        let broken = tempdir.path().join("broken-mcp.toml");
        let missing = tempdir.path().join("missing-mcp.toml");
        fs::create_dir_all(&cwd).expect("cwd");
        fs::create_dir_all(&profile).expect("profile");
        fs::write(
            profile.join(rc_mcp::DEFAULT_MCP_CONFIG_FILE),
            r#"[mcp_servers.profile]
command = "python""#,
        )
        .expect("write profile mcp");
        fs::write(&broken, "not valid toml = [").expect("write broken mcp");

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

        let discovery = discover_runtime_mcp_servers(&config, &[missing, broken]);
        let names = discovery
            .servers
            .iter()
            .map(|entry| entry.server.name.clone())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["profile".to_owned()]);
        assert_eq!(discovery.warnings.len(), 2);
        assert!(discovery.warnings.iter().any(|warning| {
            warning.contains("Explicit MCP config") && warning.contains("missing-mcp.toml")
        }));
        assert!(discovery.warnings.iter().any(|warning| {
            warning.contains("Failed to load MCP config") && warning.contains("broken-mcp.toml")
        }));
    }
}

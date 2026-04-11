use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use rc_config::{RUNTIME_VERSION, RuntimeConfig};

use crate::cli::{PluginsCommand, PluginsInspectArgs, PluginsInvokeArgs, PluginsListArgs};
use crate::mcp_cli::parse_named_json_object_args;

pub(crate) async fn run_plugins(config: &RuntimeConfig, command: PluginsCommand) -> Result<()> {
    match command {
        PluginsCommand::List(args) => run_plugins_list(config, args).await,
        PluginsCommand::Inspect(args) => run_plugins_inspect(config, args).await,
        PluginsCommand::Invoke(args) => run_plugins_invoke(config, args).await,
    }
}

async fn run_plugins_list(config: &RuntimeConfig, args: PluginsListArgs) -> Result<()> {
    let output = build_plugins_list_output(config, &args).await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    if output.plugins.is_empty() {
        println!("No plugins found.");
        for warning in output.warnings {
            println!("  - {warning}");
        }
        return Ok(());
    }

    for warning in &output.warnings {
        println!("warning: {warning}");
    }
    for plugin in &output.plugins {
        println!(
            "{}  {}  runtime={}  skills={}  mcp={}  {}",
            plugin.name,
            plugin.version,
            if plugin.has_runtime { "yes" } else { "no" },
            if plugin.has_skills { "yes" } else { "no" },
            if plugin.has_mcp { "yes" } else { "no" },
            format_plugin_source(plugin)
        );
        if let Some(live) = &plugin.live {
            match live.status.as_str() {
                "ok" => {
                    let peer = live.plugin_info.as_ref().map_or_else(
                        || "unknown-plugin".to_owned(),
                        |info| match &info.version {
                            Some(version) => format!("{} {}", info.name, version),
                            None => info.name.clone(),
                        },
                    );
                    println!(
                        "  connect: ok  protocol={}  actions={}  peer={peer}",
                        live.protocol_version.as_deref().unwrap_or("unknown"),
                        live.action_count
                    );
                    for action in &live.actions {
                        match &action.description {
                            Some(description) => println!("    - {}: {description}", action.name),
                            None => println!("    - {}", action.name),
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

async fn run_plugins_inspect(config: &RuntimeConfig, args: PluginsInspectArgs) -> Result<()> {
    let output = build_plugins_inspect_output(config, &args).await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    for warning in &output.warnings {
        println!("warning: {warning}");
    }
    println!(
        "plugin: {} {}  {}",
        output.plugin.name,
        output.plugin.version,
        format_plugin_source(&output.plugin)
    );
    println!(
        "features: runtime={}  skills={}  mcp={}",
        if output.plugin.has_runtime {
            "yes"
        } else {
            "no"
        },
        if output.plugin.has_skills {
            "yes"
        } else {
            "no"
        },
        if output.plugin.has_mcp { "yes" } else { "no" }
    );
    match &output.plugin.live {
        Some(live) if live.status == "ok" => {
            println!(
                "runtime: ok  protocol={}  actions={}",
                live.protocol_version.as_deref().unwrap_or("unknown"),
                live.action_count
            );
            if let Some(info) = &live.plugin_info {
                match &info.version {
                    Some(version) => println!("peer: {} {}", info.name, version),
                    None => println!("peer: {}", info.name),
                }
            }
            for action in &live.actions {
                match &action.description {
                    Some(description) => println!("  - {}: {description}", action.name),
                    None => println!("  - {}", action.name),
                }
            }
        }
        Some(live) => {
            println!(
                "runtime: {}  {}",
                live.status,
                live.error.as_deref().unwrap_or("inspection failed")
            );
        }
        None => {
            println!("runtime: not inspected");
        }
    }
    Ok(())
}

async fn run_plugins_invoke(config: &RuntimeConfig, args: PluginsInvokeArgs) -> Result<()> {
    let output = build_plugins_invoke_output(config, &args).await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    for warning in &output.warnings {
        println!("warning: {warning}");
    }
    println!(
        "plugin: {} {}  {}",
        output.plugin.name,
        output.plugin.version,
        format_plugin_source(&output.plugin)
    );
    println!("action: {}", output.response.action);
    println!(
        "status: {}",
        if output.response.result.is_error {
            "error"
        } else {
            "ok"
        }
    );
    println!("protocol: {}", output.response.protocol_version);
    if let Some(info) = &output.response.plugin_info {
        match &info.version {
            Some(version) => println!("peer: {} {}", info.name, version),
            None => println!("peer: {}", info.name),
        }
    }
    println!("output:");
    println!(
        "{}",
        serde_json::to_string_pretty(&output.response.result.output)?
    );
    Ok(())
}

#[derive(Debug, Clone)]
pub(crate) struct RuntimePluginEntry {
    pub(crate) origin_kind: &'static str,
    pub(crate) origin_name: String,
    pub(crate) bundle: rc_plugins::PluginBundle,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct RuntimePluginDiscovery {
    pub(crate) plugins: Vec<RuntimePluginEntry>,
    pub(crate) warnings: Vec<String>,
}

#[derive(Debug, Clone)]
struct RuntimePluginResolution {
    entry: RuntimePluginEntry,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct PluginsListOutput {
    warnings: Vec<String>,
    plugins: Vec<PluginRecord>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct PluginRecord {
    pub(crate) name: String,
    pub(crate) version: String,
    pub(crate) has_runtime: bool,
    pub(crate) has_skills: bool,
    pub(crate) has_mcp: bool,
    pub(crate) origin_kind: String,
    pub(crate) origin_name: String,
    pub(crate) root: PathBuf,
    pub(crate) manifest_path: PathBuf,
    pub(crate) live: Option<PluginLiveRecord>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub(crate) struct PluginLiveRecord {
    status: String,
    protocol_version: Option<String>,
    plugin_info: Option<rc_plugins::PluginPeerInfo>,
    action_count: usize,
    actions: Vec<rc_plugins::PluginRuntimeActionDescriptor>,
    error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct PluginInspectOutput {
    warnings: Vec<String>,
    plugin: PluginRecord,
}

#[derive(Debug, Clone, serde::Serialize)]
struct PluginInvokeOutput {
    warnings: Vec<String>,
    plugin: PluginRecord,
    input: serde_json::Value,
    response: rc_plugins::PluginInvokeResponse,
}

impl PluginLiveRecord {
    fn from_inspection(inspection: rc_plugins::PluginRuntimeInspection) -> Self {
        Self {
            status: "ok".to_owned(),
            protocol_version: Some(inspection.protocol_version),
            plugin_info: inspection.plugin_info,
            action_count: inspection.actions.len(),
            actions: inspection.actions,
            error: None,
        }
    }

    fn skipped(reason: impl Into<String>) -> Self {
        Self {
            status: "skipped".to_owned(),
            protocol_version: None,
            plugin_info: None,
            action_count: 0,
            actions: Vec::new(),
            error: Some(reason.into()),
        }
    }

    fn failed(error: &impl ToString) -> Self {
        Self {
            status: "error".to_owned(),
            protocol_version: None,
            plugin_info: None,
            action_count: 0,
            actions: Vec::new(),
            error: Some(error.to_string()),
        }
    }
}

async fn build_plugins_list_output(
    config: &RuntimeConfig,
    args: &PluginsListArgs,
) -> Result<PluginsListOutput> {
    let discovery = discover_runtime_plugins(config, &args.plugin_roots);
    let filters = args.plugins.iter().cloned().collect::<BTreeSet<_>>();
    let mut plugins = Vec::new();

    for entry in discovery.plugins {
        if !filters.is_empty() && !filters.contains(&entry.bundle.manifest.name) {
            continue;
        }
        let has_runtime = entry.bundle.runtime_config().is_some();
        let live = if args.connect {
            if has_runtime {
                Some(
                    match rc_plugins::inspect_runtime(
                        &entry.bundle,
                        &rc_plugins::PluginHostInfo::new("remote-code-rust", RUNTIME_VERSION),
                    )
                    .await
                    {
                        Ok(inspection) => PluginLiveRecord::from_inspection(inspection),
                        Err(error) => PluginLiveRecord::failed(&error),
                    },
                )
            } else {
                Some(PluginLiveRecord::skipped(
                    "plugin does not define a runtime adapter",
                ))
            }
        } else {
            None
        };
        plugins.push(plugin_record_from_entry(&entry, has_runtime, live));
    }

    if !filters.is_empty() && plugins.is_empty() {
        return Err(anyhow!(
            "No matching plugins found for: {}",
            filters.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }

    Ok(PluginsListOutput {
        warnings: discovery.warnings,
        plugins,
    })
}

async fn build_plugins_inspect_output(
    config: &RuntimeConfig,
    args: &PluginsInspectArgs,
) -> Result<PluginInspectOutput> {
    let resolution = resolve_runtime_plugin(config, &args.plugin, &args.plugin_roots)?;
    let has_runtime = resolution.entry.bundle.runtime_config().is_some();
    let live = if has_runtime {
        Some(
            match rc_plugins::inspect_runtime(
                &resolution.entry.bundle,
                &rc_plugins::PluginHostInfo::new("remote-code-rust", RUNTIME_VERSION),
            )
            .await
            {
                Ok(inspection) => PluginLiveRecord::from_inspection(inspection),
                Err(error) => PluginLiveRecord::failed(&error),
            },
        )
    } else {
        Some(PluginLiveRecord::skipped(
            "plugin does not define a runtime adapter",
        ))
    };

    Ok(PluginInspectOutput {
        warnings: resolution.warnings,
        plugin: plugin_record_from_entry(&resolution.entry, has_runtime, live),
    })
}

async fn build_plugins_invoke_output(
    config: &RuntimeConfig,
    args: &PluginsInvokeArgs,
) -> Result<PluginInvokeOutput> {
    let resolution = resolve_runtime_plugin(config, &args.plugin, &args.plugin_roots)?;
    let has_runtime = resolution.entry.bundle.runtime_config().is_some();
    if !has_runtime {
        return Err(anyhow!(
            "Plugin `{}` does not define a runtime adapter",
            args.plugin
        ));
    }
    let input = parse_plugin_invoke_input(args)?;
    let response = rc_plugins::invoke_runtime(
        &resolution.entry.bundle,
        &rc_plugins::PluginHostInfo::new("remote-code-rust", RUNTIME_VERSION),
        &args.action,
        input.clone(),
    )
    .await?;

    Ok(PluginInvokeOutput {
        warnings: resolution.warnings,
        plugin: plugin_record_from_entry(&resolution.entry, true, None),
        input,
        response,
    })
}

fn parse_plugin_invoke_input(args: &PluginsInvokeArgs) -> Result<serde_json::Value> {
    parse_named_json_object_args("--input-json", args.input_json.as_ref(), &args.args)
}

pub(crate) fn discover_runtime_plugins(
    config: &RuntimeConfig,
    extra_plugin_roots: &[PathBuf],
) -> RuntimePluginDiscovery {
    let mut discovery = RuntimePluginDiscovery::default();
    let mut seen_manifest_paths = BTreeSet::new();
    load_runtime_plugins_root(
        &mut discovery,
        &mut seen_manifest_paths,
        "profile",
        &config.paths.plugins_dir.display().to_string(),
        &config.paths.plugins_dir,
    );
    for root in extra_plugin_roots {
        load_runtime_plugins_root(
            &mut discovery,
            &mut seen_manifest_paths,
            "explicit",
            &root.display().to_string(),
            root,
        );
    }

    discovery.plugins.sort_by(|left, right| {
        left.bundle
            .manifest
            .name
            .cmp(&right.bundle.manifest.name)
            .then_with(|| left.origin_kind.cmp(right.origin_kind))
            .then_with(|| left.origin_name.cmp(&right.origin_name))
    });
    discovery
}

fn load_runtime_plugins_root(
    discovery: &mut RuntimePluginDiscovery,
    seen_manifest_paths: &mut BTreeSet<PathBuf>,
    origin_kind: &'static str,
    origin_name: &str,
    root: &Path,
) {
    if !root.exists() {
        if origin_kind == "explicit" {
            discovery.warnings.push(format!(
                "Explicit plugin root {} was not found",
                root.display()
            ));
        }
        return;
    }
    match rc_plugins::discover_plugins(root) {
        Ok(plugins) => {
            for plugin in plugins {
                if !seen_manifest_paths.insert(plugin.manifest_path.clone()) {
                    continue;
                }
                discovery.plugins.push(RuntimePluginEntry {
                    origin_kind,
                    origin_name: origin_name.to_string(),
                    bundle: plugin,
                });
            }
        }
        Err(error) => discovery.warnings.push(format!(
            "Failed to discover plugins in {}: {error}",
            root.display()
        )),
    }
}

fn resolve_runtime_plugin(
    config: &RuntimeConfig,
    plugin_name: &str,
    extra_plugin_roots: &[PathBuf],
) -> Result<RuntimePluginResolution> {
    let mut discovery = discover_runtime_plugins(config, extra_plugin_roots);
    let mut matches = discovery
        .plugins
        .iter()
        .filter(|entry| entry.bundle.manifest.name == plugin_name)
        .cloned()
        .collect::<Vec<_>>();

    match matches.len() {
        0 => Err(anyhow!("No plugin named `{plugin_name}` was found")),
        1 => Ok(RuntimePluginResolution {
            entry: matches.pop().expect("single plugin match must exist"),
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
                        entry.bundle.manifest_path.display()
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            discovery.warnings.push(format!(
                "Multiple plugins named `{plugin_name}` were discovered; use a unique plugin layout"
            ));
            Err(anyhow!(
                "Plugin `{plugin_name}` is ambiguous across: {candidates}"
            ))
        }
    }
}

fn plugin_record_from_entry(
    entry: &RuntimePluginEntry,
    has_runtime: bool,
    live: Option<PluginLiveRecord>,
) -> PluginRecord {
    PluginRecord {
        name: entry.bundle.manifest.name.clone(),
        version: entry.bundle.manifest.version.clone(),
        has_runtime,
        has_skills: entry.bundle.skills_root().is_some(),
        has_mcp: entry.bundle.mcp_config_path().is_some(),
        origin_kind: entry.origin_kind.to_owned(),
        origin_name: entry.origin_name.clone(),
        root: entry.bundle.root.clone(),
        manifest_path: entry.bundle.manifest_path.clone(),
        live,
    }
}

pub(crate) fn format_plugin_source(plugin: &PluginRecord) -> String {
    match plugin.origin_kind.as_str() {
        "explicit" => format!(
            "explicit:{} ({})",
            plugin.origin_name,
            plugin.manifest_path.display()
        ),
        _ => format!(
            "{} ({})",
            plugin.origin_kind,
            plugin.manifest_path.display()
        ),
    }
}

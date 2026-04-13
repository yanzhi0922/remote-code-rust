use std::path::Path;

use rc_config::RuntimeConfig;

pub fn dispatch(input: &str, config: &RuntimeConfig) {
    let remainder = input
        .trim()
        .strip_prefix("/plugins")
        .unwrap_or_default()
        .trim();
    if remainder.is_empty() || remainder == "list" {
        render(config);
        return;
    }

    let mut parts = remainder.split_whitespace();
    match parts.next().unwrap_or_default() {
        "show" | "inspect" => {
            let Some(name) = parts.next() else {
                println!("Usage: /plugins [list|show <plugin>|validate [plugin]]");
                return;
            };
            render_plugin(config, name);
        }
        "validate" => {
            if let Some(name) = parts.next() {
                validate_plugin(config, Some(name));
            } else {
                validate_plugin(config, None);
            }
        }
        other => {
            println!("Unknown /plugins subcommand '{other}'.");
            println!("Usage: /plugins [list|show <plugin>|validate [plugin]]");
        }
    }
}

pub fn render(config: &RuntimeConfig) {
    match rc_plugins::discover_plugins(&config.paths.plugins_dir) {
        Ok(mut plugins) => {
            if plugins.is_empty() {
                println!(
                    "Plugins: none discovered in {}.",
                    config.paths.plugins_dir.display()
                );
                return;
            }
            plugins.sort_by(|left, right| left.manifest.name.cmp(&right.manifest.name));
            println!("Plugins ({}):", plugins.len());
            for plugin in plugins {
                let disabled = plugin.root.join(rc_plugins::PLUGIN_DISABLED_MARKER).exists();
                println!(
                    "  {}  {}  runtime={}  skills={}  mcp={}  disabled={}  {}",
                    plugin.manifest.name,
                    plugin.manifest.version,
                    yes_no(plugin.runtime_config().is_some()),
                    yes_no(plugin.skills_root().is_some()),
                    yes_no(plugin.mcp_config_path().is_some()),
                    yes_no(disabled),
                    plugin.manifest_path.display()
                );
            }
            println!("Tip: /plugins show <plugin> or /plugins validate");
        }
        Err(error) => eprintln!(
            "Failed to discover plugins in {}: {error}",
            config.paths.plugins_dir.display()
        ),
    }
}

pub(crate) fn discovered_plugin_count(config: &RuntimeConfig) -> usize {
    match rc_plugins::discover_plugins(&config.paths.plugins_dir) {
        Ok(plugins) => plugins.len(),
        Err(_) => 0,
    }
}

fn render_plugin(config: &RuntimeConfig, name: &str) {
    match resolve_plugin(config, name) {
        Ok(plugin) => {
            let report = rc_plugins::validate_plugin_bundle(&plugin);
            let disabled = plugin.root.join(rc_plugins::PLUGIN_DISABLED_MARKER).exists();
            println!("Plugin: {} {}", plugin.manifest.name, plugin.manifest.version);
            println!("  root: {}", plugin.root.display());
            println!("  manifest: {}", plugin.manifest_path.display());
            println!("  disabled: {}", yes_no(disabled));
            println!(
                "  surfaces: runtime={}  skills={}  hooks={}  mcp={}  apps={}",
                yes_no(plugin.runtime_config().is_some()),
                yes_no(plugin.skills_root().is_some()),
                yes_no(plugin.hooks_config_path().is_some()),
                yes_no(plugin.mcp_config_path().is_some()),
                yes_no(plugin.app_manifest_path().is_some())
            );
            if let Some(summary) = plugin.manifest.description.as_deref() {
                println!("  description: {summary}");
            }
            if let Some(runtime) = plugin.runtime_config() {
                println!("  runtime command: {}", runtime.command);
                if !runtime.args.is_empty() {
                    println!("  runtime args: {}", runtime.args.join(" "));
                }
                println!("  runtime cwd: {}", runtime.cwd.display());
            }
            if let Some(path) = plugin.skills_root() {
                println!("  skills root: {}", path.display());
            }
            if let Some(path) = plugin.hooks_config_path() {
                println!("  hooks: {}", path.display());
            }
            if let Some(path) = plugin.mcp_config_path() {
                println!("  mcp: {}", path.display());
            }
            if let Some(path) = plugin.app_manifest_path() {
                println!("  app manifest: {}", path.display());
            }
            println!(
                "  validation: errors={} warnings={}",
                report.errors.len(),
                report.warnings.len()
            );
            for error in report.errors {
                println!("    error: {error}");
            }
            for warning in report.warnings {
                println!("    warning: {warning}");
            }
        }
        Err(error) => eprintln!("{error}"),
    }
}

fn validate_plugin(config: &RuntimeConfig, name: Option<&str>) {
    let plugins = match rc_plugins::discover_plugins(&config.paths.plugins_dir) {
        Ok(plugins) => plugins,
        Err(error) => {
            eprintln!(
                "Failed to discover plugins in {}: {error}",
                config.paths.plugins_dir.display()
            );
            return;
        }
    };

    let filtered = plugins
        .into_iter()
        .filter(|plugin| name.is_none_or(|expected| plugin.manifest.name == expected))
        .collect::<Vec<_>>();
    if filtered.is_empty() {
        if let Some(name) = name {
            println!("No plugin named `{name}` was found.");
        } else {
            println!("No plugins found.");
        }
        return;
    }

    for plugin in filtered {
        let report = rc_plugins::validate_plugin_bundle(&plugin);
        println!(
            "{}  errors={} warnings={} skills={} runtime={} mcp={}",
            report.plugin_name,
            report.errors.len(),
            report.warnings.len(),
            report.bundled_skills,
            report.has_runtime,
            report.has_mcp
        );
        for error in report.errors {
            println!("  error: {error}");
        }
        for warning in report.warnings {
            println!("  warning: {warning}");
        }
    }
}

fn resolve_plugin(config: &RuntimeConfig, name: &str) -> anyhow::Result<rc_plugins::PluginBundle> {
    let plugins = rc_plugins::discover_plugins(&config.paths.plugins_dir)?;
    let mut matches = plugins
        .into_iter()
        .filter(|plugin| plugin.manifest.name == name)
        .collect::<Vec<_>>();
    match matches.len() {
        0 => anyhow::bail!("No plugin named `{name}` was found"),
        1 => Ok(matches.pop().expect("single plugin must exist")),
        _ => {
            let locations = matches
                .into_iter()
                .map(|plugin| plugin.manifest_path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            anyhow::bail!("Plugin `{name}` is ambiguous across: {locations}");
        }
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "yes" } else { "no" }
}

#[allow(dead_code)]
fn _display_path(path: &Path) -> String {
    path.display().to_string()
}

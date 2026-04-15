use std::path::PathBuf;

use anyhow::Result;
use rc_config::{RuntimeConfig, validate_provider_config};
use rc_permissions::{load_layered_rules, rules::summarize_rule_sources};
use serde::Serialize;

use super::install::{InstallSource, detect_install_source, release_repository_slug};
use super::network::{ProbeResult, ProbeSpec, run_probe};
use super::providers::{
    EnvProviderSummary, env_provider_summaries, provider_endpoint_url, provider_probe_spec,
};
use crate::cli::DoctorArgs;
use crate::conversation::discover_runtime_extensions;

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RuleSourceCount {
    pub source: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct RuntimeSection {
    pub version: String,
    pub cwd: PathBuf,
    pub profile_dir: PathBuf,
    pub session_id: String,
    pub session_name: Option<String>,
    pub permission_mode: String,
    pub input_format: String,
    pub output_format: String,
    pub setting_sources: Vec<String>,
    pub settings_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct InstallSection {
    pub source: String,
    pub update_supported: bool,
    pub executable: PathBuf,
    pub repository_url: String,
    pub repository_slug: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ProviderSection {
    pub name: String,
    pub protocol: String,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub api_key_present: bool,
    pub auth_source: Option<String>,
    pub effort: Option<String>,
    pub fallback_model: Option<String>,
    pub context_window_tokens: u64,
    pub output_reserve_tokens: u64,
    pub multimodal: bool,
    pub reasoning: bool,
    pub validation_ok: bool,
    pub validation_issues: Vec<String>,
    pub probe: Option<ProbeResult>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ToolsSection {
    pub builtin_tools: usize,
    pub allowed_tools: Vec<String>,
    pub disallowed_tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PermissionsSection {
    pub layered_rules: usize,
    pub rule_sources: Vec<RuleSourceCount>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ExtensionsSection {
    pub skills: usize,
    pub plugins: usize,
    pub plugin_runtimes: usize,
    pub mcp_servers: usize,
    pub hooks: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct DoctorReport {
    pub ok: bool,
    pub runtime: RuntimeSection,
    pub install: InstallSection,
    pub provider: ProviderSection,
    pub tools: ToolsSection,
    pub permissions: PermissionsSection,
    pub extensions: ExtensionsSection,
    pub network: Option<Vec<ProbeResult>>,
    pub env_providers: Option<Vec<EnvProviderSummary>>,
    pub issues: Vec<String>,
    pub warnings: Vec<String>,
}

pub(crate) async fn collect_report(
    config: &RuntimeConfig,
    args: &DoctorArgs,
) -> Result<DoctorReport> {
    let validation = validate_provider_config(&config.provider);
    let discovery = discover_runtime_extensions(config);
    let hooks = crate::runtime_hooks::HookRuntime::discover(config);
    let layered_rules = load_layered_rules(
        &config.cwd,
        &config.paths.profile_dir,
        &config.settings_files,
        &config.cli_settings_files,
    )?;
    let model_info = rc_provider::model_info::get_model_info(
        config.provider.model.as_deref().unwrap_or("unknown"),
    );
    let install_source = detect_install_source();

    let mut issues = validation.issues.clone();
    let mut warnings = Vec::new();
    warnings.extend(discovery.warnings.clone());
    warnings.extend(hooks.warnings().to_vec());

    let provider_probe = if args.probe_provider {
        if let Some(spec) = provider_probe_spec(&config.provider) {
            let probe = run_probe(spec).await;
            if probe.is_issue() {
                issues.push(format!("Provider probe failed: {}", probe.detail));
            } else if probe.is_warning() {
                warnings.push(format!("Provider probe warning: {}", probe.detail));
            }
            Some(probe)
        } else {
            warnings.push(
                "Provider probe skipped: no probeable endpoint for the active protocol.".to_owned(),
            );
            None
        }
    } else {
        None
    };

    let network = if args.probe_network {
        let mut probes = Vec::new();
        if let Some(repository_slug) = release_repository_slug() {
            probes.push(
                ProbeSpec::new(
                    "github:releases",
                    format!("https://api.github.com/repos/{repository_slug}/releases/latest"),
                )
                .with_header("accept", "application/vnd.github+json"),
            );
        }
        if !args.probe_provider
            && let Some(provider_url) = provider_endpoint_url(&config.provider)
        {
            probes.push(ProbeSpec::new("provider:network", provider_url));
        }

        let mut results = Vec::new();
        for probe in probes {
            let result = run_probe(probe).await;
            if result.is_issue() || result.is_warning() {
                warnings.push(format!("Network probe warning: {}", result.detail));
            }
            results.push(result);
        }
        Some(results)
    } else {
        None
    };

    let runtime = RuntimeSection {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        cwd: config.cwd.clone(),
        profile_dir: config.paths.profile_dir.clone(),
        session_id: config.session_id.to_string(),
        session_name: config.session_name.clone(),
        permission_mode: format!("{:?}", config.permission_mode),
        input_format: format!("{:?}", config.input_format),
        output_format: format!("{:?}", config.output_format),
        setting_sources: config.setting_sources.clone(),
        settings_files: config.settings_files.clone(),
    };
    let install = build_install_section(&install_source);
    let provider = ProviderSection {
        name: config.provider.name.clone(),
        protocol: config.provider.protocol.as_str().to_owned(),
        base_url: config.provider.base_url.clone(),
        model: config.provider.model.clone(),
        api_key_present: config.provider.api_key.is_some(),
        auth_source: config.auth_source.clone(),
        effort: config.effort.clone(),
        fallback_model: config.fallback_model.clone(),
        context_window_tokens: model_info.max_context,
        output_reserve_tokens: model_info.max_output,
        multimodal: model_info.multimodal,
        reasoning: model_info
            .capabilities
            .contains(&rc_provider::model_info::ModelCapability::Reasoning),
        validation_ok: validation.ok,
        validation_issues: validation.issues,
        probe: provider_probe,
    };
    let tools = ToolsSection {
        builtin_tools: rc_tools::runtime_builtin_tool_specs().len(),
        allowed_tools: config.allowed_tools.clone(),
        disallowed_tools: config.disallowed_tools.clone(),
    };
    let permissions = PermissionsSection {
        layered_rules: layered_rules.len(),
        rule_sources: summarize_rule_sources(&layered_rules)
            .into_iter()
            .map(|(source, count)| RuleSourceCount {
                source: source.as_str().to_owned(),
                count,
            })
            .collect(),
    };
    let extensions = ExtensionsSection {
        skills: discovery.skills.len(),
        plugins: discovery.plugins.len(),
        plugin_runtimes: discovery.plugin_runtimes.len(),
        mcp_servers: discovery.mcp_servers.len(),
        hooks: hooks.list(None).len(),
    };
    let env_providers = args.include_env_providers.then(env_provider_summaries);

    Ok(DoctorReport {
        ok: issues.is_empty(),
        runtime,
        install,
        provider,
        tools,
        permissions,
        extensions,
        network,
        env_providers,
        issues,
        warnings,
    })
}

fn build_install_section(install_source: &InstallSource) -> InstallSection {
    InstallSection {
        source: install_source.label().to_owned(),
        update_supported: install_source.supports_in_place_update(),
        executable: install_source.executable.clone(),
        repository_url: install_source.repository_url.clone(),
        repository_slug: install_source.repository_slug.clone(),
    }
}

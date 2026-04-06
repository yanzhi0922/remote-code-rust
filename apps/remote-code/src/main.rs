use std::collections::{BTreeSet, HashMap};
use std::env;
use std::io;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::time::Instant;

use anyhow::{Result, anyhow};
use clap::{Args, Parser, Subcommand};
use rc_config::{
    ProviderOverrides, RUNTIME_VERSION, RuntimeConfig, import_legacy_profile, load_runtime_config,
    normalize_base_url, validate_provider_config,
};
use rc_core::{
    ConversationEntry, InputFormat, OutputFormat, PermissionMode, SessionState,
    default_system_prompt,
};
use rc_permissions::{
    PermissionBroker, PermissionDecision, PermissionRequest, StaticPermissionBroker,
};
use rc_protocol::{
    InitPayload, PermissionRequestPayload, ProtocolEmitter, ProtocolInput, ResultPayload,
    UsagePayload, parse_input_line,
};
use rc_provider::ProviderClient;
use rc_session::{SessionStore, SessionSummary};
use rc_skills::SkillDocument;
use rc_telemetry::install_tracing;
use rc_tools::{ToolExecutionContext, builtin_tool_specs, execute_tool_call};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::warn;
use uuid::Uuid;

#[derive(Parser, Debug)]
#[command(
    name = "remote-code",
    version,
    about = "Remote Code Rust CLI/runtime shell"
)]
struct Cli {
    #[arg(short = 'p', long = "print")]
    print_mode: bool,

    #[arg(long, value_enum, default_value_t = InputFormat::Text)]
    input_format: InputFormat,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    output_format: OutputFormat,

    #[arg(long, value_enum, env = "REMOTE_CODE_PERMISSION_MODE", default_value_t = PermissionMode::Default)]
    permission_mode: PermissionMode,

    #[arg(long)]
    cwd: Option<PathBuf>,

    #[arg(long, env = "REMOTE_CODE_PROFILE_DIR")]
    profile_dir: Option<PathBuf>,

    #[arg(long)]
    session_id: Option<Uuid>,

    #[arg(long)]
    provider: Option<String>,

    #[arg(long)]
    base_url: Option<String>,

    #[arg(long)]
    api_key: Option<String>,

    #[arg(long)]
    model: Option<String>,

    #[arg(long, value_enum)]
    protocol: Option<rc_core::ProviderProtocol>,

    #[arg(long, default_value_t = 12)]
    max_turns: usize,

    #[arg(short, long)]
    verbose: bool,

    #[arg(long)]
    replay_user_messages: bool,

    #[arg(long)]
    include_partial_messages: bool,

    #[command(subcommand)]
    command: Option<Commands>,

    prompt: Vec<String>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Doctor,
    Sessions {
        #[command(subcommand)]
        command: Option<SessionsCommand>,
    },
    Resume(ResumeArgs),
    Export(ExportArgs),
    Tui,
    Migrate {
        #[command(subcommand)]
        command: MigrateCommand,
    },
}

#[derive(Subcommand, Debug)]
enum SessionsCommand {
    List,
    Show(ShowArgs),
}

#[derive(Args, Debug)]
struct ResumeArgs {
    session_id: Uuid,
    prompt: Vec<String>,
}

#[derive(Args, Debug)]
struct ExportArgs {
    session_id: Uuid,
    #[arg(long)]
    output: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = ExportFormat::Json)]
    format: ExportFormat,
}

#[derive(Args, Debug)]
struct ShowArgs {
    session_id: Uuid,
    #[arg(long)]
    json: bool,
}

#[derive(Subcommand, Debug)]
enum MigrateCommand {
    Import {
        #[arg(long)]
        source: Option<PathBuf>,
    },
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
enum ExportFormat {
    Ndjson,
    Json,
}

#[tokio::main]
async fn main() -> Result<()> {
    install_tracing("remote_code_rust", false)?;
    let cli = Cli::parse();

    let resume_session = match &cli.command {
        Some(Commands::Resume(args)) => Some(args.session_id),
        _ => cli.session_id,
    };
    let overrides = ProviderOverrides {
        provider: cli.provider.clone(),
        base_url: cli.base_url.clone(),
        api_key: cli.api_key.clone(),
        model: cli.model.clone(),
        protocol: cli.protocol,
    };
    let mut config = load_runtime_config(
        cli.cwd.clone(),
        cli.profile_dir.clone(),
        resume_session,
        cli.permission_mode,
        cli.input_format,
        cli.output_format,
        cli.print_mode,
        cli.verbose,
        cli.replay_user_messages,
        cli.include_partial_messages,
        cli.max_turns,
        overrides,
    )?;
    let store = SessionStore::open(config.paths.clone())?;
    if resume_session.is_some() {
        restore_session_context(&store, &mut config)?;
        reapply_cli_overrides(&cli, &mut config);
    }

    match cli.command {
        Some(Commands::Doctor) => run_doctor(&config),
        Some(Commands::Sessions { command }) => run_sessions(&store, command),
        Some(Commands::Export(args)) => run_export(&store, args),
        Some(Commands::Migrate { command }) => run_migrate(&config, command),
        Some(Commands::Resume(args)) => {
            let prompt = join_prompt(args.prompt);
            if should_run_headless(&config) {
                run_headless(&config, prompt).await
            } else if let Some(prompt) = prompt {
                run_oneshot_text(&config, &store, prompt).await
            } else {
                run_interactive_shell(config.clone(), &store).await
            }
        }
        Some(Commands::Tui) => rc_tui::run_dashboard(&config, &store),
        None => {
            let prompt = join_prompt(cli.prompt);
            if should_run_headless(&config) {
                run_headless(&config, prompt).await
            } else if let Some(prompt) = prompt {
                run_oneshot_text(&config, &store, prompt).await
            } else {
                run_interactive_shell(config.clone(), &store).await
            }
        }
    }
}

fn join_prompt(parts: Vec<String>) -> Option<String> {
    let prompt = parts.join(" ");
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

fn run_doctor(config: &RuntimeConfig) -> Result<()> {
    let report = validate_provider_config(&config.provider);
    let discovery = discover_runtime_extensions(config);
    let api_key_state = if config.provider.api_key.is_some() {
        "present"
    } else {
        "missing"
    };
    let lines = [
        "Remote Code Rust runtime doctor".to_owned(),
        format!("- cwd: {}", config.cwd.display()),
        format!("- provider: {}", config.provider.name),
        format!("- protocol: {}", config.provider.protocol.as_str()),
        format!(
            "- base URL: {}",
            config.provider.base_url.as_deref().unwrap_or("(missing)")
        ),
        format!(
            "- model: {}",
            config.provider.model.as_deref().unwrap_or("(missing)")
        ),
        format!("- api key: {api_key_state}"),
        format!("- input format: {:?}", config.input_format),
        format!("- output format: {:?}", config.output_format),
        format!("- print mode: {}", config.print_mode),
        format!("- discovered skills: {}", discovery.skills.len()),
        format!("- discovered plugins: {}", discovery.plugins.len()),
        format!("- discovered mcp servers: {}", discovery.mcp_servers.len()),
        format!(
            "- readiness: {}",
            if report.ok { "ready" } else { "not-ready" }
        ),
    ];
    for line in lines {
        println!("{line}");
    }
    for issue in report.issues {
        println!("  - {issue}");
    }
    for warning in discovery.warnings {
        println!("  - {warning}");
    }
    Ok(())
}

fn run_sessions(store: &SessionStore, command: Option<SessionsCommand>) -> Result<()> {
    match command.unwrap_or(SessionsCommand::List) {
        SessionsCommand::List => {
            let sessions = store.list_sessions()?;
            if sessions.is_empty() {
                println!("No sessions found.");
                return Ok(());
            }
            for session in sessions {
                println!(
                    "{}  {}  {}  {}",
                    session.session_id, session.updated_at, session.provider_name, session.title
                );
            }
            Ok(())
        }
        SessionsCommand::Show(args) => {
            let bundle = store.load_session_bundle(args.session_id)?;
            if args.json {
                println!("{}", serde_json::to_string_pretty(&bundle)?);
            } else {
                print_session_summary(&bundle.summary);
                println!("- transcript: {}", bundle.summary.transcript_path.display());
                println!("- events: {}", bundle.stats.total_events);
                println!("- messages: {}", bundle.stats.conversation_entries);
                println!(
                    "- usage: {} input / {} output",
                    bundle.stats.usage.input_tokens, bundle.stats.usage.output_tokens
                );
                if let Some(stop_reason) = &bundle.stats.last_stop_reason {
                    println!("- last stop reason: {stop_reason}");
                }
                if !bundle.conversation.is_empty() {
                    println!("\nRecent conversation:");
                    for entry in bundle.conversation.iter().rev().take(5).rev() {
                        println!(
                            "  {}: {}",
                            entry_role_label(&entry.role),
                            truncate_preview(&entry.history_text(), 120)
                        );
                    }
                }
            }
            Ok(())
        }
    }
}

fn run_export(store: &SessionStore, args: ExportArgs) -> Result<()> {
    let path = match args.format {
        ExportFormat::Ndjson => store.export_session(args.session_id, args.output)?,
        ExportFormat::Json => store.export_session_bundle_json(args.session_id, args.output)?,
    };
    println!("{}", path.display());
    Ok(())
}

fn run_migrate(config: &RuntimeConfig, command: MigrateCommand) -> Result<()> {
    match command {
        MigrateCommand::Import { source } => {
            let summary = import_legacy_profile(source, &config.paths)?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
            Ok(())
        }
    }
}

async fn run_oneshot_text(
    config: &RuntimeConfig,
    store: &SessionStore,
    prompt: String,
) -> Result<()> {
    let provider = ProviderClient::new()?;
    let broker = StaticPermissionBroker::new(config.permission_mode);
    let mut conversation = initialize_conversation(store, config, Some(&prompt))?;
    let response = run_prompt(
        config,
        store,
        &provider,
        &broker,
        &mut conversation,
        &prompt,
    )
    .await?;
    println!("{}", response.text);
    Ok(())
}

async fn run_interactive_shell(mut config: RuntimeConfig, store: &SessionStore) -> Result<()> {
    let provider = ProviderClient::new()?;
    let broker = StaticPermissionBroker::new(config.permission_mode);
    let mut conversation = initialize_conversation(store, &config, None)?;

    println!("Remote Code Rust interactive shell");
    println!(
        "Session {}  Provider {} ({})  Model {}",
        config.session_id,
        config.provider.name,
        config.provider.protocol.as_str(),
        config.provider.model.as_deref().unwrap_or("(missing)")
    );
    println!("Type `/help` for commands, `/quit` to exit, or `remote-code tui` for the dashboard.");

    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();
    loop {
        let mut stdout = tokio::io::stdout();
        stdout
            .write_all(format!("remote-code:{}> ", short_session_id(config.session_id)).as_bytes())
            .await?;
        stdout.flush().await?;

        let Some(line) = lines.next_line().await? else {
            println!();
            break;
        };
        let input = line.trim();
        if input.is_empty() {
            continue;
        }
        if input.starts_with('/') {
            if handle_shell_command(input, &mut config, store, &mut conversation)? {
                break;
            }
            continue;
        }

        match run_prompt(&config, store, &provider, &broker, &mut conversation, input).await {
            Ok(outcome) => {
                println!("\n{}", outcome.text);
                println!(
                    "-- {} turn(s), {} input tokens, {} output tokens, stop={}",
                    outcome.num_turns,
                    outcome.usage.input_tokens,
                    outcome.usage.output_tokens,
                    outcome.stop_reason
                );
            }
            Err(error) => {
                eprintln!("error: {error}");
            }
        }
    }

    Ok(())
}

async fn run_headless(config: &RuntimeConfig, inline_prompt: Option<String>) -> Result<()> {
    let discovery = discover_runtime_extensions(config);
    let emitter = Arc::new(Mutex::new(ProtocolEmitter::new(
        io::stdout(),
        config.session_id,
    )));
    {
        let mut emitter_guard = emitter.lock().await;
        emitter_guard.emit_init(InitPayload {
            api_key_source: if config.provider.api_key.is_some() {
                "user".to_owned()
            } else {
                "missing".to_owned()
            },
            version: RUNTIME_VERSION.to_owned(),
            cwd: config.cwd.display().to_string(),
            tools: builtin_tool_specs()
                .into_iter()
                .map(|tool| tool.protocol_name)
                .collect(),
            mcp_servers: discovery.mcp_servers,
            model: config.provider.model.clone(),
            permission_mode: config.permission_mode.as_legacy_str().to_owned(),
            slash_commands: Vec::new(),
            output_style: "default".to_owned(),
            skills: discovery.skills,
            plugins: discovery.plugins,
        })?;
        emitter_guard.emit_state(SessionState::Idle)?;
    }

    let pending_permissions = Arc::new(Mutex::new(HashMap::<
        String,
        oneshot::Sender<PermissionDecision>,
    >::new()));
    let interrupted = Arc::new(AtomicBool::new(false));
    let broker = Arc::new(ChannelPermissionBroker {
        mode: config.permission_mode,
        emitter: emitter.clone(),
        pending_permissions: pending_permissions.clone(),
    });
    let (prompt_tx, mut prompt_rx) = mpsc::channel::<String>(8);

    if let Some(prompt) = inline_prompt {
        prompt_tx.send(prompt).await?;
    }

    let processor_config = config.clone();
    let processor_store = SessionStore::open(config.paths.clone())?;
    let processor_broker = broker.clone();
    let processor_emitter = emitter.clone();
    let processor_interrupted = interrupted.clone();
    let processor = tokio::spawn(async move {
        let provider = ProviderClient::new()?;
        let mut conversation = initialize_conversation(&processor_store, &processor_config, None)?;
        while let Some(prompt) = prompt_rx.recv().await {
            if processor_interrupted.load(Ordering::Relaxed) {
                processor_interrupted.store(false, Ordering::Relaxed);
                continue;
            }
            let started = Instant::now();
            {
                let mut emitter = processor_emitter.lock().await;
                emitter.emit_state(SessionState::Running)?;
            }
            let result = run_prompt(
                &processor_config,
                &processor_store,
                &provider,
                processor_broker.as_ref(),
                &mut conversation,
                &prompt,
            )
            .await;
            let mut emitter = processor_emitter.lock().await;
            match result {
                Ok(outcome) => {
                    emitter.emit_assistant(&outcome.text)?;
                    emitter.emit_result(ResultPayload {
                        is_error: false,
                        duration_ms: outcome.duration_ms,
                        duration_api_ms: outcome.duration_api_ms,
                        num_turns: outcome.num_turns,
                        result: outcome.text,
                        stop_reason: outcome.stop_reason,
                        total_cost_usd: outcome.total_cost_usd,
                        usage: outcome.usage,
                        model_usage: outcome.model_usage,
                        permission_denials: outcome.permission_denials,
                        errors: Vec::new(),
                    })?;
                }
                Err(error) => {
                    let duration_ms = started.elapsed().as_millis() as u64;
                    emitter.emit_result(ResultPayload {
                        is_error: true,
                        duration_ms,
                        duration_api_ms: duration_ms,
                        num_turns: 1,
                        result: error.to_string(),
                        stop_reason: "error".to_owned(),
                        total_cost_usd: 0.0,
                        usage: UsagePayload::default(),
                        model_usage: serde_json::json!({}),
                        permission_denials: Vec::new(),
                        errors: vec![error.to_string()],
                    })?;
                }
            }
            emitter.emit_state(SessionState::Idle)?;
        }
        Ok::<(), anyhow::Error>(())
    });

    let stdin = BufReader::new(tokio::io::stdin());
    let mut lines = stdin.lines();
    while let Some(line) = lines.next_line().await? {
        let Some(input) = parse_input_line(&line) else {
            let mut emitter = emitter.lock().await;
            emitter.emit_status(format!("Ignored unsupported input: {line}"))?;
            continue;
        };
        match input {
            ProtocolInput::User { content } => {
                if config.replay_user_messages {
                    let mut emitter = emitter.lock().await;
                    emitter.emit_status(format!("Replayed user prompt: {content}"))?;
                }
                prompt_tx.send(content).await?;
            }
            ProtocolInput::ControlResponse {
                request_id,
                allow,
                message,
            } => {
                if let Some(sender) = pending_permissions.lock().await.remove(&request_id) {
                    let _ = sender.send(PermissionDecision {
                        allowed: allow,
                        message,
                    });
                }
            }
            ProtocolInput::Interrupt => {
                interrupted.store(true, Ordering::Relaxed);
                let mut pending = pending_permissions.lock().await;
                for (request_id, sender) in pending.drain() {
                    let _ = sender.send(PermissionDecision::deny("Interrupted by operator."));
                    let mut emitter = emitter.lock().await;
                    let _ = emitter.emit_permission_cancelled(&request_id);
                }
            }
        }
    }
    drop(prompt_tx);
    processor.await??;
    Ok(())
}

fn should_run_headless(config: &RuntimeConfig) -> bool {
    config.print_mode
        || matches!(config.input_format, InputFormat::StreamJson)
        || matches!(config.output_format, OutputFormat::StreamJson)
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct PersistedProviderContext {
    name: String,
    base_url: Option<String>,
    model: Option<String>,
    protocol: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct PersistedSessionContext {
    cwd: PathBuf,
    permission_mode: String,
    provider: PersistedProviderContext,
}

#[derive(Debug, Default)]
struct RuntimeExtensionDiscovery {
    skills: Vec<String>,
    plugins: Vec<String>,
    mcp_servers: Vec<String>,
    warnings: Vec<String>,
}

fn persist_session_context(store: &SessionStore, config: &RuntimeConfig) -> Result<()> {
    store.append_named_event(
        config.session_id,
        "session_context",
        serde_json::to_value(PersistedSessionContext {
            cwd: config.cwd.clone(),
            permission_mode: config.permission_mode.as_legacy_str().to_owned(),
            provider: PersistedProviderContext {
                name: config.provider.name.clone(),
                base_url: config.provider.base_url.clone(),
                model: config.provider.model.clone(),
                protocol: config.provider.protocol.as_str().to_owned(),
            },
        })?,
    )
}

fn restore_session_context(store: &SessionStore, config: &mut RuntimeConfig) -> Result<()> {
    if let Ok(summary) = store.get_session_summary(config.session_id) {
        config.cwd = summary.cwd;
        config.provider.name = summary.provider_name;
        if summary.model.is_some() {
            config.provider.model = summary.model;
        }
    }

    let Ok(events) = store.load_events(config.session_id) else {
        return Ok(());
    };
    let payload = events.into_iter().rev().find_map(|event| {
        (event.event_type == "session_context")
            .then_some(event.payload)
            .flatten()
    });
    let Some(payload) = payload else {
        return Ok(());
    };
    let persisted = serde_json::from_value::<PersistedSessionContext>(payload)?;
    config.cwd = persisted.cwd;
    if let Some(permission_mode) = parse_permission_mode(&persisted.permission_mode) {
        config.permission_mode = permission_mode;
    }
    config.provider.name = persisted.provider.name;
    config.provider.base_url = persisted.provider.base_url;
    config.provider.model = persisted.provider.model;
    if let Some(protocol) = parse_provider_protocol(&persisted.provider.protocol) {
        config.provider.protocol = protocol;
    }
    Ok(())
}

fn reapply_cli_overrides(cli: &Cli, config: &mut RuntimeConfig) {
    if let Some(cwd) = &cli.cwd {
        config.cwd = cwd.clone();
    }
    if let Some(provider) = &cli.provider {
        config.provider.name = provider.clone();
    }
    if let Some(model) = &cli.model {
        config.provider.model = Some(model.clone());
    }
    if let Some(api_key) = &cli.api_key {
        config.provider.api_key = Some(api_key.clone());
    }
    if cli.api_key.is_none() && env::var("REMOTE_CODE_API_KEY").is_ok() {
        config.provider.api_key = env::var("REMOTE_CODE_API_KEY").ok();
    }
    if let Some(protocol) = cli.protocol {
        config.provider.protocol = protocol;
    }
    if let Some(base_url) = &cli.base_url {
        config.provider.base_url =
            normalize_base_url(Some(base_url.clone()), config.provider.protocol);
    } else if cli.protocol.is_some() {
        config.provider.base_url =
            normalize_base_url(config.provider.base_url.clone(), config.provider.protocol);
    }
}

fn parse_permission_mode(value: &str) -> Option<PermissionMode> {
    match value.trim() {
        "default" => Some(PermissionMode::Default),
        "acceptEdits" => Some(PermissionMode::AcceptEdits),
        "bypassPermissions" => Some(PermissionMode::BypassPermissions),
        "dontAsk" => Some(PermissionMode::DontAsk),
        "plan" => Some(PermissionMode::Plan),
        _ => None,
    }
}

fn parse_provider_protocol(value: &str) -> Option<rc_core::ProviderProtocol> {
    match value.trim().to_ascii_lowercase().as_str() {
        "openai" => Some(rc_core::ProviderProtocol::OpenAi),
        "anthropic" => Some(rc_core::ProviderProtocol::Anthropic),
        _ => None,
    }
}

fn discover_runtime_extensions(config: &RuntimeConfig) -> RuntimeExtensionDiscovery {
    let mut skills = BTreeSet::new();
    let mut plugins = BTreeSet::new();
    let mut mcp_servers = BTreeSet::new();
    let mut warnings = Vec::new();

    if config.paths.skills_dir.exists() {
        collect_skill_names(
            rc_skills::discover_skills(&config.paths.skills_dir),
            &mut skills,
            &mut warnings,
            "profile skills",
        );
    }

    if config.paths.plugins_dir.exists() {
        match rc_plugins::discover_plugins(&config.paths.plugins_dir) {
            Ok(discovered_plugins) => {
                for plugin in discovered_plugins {
                    plugins.insert(plugin.manifest.name.clone());
                    collect_skill_names(
                        plugin.discover_bundled_skills(),
                        &mut skills,
                        &mut warnings,
                        &format!("plugin {}", plugin.manifest.name),
                    );
                    match plugin.load_mcp_config() {
                        Ok(Some(mcp)) => {
                            mcp_servers.extend(mcp.servers.keys().cloned());
                        }
                        Ok(None) => {}
                        Err(error) => warnings.push(format!(
                            "Failed to load plugin MCP config for {}: {error}",
                            plugin.manifest.name
                        )),
                    }
                }
            }
            Err(error) => warnings.push(format!("Failed to discover plugins: {error}")),
        }
    }

    for root in [&config.cwd, &config.paths.profile_dir] {
        let candidate = root.join(rc_mcp::DEFAULT_MCP_CONFIG_FILE);
        if !candidate.exists() {
            continue;
        }
        match rc_mcp::McpConfig::load(&candidate) {
            Ok(config) => {
                mcp_servers.extend(config.servers.keys().cloned());
            }
            Err(error) => warnings.push(format!(
                "Failed to load MCP config {}: {error}",
                candidate.display()
            )),
        }
    }

    RuntimeExtensionDiscovery {
        skills: skills.into_iter().collect(),
        plugins: plugins.into_iter().collect(),
        mcp_servers: mcp_servers.into_iter().collect(),
        warnings,
    }
}

fn collect_skill_names(
    result: std::result::Result<Vec<SkillDocument>, rc_skills::SkillError>,
    skills: &mut BTreeSet<String>,
    warnings: &mut Vec<String>,
    source: &str,
) {
    match result {
        Ok(discovered) => {
            skills.extend(
                discovered
                    .into_iter()
                    .map(|skill| skill.metadata.slug)
                    .collect::<Vec<_>>(),
            );
        }
        Err(error) => warnings.push(format!("Failed to discover {source}: {error}")),
    }
}

fn initialize_conversation(
    store: &SessionStore,
    config: &RuntimeConfig,
    title_hint: Option<&str>,
) -> Result<Vec<ConversationEntry>> {
    let title_hint = title_hint.or(config.provider.model.as_deref());
    store.ensure_session(
        config.session_id,
        &config.cwd,
        &config.provider.name,
        config.provider.model.as_deref(),
        title_hint,
    )?;
    persist_session_context(store, config)?;
    let mut conversation = store
        .load_conversation(config.session_id)
        .unwrap_or_default();
    if conversation.is_empty() {
        let system = ConversationEntry::system(default_system_prompt(&config.cwd));
        store.append_conversation_entry(config.session_id, &system)?;
        conversation.push(system);
    }
    Ok(conversation)
}

#[derive(Debug, Clone)]
struct PromptRunOutcome {
    text: String,
    duration_ms: u64,
    duration_api_ms: u64,
    num_turns: u32,
    stop_reason: String,
    total_cost_usd: f64,
    usage: UsagePayload,
    model_usage: serde_json::Value,
    permission_denials: Vec<serde_json::Value>,
}

async fn run_prompt(
    config: &RuntimeConfig,
    store: &SessionStore,
    provider: &ProviderClient,
    broker: &dyn PermissionBroker,
    conversation: &mut Vec<ConversationEntry>,
    prompt: &str,
) -> Result<PromptRunOutcome> {
    let readiness = validate_provider_config(&config.provider);
    if !readiness.ok {
        return Err(anyhow!(readiness.issues.join(" ")));
    }

    let started = Instant::now();
    store.ensure_session(
        config.session_id,
        &config.cwd,
        &config.provider.name,
        config.provider.model.as_deref(),
        Some(prompt),
    )?;
    store.append_named_event(
        config.session_id,
        "prompt_started",
        serde_json::json!({
            "prompt": prompt,
            "provider": config.provider.name.clone(),
            "model": config.provider.model.clone(),
            "protocol": config.provider.protocol.as_str(),
        }),
    )?;
    let user_entry = ConversationEntry::user(prompt);
    store.append_conversation_entry(config.session_id, &user_entry)?;
    conversation.push(user_entry);

    let tool_context = ToolExecutionContext {
        cwd: config.cwd.clone(),
        timeout_ms: config.provider.timeout_ms,
    };
    let mut usage = UsagePayload::default();
    let mut num_turns = 0u32;
    let mut permission_denials = Vec::new();
    let mut total_tool_calls = 0usize;
    for turn_index in 0..config.max_turns {
        num_turns += 1;
        let response = provider.complete(&config.provider, conversation).await?;
        usage.input_tokens += response.usage.input_tokens;
        usage.output_tokens += response.usage.output_tokens;
        total_tool_calls += response.tool_calls.len();
        let assistant_entry = ConversationEntry {
            role: rc_core::ConversationRole::Assistant,
            text: response.text.clone(),
            history_text: response.history_text.clone(),
            content_blocks: response.content_blocks.clone(),
            tool_calls: response.tool_calls.clone(),
            tool_call_id: None,
            name: None,
            is_error: false,
        };
        store.append_conversation_entry(config.session_id, &assistant_entry)?;
        conversation.push(assistant_entry);
        store.append_named_event(
            config.session_id,
            "assistant_turn",
            serde_json::json!({
                "turn": turn_index + 1,
                "stop_reason": response.stop_reason,
                "usage": {
                    "input_tokens": response.usage.input_tokens,
                    "output_tokens": response.usage.output_tokens,
                },
                "tool_calls": response.tool_calls.len(),
                "text_preview": truncate_preview(&response.text, 160),
            }),
        )?;

        if response.tool_calls.is_empty() {
            let duration_ms = started.elapsed().as_millis() as u64;
            let outcome = PromptRunOutcome {
                text: response.text,
                duration_ms,
                duration_api_ms: duration_ms,
                num_turns,
                stop_reason: response.stop_reason.clone(),
                total_cost_usd: 0.0,
                usage,
                model_usage: serde_json::json!({
                    "provider": config.provider.name.clone(),
                    "model": config.provider.model.clone(),
                    "protocol": config.provider.protocol.as_str(),
                    "turns": num_turns,
                    "tool_calls": total_tool_calls,
                }),
                permission_denials,
            };
            store.append_named_event(
                config.session_id,
                "result",
                serde_json::json!({
                    "is_error": false,
                    "stop_reason": response.stop_reason,
                    "usage": {
                        "input_tokens": outcome.usage.input_tokens,
                        "output_tokens": outcome.usage.output_tokens,
                    },
                    "duration_ms": duration_ms,
                    "num_turns": outcome.num_turns,
                }),
            )?;
            return Ok(outcome);
        }

        for tool_call in &response.tool_calls {
            let tool_result = execute_tool_call(tool_call, &tool_context, broker).await?;
            let is_permission_denied = tool_result.is_error
                && tool_result
                    .content
                    .to_ascii_lowercase()
                    .contains("permission denied");
            if is_permission_denied {
                permission_denials.push(serde_json::json!({
                    "tool_name": tool_call.name,
                    "tool_use_id": tool_call.id,
                    "message": tool_result.content.clone(),
                }));
            }
            let tool_preview = truncate_preview(&tool_result.content, 160);
            let tool_entry = ConversationEntry::tool(
                tool_call.id.clone(),
                tool_call.name.clone(),
                tool_result.content,
                tool_result.is_error,
            );
            store.append_conversation_entry(config.session_id, &tool_entry)?;
            store.append_named_event(
                config.session_id,
                "tool_result",
                serde_json::json!({
                    "tool_name": tool_call.name,
                    "tool_use_id": tool_call.id,
                    "is_error": tool_entry.is_error,
                    "content_preview": tool_preview,
                }),
            )?;
            conversation.push(tool_entry);
        }
    }
    let error = anyhow!(
        "Maximum turn budget reached ({}) without a final assistant reply.",
        config.max_turns
    );
    store.append_named_event(
        config.session_id,
        "result",
        serde_json::json!({
            "is_error": true,
            "stop_reason": "max_turns",
            "usage": {
                "input_tokens": usage.input_tokens,
                "output_tokens": usage.output_tokens,
            },
            "duration_ms": started.elapsed().as_millis() as u64,
            "num_turns": num_turns,
            "error": error.to_string(),
        }),
    )?;
    Err(error)
}

fn handle_shell_command(
    input: &str,
    config: &mut RuntimeConfig,
    store: &SessionStore,
    conversation: &mut Vec<ConversationEntry>,
) -> Result<bool> {
    let trimmed = input.trim();
    let mut parts = trimmed.split_whitespace();
    let command = parts.next().unwrap_or_default();
    match command {
        "/help" => {
            println!("Available commands:");
            println!("  /help                 Show this help");
            println!("  /status               Show session and provider details");
            println!("  /sessions             List recent sessions");
            println!("  /resume <session-id>  Switch to an existing session");
            println!("  /export [json|ndjson] [path]");
            println!("  /model [value]        Show or override the active model");
            println!("  /base-url [value]     Show or override the provider base URL");
            println!("  /protocol [value]     Show or set openai/anthropic mode");
            println!("  /api-key [value]      Show presence, set a key, or pass `clear`");
            println!(
                "  /interrupt            Cancel in-flight headless work (interactive shell is synchronous)"
            );
            println!("  /doctor               Run provider readiness checks");
            println!("  /quit                 Exit the shell");
        }
        "/status" => {
            print_shell_status(config, store)?;
            println!("- conversation entries: {}", conversation.len());
        }
        "/sessions" => {
            for session in store.list_sessions()?.into_iter().take(10) {
                println!(
                    "{}  {}  {}",
                    session.session_id, session.updated_at, session.title
                );
            }
        }
        "/resume" => {
            let Some(raw_session_id) = parts.next() else {
                return Err(anyhow!("usage: /resume <session-id>"));
            };
            let session_id = Uuid::parse_str(raw_session_id)?;
            store.get_session_summary(session_id)?;
            config.session_id = session_id;
            restore_session_context(store, config)?;
            *conversation = initialize_conversation(store, config, None)?;
            println!("Resumed session {session_id}");
        }
        "/export" => {
            let first = parts.next();
            let second = parts.next();
            let (format, output) = match first {
                Some("json") => (ExportFormat::Json, second.map(PathBuf::from)),
                Some("ndjson") => (ExportFormat::Ndjson, second.map(PathBuf::from)),
                Some(path) => (ExportFormat::Json, Some(PathBuf::from(path))),
                None => (ExportFormat::Json, None),
            };
            let path = match format {
                ExportFormat::Ndjson => store.export_session(config.session_id, output)?,
                ExportFormat::Json => {
                    store.export_session_bundle_json(config.session_id, output)?
                }
            };
            println!("Exported {}", path.display());
        }
        "/model" => {
            if let Some(model) = parts.next() {
                config.provider.model = Some(model.to_owned());
                persist_session_context(store, config)?;
                println!("Model set to {model}");
            } else {
                println!(
                    "{}",
                    config.provider.model.as_deref().unwrap_or("(missing)")
                );
            }
        }
        "/base-url" => {
            if let Some(base_url) = parts.next() {
                config.provider.base_url =
                    normalize_base_url(Some(base_url.to_owned()), config.provider.protocol);
                persist_session_context(store, config)?;
                println!(
                    "Base URL set to {}",
                    config.provider.base_url.as_deref().unwrap_or("(missing)")
                );
            } else {
                println!(
                    "{}",
                    config.provider.base_url.as_deref().unwrap_or("(missing)")
                );
            }
        }
        "/protocol" => {
            if let Some(protocol) = parts.next() {
                config.provider.protocol = parse_protocol(protocol)?;
                config.provider.base_url =
                    normalize_base_url(config.provider.base_url.clone(), config.provider.protocol);
                persist_session_context(store, config)?;
            }
            println!("{}", config.provider.protocol.as_str());
        }
        "/api-key" => {
            if let Some(api_key) = parts.next() {
                if matches!(api_key, "clear" | "-") {
                    config.provider.api_key = None;
                    println!("API key cleared");
                } else {
                    config.provider.api_key = Some(api_key.to_owned());
                    println!("API key updated");
                }
                persist_session_context(store, config)?;
            } else {
                println!(
                    "api key: {}",
                    if config.provider.api_key.is_some() {
                        "present"
                    } else {
                        "missing"
                    }
                );
            }
        }
        "/interrupt" => {
            println!(
                "Interactive shell turns run synchronously; use stream-json control_request interrupt for live cancellation."
            );
        }
        "/doctor" => run_doctor(config)?,
        "/quit" | "/exit" => return Ok(true),
        _ => {
            println!("Unknown command `{trimmed}`. Type `/help` for a list of commands.");
        }
    }
    Ok(false)
}

fn parse_protocol(value: &str) -> Result<rc_core::ProviderProtocol> {
    match value.trim().to_ascii_lowercase().as_str() {
        "openai" => Ok(rc_core::ProviderProtocol::OpenAi),
        "anthropic" => Ok(rc_core::ProviderProtocol::Anthropic),
        other => Err(anyhow!("unsupported protocol `{other}`")),
    }
}

fn print_shell_status(config: &RuntimeConfig, store: &SessionStore) -> Result<()> {
    if let Ok(summary) = store.get_session_summary(config.session_id) {
        print_session_summary(&summary);
    } else {
        println!("Session: {}", config.session_id);
    }
    println!("- cwd: {}", config.cwd.display());
    println!(
        "- provider: {} ({})",
        config.provider.name,
        config.provider.protocol.as_str()
    );
    println!(
        "- model: {}",
        config.provider.model.as_deref().unwrap_or("(missing)")
    );
    println!(
        "- base URL: {}",
        config.provider.base_url.as_deref().unwrap_or("(missing)")
    );
    println!(
        "- api key: {}",
        if config.provider.api_key.is_some() {
            "present"
        } else {
            "missing"
        }
    );
    Ok(())
}

fn print_session_summary(summary: &SessionSummary) {
    println!("Session {}", summary.session_id);
    println!("- title: {}", summary.title);
    println!("- cwd: {}", summary.cwd.display());
    println!("- provider: {}", summary.provider_name);
    println!(
        "- model: {}",
        summary.model.as_deref().unwrap_or("(missing)")
    );
    println!("- created: {}", summary.created_at);
    println!("- updated: {}", summary.updated_at);
}

fn entry_role_label(role: &rc_core::ConversationRole) -> &'static str {
    match role {
        rc_core::ConversationRole::System => "system",
        rc_core::ConversationRole::User => "user",
        rc_core::ConversationRole::Assistant => "assistant",
        rc_core::ConversationRole::Tool => "tool",
    }
}

fn short_session_id(session_id: Uuid) -> String {
    session_id.to_string().chars().take(8).collect()
}

fn truncate_preview(value: &str, max_chars: usize) -> String {
    let collapsed = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut preview = collapsed.chars().take(max_chars).collect::<String>();
    if collapsed.chars().count() > max_chars {
        preview.push_str("...");
    }
    preview
}

#[derive(Clone)]
struct ChannelPermissionBroker {
    mode: PermissionMode,
    emitter: Arc<Mutex<ProtocolEmitter<io::Stdout>>>,
    pending_permissions: Arc<Mutex<HashMap<String, oneshot::Sender<PermissionDecision>>>>,
}

#[async_trait::async_trait]
impl PermissionBroker for ChannelPermissionBroker {
    fn mode(&self) -> PermissionMode {
        self.mode
    }

    async fn decide(&self, request: PermissionRequest) -> PermissionDecision {
        let request_id = Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        self.pending_permissions
            .lock()
            .await
            .insert(request_id.clone(), tx);
        {
            let mut emitter = self.emitter.lock().await;
            if let Err(error) = emitter.emit_state(SessionState::RequiresAction) {
                warn!("failed to emit state change: {error}");
            }
            if let Err(error) = emitter.emit_permission_request(PermissionRequestPayload {
                request_id: request_id.clone(),
                tool_name: request.tool_name.clone(),
                tool_use_id: request.tool_use_id.clone(),
                title: request.title.clone(),
                description: request.description.clone(),
                input: request.input.clone(),
                blocked_path: request.blocked_path.clone(),
                permission_suggestions: Vec::new(),
            }) {
                warn!("failed to emit permission request: {error}");
            }
        }

        match rx.await {
            Ok(decision) => decision,
            Err(_) => PermissionDecision::deny("Permission request channel closed."),
        }
    }
}

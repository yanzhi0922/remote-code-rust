use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use anyhow::{Result, anyhow};
use clap::{Args, Parser, Subcommand};
use rc_config::{
    ProviderOverrides, RUNTIME_VERSION, RuntimeConfig, import_legacy_profile, load_runtime_config,
    validate_provider_config,
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
use rc_session::SessionStore;
use rc_telemetry::install_tracing;
use rc_tools::{ToolExecutionContext, builtin_tool_specs, execute_tool_call};
use tokio::io::{AsyncBufReadExt, BufReader};
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
}

#[derive(Subcommand, Debug)]
enum MigrateCommand {
    Import {
        #[arg(long)]
        source: Option<PathBuf>,
    },
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
    let config = load_runtime_config(
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

    match cli.command {
        Some(Commands::Doctor) => run_doctor(&config),
        Some(Commands::Sessions { command }) => run_sessions(&store, command),
        Some(Commands::Export(args)) => run_export(&store, args),
        Some(Commands::Migrate { command }) => run_migrate(&config, command),
        Some(Commands::Resume(args)) => {
            let prompt = join_prompt(args.prompt);
            if config.print_mode || matches!(config.output_format, OutputFormat::StreamJson) {
                run_headless(&config, prompt).await
            } else if let Some(prompt) = prompt {
                run_oneshot_text(&config, &store, prompt).await
            } else {
                rc_tui::run_dashboard(&config, &store)
            }
        }
        Some(Commands::Tui) => rc_tui::run_dashboard(&config, &store),
        None => {
            let prompt = join_prompt(cli.prompt);
            if config.print_mode || matches!(config.output_format, OutputFormat::StreamJson) {
                run_headless(&config, prompt).await
            } else if let Some(prompt) = prompt {
                run_oneshot_text(&config, &store, prompt).await
            } else {
                rc_tui::run_dashboard(&config, &store)
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
    }
}

fn run_export(store: &SessionStore, args: ExportArgs) -> Result<()> {
    let path = store.export_session(args.session_id, args.output)?;
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
    println!("{response}");
    Ok(())
}

async fn run_headless(config: &RuntimeConfig, inline_prompt: Option<String>) -> Result<()> {
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
            mcp_servers: Vec::new(),
            model: config.provider.model.clone(),
            permission_mode: config.permission_mode.as_legacy_str().to_owned(),
            slash_commands: vec![
                "/doctor".to_owned(),
                "/sessions".to_owned(),
                "/resume".to_owned(),
                "/export".to_owned(),
            ],
            output_style: "default".to_owned(),
            skills: Vec::new(),
            plugins: Vec::new(),
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
                Ok(text) => {
                    emitter.emit_assistant(&text)?;
                    emitter.emit_result(ResultPayload {
                        is_error: false,
                        duration_ms: 0,
                        duration_api_ms: 0,
                        num_turns: 1,
                        result: text,
                        stop_reason: "end_turn".to_owned(),
                        total_cost_usd: 0.0,
                        usage: UsagePayload::default(),
                        model_usage: serde_json::json!({}),
                        permission_denials: Vec::new(),
                        errors: Vec::new(),
                    })?;
                }
                Err(error) => {
                    emitter.emit_result(ResultPayload {
                        is_error: true,
                        duration_ms: 0,
                        duration_api_ms: 0,
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

async fn run_prompt(
    config: &RuntimeConfig,
    store: &SessionStore,
    provider: &ProviderClient,
    broker: &dyn PermissionBroker,
    conversation: &mut Vec<ConversationEntry>,
    prompt: &str,
) -> Result<String> {
    let readiness = validate_provider_config(&config.provider);
    if !readiness.ok {
        return Err(anyhow!(readiness.issues.join(" ")));
    }

    store.ensure_session(
        config.session_id,
        &config.cwd,
        &config.provider.name,
        config.provider.model.as_deref(),
        Some(prompt),
    )?;
    let user_entry = ConversationEntry::user(prompt);
    store.append_conversation_entry(config.session_id, &user_entry)?;
    conversation.push(user_entry);

    let tool_context = ToolExecutionContext {
        cwd: config.cwd.clone(),
        timeout_ms: config.provider.timeout_ms,
    };
    for _turn in 0..config.max_turns {
        let response = provider.complete(&config.provider, conversation).await?;
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

        if response.tool_calls.is_empty() {
            return Ok(response.text);
        }

        for tool_call in &response.tool_calls {
            let tool_result = execute_tool_call(tool_call, &tool_context, broker).await?;
            let tool_entry = ConversationEntry::tool(
                tool_call.id.clone(),
                tool_call.name.clone(),
                tool_result.content,
                tool_result.is_error,
            );
            store.append_conversation_entry(config.session_id, &tool_entry)?;
            conversation.push(tool_entry);
        }
    }
    Err(anyhow!(
        "Maximum turn budget reached ({}) without a final assistant reply.",
        config.max_turns
    ))
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

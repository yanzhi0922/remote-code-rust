use std::collections::{BTreeMap, BTreeSet, HashMap};
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
use futures::StreamExt;
use rc_agents::{AgentIdentity, AgentScheduler, AgentTask};
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
use reqwest::Client;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{Mutex, mpsc, oneshot};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message as TungsteniteMessage};
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
    Remote {
        #[command(subcommand)]
        command: RemoteCommand,
    },
    Sessions {
        #[command(subcommand)]
        command: Option<SessionsCommand>,
    },
    Resume(ResumeArgs),
    Export(ExportArgs),
    Tui,
    Agents {
        #[command(subcommand)]
        command: AgentsCommand,
    },
    Plugins {
        #[command(subcommand)]
        command: PluginsCommand,
    },
    Mcp {
        #[command(subcommand)]
        command: McpCommand,
    },
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

#[derive(Subcommand, Debug)]
enum RemoteCommand {
    Runners {
        #[command(subcommand)]
        command: RemoteRunnersCommand,
    },
    Approvals {
        #[command(subcommand)]
        command: RemoteApprovalsCommand,
    },
    Events(RemoteEventsArgs),
    Sessions {
        #[command(subcommand)]
        command: RemoteSessionsCommand,
    },
}

#[derive(Subcommand, Debug)]
enum RemoteRunnersCommand {
    List(RemoteRunnersListArgs),
}

#[derive(Subcommand, Debug)]
enum RemoteApprovalsCommand {
    List(RemoteApprovalsListArgs),
    Respond(RemoteApprovalRespondArgs),
}

#[derive(Subcommand, Debug)]
enum RemoteSessionsCommand {
    List(RemoteSessionsListArgs),
    Show(RemoteSessionShowArgs),
    Create(RemoteSessionCreateArgs),
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

#[derive(Args, Debug, Clone)]
struct RemoteTargetArgs {
    #[arg(long, env = "REMOTE_CODE_CONTROL_PLANE_URL")]
    control_plane_url: Option<String>,
}

#[derive(Args, Debug)]
struct RemoteRunnersListArgs {
    #[command(flatten)]
    target: RemoteTargetArgs,

    #[arg(long)]
    json: bool,
}

#[derive(Args, Debug)]
struct RemoteSessionsListArgs {
    #[command(flatten)]
    target: RemoteTargetArgs,

    #[arg(long)]
    json: bool,
}

#[derive(Args, Debug)]
struct RemoteSessionShowArgs {
    #[command(flatten)]
    target: RemoteTargetArgs,

    session_id: Uuid,

    #[arg(long)]
    json: bool,
}

#[derive(Args, Debug)]
struct RemoteSessionCreateArgs {
    #[command(flatten)]
    target: RemoteTargetArgs,

    #[arg(long)]
    workspace_id: String,

    #[arg(long)]
    preferred_runner_id: Option<String>,

    #[arg(long = "meta")]
    metadata: Vec<String>,

    #[arg(long)]
    json: bool,
}

#[derive(Args, Debug)]
struct RemoteApprovalsListArgs {
    #[command(flatten)]
    target: RemoteTargetArgs,

    #[arg(long)]
    session_id: Option<Uuid>,

    #[arg(long)]
    runner_id: Option<String>,

    #[arg(long)]
    json: bool,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum RemoteApprovalDecision {
    Approved,
    Denied,
    Cancelled,
}

#[derive(Args, Debug)]
struct RemoteApprovalRespondArgs {
    #[command(flatten)]
    target: RemoteTargetArgs,

    approval_id: Uuid,

    #[arg(long, value_enum)]
    decision: RemoteApprovalDecision,

    #[arg(long)]
    responder: Option<String>,

    #[arg(long)]
    note: Option<String>,

    #[arg(long)]
    json: bool,
}

#[derive(Args, Debug)]
struct RemoteEventsArgs {
    #[command(flatten)]
    target: RemoteTargetArgs,

    #[arg(long)]
    session_id: Option<Uuid>,

    #[arg(long)]
    after: Option<u64>,

    #[arg(long, default_value_t = 20)]
    limit: usize,

    #[arg(long)]
    follow: bool,

    #[arg(long, default_value_t = 2)]
    poll_interval_secs: u64,

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

#[derive(Subcommand, Debug)]
enum AgentsCommand {
    Plan(AgentsPlanArgs),
}

#[derive(Subcommand, Debug)]
enum McpCommand {
    List(McpListArgs),
    Call(McpCallArgs),
}

#[derive(Subcommand, Debug)]
enum PluginsCommand {
    List(PluginsListArgs),
    Inspect(PluginsInspectArgs),
    Invoke(PluginsInvokeArgs),
}

#[derive(Args, Debug)]
struct AgentsPlanArgs {
    #[arg(long, default_value = "codex-lead")]
    lead: String,

    #[arg(long)]
    objective: String,

    #[arg(long = "agent")]
    agents: Vec<String>,

    #[arg(long = "task")]
    tasks: Vec<String>,

    #[arg(long)]
    json: bool,
}

#[derive(Args, Debug)]
struct McpListArgs {
    #[arg(long)]
    connect: bool,

    #[arg(long)]
    json: bool,

    #[arg(long = "server")]
    servers: Vec<String>,

    #[arg(long)]
    include_disabled: bool,

    #[arg(long = "config")]
    config_paths: Vec<PathBuf>,
}

#[derive(Args, Debug)]
struct McpCallArgs {
    #[arg(long)]
    server: String,

    #[arg(long)]
    tool: String,

    #[arg(long)]
    json: bool,

    #[arg(long = "include-disabled")]
    include_disabled: bool,

    #[arg(long = "arg")]
    args: Vec<String>,

    #[arg(long = "args-json")]
    args_json: Option<String>,

    #[arg(long = "config")]
    config_paths: Vec<PathBuf>,
}

#[derive(Args, Debug)]
struct PluginsListArgs {
    #[arg(long)]
    connect: bool,

    #[arg(long)]
    json: bool,

    #[arg(long = "plugin")]
    plugins: Vec<String>,

    #[arg(long = "plugins-dir")]
    plugin_roots: Vec<PathBuf>,
}

#[derive(Args, Debug)]
struct PluginsInspectArgs {
    #[arg(long)]
    plugin: String,

    #[arg(long)]
    json: bool,

    #[arg(long = "plugins-dir")]
    plugin_roots: Vec<PathBuf>,
}

#[derive(Args, Debug)]
struct PluginsInvokeArgs {
    #[arg(long)]
    plugin: String,

    #[arg(long)]
    action: String,

    #[arg(long)]
    json: bool,

    #[arg(long = "arg")]
    args: Vec<String>,

    #[arg(long = "input-json")]
    input_json: Option<String>,

    #[arg(long = "plugins-dir")]
    plugin_roots: Vec<PathBuf>,
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
        Some(Commands::Remote { command }) => run_remote(command).await,
        Some(Commands::Sessions { command }) => run_sessions(&store, command),
        Some(Commands::Export(args)) => run_export(&store, args),
        Some(Commands::Agents { command }) => run_agents(&config, command),
        Some(Commands::Plugins { command }) => run_plugins(&config, command).await,
        Some(Commands::Mcp { command }) => run_mcp(&config, command).await,
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

fn parse_agent_spec(spec: &str) -> Result<AgentIdentity> {
    let mut segments = spec.splitn(4, ';').map(str::trim);
    let name = segments
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("invalid --agent spec `{spec}`; expected name;role;paths;labels"))?;
    let role = segments
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("invalid --agent spec `{spec}`; role is missing"))?;
    let mut agent = AgentIdentity::new(name, role);
    agent.ownership_paths = segments.next().map(parse_csv_list).unwrap_or_default();
    agent.labels = segments
        .next()
        .map(parse_key_value_pairs)
        .unwrap_or_default();
    Ok(agent)
}

fn parse_task_spec(spec: &str) -> Result<AgentTask> {
    let mut segments = spec.splitn(4, ';').map(str::trim);
    let title = segments
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            anyhow!("invalid --task spec `{spec}`; expected title;paths;labels;description")
        })?;
    let mut task = AgentTask::new(title);
    task.ownership_paths = segments.next().map(parse_csv_list).unwrap_or_default();
    task.required_labels = segments
        .next()
        .map(parse_key_value_pairs)
        .unwrap_or_default();
    task.description = segments.next().unwrap_or_default().to_owned();
    task.budget.read_calls = 32;
    task.budget.edit_calls = 12;
    task.budget.command_calls = 8;
    Ok(task)
}

fn default_agent_specs() -> Vec<AgentIdentity> {
    vec![
        parse_agent_spec("planner;planner;;phase=plan").unwrap_or_else(|_| {
            let mut agent = AgentIdentity::new("planner", "planner");
            agent.labels.insert("phase".to_owned(), "plan".to_owned());
            agent
        }),
        parse_agent_spec("runtime;implementer;apps/remote-code,crates/rc-session,crates/rc-tools;phase=local")
            .unwrap_or_else(|_| AgentIdentity::new("runtime", "implementer")),
        parse_agent_spec(
            "remote;implementer;apps/remote-code-runner,apps/remote-code-control-plane,crates/rc-runner,crates/rc-control-plane;phase=remote",
        )
        .unwrap_or_else(|_| AgentIdentity::new("remote", "implementer")),
        parse_agent_spec("review;reviewer;.;phase=review")
            .unwrap_or_else(|_| AgentIdentity::new("review", "reviewer")),
    ]
}

fn default_task_for_objective(objective: &str, config: &RuntimeConfig) -> AgentTask {
    let mut task = AgentTask::new(objective);
    task.description = format!(
        "Coordinate work for {} in {}",
        objective,
        config.cwd.display()
    );
    task.ownership_paths = vec![config.cwd.display().to_string()];
    task.budget.read_calls = 64;
    task.budget.edit_calls = 16;
    task.budget.command_calls = 12;
    task
}

fn parse_csv_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn parse_key_value_pairs(value: &str) -> std::collections::BTreeMap<String, String> {
    value
        .split(',')
        .filter_map(|entry| {
            let (key, value) = entry.split_once('=')?;
            let key = key.trim();
            let value = value.trim();
            if key.is_empty() || value.is_empty() {
                None
            } else {
                Some((key.to_owned(), value.to_owned()))
            }
        })
        .collect()
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct RemoteListResponse<T> {
    items: Vec<T>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct RemoteRunnerSnapshot {
    registration: RemoteRunnerRegistration,
    state: RemoteRunnerState,
    active_sessions: usize,
    queued_sessions: usize,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct RemoteRunnerRegistration {
    runner_id: String,
    public_base_url: Option<String>,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum RemoteRunnerState {
    Starting,
    Idle,
    Busy,
    Draining,
    Unhealthy,
    Offline,
}

impl RemoteRunnerState {
    fn label(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Idle => "idle",
            Self::Busy => "busy",
            Self::Draining => "draining",
            Self::Unhealthy => "unhealthy",
            Self::Offline => "offline",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct RemoteSessionRecord {
    session_id: Uuid,
    workspace_id: String,
    owner_runner_id: Option<String>,
    state: RemoteSessionState,
    #[serde(default)]
    metadata: BTreeMap<String, String>,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum RemoteSessionState {
    Pending,
    Assigned,
    Running,
    WaitingApproval,
    Completed,
    Failed,
    Cancelled,
}

impl RemoteSessionState {
    fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Assigned => "assigned",
            Self::Running => "running",
            Self::WaitingApproval => "waiting_approval",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
struct RemoteCreateSessionRequest {
    session_id: Option<Uuid>,
    workspace_id: String,
    preferred_runner_id: Option<String>,
    metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct RemoteApprovalRecord {
    approval_id: Uuid,
    session_id: Uuid,
    runner_id: String,
    state: RemoteApprovalState,
    title: String,
    description: String,
    #[serde(default)]
    metadata: BTreeMap<String, String>,
    created_at: String,
    updated_at: String,
    responded_at: Option<String>,
    #[serde(default)]
    responder: Option<String>,
    #[serde(default)]
    note: Option<String>,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum RemoteApprovalState {
    Pending,
    Approved,
    Denied,
    Cancelled,
}

impl RemoteApprovalState {
    fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Denied => "denied",
            Self::Cancelled => "cancelled",
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
struct RemoteApprovalDecisionRequest {
    decision: RemoteApprovalDecision,
    responder: Option<String>,
    note: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct RemoteTimelineEvent {
    sequence: u64,
    recorded_at: String,
    runner_id: Option<String>,
    session_id: Option<Uuid>,
    detail: RemoteTimelineEventDetail,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum RemoteTimelineEventDetail {
    RunnerRegistered {
        lease_ttl_secs: u64,
        workspace_ids: Vec<String>,
        state: RemoteRunnerState,
    },
    RunnerHeartbeat {
        state: RemoteRunnerState,
        active_sessions: usize,
        queued_sessions: usize,
        reported_at: String,
    },
    SessionCreated {
        workspace_id: String,
        owner_runner_id: Option<String>,
        state: RemoteSessionState,
    },
    ApprovalRequested {
        approval_id: Uuid,
        title: String,
        state: RemoteApprovalState,
    },
    ApprovalResolved {
        approval_id: Uuid,
        state: RemoteApprovalState,
        responder: Option<String>,
    },
    ArtifactCreated {
        artifact_id: Uuid,
        name: String,
        file_name: String,
        media_type: String,
        size_bytes: u64,
    },
}

#[derive(Debug, Clone, serde::Deserialize)]
struct RemoteErrorEnvelope {
    error: RemoteErrorDetail,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct RemoteErrorDetail {
    message: String,
}

fn require_control_plane_url(target: &RemoteTargetArgs) -> Result<String> {
    target
        .control_plane_url
        .clone()
        .ok_or_else(|| anyhow!("missing control plane URL; pass --control-plane-url or set REMOTE_CODE_CONTROL_PLANE_URL"))
}

fn parse_repeated_key_value_args(
    flag_name: &str,
    values: &[String],
) -> Result<BTreeMap<String, String>> {
    let mut parsed = BTreeMap::new();
    for value in values {
        let (key, entry_value) = value
            .split_once('=')
            .ok_or_else(|| anyhow!("invalid {flag_name} `{value}`; expected key=value"))?;
        let key = key.trim();
        let entry_value = entry_value.trim();
        if key.is_empty() {
            return Err(anyhow!(
                "invalid {flag_name} `{value}`; key cannot be empty"
            ));
        }
        parsed.insert(key.to_owned(), entry_value.to_owned());
    }
    Ok(parsed)
}

fn normalize_remote_base_url(raw: &str) -> Result<String> {
    let trimmed = raw.trim().trim_end_matches('/').to_owned();
    if trimmed.is_empty() {
        return Err(anyhow!("control plane URL is empty"));
    }
    if !(trimmed.starts_with("http://") || trimmed.starts_with("https://")) {
        return Err(anyhow!(
            "control plane URL must start with http:// or https://"
        ));
    }
    Ok(trimmed)
}

fn build_remote_http_url(base_url: &str, path: &str) -> Result<String> {
    Ok(format!(
        "{}{}",
        normalize_remote_base_url(base_url)?,
        normalize_remote_request_path(path)
    ))
}

fn build_remote_ws_url(base_url: &str, path: &str) -> Result<String> {
    let base = normalize_remote_base_url(base_url)?;
    if let Some(rest) = base.strip_prefix("http://") {
        return Ok(format!(
            "ws://{rest}{}",
            normalize_remote_request_path(path)
        ));
    }
    if let Some(rest) = base.strip_prefix("https://") {
        return Ok(format!(
            "wss://{rest}{}",
            normalize_remote_request_path(path)
        ));
    }
    Err(anyhow!(
        "control plane URL must start with http:// or https://"
    ))
}

async fn remote_get_json<T>(base_url: &str, path: &str) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let client = Client::new();
    let response = client
        .get(build_remote_http_url(base_url, path)?)
        .send()
        .await?;
    decode_remote_json_response(response).await
}

async fn remote_post_json<I, O>(base_url: &str, path: &str, input: &I) -> Result<O>
where
    I: serde::Serialize,
    O: serde::de::DeserializeOwned,
{
    let client = Client::new();
    let response = client
        .post(build_remote_http_url(base_url, path)?)
        .json(input)
        .send()
        .await?;
    decode_remote_json_response(response).await
}

async fn decode_remote_json_response<T>(response: reqwest::Response) -> Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let status = response.status();
    let bytes = response.bytes().await?;
    if status.is_success() {
        return Ok(serde_json::from_slice(&bytes)?);
    }

    let message = serde_json::from_slice::<RemoteErrorEnvelope>(&bytes)
        .map(|error| error.error.message)
        .unwrap_or_else(|_| String::from_utf8_lossy(&bytes).trim().to_owned());
    Err(anyhow!(
        "control plane request failed with HTTP {}: {}",
        status.as_u16(),
        if message.is_empty() {
            "unknown error"
        } else {
            &message
        }
    ))
}

fn normalize_remote_request_path(path: &str) -> String {
    if path.starts_with('/') {
        path.to_owned()
    } else {
        format!("/{path}")
    }
}

fn print_remote_session_summary(session: &RemoteSessionRecord) {
    println!("Remote session {}", session.session_id);
    println!("- workspace: {}", session.workspace_id);
    println!("- state: {}", session.state.label());
    println!(
        "- runner: {}",
        session.owner_runner_id.as_deref().unwrap_or("(unassigned)")
    );
    println!("- created: {}", session.created_at);
    println!("- updated: {}", session.updated_at);
    if !session.metadata.is_empty() {
        println!("- metadata: {}", format_remote_metadata(&session.metadata));
    }
}

fn print_remote_approval_summary(approval: &RemoteApprovalRecord) {
    println!("Remote approval {}", approval.approval_id);
    println!("- state: {}", approval.state.label());
    println!("- session: {}", approval.session_id);
    println!(
        "- runner: {}",
        if approval.runner_id.is_empty() {
            "(unassigned-runner)"
        } else {
            approval.runner_id.as_str()
        }
    );
    println!("- title: {}", approval.title);
    println!("- description: {}", approval.description);
    println!("- created: {}", approval.created_at);
    println!("- updated: {}", approval.updated_at);
    if let Some(responder) = &approval.responder {
        println!("- responder: {responder}");
    }
    if let Some(note) = &approval.note {
        println!("- note: {note}");
    }
    if !approval.metadata.is_empty() {
        println!("- metadata: {}", format_remote_metadata(&approval.metadata));
    }
}

fn print_remote_events(events: &[RemoteTimelineEvent]) {
    if events.is_empty() {
        println!("No remote events found.");
        return;
    }
    for event in events {
        println!(
            "{}  {}  {}  session={}  runner={}  {}",
            event.sequence,
            event.recorded_at,
            remote_event_kind(&event.detail),
            event
                .session_id
                .map(|value| value.to_string())
                .unwrap_or_else(|| "-".to_owned()),
            event.runner_id.as_deref().unwrap_or("-"),
            remote_event_summary(&event.detail)
        );
    }
}

fn format_remote_metadata(metadata: &BTreeMap<String, String>) -> String {
    metadata
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join(", ")
}

fn remote_approvals_path(session_id: Option<Uuid>, runner_id: Option<&str>) -> Result<String> {
    match (session_id, runner_id) {
        (Some(_), Some(_)) => Err(anyhow!(
            "choose either --session-id or --runner-id when listing approvals"
        )),
        (Some(session_id), None) => Ok(format!("/v1/sessions/{session_id}/approvals")),
        (None, Some(runner_id)) => Ok(format!("/v1/runners/{runner_id}/approvals")),
        (None, None) => Ok("/v1/approvals".to_owned()),
    }
}

fn remote_events_path(session_id: Option<Uuid>, after: Option<u64>, limit: usize) -> String {
    let mut path = match session_id {
        Some(session_id) => format!("/v1/sessions/{session_id}/events"),
        None => "/v1/events".to_owned(),
    };
    let mut query = Vec::new();
    if let Some(after) = after {
        query.push(format!("after={after}"));
    }
    query.push(format!("limit={}", limit.clamp(1, 200)));
    if !query.is_empty() {
        path.push('?');
        path.push_str(&query.join("&"));
    }
    path
}

fn remote_event_kind(detail: &RemoteTimelineEventDetail) -> &'static str {
    match detail {
        RemoteTimelineEventDetail::RunnerRegistered { .. } => "runner_registered",
        RemoteTimelineEventDetail::RunnerHeartbeat { .. } => "runner_heartbeat",
        RemoteTimelineEventDetail::SessionCreated { .. } => "session_created",
        RemoteTimelineEventDetail::ApprovalRequested { .. } => "approval_requested",
        RemoteTimelineEventDetail::ApprovalResolved { .. } => "approval_resolved",
        RemoteTimelineEventDetail::ArtifactCreated { .. } => "artifact_created",
    }
}

fn remote_event_summary(detail: &RemoteTimelineEventDetail) -> String {
    match detail {
        RemoteTimelineEventDetail::RunnerRegistered {
            workspace_ids,
            state,
            ..
        } => format!(
            "workspaces={} state={}",
            workspace_ids.join(","),
            state.label()
        ),
        RemoteTimelineEventDetail::RunnerHeartbeat {
            state,
            active_sessions,
            queued_sessions,
            ..
        } => format!(
            "state={} active={} queued={}",
            state.label(),
            active_sessions,
            queued_sessions
        ),
        RemoteTimelineEventDetail::SessionCreated {
            workspace_id,
            owner_runner_id,
            state,
        } => format!(
            "workspace={} runner={} state={}",
            workspace_id,
            owner_runner_id.as_deref().unwrap_or("(unassigned)"),
            state.label()
        ),
        RemoteTimelineEventDetail::ApprovalRequested { title, state, .. } => {
            format!("title={title} state={}", state.label())
        }
        RemoteTimelineEventDetail::ApprovalResolved {
            state, responder, ..
        } => format!(
            "state={} responder={}",
            state.label(),
            responder.as_deref().unwrap_or("(none)")
        ),
        RemoteTimelineEventDetail::ArtifactCreated {
            file_name,
            media_type,
            size_bytes,
            ..
        } => format!(
            "file={} media_type={} size={}B",
            file_name, media_type, size_bytes
        ),
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
        format!(
            "- discovered plugin runtimes: {}",
            discovery.plugin_runtimes.len()
        ),
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

async fn run_remote(command: RemoteCommand) -> Result<()> {
    match command {
        RemoteCommand::Runners { command } => run_remote_runners(command).await,
        RemoteCommand::Approvals { command } => run_remote_approvals(command).await,
        RemoteCommand::Events(args) => run_remote_events(args).await,
        RemoteCommand::Sessions { command } => run_remote_sessions(command).await,
    }
}

async fn run_remote_runners(command: RemoteRunnersCommand) -> Result<()> {
    match command {
        RemoteRunnersCommand::List(args) => run_remote_runners_list(args).await,
    }
}

async fn run_remote_sessions(command: RemoteSessionsCommand) -> Result<()> {
    match command {
        RemoteSessionsCommand::List(args) => run_remote_sessions_list(args).await,
        RemoteSessionsCommand::Show(args) => run_remote_sessions_show(args).await,
        RemoteSessionsCommand::Create(args) => run_remote_sessions_create(args).await,
    }
}

async fn run_remote_runners_list(args: RemoteRunnersListArgs) -> Result<()> {
    let control_plane_url = require_control_plane_url(&args.target)?;
    let response: RemoteListResponse<RemoteRunnerSnapshot> =
        remote_get_json(&control_plane_url, "/v1/runners").await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&response)?);
        return Ok(());
    }
    if response.items.is_empty() {
        println!("No remote runners found.");
        return Ok(());
    }
    for runner in response.items {
        println!(
            "{}  {}  active={}  queued={}  {}",
            runner.registration.runner_id,
            runner.state.label(),
            runner.active_sessions,
            runner.queued_sessions,
            runner
                .registration
                .public_base_url
                .as_deref()
                .unwrap_or("(missing-public-base-url)")
        );
    }
    Ok(())
}

async fn run_remote_sessions_list(args: RemoteSessionsListArgs) -> Result<()> {
    let control_plane_url = require_control_plane_url(&args.target)?;
    let response: RemoteListResponse<RemoteSessionRecord> =
        remote_get_json(&control_plane_url, "/v1/sessions").await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&response)?);
        return Ok(());
    }
    if response.items.is_empty() {
        println!("No remote sessions found.");
        return Ok(());
    }
    for session in response.items {
        println!(
            "{}  {}  {}  {}  {}",
            session.session_id,
            session.updated_at,
            session.state.label(),
            session.workspace_id,
            session.owner_runner_id.as_deref().unwrap_or("(unassigned)")
        );
    }
    Ok(())
}

async fn run_remote_sessions_show(args: RemoteSessionShowArgs) -> Result<()> {
    let control_plane_url = require_control_plane_url(&args.target)?;
    let path = format!("/v1/sessions/{}", args.session_id);
    let session: RemoteSessionRecord = remote_get_json(&control_plane_url, &path).await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&session)?);
        return Ok(());
    }
    print_remote_session_summary(&session);
    Ok(())
}

async fn run_remote_approvals(command: RemoteApprovalsCommand) -> Result<()> {
    match command {
        RemoteApprovalsCommand::List(args) => run_remote_approvals_list(args).await,
        RemoteApprovalsCommand::Respond(args) => run_remote_approvals_respond(args).await,
    }
}

async fn run_remote_approvals_list(args: RemoteApprovalsListArgs) -> Result<()> {
    let control_plane_url = require_control_plane_url(&args.target)?;
    let path = remote_approvals_path(args.session_id, args.runner_id.as_deref())?;
    let response: RemoteListResponse<RemoteApprovalRecord> =
        remote_get_json(&control_plane_url, &path).await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&response)?);
        return Ok(());
    }
    if response.items.is_empty() {
        println!("No remote approvals found.");
        return Ok(());
    }
    for approval in response.items {
        println!(
            "{}  {}  {}  {}  {}",
            approval.approval_id,
            approval.state.label(),
            approval.session_id,
            if approval.runner_id.is_empty() {
                "(unassigned-runner)"
            } else {
                approval.runner_id.as_str()
            },
            approval.title
        );
    }
    Ok(())
}

async fn run_remote_approvals_respond(args: RemoteApprovalRespondArgs) -> Result<()> {
    let control_plane_url = require_control_plane_url(&args.target)?;
    let path = format!("/v1/approvals/{}/decision", args.approval_id);
    let request = RemoteApprovalDecisionRequest {
        decision: args.decision,
        responder: args.responder,
        note: args.note,
    };
    let approval: RemoteApprovalRecord =
        remote_post_json(&control_plane_url, &path, &request).await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&approval)?);
        return Ok(());
    }
    print_remote_approval_summary(&approval);
    Ok(())
}

async fn run_remote_events(args: RemoteEventsArgs) -> Result<()> {
    let control_plane_url = require_control_plane_url(&args.target)?;
    if args.follow {
        return run_remote_events_follow(control_plane_url, args).await;
    }

    let path = remote_events_path(args.session_id, args.after, args.limit);
    let response: RemoteListResponse<RemoteTimelineEvent> =
        remote_get_json(&control_plane_url, &path).await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&response)?);
    } else {
        print_remote_events(&response.items);
    }
    Ok(())
}

async fn run_remote_events_follow(control_plane_url: String, args: RemoteEventsArgs) -> Result<()> {
    let history_path = remote_events_path(args.session_id, args.after, args.limit);
    let response: RemoteListResponse<RemoteTimelineEvent> =
        remote_get_json(&control_plane_url, &history_path).await?;
    if args.json {
        for event in &response.items {
            println!("{}", serde_json::to_string(event)?);
        }
    } else {
        print_remote_events(&response.items);
    }

    let ws_path = match args.session_id {
        Some(session_id) => format!("/v1/sessions/{session_id}/events/stream"),
        None => "/v1/events/stream".to_owned(),
    };
    let ws_url = build_remote_ws_url(&control_plane_url, &ws_path)?;
    let (mut socket, _) = connect_async(&ws_url).await?;
    loop {
        tokio::select! {
            result = tokio::signal::ctrl_c() => {
                result?;
                break;
            }
            message = socket.next() => {
                let Some(message) = message else {
                    break;
                };
                let message = message?;
                match message {
                    TungsteniteMessage::Text(text) => {
                        let event: RemoteTimelineEvent = serde_json::from_str(&text)?;
                        if args.json {
                            println!("{}", serde_json::to_string(&event)?);
                        } else {
                            print_remote_events(&[event]);
                        }
                    }
                    TungsteniteMessage::Binary(bytes) => {
                        let event: RemoteTimelineEvent = serde_json::from_slice(&bytes)?;
                        if args.json {
                            println!("{}", serde_json::to_string(&event)?);
                        } else {
                            print_remote_events(&[event]);
                        }
                    }
                    TungsteniteMessage::Close(_) => break,
                    _ => {}
                }
            }
        }
    }
    Ok(())
}

async fn run_remote_sessions_create(args: RemoteSessionCreateArgs) -> Result<()> {
    let control_plane_url = require_control_plane_url(&args.target)?;
    let request = RemoteCreateSessionRequest {
        session_id: None,
        workspace_id: args.workspace_id,
        preferred_runner_id: args.preferred_runner_id,
        metadata: parse_repeated_key_value_args("--meta", &args.metadata)?,
    };
    let session: RemoteSessionRecord =
        remote_post_json(&control_plane_url, "/v1/sessions", &request).await?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&session)?);
        return Ok(());
    }
    print_remote_session_summary(&session);
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

fn run_agents(config: &RuntimeConfig, command: AgentsCommand) -> Result<()> {
    match command {
        AgentsCommand::Plan(args) => run_agents_plan(config, args),
    }
}

async fn run_plugins(config: &RuntimeConfig, command: PluginsCommand) -> Result<()> {
    match command {
        PluginsCommand::List(args) => run_plugins_list(config, args).await,
        PluginsCommand::Inspect(args) => run_plugins_inspect(config, args).await,
        PluginsCommand::Invoke(args) => run_plugins_invoke(config, args).await,
    }
}

async fn run_mcp(config: &RuntimeConfig, command: McpCommand) -> Result<()> {
    match command {
        McpCommand::List(args) => run_mcp_list(config, args).await,
        McpCommand::Call(args) => run_mcp_call(config, args).await,
    }
}

fn run_agents_plan(config: &RuntimeConfig, args: AgentsPlanArgs) -> Result<()> {
    let mut scheduler = AgentScheduler::new(args.lead.clone(), args.objective.clone());
    let agents = if args.agents.is_empty() {
        default_agent_specs()
    } else {
        args.agents
            .iter()
            .map(|spec| parse_agent_spec(spec))
            .collect::<Result<Vec<_>>>()?
    };
    for agent in agents {
        scheduler.register_agent(agent);
    }

    let tasks = if args.tasks.is_empty() {
        vec![default_task_for_objective(&args.objective, config)]
    } else {
        args.tasks
            .iter()
            .map(|spec| parse_task_spec(spec))
            .collect::<Result<Vec<_>>>()?
    };
    for task in tasks {
        scheduler.add_task(task);
    }

    while let Some((task_id, agent_id)) = scheduler.assign_next_task() {
        let agent = scheduler
            .agents()
            .into_iter()
            .find(|agent| agent.agent_id == agent_id)
            .ok_or_else(|| anyhow!("assigned agent {agent_id} was not found"))?;
        let task = scheduler
            .tasks()
            .into_iter()
            .find(|task| task.id == task_id)
            .ok_or_else(|| anyhow!("assigned task {task_id} was not found"))?;
        let _ = scheduler.queue_instruction(
            agent_id,
            args.lead.clone(),
            format!("Task: {}", task.title),
            format!(
                "Objective: {}\nTask: {}\nOwnership: {}",
                args.objective,
                task.title,
                if task.ownership_paths.is_empty() {
                    "(unscoped)".to_owned()
                } else {
                    task.ownership_paths.join(", ")
                }
            ),
        );
        if args.json {
            continue;
        }
        println!(
            "Assigned `{}` -> {} ({})",
            task.title, agent.name, agent.role
        );
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&scheduler.snapshot())?);
    } else {
        let summary = scheduler.summary();
        println!(
            "\nTeam {}: {} agent(s), {} task(s), {} pending message(s)",
            summary.team_id, summary.total_agents, summary.total_tasks, summary.pending_messages
        );
    }
    Ok(())
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
                    let peer = live
                        .server_info
                        .as_ref()
                        .map(|info| match &info.version {
                            Some(version) => format!("{} {}", info.name, version),
                            None => info.name.clone(),
                        })
                        .unwrap_or_else(|| "unknown-server".to_owned());
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
                    let peer = live
                        .plugin_info
                        .as_ref()
                        .map(|info| match &info.version {
                            Some(version) => format!("{} {}", info.name, version),
                            None => info.name.clone(),
                        })
                        .unwrap_or_else(|| "unknown-plugin".to_owned());
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
    plugin_runtimes: Vec<String>,
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

#[derive(Debug, Clone)]
struct RuntimeMcpServerEntry {
    origin_kind: &'static str,
    origin_name: String,
    config_path: PathBuf,
    server: rc_mcp::McpServerConfig,
}

#[derive(Debug, Clone, Default)]
struct RuntimeMcpDiscovery {
    servers: Vec<RuntimeMcpServerEntry>,
    warnings: Vec<String>,
}

#[derive(Debug, Clone)]
struct RuntimeMcpResolution {
    entry: RuntimeMcpServerEntry,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct McpListOutput {
    warnings: Vec<String>,
    servers: Vec<McpServerRecord>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct McpServerRecord {
    name: String,
    enabled: bool,
    transport: rc_mcp::McpTransport,
    origin_kind: String,
    origin_name: String,
    config_path: PathBuf,
    live: Option<McpLiveRecord>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct McpLiveRecord {
    status: String,
    protocol_version: Option<String>,
    server_info: Option<rc_mcp::McpPeerInfo>,
    tool_count: usize,
    tools: Vec<rc_mcp::McpToolDescriptor>,
    error: Option<String>,
}

impl McpLiveRecord {
    fn from_inspection(inspection: rc_mcp::McpServerInspection) -> Self {
        Self {
            status: "ok".to_owned(),
            protocol_version: Some(inspection.protocol_version),
            server_info: inspection.server_info,
            tool_count: inspection.tools.len(),
            tools: inspection.tools,
            error: None,
        }
    }

    fn skipped(reason: impl Into<String>) -> Self {
        Self {
            status: "skipped".to_owned(),
            protocol_version: None,
            server_info: None,
            tool_count: 0,
            tools: Vec::new(),
            error: Some(reason.into()),
        }
    }

    fn failed(error: impl ToString) -> Self {
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
struct McpCallOutput {
    warnings: Vec<String>,
    server: McpCallServerRecord,
    arguments: serde_json::Value,
    response: rc_mcp::McpToolCallResponse,
}

#[derive(Debug, Clone, serde::Serialize)]
struct McpCallServerRecord {
    name: String,
    enabled: bool,
    origin_kind: String,
    origin_name: String,
    config_path: PathBuf,
}

#[derive(Debug, Clone)]
struct RuntimePluginEntry {
    origin_kind: &'static str,
    origin_name: String,
    bundle: rc_plugins::PluginBundle,
}

#[derive(Debug, Clone, Default)]
struct RuntimePluginDiscovery {
    plugins: Vec<RuntimePluginEntry>,
    warnings: Vec<String>,
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
struct PluginRecord {
    name: String,
    version: String,
    has_runtime: bool,
    has_skills: bool,
    has_mcp: bool,
    origin_kind: String,
    origin_name: String,
    root: PathBuf,
    manifest_path: PathBuf,
    live: Option<PluginLiveRecord>,
}

#[derive(Debug, Clone, serde::Serialize)]
struct PluginLiveRecord {
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

    fn failed(error: impl ToString) -> Self {
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

fn discover_runtime_extensions(config: &RuntimeConfig) -> RuntimeExtensionDiscovery {
    let mut skills = BTreeSet::new();
    let mut plugins = BTreeSet::new();
    let mut plugin_runtimes = BTreeSet::new();
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
                    if plugin.runtime_config().is_some() {
                        plugin_runtimes.insert(plugin.manifest.name.clone());
                    }
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
        plugin_runtimes: plugin_runtimes.into_iter().collect(),
        mcp_servers: mcp_servers.into_iter().collect(),
        warnings,
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
            if !has_runtime {
                Some(PluginLiveRecord::skipped(
                    "plugin does not define a runtime adapter",
                ))
            } else {
                Some(
                    match rc_plugins::inspect_runtime(
                        &entry.bundle,
                        &rc_plugins::PluginHostInfo::new("remote-code-rust", RUNTIME_VERSION),
                    )
                    .await
                    {
                        Ok(inspection) => PluginLiveRecord::from_inspection(inspection),
                        Err(error) => PluginLiveRecord::failed(error),
                    },
                )
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
                Err(error) => PluginLiveRecord::failed(error),
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

async fn build_mcp_list_output(
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
                        Err(error) => McpLiveRecord::failed(error),
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

async fn build_mcp_call_output(
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

fn parse_plugin_invoke_input(args: &PluginsInvokeArgs) -> Result<serde_json::Value> {
    parse_named_json_object_args("--input-json", &args.input_json, &args.args)
}

fn parse_mcp_call_arguments(args: &McpCallArgs) -> Result<serde_json::Value> {
    parse_named_json_object_args("--args-json", &args.args_json, &args.args)
}

fn resolve_runtime_mcp_server(
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

fn discover_runtime_plugins(
    config: &RuntimeConfig,
    extra_plugin_roots: &[PathBuf],
) -> RuntimePluginDiscovery {
    let mut discovery = RuntimePluginDiscovery::default();
    let mut seen_manifest_paths = BTreeSet::new();
    load_runtime_plugins_root(
        &mut discovery,
        &mut seen_manifest_paths,
        "profile",
        config.paths.plugins_dir.display().to_string(),
        config.paths.plugins_dir.clone(),
    );
    for root in extra_plugin_roots {
        load_runtime_plugins_root(
            &mut discovery,
            &mut seen_manifest_paths,
            "explicit",
            root.display().to_string(),
            root.clone(),
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
    origin_name: String,
    root: PathBuf,
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
    match rc_plugins::discover_plugins(&root) {
        Ok(plugins) => {
            for plugin in plugins {
                if !seen_manifest_paths.insert(plugin.manifest_path.clone()) {
                    continue;
                }
                discovery.plugins.push(RuntimePluginEntry {
                    origin_kind,
                    origin_name: origin_name.clone(),
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

fn parse_named_json_object_args(
    json_flag_name: &str,
    json_value: &Option<String>,
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

fn discover_runtime_mcp_servers(
    config: &RuntimeConfig,
    extra_config_paths: &[PathBuf],
) -> RuntimeMcpDiscovery {
    let mut discovery = RuntimeMcpDiscovery::default();
    let mut loaded_paths = BTreeSet::new();
    load_runtime_mcp_file(
        &mut discovery,
        &mut loaded_paths,
        "cwd",
        config.cwd.display().to_string(),
        config.cwd.join(rc_mcp::DEFAULT_MCP_CONFIG_FILE),
    );
    load_runtime_mcp_file(
        &mut discovery,
        &mut loaded_paths,
        "profile",
        config.paths.profile_dir.display().to_string(),
        config
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
            path.display().to_string(),
            candidate,
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
                                plugin.manifest.name,
                                path,
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
    origin_name: String,
    path: PathBuf,
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
    if !loaded_paths.insert(path.clone()) {
        return;
    }
    match rc_mcp::McpConfig::load(&path) {
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
    origin_name: String,
    config_path: PathBuf,
    config: rc_mcp::McpConfig,
) {
    for server in config.servers.into_values() {
        servers.push(RuntimeMcpServerEntry {
            origin_kind,
            origin_name: origin_name.clone(),
            config_path: config_path.clone(),
            server,
        });
    }
}

fn format_mcp_transport(transport: rc_mcp::McpTransport) -> &'static str {
    match transport {
        rc_mcp::McpTransport::Stdio => "stdio",
        rc_mcp::McpTransport::Http => "http",
        rc_mcp::McpTransport::WebSocket => "websocket",
    }
}

fn format_mcp_source(server: &McpServerRecord) -> String {
    match server.origin_kind.as_str() {
        "plugin" => format!(
            "plugin:{} ({})",
            server.origin_name,
            server.config_path.display()
        ),
        _ => format!("{} ({})", server.origin_kind, server.config_path.display()),
    }
}

fn format_plugin_source(plugin: &PluginRecord) -> String {
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

fn format_mcp_call_source(server: &McpCallServerRecord) -> String {
    match server.origin_kind.as_str() {
        "plugin" => format!(
            "plugin:{} ({})",
            server.origin_name,
            server.config_path.display()
        ),
        _ => format!("{} ({})", server.origin_kind, server.config_path.display()),
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

#[cfg(test)]
mod tests {
    use super::{
        McpCallArgs, McpListArgs, build_mcp_call_output, build_mcp_list_output,
        build_remote_http_url, build_remote_ws_url, default_task_for_objective,
        discover_runtime_mcp_servers, normalize_remote_base_url, parse_agent_spec,
        parse_mcp_call_arguments, parse_repeated_key_value_args, parse_task_spec,
        remote_approvals_path, remote_events_path, remote_get_json, remote_post_json,
        resolve_runtime_mcp_server,
    };
    use rc_config::{ProviderOverrides, load_runtime_config};
    use std::{collections::BTreeSet, fs, process::Command as ProcessCommand};
    use tempfile::tempdir;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };
    use uuid::Uuid;

    #[test]
    fn agent_spec_parser_extracts_paths_and_labels() {
        let agent = parse_agent_spec("runtime;implementer;src,crates;phase=local,os=windows")
            .unwrap_or_else(|error| panic!("failed to parse agent spec: {error}"));
        assert_eq!(agent.name, "runtime");
        assert_eq!(agent.role, "implementer");
        assert_eq!(agent.ownership_paths, vec!["src", "crates"]);
        assert_eq!(agent.labels.get("phase").map(String::as_str), Some("local"));
        assert_eq!(agent.labels.get("os").map(String::as_str), Some("windows"));
    }

    #[test]
    fn task_spec_parser_and_default_task_apply_budgets() {
        let task =
            parse_task_spec("Wire events;crates/rc-control-plane;phase=remote;Add websocket")
                .unwrap_or_else(|error| panic!("failed to parse task spec: {error}"));
        assert_eq!(task.title, "Wire events");
        assert_eq!(task.ownership_paths, vec!["crates/rc-control-plane"]);
        assert_eq!(
            task.required_labels.get("phase").map(String::as_str),
            Some("remote")
        );
        assert_eq!(task.description, "Add websocket");
        assert_eq!(task.budget.command_calls, 8);

        let tempdir = tempdir().unwrap_or_else(|error| panic!("tempdir failed: {error}"));
        let config = load_runtime_config(
            Some(tempdir.path().to_path_buf()),
            Some(tempdir.path().join(".remote-code-rust")),
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
        )
        .unwrap_or_else(|error| panic!("config load failed: {error}"));
        let default_task = default_task_for_objective("Ship the next slice", &config);
        assert!(default_task.description.contains("Ship the next slice"));
        assert_eq!(default_task.budget.edit_calls, 16);
    }

    #[test]
    fn normalize_remote_base_url_preserves_base_path() {
        let target = normalize_remote_base_url("http://127.0.0.1:8787/api/v1/")
            .unwrap_or_else(|error| panic!("base URL normalize failed: {error}"));
        assert_eq!(target, "http://127.0.0.1:8787/api/v1");
        assert_eq!(
            build_remote_http_url(&target, "sessions").unwrap_or_else(|error| panic!("{error}")),
            "http://127.0.0.1:8787/api/v1/sessions"
        );
    }

    #[test]
    fn build_remote_ws_url_switches_protocol_and_keeps_base_path() {
        let ws_url = build_remote_ws_url("https://example.com/control/", "/v1/events/stream")
            .unwrap_or_else(|error| panic!("ws URL build failed: {error}"));
        assert_eq!(ws_url, "wss://example.com/control/v1/events/stream");
    }

    #[test]
    fn parse_repeated_key_value_args_collects_metadata() {
        let metadata = parse_repeated_key_value_args(
            "--meta",
            &["phase=remote".to_owned(), "owner=cli".to_owned()],
        )
        .unwrap_or_else(|error| panic!("metadata parse failed: {error}"));
        assert_eq!(metadata.get("phase").map(String::as_str), Some("remote"));
        assert_eq!(metadata.get("owner").map(String::as_str), Some("cli"));
    }

    #[test]
    fn remote_approvals_path_supports_global_runner_and_session_scopes() {
        assert_eq!(
            remote_approvals_path(None, None).unwrap_or_else(|error| panic!("{error}")),
            "/v1/approvals"
        );
        assert_eq!(
            remote_approvals_path(Some(Uuid::nil()), None)
                .unwrap_or_else(|error| panic!("{error}")),
            format!("/v1/sessions/{}/approvals", Uuid::nil())
        );
        assert_eq!(
            remote_approvals_path(None, Some("runner-a")).unwrap_or_else(|error| panic!("{error}")),
            "/v1/runners/runner-a/approvals"
        );
        assert!(remote_approvals_path(Some(Uuid::nil()), Some("runner-a")).is_err());
    }

    #[test]
    fn remote_events_path_builds_queries() {
        assert_eq!(remote_events_path(None, None, 20), "/v1/events?limit=20");
        assert_eq!(
            remote_events_path(Some(Uuid::nil()), Some(41), 500),
            format!("/v1/sessions/{}/events?after=41&limit=200", Uuid::nil())
        );
    }

    #[test]
    fn runtime_mcp_discovery_collects_cwd_profile_and_plugin_servers() {
        let tempdir = tempdir().unwrap_or_else(|error| panic!("tempdir failed: {error}"));
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join(".remote-code-rust");
        fs::create_dir_all(&cwd).unwrap_or_else(|error| panic!("cwd create failed: {error}"));
        fs::create_dir_all(&profile)
            .unwrap_or_else(|error| panic!("profile create failed: {error}"));

        fs::write(
            cwd.join("mcp.toml"),
            "[mcp_servers.local]\ncommand = \"python\"\n",
        )
        .unwrap_or_else(|error| panic!("cwd mcp write failed: {error}"));
        fs::write(
            profile.join("mcp.toml"),
            "[mcp_servers.profile]\nurl = \"https://example.com/mcp\"\n",
        )
        .unwrap_or_else(|error| panic!("profile mcp write failed: {error}"));

        let plugin_root = profile.join("plugins").join("example-plugin");
        fs::create_dir_all(plugin_root.join(".codex-plugin"))
            .unwrap_or_else(|error| panic!("plugin manifest dir create failed: {error}"));
        fs::write(
            plugin_root.join(".codex-plugin").join("plugin.json"),
            r#"{
                "name": "example-plugin",
                "version": "0.1.0",
                "mcp": "mcp.toml"
            }"#,
        )
        .unwrap_or_else(|error| panic!("plugin manifest write failed: {error}"));
        fs::write(
            plugin_root.join("mcp.toml"),
            "[mcp_servers.plugin]\ncommand = \"python\"\n",
        )
        .unwrap_or_else(|error| panic!("plugin mcp write failed: {error}"));

        let config = load_runtime_config(
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
        )
        .unwrap_or_else(|error| panic!("config load failed: {error}"));

        let discovery = discover_runtime_mcp_servers(&config, &[]);
        let names = discovery
            .servers
            .iter()
            .map(|entry| entry.server.name.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            names,
            BTreeSet::from([
                "local".to_owned(),
                "plugin".to_owned(),
                "profile".to_owned()
            ])
        );
        assert!(discovery.warnings.is_empty());
    }

    #[test]
    fn runtime_mcp_discovery_loads_explicit_config_paths() {
        let tempdir = tempdir().unwrap_or_else(|error| panic!("tempdir failed: {error}"));
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join(".remote-code-rust");
        let extra_dir = tempdir.path().join("custom");
        fs::create_dir_all(&cwd).unwrap_or_else(|error| panic!("cwd create failed: {error}"));
        fs::create_dir_all(&profile)
            .unwrap_or_else(|error| panic!("profile create failed: {error}"));
        fs::create_dir_all(&extra_dir)
            .unwrap_or_else(|error| panic!("extra dir create failed: {error}"));
        fs::write(
            extra_dir.join("mcp.toml"),
            "[mcp_servers.explicit]\ncommand = \"python\"\n",
        )
        .unwrap_or_else(|error| panic!("extra mcp write failed: {error}"));

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
        )
        .unwrap_or_else(|error| panic!("config load failed: {error}"));

        let discovery = discover_runtime_mcp_servers(&config, &[extra_dir]);
        assert!(
            discovery
                .servers
                .iter()
                .any(|entry| entry.server.name == "explicit" && entry.origin_kind == "explicit")
        );
        assert!(discovery.warnings.is_empty());
    }

    #[tokio::test]
    async fn mcp_list_output_skips_disabled_servers_without_connecting() {
        let tempdir = tempdir().unwrap_or_else(|error| panic!("tempdir failed: {error}"));
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join(".remote-code-rust");
        fs::create_dir_all(&cwd).unwrap_or_else(|error| panic!("cwd create failed: {error}"));
        fs::create_dir_all(&profile)
            .unwrap_or_else(|error| panic!("profile create failed: {error}"));

        fs::write(
            profile.join("mcp.toml"),
            "[mcp_servers.disabled]\ncommand = \"python\"\nenabled = false\n",
        )
        .unwrap_or_else(|error| panic!("profile mcp write failed: {error}"));

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
        )
        .unwrap_or_else(|error| panic!("config load failed: {error}"));

        let output = build_mcp_list_output(
            &config,
            &McpListArgs {
                connect: true,
                json: false,
                servers: Vec::new(),
                include_disabled: false,
                config_paths: Vec::new(),
            },
        )
        .await
        .unwrap_or_else(|error| panic!("mcp list output build failed: {error}"));

        assert_eq!(output.servers.len(), 1);
        let live = output.servers[0]
            .live
            .as_ref()
            .unwrap_or_else(|| panic!("expected live inspection metadata"));
        assert_eq!(live.status, "skipped");
        assert!(
            live.error
                .as_deref()
                .unwrap_or_default()
                .contains("include-disabled")
        );
    }

    #[test]
    fn parse_mcp_call_arguments_merges_json_and_key_value_overrides() {
        let parsed = parse_mcp_call_arguments(&McpCallArgs {
            server: "mock".to_owned(),
            tool: "search".to_owned(),
            json: false,
            include_disabled: false,
            args: vec![
                "query=rust".to_owned(),
                "count=3".to_owned(),
                "exact=true".to_owned(),
            ],
            args_json: Some(r#"{"scope":"docs","count":1}"#.to_owned()),
            config_paths: Vec::new(),
        })
        .unwrap_or_else(|error| panic!("argument parse failed: {error}"));

        assert_eq!(
            parsed,
            serde_json::json!({
                "scope": "docs",
                "query": "rust",
                "count": 3,
                "exact": true
            })
        );
    }

    #[test]
    fn resolve_runtime_mcp_server_rejects_ambiguous_names() {
        let tempdir = tempdir().unwrap_or_else(|error| panic!("tempdir failed: {error}"));
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join(".remote-code-rust");
        fs::create_dir_all(&cwd).unwrap_or_else(|error| panic!("cwd create failed: {error}"));
        fs::create_dir_all(&profile)
            .unwrap_or_else(|error| panic!("profile create failed: {error}"));

        fs::write(
            cwd.join("mcp.toml"),
            "[mcp_servers.shared]\ncommand = \"python\"\n",
        )
        .unwrap_or_else(|error| panic!("cwd mcp write failed: {error}"));
        fs::write(
            profile.join("mcp.toml"),
            "[mcp_servers.shared]\ncommand = \"python\"\n",
        )
        .unwrap_or_else(|error| panic!("profile mcp write failed: {error}"));

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
        )
        .unwrap_or_else(|error| panic!("config load failed: {error}"));

        let error = resolve_runtime_mcp_server(&config, "shared", &[])
            .expect_err("duplicate names should be rejected");
        assert!(error.to_string().contains("ambiguous"));
    }

    #[tokio::test]
    async fn mcp_call_output_invokes_stdio_tool() {
        let Some((python, mut prefix_args)) = python_command() else {
            eprintln!("Skipping MCP call output test because Python is unavailable.");
            return;
        };

        let tempdir = tempdir().unwrap_or_else(|error| panic!("tempdir failed: {error}"));
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join(".remote-code-rust");
        fs::create_dir_all(&cwd).unwrap_or_else(|error| panic!("cwd create failed: {error}"));
        fs::create_dir_all(&profile)
            .unwrap_or_else(|error| panic!("profile create failed: {error}"));

        let script = cwd.join("mock_tool_call.py");
        fs::write(&script, mock_tool_call_server_script())
            .unwrap_or_else(|error| panic!("mock tool script write failed: {error}"));
        prefix_args.push("mock_tool_call.py".to_owned());
        prefix_args.push("success".to_owned());

        fs::write(
            cwd.join("mcp.toml"),
            format!(
                "[mcp_servers.local]\ncommand = \"{}\"\nargs = [{}]\ncwd = \"{}\"\n",
                python,
                prefix_args
                    .iter()
                    .map(|arg| format!("\"{}\"", arg.replace('\\', "\\\\").replace('"', "\\\"")))
                    .collect::<Vec<_>>()
                    .join(", "),
                cwd.display().to_string().replace('\\', "\\\\")
            ),
        )
        .unwrap_or_else(|error| panic!("cwd mcp write failed: {error}"));

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
        )
        .unwrap_or_else(|error| panic!("config load failed: {error}"));

        let output = build_mcp_call_output(
            &config,
            &McpCallArgs {
                server: "local".to_owned(),
                tool: "echo".to_owned(),
                json: false,
                include_disabled: false,
                args: vec!["text=hello".to_owned()],
                args_json: None,
                config_paths: Vec::new(),
            },
        )
        .await
        .unwrap_or_else(|error| panic!("mcp call output build failed: {error}"));

        assert!(output.warnings.is_empty());
        assert_eq!(output.server.name, "local");
        assert_eq!(output.response.tool_name, "echo");
        assert_eq!(
            output.response.result.content[0]
                .fields
                .get("text")
                .and_then(serde_json::Value::as_str),
            Some("echo: hello")
        );
    }

    #[tokio::test]
    async fn remote_http_helpers_round_trip_control_plane_json() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap_or_else(|error| panic!("listener bind failed: {error}"));
        let address = listener
            .local_addr()
            .unwrap_or_else(|error| panic!("local addr failed: {error}"));
        let server = tokio::spawn(async move {
            for _ in 0..4 {
                let (mut socket, _) = listener
                    .accept()
                    .await
                    .unwrap_or_else(|error| panic!("accept failed: {error}"));
                let mut request = Vec::new();
                let mut buffer = [0_u8; 1024];
                loop {
                    let read = socket
                        .read(&mut buffer)
                        .await
                        .unwrap_or_else(|error| panic!("read failed: {error}"));
                    if read == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..read]);
                    if request.windows(4).any(|window| window == b"\r\n\r\n") {
                        break;
                    }
                }
                let request_text = String::from_utf8(request)
                    .unwrap_or_else(|error| panic!("request utf8 failed: {error}"));
                let body = if request_text.starts_with("GET /v1/runners ") {
                    serde_json::json!({
                        "items": [
                            {
                                "registration": {
                                    "runner_id": "runner-a",
                                    "public_base_url": "http://127.0.0.1:9000"
                                },
                                "state": "idle",
                                "active_sessions": 0,
                                "queued_sessions": 0
                            }
                        ]
                    })
                } else if request_text.starts_with("POST /v1/sessions ") {
                    serde_json::json!({
                        "session_id": Uuid::nil(),
                        "workspace_id": "default",
                        "owner_runner_id": "runner-a",
                        "state": "assigned",
                        "metadata": {"phase": "remote"},
                        "created_at": "2026-04-07T00:00:00Z",
                        "updated_at": "2026-04-07T00:00:01Z"
                    })
                } else if request_text.starts_with("GET /v1/approvals ") {
                    serde_json::json!({
                        "items": [
                            {
                                "approval_id": Uuid::nil(),
                                "session_id": Uuid::nil(),
                                "runner_id": "runner-a",
                                "state": "pending",
                                "title": "Run shell",
                                "description": "Need confirmation",
                                "metadata": {"tool": "bash_command"},
                                "created_at": "2026-04-07T00:00:02Z",
                                "updated_at": "2026-04-07T00:00:02Z",
                                "responded_at": null,
                                "responder": null,
                                "note": null
                            }
                        ]
                    })
                } else if request_text.starts_with("GET /v1/events?after=1&limit=5 ") {
                    serde_json::json!({
                        "items": [
                            {
                                "sequence": 2,
                                "recorded_at": "2026-04-07T00:00:03Z",
                                "runner_id": "runner-a",
                                "session_id": Uuid::nil(),
                                "detail": {
                                    "kind": "approval_requested",
                                    "approval_id": Uuid::nil(),
                                    "title": "Run shell",
                                    "state": "pending"
                                }
                            }
                        ]
                    })
                } else {
                    panic!("unexpected request: {request_text}");
                };
                let payload = serde_json::to_vec(&body)
                    .unwrap_or_else(|error| panic!("serialize failed: {error}"));
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    payload.len()
                );
                socket
                    .write_all(response.as_bytes())
                    .await
                    .unwrap_or_else(|error| panic!("response header write failed: {error}"));
                socket
                    .write_all(&payload)
                    .await
                    .unwrap_or_else(|error| panic!("response body write failed: {error}"));
            }
        });

        let base_url = format!("http://{address}");
        let runners: super::RemoteListResponse<super::RemoteRunnerSnapshot> =
            remote_get_json(&base_url, "/v1/runners")
                .await
                .unwrap_or_else(|error| panic!("remote get failed: {error}"));
        assert_eq!(runners.items.len(), 1);
        assert_eq!(runners.items[0].registration.runner_id, "runner-a");

        let created: super::RemoteSessionRecord = remote_post_json(
            &base_url,
            "/v1/sessions",
            &serde_json::json!({"workspace_id": "default"}),
        )
        .await
        .unwrap_or_else(|error| panic!("remote post failed: {error}"));
        assert_eq!(created.workspace_id, "default");
        assert_eq!(created.owner_runner_id.as_deref(), Some("runner-a"));

        let approvals: super::RemoteListResponse<super::RemoteApprovalRecord> =
            remote_get_json(&base_url, "/v1/approvals")
                .await
                .unwrap_or_else(|error| panic!("remote approvals get failed: {error}"));
        assert_eq!(approvals.items.len(), 1);
        assert_eq!(approvals.items[0].title, "Run shell");
        assert_eq!(approvals.items[0].state.label(), "pending");

        let events: super::RemoteListResponse<super::RemoteTimelineEvent> =
            remote_get_json(&base_url, "/v1/events?after=1&limit=5")
                .await
                .unwrap_or_else(|error| panic!("remote events get failed: {error}"));
        assert_eq!(events.items.len(), 1);
        assert_eq!(events.items[0].sequence, 2);
        match &events.items[0].detail {
            super::RemoteTimelineEventDetail::ApprovalRequested { title, .. } => {
                assert_eq!(title, "Run shell");
            }
            other => panic!("unexpected event detail: {other:?}"),
        }

        server
            .await
            .unwrap_or_else(|error| panic!("server join failed: {error}"));
    }

    fn python_command() -> Option<(String, Vec<String>)> {
        if let Ok(path) = std::env::var("PYTHON")
            && ProcessCommand::new(&path)
                .arg("--version")
                .output()
                .is_ok_and(|output| output.status.success())
        {
            return Some((path, Vec::new()));
        }

        for candidate in ["python", "python3"] {
            if ProcessCommand::new(candidate)
                .arg("--version")
                .output()
                .is_ok_and(|output| output.status.success())
            {
                return Some((candidate.to_owned(), Vec::new()));
            }
        }

        if cfg!(windows)
            && ProcessCommand::new("py")
                .args(["-3", "--version"])
                .output()
                .is_ok_and(|output| output.status.success())
        {
            return Some(("py".to_owned(), vec!["-3".to_owned()]));
        }

        None
    }

    fn mock_tool_call_server_script() -> &'static str {
        r#"
import json
import sys

mode = sys.argv[1] if len(sys.argv) > 1 else "success"

for raw in sys.stdin:
    raw = raw.strip()
    if not raw:
        continue
    message = json.loads(raw)
    method = message.get("method")
    message_id = message.get("id")

    if method == "initialize":
        print(json.dumps({
            "jsonrpc": "2.0",
            "id": message_id,
            "result": {
                "protocolVersion": "2025-03-26",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "mock-mcp", "version": "0.1.0"}
            }
        }), flush=True)
    elif method == "notifications/initialized":
        continue
    elif method == "tools/call":
        text = message["params"]["arguments"]["text"]
        if mode == "success":
            print(json.dumps({
                "jsonrpc": "2.0",
                "id": message_id,
                "result": {
                    "content": [{"type": "text", "text": f"echo: {text}"}],
                    "structuredContent": {"echoed": text},
                    "isError": False
                }
            }), flush=True)
        else:
            print(json.dumps({
                "jsonrpc": "2.0",
                "id": message_id,
                "error": {"code": -32001, "message": "tool call failed"}
            }), flush=True)
        break
"#
    }
}

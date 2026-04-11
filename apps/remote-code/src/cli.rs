use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use rc_control_plane::SessionState as RemoteSessionState;
use rc_core::{InputFormat, OutputFormat, PermissionMode, ProviderProtocol};
use rc_runner::ApprovalDecision;
use uuid::Uuid;

use crate::hooks::HooksCommand;

#[derive(Parser, Debug)]
#[command(
    name = "remote-code",
    version,
    about = "Remote Code Rust CLI/runtime shell"
)]
#[allow(clippy::struct_excessive_bools)]
pub struct Cli {
    #[arg(short = 'p', long = "print")]
    pub print_mode: bool,

    #[arg(long, value_enum, default_value_t = InputFormat::Text)]
    pub input_format: InputFormat,

    #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
    pub output_format: OutputFormat,

    #[arg(long, value_enum, env = "REMOTE_CODE_PERMISSION_MODE", default_value_t = PermissionMode::Default)]
    pub permission_mode: PermissionMode,

    #[arg(long)]
    pub cwd: Option<PathBuf>,

    #[arg(long, env = "REMOTE_CODE_PROFILE_DIR")]
    pub profile_dir: Option<PathBuf>,

    #[arg(long)]
    pub session_id: Option<Uuid>,

    #[arg(long)]
    pub provider: Option<String>,

    #[arg(long)]
    pub base_url: Option<String>,

    #[arg(long)]
    pub api_key: Option<String>,

    #[arg(long)]
    pub model: Option<String>,

    #[arg(long, value_enum)]
    pub protocol: Option<ProviderProtocol>,

    #[arg(long, default_value_t = 12)]
    pub max_turns: usize,

    #[arg(short, long)]
    pub verbose: bool,

    #[arg(long)]
    pub replay_user_messages: bool,

    #[arg(long)]
    pub include_partial_messages: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,

    pub prompt: Vec<String>,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Doctor,
    Hooks {
        #[command(subcommand)]
        command: HooksCommand,
    },
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
    /// Connect to a remote host via SSH and run remote-code.
    Ssh(SshArgs),
    /// Check for updates or self-update.
    Update {
        #[command(subcommand)]
        command: UpdateCommand,
    },
}

/// Subcommands for the update command.
#[derive(Subcommand, Debug)]
pub enum UpdateCommand {
    /// Check if a newer version is available.
    Check,
    /// Download and install the latest version.
    Run,
}

#[derive(Subcommand, Debug)]
pub enum SessionsCommand {
    List,
    Show(ShowArgs),
}

#[derive(Subcommand, Debug)]
pub enum RemoteCommand {
    Meta(RemoteMetaArgs),
    Runners {
        #[command(subcommand)]
        command: RemoteRunnersCommand,
    },
    Artifacts {
        #[command(subcommand)]
        command: RemoteArtifactsCommand,
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
pub enum RemoteRunnersCommand {
    List(RemoteRunnersListArgs),
    Show(RemoteRunnerShowArgs),
}

#[derive(Subcommand, Debug)]
pub enum RemoteArtifactsCommand {
    List(RemoteArtifactsListArgs),
    Show(RemoteArtifactShowArgs),
    Download(RemoteArtifactDownloadArgs),
    Upload(RemoteArtifactUploadArgs),
}

#[derive(Subcommand, Debug)]
pub enum RemoteApprovalsCommand {
    List(RemoteApprovalsListArgs),
    Create(RemoteApprovalCreateArgs),
    Show(RemoteApprovalShowArgs),
    Respond(RemoteApprovalRespondArgs),
}

#[derive(Subcommand, Debug)]
pub enum RemoteSessionsCommand {
    List(RemoteSessionsListArgs),
    Show(RemoteSessionShowArgs),
    Create(RemoteSessionCreateArgs),
    State(RemoteSessionStateArgs),
}

#[derive(Args, Debug)]
pub struct ResumeArgs {
    pub session_id: Uuid,
    pub prompt: Vec<String>,
}

#[derive(Args, Debug)]
pub struct ExportArgs {
    pub session_id: Uuid,
    #[arg(long)]
    pub output: Option<PathBuf>,
    #[arg(long, value_enum, default_value_t = ExportFormat::Json)]
    pub format: ExportFormat,
}

#[derive(Args, Debug)]
pub struct ShowArgs {
    pub session_id: Uuid,
    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug, Clone)]
pub struct RemoteTargetArgs {
    #[arg(long, env = "REMOTE_CODE_CONTROL_PLANE_URL")]
    pub control_plane_url: Option<String>,
}

#[derive(Args, Debug)]
pub struct RemoteMetaArgs {
    #[command(flatten)]
    pub target: RemoteTargetArgs,

    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct RemoteRunnersListArgs {
    #[command(flatten)]
    pub target: RemoteTargetArgs,

    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct RemoteRunnerShowArgs {
    #[command(flatten)]
    pub target: RemoteTargetArgs,

    pub runner_id: String,

    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct RemoteSessionsListArgs {
    #[command(flatten)]
    pub target: RemoteTargetArgs,

    #[arg(long)]
    pub runner_id: Option<String>,

    #[arg(long)]
    pub workspace_id: Option<String>,

    #[arg(long, value_enum)]
    pub state: Option<RemoteSessionStateValue>,

    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct RemoteSessionShowArgs {
    #[command(flatten)]
    pub target: RemoteTargetArgs,

    pub session_id: Uuid,

    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct RemoteSessionCreateArgs {
    #[command(flatten)]
    pub target: RemoteTargetArgs,

    #[arg(long)]
    pub workspace_id: String,

    #[arg(long)]
    pub preferred_runner_id: Option<String>,

    #[arg(long = "meta")]
    pub metadata: Vec<String>,

    #[arg(long)]
    pub json: bool,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
pub enum RemoteSessionStateValue {
    Pending,
    Assigned,
    Running,
    WaitingApproval,
    Completed,
    Failed,
    Cancelled,
}

impl From<RemoteSessionStateValue> for RemoteSessionState {
    fn from(value: RemoteSessionStateValue) -> Self {
        match value {
            RemoteSessionStateValue::Pending => RemoteSessionState::Pending,
            RemoteSessionStateValue::Assigned => RemoteSessionState::Assigned,
            RemoteSessionStateValue::Running => RemoteSessionState::Running,
            RemoteSessionStateValue::WaitingApproval => RemoteSessionState::WaitingApproval,
            RemoteSessionStateValue::Completed => RemoteSessionState::Completed,
            RemoteSessionStateValue::Failed => RemoteSessionState::Failed,
            RemoteSessionStateValue::Cancelled => RemoteSessionState::Cancelled,
        }
    }
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
pub enum RemoteEventKindValue {
    RunnerRegistered,
    RunnerHeartbeat,
    SessionCreated,
    SessionStateChanged,
    ApprovalRequested,
    ApprovalResolved,
    ArtifactCreated,
}

impl RemoteEventKindValue {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RunnerRegistered => "runner_registered",
            Self::RunnerHeartbeat => "runner_heartbeat",
            Self::SessionCreated => "session_created",
            Self::SessionStateChanged => "session_state_changed",
            Self::ApprovalRequested => "approval_requested",
            Self::ApprovalResolved => "approval_resolved",
            Self::ArtifactCreated => "artifact_created",
        }
    }
}

#[derive(Args, Debug)]
pub struct RemoteSessionStateArgs {
    #[command(flatten)]
    pub target: RemoteTargetArgs,

    pub session_id: Uuid,

    #[arg(long, value_enum)]
    pub state: RemoteSessionStateValue,

    #[arg(long = "meta")]
    pub metadata: Vec<String>,

    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct RemoteArtifactsListArgs {
    #[command(flatten)]
    pub target: RemoteTargetArgs,

    #[arg(long)]
    pub session_id: Option<Uuid>,

    #[arg(long)]
    pub runner_id: Option<String>,

    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct RemoteArtifactShowArgs {
    #[command(flatten)]
    pub target: RemoteTargetArgs,

    pub artifact_id: Uuid,

    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct RemoteArtifactDownloadArgs {
    #[command(flatten)]
    pub target: RemoteTargetArgs,

    pub artifact_id: Uuid,

    #[arg(long)]
    pub output: Option<PathBuf>,

    #[arg(long)]
    pub overwrite: bool,

    #[arg(long)]
    pub stdout: bool,
}

#[derive(Args, Debug)]
pub struct RemoteArtifactUploadArgs {
    #[command(flatten)]
    pub target: RemoteTargetArgs,

    #[arg(long)]
    pub session_id: Uuid,

    #[arg(long)]
    pub file: PathBuf,

    #[arg(long)]
    pub name: Option<String>,

    #[arg(long)]
    pub file_name: Option<String>,

    #[arg(long)]
    pub media_type: Option<String>,

    #[arg(long = "meta")]
    pub metadata: Vec<String>,

    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct RemoteApprovalsListArgs {
    #[command(flatten)]
    pub target: RemoteTargetArgs,

    #[arg(long)]
    pub session_id: Option<Uuid>,

    #[arg(long)]
    pub runner_id: Option<String>,

    #[arg(long)]
    pub after: Option<u64>,

    #[arg(long)]
    pub follow: bool,

    #[arg(long, default_value_t = 2)]
    pub reconnect_delay_secs: u64,

    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct RemoteApprovalShowArgs {
    #[command(flatten)]
    pub target: RemoteTargetArgs,

    pub approval_id: Uuid,

    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct RemoteApprovalCreateArgs {
    #[command(flatten)]
    pub target: RemoteTargetArgs,

    #[arg(long)]
    pub session_id: Uuid,

    #[arg(long)]
    pub title: String,

    #[arg(long)]
    pub description: String,

    #[arg(long = "meta")]
    pub metadata: Vec<String>,

    #[arg(long)]
    pub json: bool,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RemoteApprovalDecision {
    Approved,
    Denied,
    Cancelled,
}

impl From<RemoteApprovalDecision> for ApprovalDecision {
    fn from(value: RemoteApprovalDecision) -> Self {
        match value {
            RemoteApprovalDecision::Approved => ApprovalDecision::Approved,
            RemoteApprovalDecision::Denied => ApprovalDecision::Denied,
            RemoteApprovalDecision::Cancelled => ApprovalDecision::Cancelled,
        }
    }
}

#[derive(Args, Debug)]
pub struct RemoteApprovalRespondArgs {
    #[command(flatten)]
    pub target: RemoteTargetArgs,

    pub approval_id: Uuid,

    #[arg(long, value_enum)]
    pub decision: RemoteApprovalDecision,

    #[arg(long)]
    pub responder: Option<String>,

    #[arg(long)]
    pub note: Option<String>,

    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct RemoteEventsArgs {
    #[command(flatten)]
    pub target: RemoteTargetArgs,

    #[arg(long)]
    pub session_id: Option<Uuid>,

    #[arg(long)]
    pub runner_id: Option<String>,

    #[arg(long, value_enum)]
    pub kind: Option<RemoteEventKindValue>,

    #[arg(long)]
    pub after: Option<u64>,

    #[arg(long, default_value_t = 20)]
    pub limit: usize,

    #[arg(long)]
    pub follow: bool,

    #[arg(long, default_value_t = 2)]
    pub reconnect_delay_secs: u64,

    #[arg(long)]
    pub json: bool,
}

#[derive(Subcommand, Debug)]
pub enum MigrateCommand {
    Import {
        #[arg(long)]
        source: Option<PathBuf>,
    },
}

#[derive(Subcommand, Debug)]
pub enum AgentsCommand {
    Plan(AgentsPlanArgs),
}

#[derive(Subcommand, Debug)]
pub enum McpCommand {
    List(McpListArgs),
    Call(McpCallArgs),
}

#[derive(Subcommand, Debug)]
pub enum PluginsCommand {
    List(PluginsListArgs),
    Inspect(PluginsInspectArgs),
    Invoke(PluginsInvokeArgs),
}

#[derive(Args, Debug)]
pub struct AgentsPlanArgs {
    #[arg(long, default_value = "codex-lead")]
    pub lead: String,

    #[arg(long)]
    pub objective: String,

    #[arg(long = "agent")]
    pub agents: Vec<String>,

    #[arg(long = "task")]
    pub tasks: Vec<String>,

    #[arg(long)]
    pub json: bool,
}

#[derive(Args, Debug)]
pub struct McpListArgs {
    #[arg(long)]
    pub connect: bool,

    #[arg(long)]
    pub json: bool,

    #[arg(long = "server")]
    pub servers: Vec<String>,

    #[arg(long)]
    pub include_disabled: bool,

    #[arg(long = "config")]
    pub config_paths: Vec<PathBuf>,
}

#[derive(Args, Debug)]
pub struct McpCallArgs {
    #[arg(long)]
    pub server: String,

    #[arg(long)]
    pub tool: String,

    #[arg(long)]
    pub json: bool,

    #[arg(long = "include-disabled")]
    pub include_disabled: bool,

    #[arg(long = "arg")]
    pub args: Vec<String>,

    #[arg(long = "args-json")]
    pub args_json: Option<String>,

    #[arg(long = "config")]
    pub config_paths: Vec<PathBuf>,
}

#[derive(Args, Debug)]
pub struct PluginsListArgs {
    #[arg(long)]
    pub connect: bool,

    #[arg(long)]
    pub json: bool,

    #[arg(long = "plugin")]
    pub plugins: Vec<String>,

    #[arg(long = "plugins-dir")]
    pub plugin_roots: Vec<PathBuf>,
}

#[derive(Args, Debug)]
pub struct PluginsInspectArgs {
    #[arg(long)]
    pub plugin: String,

    #[arg(long)]
    pub json: bool,

    #[arg(long = "plugins-dir")]
    pub plugin_roots: Vec<PathBuf>,
}

#[derive(Args, Debug)]
pub struct PluginsInvokeArgs {
    #[arg(long)]
    pub plugin: String,

    #[arg(long)]
    pub action: String,

    #[arg(long)]
    pub json: bool,

    #[arg(long = "arg")]
    pub args: Vec<String>,

    #[arg(long = "input-json")]
    pub input_json: Option<String>,

    #[arg(long = "plugins-dir")]
    pub plugin_roots: Vec<PathBuf>,
}

#[derive(Args, Debug)]
pub struct SshArgs {
    /// Remote host to connect to (e.g. user@host or just host).
    #[arg(long)]
    pub host: String,

    /// SSH user name (overrides user in host if both given).
    #[arg(long)]
    pub user: Option<String>,

    /// SSH port (default 22).
    #[arg(long, default_value_t = 22)]
    pub port: u16,

    /// Remote command to execute on the host.
    #[arg(long)]
    pub command: Option<String>,

    /// Identity file (SSH private key path).
    #[arg(short = 'i', long)]
    pub identity: Option<PathBuf>,
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
pub enum ExportFormat {
    Ndjson,
    Json,
}

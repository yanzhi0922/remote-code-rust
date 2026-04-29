//! VS Code Extension Host message type definitions.
//!
//! Derived from `packages/types/src/vscode-extension-host.ts`.
//! These types define the communication protocol between the extension host
//! (backend) and the webview / CLI frontend.

use serde::{Deserialize, Serialize};

use crate::message::{ClineMessage, QueuedMessage};

// ---------------------------------------------------------------------------
// ExtensionMessage  (Extension → Webview | CLI)
// ---------------------------------------------------------------------------

/// Discriminator for [`ExtensionMessage`].
///
/// Source: `packages/types/src/vscode-extension-host.ts` L31-106
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExtensionMessageType {
    Action,
    State,
    TaskHistoryUpdated,
    TaskHistoryItemUpdated,
    SelectedImages,
    Theme,
    WorkspaceUpdated,
    Invoke,
    MessageUpdated,
    McpServers,
    EnhancedPrompt,
    CommitSearchResults,
    ListApiConfig,
    RouterModels,
    OpenAiModels,
    OllamaModels,
    LmStudioModels,
    VsCodeLmModels,
    VsCodeLmApiAvailable,
    UpdatePrompt,
    SystemPrompt,
    AutoApprovalEnabled,
    UpdateCustomMode,
    DeleteCustomMode,
    ExportModeResult,
    ImportModeResult,
    CheckRulesDirectoryResult,
    DeleteCustomModeCheck,
    CurrentCheckpointUpdated,
    CheckpointInitWarning,
    TtsStart,
    TtsStop,
    FileSearchResults,
    ToggleApiConfigPin,
    AcceptInput,
    SetHistoryPreviewCollapsed,
    CommandExecutionStatus,
    McpExecutionStatus,
    VsCodeSetting,
    AuthenticatedUser,
    CondenseTaskContextStarted,
    CondenseTaskContextResponse,
    SingleRouterModelFetchResponse,
    RooCreditBalance,
    IndexingStatusUpdate,
    IndexCleared,
    CodebaseIndexConfig,
    MarketplaceInstallResult,
    MarketplaceRemoveResult,
    MarketplaceData,
    ShareTaskSuccess,
    CodeIndexSettingsSaved,
    CodeIndexSecretStatus,
    ShowDeleteMessageDialog,
    ShowEditMessageDialog,
    Commands,
    InsertTextIntoTextarea,
    DismissedUpsells,
    OrganizationSwitchResult,
    InteractionRequired,
    CustomToolsResult,
    Modes,
    TaskWithAggregatedCosts,
    OpenAiCodexRateLimits,
    // Worktree response types
    WorktreeList,
    WorktreeResult,
    WorktreeCopyProgress,
    BranchList,
    WorktreeDefaults,
    WorktreeIncludeStatus,
    BranchWorktreeIncludeResult,
    FolderSelected,
    Skills,
    FileContent,
}

/// Checkpoint warning payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckpointWarning {
    #[serde(rename = "type")]
    pub warning_type: CheckpointWarningType,
    pub timeout: u64,
}

/// Type of checkpoint warning.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CheckpointWarningType {
    WaitTimeout,
    InitTimeout,
}

/// Action types sent from extension to webview.
///
/// Source: L115-125
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExtensionAction {
    ChatButtonClicked,
    SettingsButtonClicked,
    HistoryButtonClicked,
    MarketplaceButtonClicked,
    CloudButtonClicked,
    DidBecomeVisible,
    FocusInput,
    SwitchTab,
    ToggleAutoApprove,
}

/// Invoke types.
///
/// Source: L125
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InvokeType {
    NewChat,
    SendMessage,
    PrimaryButtonClick,
    SecondaryButtonClick,
    SetChatBoxMessage,
}

/// Aggregated cost breakdown.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AggregatedCosts {
    pub total_cost: f64,
    pub own_cost: f64,
    pub children_cost: f64,
}

/// Opened tab info.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpenedTab {
    pub label: String,
    pub is_active: bool,
    pub path: Option<String>,
}

/// File content response.
///
/// Source: L108-109
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileContentPayload {
    pub path: String,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// The `ExtensionMessage` interface.
///
/// Source: `packages/types/src/vscode-extension-host.ts` L30-241
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionMessage {
    #[serde(rename = "type")]
    pub msg_type: Option<ExtensionMessageType>,
    pub text: Option<String>,
    pub file_content: Option<FileContentPayload>,
    pub payload: Option<serde_json::Value>,
    pub checkpoint_warning: Option<CheckpointWarning>,
    pub action: Option<ExtensionAction>,
    pub invoke: Option<InvokeType>,
    pub state: Option<serde_json::Value>,
    pub images: Option<Vec<String>>,
    pub file_paths: Option<Vec<String>>,
    pub opened_tabs: Option<Vec<OpenedTab>>,
    pub cline_message: Option<ClineMessage>,
    pub router_models: Option<serde_json::Value>,
    pub open_ai_models: Option<Vec<String>>,
    pub ollama_models: Option<serde_json::Value>,
    pub lm_studio_models: Option<serde_json::Value>,
    pub vs_code_lm_models: Option<serde_json::Value>,
    pub mcp_servers: Option<serde_json::Value>,
    pub commits: Option<serde_json::Value>,
    pub list_api_config: Option<serde_json::Value>,
    pub mode: Option<String>,
    pub custom_mode: Option<serde_json::Value>,
    pub slug: Option<String>,
    pub success: Option<bool>,
    pub values: Option<serde_json::Value>,
    pub request_id: Option<String>,
    pub prompt_text: Option<String>,
    pub results: Option<serde_json::Value>,
    pub error: Option<String>,
    pub setting: Option<String>,
    pub value: Option<serde_json::Value>,
    pub has_content: Option<bool>,
    pub items: Option<serde_json::Value>,
    pub user_info: Option<serde_json::Value>,
    pub organization_allow_list: Option<serde_json::Value>,
    pub tab: Option<String>,
    pub marketplace_items: Option<serde_json::Value>,
    pub organization_mcps: Option<serde_json::Value>,
    pub marketplace_installed_metadata: Option<serde_json::Value>,
    pub errors: Option<Vec<String>>,
    pub visibility: Option<serde_json::Value>,
    pub rules_folder_path: Option<String>,
    pub settings: Option<serde_json::Value>,
    pub message_ts: Option<f64>,
    pub has_checkpoint: Option<bool>,
    pub context: Option<String>,
    pub commands: Option<Vec<Command>>,
    pub queued_messages: Option<Vec<QueuedMessage>>,
    pub list: Option<Vec<String>>,
    pub organization_id: Option<Option<String>>,
    pub tools: Option<serde_json::Value>,
    pub skills: Option<serde_json::Value>,
    pub modes: Option<serde_json::Value>,
    pub aggregated_costs: Option<AggregatedCosts>,
    pub history_item: Option<serde_json::Value>,
    pub task_history: Option<serde_json::Value>,
    pub task_history_item: Option<serde_json::Value>,
    // Worktree response properties (L195-241)
    pub worktrees: Option<serde_json::Value>,
    pub is_git_repo: Option<bool>,
    pub is_multi_root: Option<bool>,
    pub is_subfolder: Option<bool>,
    pub git_root_path: Option<String>,
    pub worktree_result: Option<serde_json::Value>,
    pub local_branches: Option<Vec<String>>,
    pub remote_branches: Option<Vec<String>>,
    pub current_branch: Option<String>,
    pub suggested_branch: Option<String>,
    pub suggested_path: Option<String>,
    pub worktree_include_exists: Option<bool>,
    pub worktree_include_status: Option<serde_json::Value>,
    pub has_gitignore: Option<bool>,
    pub gitignore_content: Option<String>,
    // branchWorktreeIncludeResult
    pub branch: Option<String>,
    pub has_worktree_include: Option<bool>,
    // worktreeCopyProgress
    pub copy_progress_bytes_copied: Option<u64>,
    pub copy_progress_total_bytes: Option<u64>,
    pub copy_progress_item_name: Option<String>,
    // folderSelected
    pub path: Option<String>,
}

// ---------------------------------------------------------------------------
// Command
// ---------------------------------------------------------------------------

/// A command exposed by the extension.
///
/// Source: L387-393
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Command {
    pub name: String,
    pub source: CommandSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub argument_hint: Option<String>,
}

/// Source of a command.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CommandSource {
    Global,
    Project,
    #[serde(rename = "built-in")]
    BuiltIn,
}

// ---------------------------------------------------------------------------
// ClineAskResponse
// ---------------------------------------------------------------------------

/// Response types for Cline asks.
///
/// Source: L400
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClineAskResponse {
    YesButtonClicked,
    NoButtonClicked,
    MessageResponse,
    ObjectResponse,
}

// ---------------------------------------------------------------------------
// AudioType
// ---------------------------------------------------------------------------

/// Audio types for sound playback.
///
/// Source: L402
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioType {
    Notification,
    Celebration,
    ProgressLoop,
}

// ---------------------------------------------------------------------------
// Tab
// ---------------------------------------------------------------------------

/// Tab identifiers for webview navigation.
///
/// Source: L587
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tab {
    Settings,
    History,
    Mcp,
    Modes,
    Chat,
    Marketplace,
    Cloud,
}

// ---------------------------------------------------------------------------
// TerminalOperation
// ---------------------------------------------------------------------------

/// Terminal operation types.
///
/// Source: L631
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TerminalOperation {
    Continue,
    Abort,
}

// ---------------------------------------------------------------------------
// WebviewMessage  (Webview | CLI → Extension)
// ---------------------------------------------------------------------------

/// Discriminator for [`WebviewMessage`].
///
/// Source: `packages/types/src/vscode-extension-host.ts` L411-583
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WebviewMessageType {
    // Task & History
    UpdateTodoList,
    DeleteMultipleTasksWithIds,
    CurrentApiConfigName,
    SaveApiConfiguration,
    UpsertApiConfiguration,
    DeleteApiConfiguration,
    LoadApiConfiguration,
    LoadApiConfigurationById,
    RenameApiConfiguration,
    GetListApiConfiguration,
    CustomInstructions,
    WebviewDidLaunch,
    NewTask,
    AskResponse,
    TerminalOperation,
    ClearTask,
    DidShowAnnouncement,
    SelectImages,
    ExportCurrentTask,
    ShareCurrentTask,
    ShowTaskWithId,
    DeleteTaskWithId,
    ExportTaskWithId,
    ImportSettings,
    ExportSettings,
    ResetState,

    // Models & Providers
    FlushRouterModels,
    RequestRouterModels,
    RequestOpenAiModels,
    RequestOllamaModels,
    RequestLmStudioModels,
    RequestRooModels,
    RequestRooCreditBalance,
    RequestVsCodeLmModels,

    // File Operations
    OpenImage,
    SaveImage,
    OpenFile,
    ReadFileContent,
    OpenMention,

    // Task Control
    CancelTask,
    CancelAutoApproval,

    // VS Code Settings
    #[serde(rename = "updateVSCodeSetting")]
    UpdateVsCodeSetting,
    #[serde(rename = "getVSCodeSetting")]
    GetVsCodeSetting,
    VsCodeSetting,

    // Condensing
    UpdateCondensingPrompt,

    // Audio
    PlaySound,
    PlayTts,
    StopTts,
    TtsEnabled,
    TtsSpeed,

    // Keyboard
    OpenKeyboardShortcuts,

    // MCP
    OpenMcpSettings,
    OpenProjectMcpSettings,
    RestartMcpServer,
    RefreshAllMcpServers,
    ToggleToolAlwaysAllow,
    ToggleToolEnabledForPrompt,
    ToggleMcpServer,
    UpdateMcpTimeout,

    // Prompt Enhancement
    EnhancePrompt,
    EnhancedPrompt,
    DraggedImages,

    // Message Editing
    DeleteMessage,
    DeleteMessageConfirm,
    SubmitEditedMessage,
    EditMessageConfirm,

    // Task Sync
    TaskSyncEnabled,

    // Git
    SearchCommits,
    SetApiConfigPassword,

    // Mode & Prompt
    Mode,
    UpdatePrompt,
    GetSystemPrompt,
    CopySystemPrompt,
    SystemPrompt,
    EnhancementApiConfigId,
    AutoApprovalEnabled,
    UpdateCustomMode,
    DeleteCustomMode,
    #[serde(rename = "setopenAiCustomModelInfo")]
    SetOpenAiCustomModelInfo,
    OpenCustomModesSettings,

    // Checkpoints
    CheckpointDiff,
    CheckpointRestore,

    // MCP Management
    DeleteMcpServer,

    // Codebase Index
    CodebaseIndexEnabled,
    TelemetrySetting,
    SearchFiles,
    ToggleApiConfigPin,
    HasOpenedModeSelector,
    LockApiConfigAcrossModes,

    // Cloud Auth
    ClearCloudAuthSkipModel,
    CloudButtonClicked,
    RooCloudSignIn,
    CloudLandingPageSignIn,
    RooCloudSignOut,
    RooCloudManualUrl,
    OpenAiCodexSignIn,
    OpenAiCodexSignOut,
    SwitchOrganization,

    // Condense & Indexing
    CondenseTaskContextRequest,
    RequestIndexingStatus,
    StartIndexing,
    StopIndexing,
    ClearIndexData,
    IndexingStatusUpdate,
    IndexCleared,
    ToggleWorkspaceIndexing,
    SetAutoEnableDefault,

    // Panel & External
    FocusPanelRequest,
    OpenExternal,

    // Marketplace
    FilterMarketplaceItems,
    MarketplaceButtonClicked,
    InstallMarketplaceItem,
    InstallMarketplaceItemWithParameters,
    CancelMarketplaceInstall,
    RemoveInstalledMarketplaceItem,
    MarketplaceInstallResult,
    FetchMarketplaceData,

    // Navigation
    SwitchTab,

    // Sharing
    ShareTaskSuccess,

    // Mode Import/Export
    ExportMode,
    ExportModeResult,
    ImportMode,
    ImportModeResult,

    // Rules
    CheckRulesDirectory,
    CheckRulesDirectoryResult,

    // Code Index Settings
    SaveCodeIndexSettingsAtomic,
    RequestCodeIndexSecretStatus,

    // Commands
    RequestCommands,
    OpenCommandFile,
    DeleteCommand,
    CreateCommand,
    InsertTextIntoTextarea,

    // MDM
    ShowMdmAuthRequiredNotification,

    // Image Generation
    ImageGenerationSettings,

    // Message Queue
    QueueMessage,
    RemoveQueuedMessage,
    EditQueuedMessage,

    // Upsells
    DismissUpsell,
    GetDismissedUpsells,

    // Markdown
    OpenMarkdownPreview,

    // Settings
    UpdateSettings,
    AllowedCommands,
    GetTaskWithAggregatedCosts,
    DeniedCommands,

    // Debug
    OpenDebugApiHistory,
    OpenDebugUiHistory,
    DownloadErrorDiagnostics,
    RequestOpenAiCodexRateLimits,

    // Custom Tools
    RefreshCustomTools,

    // Modes
    RequestModes,
    SwitchMode,
    DebugSetting,

    // Worktree messages
    ListWorktrees,
    CreateWorktree,
    DeleteWorktree,
    SwitchWorktree,
    GetAvailableBranches,
    GetWorktreeDefaults,
    GetWorktreeIncludeStatus,
    CheckBranchWorktreeInclude,
    CreateWorktreeInclude,
    CheckoutBranch,
    BrowseForWorktreePath,

    // Skills messages
    RequestSkills,
    CreateSkill,
    DeleteSkill,
    MoveSkill,
    UpdateSkillModes,
    OpenSkillFile,
}

// ---------------------------------------------------------------------------
// WebViewMessagePayload
// ---------------------------------------------------------------------------

/// Payload types for WebviewMessage.
///
/// Source: L735-742
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum WebViewMessagePayload {
    CheckpointDiff(CheckpointDiffPayload),
    CheckpointRestore(CheckpointRestorePayload),
    IndexingStatus(IndexingStatusPayload),
    IndexCleared(IndexClearedPayload),
    InstallMarketplaceItemWithParameters(InstallMarketplaceItemWithParametersPayload),
    UpdateTodoList(UpdateTodoListPayload),
    EditQueuedMessage(EditQueuedMessagePayload),
}

/// Payload for checkpoint diff requests.
///
/// Source: L699-704
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointDiffPayload {
    pub ts: Option<f64>,
    pub previous_commit_hash: Option<String>,
    pub commit_hash: String,
    pub mode: CheckpointDiffMode,
}

/// Mode for checkpoint diff.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckpointDiffMode {
    Full,
    Checkpoint,
    #[serde(rename = "from-init")]
    FromInit,
    #[serde(rename = "to-current")]
    ToCurrent,
}

/// Payload for checkpoint restore requests.
///
/// Source: L708-713
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointRestorePayload {
    pub ts: f64,
    pub commit_hash: String,
    pub mode: CheckpointRestoreMode,
}

/// Mode for checkpoint restore.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckpointRestoreMode {
    Preview,
    Restore,
}

/// Payload for indexing status updates.
///
/// Source: L716-719
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IndexingStatusPayload {
    pub state: IndexingState,
    pub message: String,
}

/// Indexing state values.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum IndexingState {
    Standby,
    Indexing,
    Indexed,
    Error,
    Stopping,
}

/// Payload for index cleared events.
///
/// Source: L721-724
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IndexClearedPayload {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Payload for install marketplace item with parameters.
///
/// Source: L726-729
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstallMarketplaceItemWithParametersPayload {
    pub item: serde_json::Value,
    pub parameters: serde_json::Value,
}

/// Payload for update todo list.
///
/// Source: L404-407
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UpdateTodoListPayload {
    pub todos: Vec<serde_json::Value>,
}

/// Payload for edit queued message.
///
/// Source: L409
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EditQueuedMessagePayload {
    pub id: String,
    pub text: String,
    pub images: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// WebviewMessage struct
// ---------------------------------------------------------------------------

/// The `WebviewMessage` interface.
///
/// Source: `packages/types/src/vscode-extension-host.ts` L411-693
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebviewMessage {
    #[serde(rename = "type")]
    pub msg_type: Option<WebviewMessageType>,
    pub text: Option<String>,
    pub task_id: Option<String>,
    pub edited_message_content: Option<String>,
    pub tab: Option<Tab>,
    pub disabled: Option<bool>,
    pub context: Option<String>,
    pub data_uri: Option<String>,
    pub ask_response: Option<ClineAskResponse>,
    pub api_configuration: Option<serde_json::Value>,
    pub images: Option<Vec<String>>,
    pub bool: Option<bool>,
    pub value: Option<f64>,
    pub step_index: Option<usize>,
    pub is_launch_action: Option<bool>,
    pub force_show: Option<bool>,
    pub commands: Option<Vec<String>>,
    pub audio_type: Option<AudioType>,
    pub server_name: Option<String>,
    pub tool_name: Option<String>,
    pub always_allow: Option<bool>,
    pub is_enabled: Option<bool>,
    pub mode: Option<String>,
    pub prompt_mode: Option<String>,
    pub custom_prompt: Option<serde_json::Value>,
    pub data_urls: Option<Vec<String>>,
    pub values: Option<serde_json::Value>,
    pub query: Option<String>,
    pub setting: Option<String>,
    pub slug: Option<String>,
    pub mode_config: Option<serde_json::Value>,
    pub timeout: Option<u64>,
    pub payload: Option<WebViewMessagePayload>,
    pub source: Option<CommandSource>,
    pub skill_name: Option<String>,
    /// Deprecated: use skill_mode_slugs instead.
    pub skill_mode: Option<String>,
    /// Deprecated: use new_skill_mode_slugs instead.
    pub new_skill_mode: Option<String>,
    pub skill_description: Option<String>,
    pub skill_mode_slugs: Option<Vec<String>>,
    pub new_skill_mode_slugs: Option<Vec<String>>,
    pub request_id: Option<String>,
    pub ids: Option<Vec<String>>,
    pub terminal_operation: Option<TerminalOperation>,
    pub message_ts: Option<f64>,
    pub restore_checkpoint: Option<bool>,
    pub history_preview_collapsed: Option<bool>,
    pub filters: Option<serde_json::Value>,
    pub settings: Option<serde_json::Value>,
    pub url: Option<String>,
    pub mp_item: Option<serde_json::Value>,
    pub mp_install_options: Option<serde_json::Value>,
    pub config: Option<serde_json::Value>,
    pub visibility: Option<serde_json::Value>,
    pub has_content: Option<bool>,
    pub check_only: Option<bool>,
    pub upsell_id: Option<String>,
    pub list: Option<Vec<String>>,
    pub organization_id: Option<Option<String>>,
    pub use_provider_signup: Option<bool>,
    pub code_index_settings: Option<serde_json::Value>,
    pub updated_settings: Option<serde_json::Value>,
    pub task_configuration: Option<serde_json::Value>,
    // Worktree properties (L686-693)
    pub worktree_path: Option<String>,
    pub worktree_branch: Option<String>,
    pub worktree_base_branch: Option<String>,
    pub worktree_create_new_branch: Option<bool>,
    pub worktree_force: Option<bool>,
    pub worktree_new_window: Option<bool>,
    pub worktree_include_content: Option<String>,
}

// ---------------------------------------------------------------------------
// ClineSayTool
// ---------------------------------------------------------------------------

/// Tool type discriminator for ClineSayTool.
///
/// Source: L768-785
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ClineSayToolType {
    EditedExistingFile,
    AppliedDiff,
    NewFileCreated,
    CodebaseSearch,
    ReadFile,
    ReadCommandOutput,
    ListFilesTopLevel,
    ListFilesRecursive,
    SearchFiles,
    SwitchMode,
    NewTask,
    FinishTask,
    GenerateImage,
    ImageGenerated,
    RunSlashCommand,
    UpdateTodoList,
    Skill,
}

/// Diff statistics.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DiffStats {
    pub added: u32,
    pub removed: u32,
}

/// A batch file entry for codebase search results.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchFile {
    pub path: String,
    pub line_snippet: String,
    pub is_outside_workspace: Option<bool>,
    pub key: String,
    pub content: Option<String>,
}

/// A batch diff entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchDiff {
    pub path: String,
    pub change_count: u32,
    pub key: String,
    pub content: String,
    pub diff_stats: Option<DiffStats>,
    pub diffs: Option<Vec<DiffEntry>>,
}

/// A single diff entry within a batch diff.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiffEntry {
    pub content: String,
    pub start_line: Option<usize>,
}

/// A batch directory entry.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchDir {
    pub path: String,
    pub recursive: bool,
    pub is_outside_workspace: Option<bool>,
    pub key: String,
}

/// The `ClineSayTool` interface.
///
/// Source: L767-843
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClineSayTool {
    pub tool: ClineSayToolType,
    pub path: Option<String>,
    // readCommandOutput fields
    pub read_start: Option<u64>,
    pub read_end: Option<u64>,
    pub total_bytes: Option<u64>,
    pub search_pattern: Option<String>,
    pub match_count: Option<u32>,
    pub diff: Option<String>,
    pub content: Option<String>,
    /// Original file content before first edit (for merged diff display).
    pub original_content: Option<String>,
    /// Unified diff statistics computed by the extension.
    pub diff_stats: Option<DiffStats>,
    pub regex: Option<String>,
    pub file_pattern: Option<String>,
    pub mode: Option<String>,
    pub reason: Option<String>,
    pub is_outside_workspace: Option<bool>,
    pub is_protected: Option<bool>,
    /// Number of additional files in the same read_file request.
    pub additional_file_count: Option<u32>,
    pub line_number: Option<usize>,
    /// Starting line for read_file operations (for navigation on click).
    pub start_line: Option<usize>,
    pub query: Option<String>,
    pub batch_files: Option<Vec<BatchFile>>,
    pub batch_diffs: Option<Vec<BatchDiff>>,
    pub batch_dirs: Option<Vec<BatchDir>>,
    pub question: Option<String>,
    /// Base64 encoded image data for generated images.
    pub image_data: Option<String>,
    // runSlashCommand fields
    pub command: Option<String>,
    pub args: Option<String>,
    pub source: Option<String>,
    pub description: Option<String>,
    // skill tool
    pub skill: Option<String>,
}

// ---------------------------------------------------------------------------
// ClineAskUseMcpServer
// ---------------------------------------------------------------------------

/// The `ClineAskUseMcpServer` interface.
///
/// Source: L845-852
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClineAskUseMcpServer {
    pub server_name: String,
    pub r#type: McpActionType,
    pub tool_name: Option<String>,
    pub arguments: Option<String>,
    pub uri: Option<String>,
    pub response: Option<String>,
}

/// MCP action type.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpActionType {
    UseMcpTool,
    AccessMcpResource,
}

// ---------------------------------------------------------------------------
// ClineApiReqInfo
// ---------------------------------------------------------------------------

/// The `ClineApiReqInfo` interface.
///
/// Source: L854-864
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClineApiReqInfo {
    pub request: Option<String>,
    pub tokens_in: Option<u64>,
    pub tokens_out: Option<u64>,
    pub cache_writes: Option<u64>,
    pub cache_reads: Option<u64>,
    pub cost: Option<f64>,
    pub cancel_reason: Option<ClineApiReqCancelReason>,
    pub streaming_failed_message: Option<String>,
    pub api_protocol: Option<ApiReqProtocol>,
}

/// Cancel reason for API requests.
///
/// Source: L866
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClineApiReqCancelReason {
    StreamingFailed,
    UserCancelled,
}

/// API protocol used for the request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ApiReqProtocol {
    Anthropic,
    Openai,
}

// ---------------------------------------------------------------------------
// IndexingStatus
// ---------------------------------------------------------------------------

/// Full indexing status information.
///
/// Source: L744-753
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexingStatus {
    pub system_status: String,
    pub message: Option<String>,
    pub processed_items: u64,
    pub total_items: u64,
    pub current_item_unit: Option<String>,
    pub workspace_path: Option<String>,
    pub workspace_enabled: Option<bool>,
    pub auto_enable_default: Option<bool>,
}

/// Language model chat selector for VS Code LM API.
///
/// Source: L760-765
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct LanguageModelChatSelector {
    pub vendor: Option<String>,
    pub family: Option<String>,
    pub version: Option<String>,
    pub id: Option<String>,
}

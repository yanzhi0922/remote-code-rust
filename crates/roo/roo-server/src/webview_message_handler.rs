//! Webview message routing and handling.
//!
//! Derived from `src/core/webview/webviewMessageHandler.ts`.
//!
//! Routes incoming webview messages to the appropriate handler functions.
//! This module provides the core message routing logic, extracting common
//! patterns from the TS webviewMessageHandler.

use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Supported webview message types.
///
/// Source: `src/core/webview/webviewMessageHandler.ts` — various message types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum WebviewMessageType {
    // Task lifecycle
    SendMessage,
    CancelTask,
    ClearTask,
    ResumeTask,
    StartNewTask,

    // State
    GetState,
    Mode,

    // History
    DeleteMessage,
    EditMessage,
    SubmitEditedMessage,

    // Search
    SearchFiles,
    ListFiles,
    CodebaseSearch,

    // Settings
    SettingsUpdate,
    SaveApiConfig,
    LoadApiConfig,
    DeleteApiConfig,
    ListApiConfigs,
    UpsertApiConfig,
    RenameApiConfig,
    CustomInstructions,
    UpdatePrompt,
    CopySystemPrompt,
    ResetState,
    ImportSettings,
    ExportSettings,
    LockApiConfigAcrossModes,
    ToggleApiConfigPin,
    EnhancementApiConfigId,
    AutoApprovalEnabled,
    DebugSetting,
    AllowedCommands,
    DeniedCommands,
    UpdateCondensingPrompt,
    SetApiConfigPassword,
    HasOpenedModeSelector,
    TaskSyncEnabled,
    UpdateSettings,
    UpdateVSCodeSetting,
    GetVSCodeSetting,

    // Skills
    RequestSkills,
    CreateSkill,
    DeleteSkill,
    MoveSkill,
    UpdateSkillModes,
    OpenSkillFile,

    // MCP
    McpToggleServer,
    McpRestartServer,
    McpDeleteServer,
    McpUpdateTimeout,
    McpRefreshAll,
    McpToggleToolAlwaysAllow,
    McpToggleToolEnabledForPrompt,
    OpenMcpSettings,
    OpenProjectMcpSettings,

    // Checkpoint
    CheckpointDiff,
    CheckpointRestore,

    // Prompt
    EnhancePrompt,
    GetSystemPrompt,

    // Terminal
    TerminalOperation,

    // Marketplace
    InstallMarketplaceItem,
    RemoveInstalledMarketplaceItem,
    InstallMarketplaceItemWithParameters,
    FetchMarketplaceData,
    FilterMarketplaceItems,
    MarketplaceButtonClicked,
    CancelMarketplaceInstall,

    // Worktree
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

    // TTS
    PlayTts,
    StopTts,
    TtsEnabled,
    TtsSpeed,

    // Image
    SaveImage,
    OpenImage,

    // Model requests
    FlushRouterModels,
    RequestRouterModels,
    RequestOpenAiModels,
    RequestOllamaModels,
    RequestLmStudioModels,
    RequestRooModels,
    RequestRooCreditBalance,
    RequestVsCodeLmModels,

    // Mentions
    OpenMention,
    ResolveMentions,

    // Commands (slash)
    RequestCommands,
    OpenCommandFile,
    DeleteCommand,
    CreateCommand,

    // Mode
    UpdateCustomMode,
    DeleteCustomMode,
    ExportMode,
    ImportMode,
    SwitchMode,
    CheckRulesDirectory,
    OpenCustomModesSettings,
    SetopenAiCustomModelInfo,

    // UI / VS Code-specific
    WebviewDidLaunch,
    DidShowAnnouncement,
    SelectImages,
    DraggedImages,
    PlaySound,
    OpenFile,
    OpenExternal,
    OpenKeyboardShortcuts,
    FocusPanelRequest,
    SwitchTab,
    InsertTextIntoTextarea,
    OpenMarkdownPreview,

    // Cloud
    RooCloudSignIn,
    RooCloudSignOut,
    RooCloudManualUrl,
    CloudButtonClicked,
    ClearCloudAuthSkipModel,
    SwitchOrganization,
    OpenAiCodexSignIn,
    OpenAiCodexSignOut,
    RequestOpenAiCodexRateLimits,

    // Codebase Index
    CodebaseIndexEnabled,
    RequestIndexingStatus,
    StartIndexing,
    StopIndexing,
    ClearIndexData,
    ToggleWorkspaceIndexing,
    SetAutoEnableDefault,
    SaveCodeIndexSettingsAtomic,
    RequestCodeIndexSecretStatus,

    // Upsell
    DismissUpsell,
    GetDismissedUpsells,

    // Debug
    OpenDebugApiHistory,
    OpenDebugUiHistory,
    DownloadErrorDiagnostics,

    // Other
    GetCommands,
    GetModes,
    GetModels,
    ShowMdmAuthRequiredNotification,
    ImageGenerationSettings,
    ShareCurrentTask,

    // Telemetry
    Telemetry,

    // Unknown
    #[serde(other)]
    Unknown,
}

/// A webview message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebviewMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub images: Option<Vec<String>>,
    #[serde(default)]
    pub mode: Option<String>,
}

/// Result of handling a webview message.
#[derive(Debug, Clone)]
pub struct MessageHandleResult {
    pub success: bool,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Message routing
// ---------------------------------------------------------------------------

/// Routes a webview message to the appropriate handler.
///
/// Source: `src/core/webview/webviewMessageHandler.ts` — `webviewMessageHandler`
///
/// This is the main entry point for processing webview messages.
/// It determines the message type and delegates to the appropriate handler.
///
/// # Arguments
/// * `message` - The incoming webview message
/// * `handlers` - Handler functions for different message types
///
/// # Returns
/// A `MessageHandleResult` indicating success or failure.
pub fn route_message(
    message: &WebviewMessage,
    handlers: &MessageHandlers,
) -> MessageHandleResult {
    let msg_type = message.msg_type.to_lowercase();

    debug!("Routing webview message: {}", msg_type);

    match msg_type.as_str() {
        // Task lifecycle
        "sendmessage" => (handlers.handle_send_message)(message),
        "canceltask" => (handlers.handle_cancel_task)(message),
        "cleartask" => (handlers.handle_clear_task)(message),
        "resumetask" => (handlers.handle_resume_task)(message),
        "startnewtask" => (handlers.handle_start_new_task)(message),

        // State
        "getstate" => (handlers.handle_get_state)(message),
        "mode" => (handlers.handle_mode_change)(message),

        // History
        "deletemessage" => (handlers.handle_delete_message)(message),
        "editmessage" => (handlers.handle_edit_message)(message),
        "submiteditedmessage" => noop_handler(message),

        // Search
        "searchfiles" => (handlers.handle_search_files)(message),
        "listfiles" => (handlers.handle_list_files)(message),
        "codebasesearch" => (handlers.handle_codebase_search)(message),

        // Settings
        "settingsupdate" | "updatesettings" => (handlers.handle_settings_update)(message),
        "saveapiconfig" | "saveapiconfiguration" => (handlers.handle_save_api_config)(message),
        "loadapiconfig" | "loadapiconfiguration" => (handlers.handle_load_api_config)(message),
        "deleteapiconfig" | "deleteapiconfiguration" => (handlers.handle_delete_api_config)(message),
        "listapiconfigs" | "getlistapiconfiguration" => (handlers.handle_list_api_configs)(message),
        "upsertapiconfiguration" => noop_handler(message),
        "renameapiconfiguration" => noop_handler(message),
        "custominstructions" => noop_handler(message),
        "updateprompt" => noop_handler(message),
        "copysystemprompt" => noop_handler(message),
        "resetstate" => noop_handler(message),
        "importsettings" => noop_handler(message),
        "exportsettings" => noop_handler(message),
        "lockapiconfigacrossmodes" => noop_handler(message),
        "toggleapiconfigpin" => noop_handler(message),
        "enhancementapiconfigid" => noop_handler(message),
        "autoapprovalenabled" => noop_handler(message),
        "debugsetting" => noop_handler(message),
        "allowedcommands" => noop_handler(message),
        "deniedcommands" => noop_handler(message),
        "updatecondensingprompt" => noop_handler(message),
        "setapiconfigpassword" => noop_handler(message),
        "hasopenedmodeselector" => noop_handler(message),
        "tasksyncenabled" => noop_handler(message),
        "updatevscodesetting" => noop_handler(message),
        "getvsCodesetting" => noop_handler(message),

        // Skills
        "requestskills" => (handlers.handle_request_skills)(message),
        "createskill" => (handlers.handle_create_skill)(message),
        "deleteskill" => (handlers.handle_delete_skill)(message),
        "moveskill" => (handlers.handle_move_skill)(message),
        "updateskillmodes" => (handlers.handle_update_skill_modes)(message),
        "openskillfile" => (handlers.handle_open_skill_file)(message),

        // MCP
        "mcptoggleserver" | "togglemcpserver" => (handlers.handle_mcp_toggle_server)(message),
        "mcprestartserver" | "restartmcpserver" => (handlers.handle_mcp_restart_server)(message),
        "mcpdeleteserver" | "deletemcpserver" => (handlers.handle_mcp_delete_server)(message),
        "mcpupdatetimeout" | "updatemcptimeout" => (handlers.handle_mcp_update_timeout)(message),
        "refreshallmcpservers" => noop_handler(message),
        "toggletoolalwaysallow" => noop_handler(message),
        "toggletoolenabledforprompt" => noop_handler(message),
        "openmcpsettings" => noop_handler(message),
        "openprojectmcpsettings" => noop_handler(message),

        // Checkpoint
        "checkpointdiff" => (handlers.handle_checkpoint_diff)(message),
        "checkpointrestore" => (handlers.handle_checkpoint_restore)(message),

        // Prompt
        "enhanceprompt" => (handlers.handle_enhance_prompt)(message),
        "getsystemprompt" => (handlers.handle_get_system_prompt)(message),

        // Terminal
        "terminaloperation" => (handlers.handle_terminal_operation)(message),

        // Marketplace
        "installmarketplaceitem" => noop_handler(message),
        "removeinstalledmarketplaceitem" => noop_handler(message),
        "installmarketplaceitemwithparameters" => noop_handler(message),
        "fetchmarketplacedata" => noop_handler(message),
        "filtermarketplaceitems" => noop_handler(message),
        "marketplacebuttonclicked" => noop_handler(message),
        "cancelmarketplaceinstall" => noop_handler(message),

        // Worktree
        "listworktrees" => noop_handler(message),
        "createworktree" => noop_handler(message),
        "deleteworktree" => noop_handler(message),
        "switchworktree" => noop_handler(message),
        "getavailablebranches" => noop_handler(message),
        "getworktreedefaults" => noop_handler(message),
        "getworktreeincludstatus" => noop_handler(message),
        "checkbranchworktreeinclude" => noop_handler(message),
        "createworktreeinclude" => noop_handler(message),
        "checkoutbranch" => noop_handler(message),
        "browseforworktreepath" => noop_handler(message),

        // TTS
        "playtts" => noop_handler(message),
        "stoptts" => noop_handler(message),
        "ttsenabled" => noop_handler(message),
        "ttsspeed" => noop_handler(message),

        // Image
        "saveimage" => noop_handler(message),
        "openimage" => noop_handler(message),

        // Model requests
        "flushroutermodels" => noop_handler(message),
        "requestroutermodels" => noop_handler(message),
        "requestopenaimodels" => noop_handler(message),
        "requestollamamodels" => noop_handler(message),
        "requestlmstudiomodels" => noop_handler(message),
        "requestroomodels" => noop_handler(message),
        "requestroocreditbalance" => noop_handler(message),
        "requestvscodelmmodels" => noop_handler(message),

        // Mentions
        "openmention" => noop_handler(message),

        // Commands (slash)
        "requestcommands" => (handlers.handle_get_commands)(message),
        "opencommandfile" => noop_handler(message),
        "deletecommand" => noop_handler(message),
        "createcommand" => noop_handler(message),

        // Mode
        "updatecustommode" => noop_handler(message),
        "deletecustommode" => noop_handler(message),
        "exportmode" => noop_handler(message),
        "importmode" => noop_handler(message),
        "switchmode" => (handlers.handle_mode_change)(message),
        "checkrulesdirectory" => noop_handler(message),
        "opencustommodessettings" => noop_handler(message),
        "setopenaicustommodelinfo" => noop_handler(message),

        // UI / VS Code-specific
        "webviewdidlaunch" => noop_handler(message),
        "didshowannouncement" => noop_handler(message),
        "selectimages" => noop_handler(message),
        "draggedimages" => noop_handler(message),
        "playsound" => noop_handler(message),
        "openfile" => noop_handler(message),
        "openexternal" => noop_handler(message),
        "openkeyboardshortcuts" => noop_handler(message),
        "focuspanelrequest" => noop_handler(message),
        "switchtab" => noop_handler(message),
        "inserttextintotextarea" => noop_handler(message),
        "openmarkdownpreview" => noop_handler(message),

        // Cloud
        "roocloudsignin" => noop_handler(message),
        "roocloudsignout" => noop_handler(message),
        "roocloudmanualurl" => noop_handler(message),
        "cloudbuttonclicked" => noop_handler(message),
        "clearcloudauthskipmodel" => noop_handler(message),
        "switchorganization" => noop_handler(message),
        "openaicodexsignin" => noop_handler(message),
        "openaicodexsignout" => noop_handler(message),
        "requestopenaicodexratelimits" => noop_handler(message),

        // Codebase Index
        "codebaseindexenabled" => noop_handler(message),
        "requestindexingstatus" => noop_handler(message),
        "startindexing" => noop_handler(message),
        "stopindexing" => noop_handler(message),
        "clearindexdata" => noop_handler(message),
        "toggleworkspaceindexing" => noop_handler(message),
        "setautoenabledefault" => noop_handler(message),
        "savecodeindexsettingsatomic" => noop_handler(message),
        "requestcodeindexsecretstatus" => noop_handler(message),

        // Upsell
        "dismissupsell" => noop_handler(message),
        "getdismissedupsells" => noop_handler(message),

        // Debug
        "opendebugapihistory" => noop_handler(message),
        "opendebuguihistory" => noop_handler(message),
        "downloaderrordiagnostics" => noop_handler(message),

        // Other
        "getcommands" => (handlers.handle_get_commands)(message),
        "getmodes" => (handlers.handle_get_modes)(message),
        "getmodels" => (handlers.handle_get_models)(message),
        "showmdmauthrequirednotification" => noop_handler(message),
        "imagegenerationsettings" => noop_handler(message),
        "sharecurrenttask" => noop_handler(message),

        // Telemetry
        "telemetry" | "telemetrysetting" => (handlers.handle_telemetry)(message),

        _ => {
            warn!("Unknown webview message type: {}", msg_type);
            MessageHandleResult {
                success: false,
                error: Some(format!("Unknown message type: {msg_type}")),
            }
        }
    }
}

/// Collection of handler functions for different message types.
///
/// Each handler takes a reference to the message and returns a `MessageHandleResult`.
pub struct MessageHandlers {
    pub handle_send_message: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_cancel_task: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_clear_task: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_resume_task: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_start_new_task: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_get_state: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_mode_change: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_delete_message: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_edit_message: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_search_files: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_list_files: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_codebase_search: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_settings_update: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_save_api_config: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_load_api_config: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_delete_api_config: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_list_api_configs: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_request_skills: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_create_skill: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_delete_skill: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_move_skill: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_update_skill_modes: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_open_skill_file: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_mcp_toggle_server: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_mcp_restart_server: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_mcp_delete_server: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_mcp_update_timeout: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_checkpoint_diff: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_checkpoint_restore: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_enhance_prompt: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_get_system_prompt: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_terminal_operation: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_get_commands: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_get_modes: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_get_models: fn(&WebviewMessage) -> MessageHandleResult,
    pub handle_telemetry: fn(&WebviewMessage) -> MessageHandleResult,
}

/// Default no-op handler that returns success.
fn noop_handler(_message: &WebviewMessage) -> MessageHandleResult {
    MessageHandleResult {
        success: true,
        error: None,
    }
}

impl Default for MessageHandlers {
    fn default() -> Self {
        Self {
            handle_send_message: noop_handler,
            handle_cancel_task: noop_handler,
            handle_clear_task: noop_handler,
            handle_resume_task: noop_handler,
            handle_start_new_task: noop_handler,
            handle_get_state: noop_handler,
            handle_mode_change: noop_handler,
            handle_delete_message: noop_handler,
            handle_edit_message: noop_handler,
            handle_search_files: noop_handler,
            handle_list_files: noop_handler,
            handle_codebase_search: noop_handler,
            handle_settings_update: noop_handler,
            handle_save_api_config: noop_handler,
            handle_load_api_config: noop_handler,
            handle_delete_api_config: noop_handler,
            handle_list_api_configs: noop_handler,
            handle_request_skills: noop_handler,
            handle_create_skill: noop_handler,
            handle_delete_skill: noop_handler,
            handle_move_skill: noop_handler,
            handle_update_skill_modes: noop_handler,
            handle_open_skill_file: noop_handler,
            handle_mcp_toggle_server: noop_handler,
            handle_mcp_restart_server: noop_handler,
            handle_mcp_delete_server: noop_handler,
            handle_mcp_update_timeout: noop_handler,
            handle_checkpoint_diff: noop_handler,
            handle_checkpoint_restore: noop_handler,
            handle_enhance_prompt: noop_handler,
            handle_get_system_prompt: noop_handler,
            handle_terminal_operation: noop_handler,
            handle_get_commands: noop_handler,
            handle_get_modes: noop_handler,
            handle_get_models: noop_handler,
            handle_telemetry: noop_handler,
        }
    }
}

// ---------------------------------------------------------------------------
// Utility functions from webviewMessageHandler.ts
// ---------------------------------------------------------------------------

/// Finds message indices based on timestamp.
///
/// Source: `src/core/webview/webviewMessageHandler.ts` — `findMessageIndices`
pub fn find_message_indices(
    message_ts: u64,
    cline_messages: &[(u64, bool)], // (ts, is_summary)
    api_history: &[(u64, bool)],    // (ts, is_summary)
) -> (isize, isize) {
    // Find the exact message by timestamp
    let message_index = cline_messages
        .iter()
        .position(|(ts, _)| *ts == message_ts)
        .map(|i| i as isize)
        .unwrap_or(-1);

    // Find all matching API messages by timestamp
    let all_api_matches: Vec<(usize, bool)> = api_history
        .iter()
        .enumerate()
        .filter(|(_, (ts, _))| *ts == message_ts)
        .map(|(i, (_, is_summary))| (i, *is_summary))
        .collect();

    // Prefer non-summary message if multiple matches exist
    let api_index = all_api_matches
        .iter()
        .find(|(_, is_summary)| !is_summary)
        .or_else(|| all_api_matches.first())
        .map(|(i, _)| *i as isize)
        .unwrap_or(-1);

    (message_index, api_index)
}

/// Finds the first API history index at or after a timestamp.
///
/// Source: `src/core/webview/webviewMessageHandler.ts` — `findFirstApiIndexAtOrAfter`
pub fn find_first_api_index_at_or_after(
    ts: u64,
    api_history: &[(u64, bool)],
) -> isize {
    api_history
        .iter()
        .position(|(msg_ts, _)| *msg_ts >= ts)
        .map(|i| i as isize)
        .unwrap_or(-1)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_route_message_unknown_type() {
        let message = WebviewMessage {
            msg_type: "unknownType".to_string(),
            text: None,
            images: None,
            mode: None,
        };
        let handlers = MessageHandlers::default();
        let result = route_message(&message, &handlers);
        assert!(!result.success);
        assert!(result.error.unwrap().contains("Unknown message type"));
    }

    #[test]
    fn test_route_message_send_message() {
        let message = WebviewMessage {
            msg_type: "sendMessage".to_string(),
            text: Some("Hello".to_string()),
            images: None,
            mode: None,
        };
        let handlers = MessageHandlers::default();
        let result = route_message(&message, &handlers);
        assert!(result.success);
    }

    #[test]
    fn test_route_message_case_insensitive() {
        let message = WebviewMessage {
            msg_type: "SENDMESSAGE".to_string(),
            text: None,
            images: None,
            mode: None,
        };
        let handlers = MessageHandlers::default();
        let result = route_message(&message, &handlers);
        assert!(result.success);
    }

    #[test]
    fn test_find_message_indices() {
        let cline_messages = vec![(100u64, false), (200u64, false), (300u64, false)];
        let api_history = vec![(100u64, false), (200u64, true), (200u64, false), (300u64, false)];

        let (msg_idx, api_idx) = find_message_indices(200, &cline_messages, &api_history);
        assert_eq!(msg_idx, 1);
        // Should prefer non-summary (index 2)
        assert_eq!(api_idx, 2);
    }

    #[test]
    fn test_find_message_indices_not_found() {
        let cline_messages = vec![(100u64, false)];
        let api_history = vec![(100u64, false)];

        let (msg_idx, api_idx) = find_message_indices(999, &cline_messages, &api_history);
        assert_eq!(msg_idx, -1);
        assert_eq!(api_idx, -1);
    }

    #[test]
    fn test_find_first_api_index_at_or_after() {
        let api_history = vec![(100u64, false), (200u64, false), (300u64, false)];
        let result = find_first_api_index_at_or_after(200, &api_history);
        assert_eq!(result, 1);
    }

    #[test]
    fn test_find_first_api_index_not_found() {
        let api_history = vec![(100u64, false), (200u64, false)];
        let result = find_first_api_index_at_or_after(300, &api_history);
        assert_eq!(result, -1);
    }
}

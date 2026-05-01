//! JSON-RPC method router.
//!
//! Provides a simple routing mechanism that maps JSON-RPC method names
//! to handler functions. This is the dispatch layer between the transport
//! and the handler.

use roo_jsonrpc::types::Message;

use crate::handler::Handler;

/// A JSON-RPC method router.
///
/// The router receives incoming JSON-RPC messages and dispatches them
/// to the appropriate handler method.
pub struct Router {
    handler: Handler,
}

impl Router {
    /// Create a new router with the given handler.
    pub fn new(handler: Handler) -> Self {
        Self { handler }
    }

    /// Route a single JSON-RPC message to the handler.
    ///
    /// For requests, returns the handler's response.
    /// For notifications, returns a notification acknowledgment.
    /// For responses (shouldn't happen server-side), returns an error.
    pub async fn route(&self, message: &Message) -> Message {
        if message.is_request() {
            self.handler.handle(message).await
        } else if message.is_notification() {
            // Notifications don't get responses, but we still process them
            tracing::debug!(
                method = ?message.method,
                "Processing notification"
            );
            self.handler.handle(message).await
        } else if message.is_response() || message.is_error() {
            // Server shouldn't receive responses
            tracing::warn!("Received unexpected response message, ignoring");
            Message::error_response(
                message.id.clone().unwrap_or(serde_json::Value::Null),
                roo_jsonrpc::types::error_codes::INVALID_REQUEST,
                "Server does not accept response messages",
            )
        } else {
            Message::error_response(
                message.id.clone().unwrap_or(serde_json::Value::Null),
                roo_jsonrpc::types::error_codes::INVALID_REQUEST,
                "Unknown message type",
            )
        }
    }

    /// Route a batch of JSON-RPC messages.
    ///
    /// Processes each message independently and returns a batch of responses.
    /// Notifications in the batch are processed but don't generate responses.
    pub async fn route_batch(&self, messages: &[Message]) -> Vec<Message> {
        let mut responses = Vec::with_capacity(messages.len());
        for msg in messages {
            let response = self.route(msg).await;
            // Only include responses for requests (not notifications)
            if msg.is_request() {
                responses.push(response);
            }
        }
        responses
    }
}

/// List of all supported JSON-RPC method names.
///
/// Used for validation and capability reporting.
pub fn supported_methods() -> &'static [&'static str] {
    &[
        // ── Lifecycle ──
        crate::handler::methods::INITIALIZE,
        crate::handler::methods::SHUTDOWN,
        crate::handler::methods::PING,
        // ── Task commands ──
        crate::handler::methods::TASK_START,
        crate::handler::methods::TASK_CANCEL,
        crate::handler::methods::TASK_CLOSE,
        crate::handler::methods::TASK_RESUME,
        crate::handler::methods::TASK_SEND_MESSAGE,
        crate::handler::methods::TASK_GET_COMMANDS,
        crate::handler::methods::TASK_GET_MODES,
        crate::handler::methods::TASK_GET_MODELS,
        crate::handler::methods::TASK_DELETE_QUEUED_MESSAGE,
        crate::handler::methods::TASK_CONDENSE,
        crate::handler::methods::TASK_CLEAR,
        crate::handler::methods::TASK_CANCEL_AUTO_APPROVAL,
        crate::handler::methods::TASK_GET_AGGREGATED_COSTS,
        crate::handler::methods::TASK_SHOW_WITH_ID,
        // ── State ──
        crate::handler::methods::STATE_GET,
        crate::handler::methods::STATE_SET_MODE,
        crate::handler::methods::SYSTEM_PROMPT_BUILD,
        // ── History ──
        crate::handler::methods::HISTORY_GET,
        crate::handler::methods::HISTORY_DELETE,
        crate::handler::methods::HISTORY_DELETE_MULTIPLE,
        crate::handler::methods::HISTORY_EXPORT,
        crate::handler::methods::HISTORY_SHARE_TASK,
        // ── Todo / Ask / Terminal ──
        crate::handler::methods::TODO_UPDATE,
        crate::handler::methods::ASK_RESPONSE,
        crate::handler::methods::TERMINAL_OPERATION,
        // ── Checkpoint ──
        crate::handler::methods::CHECKPOINT_DIFF,
        crate::handler::methods::CHECKPOINT_RESTORE,
        // ── Prompt / Search ──
        crate::handler::methods::PROMPT_ENHANCE,
        crate::handler::methods::SEARCH_FILES,
        crate::handler::methods::FILE_READ,
        crate::handler::methods::GIT_SEARCH_COMMITS,
        // ── MCP ──
        crate::handler::methods::MCP_LIST_SERVERS,
        crate::handler::methods::MCP_RESTART_SERVER,
        crate::handler::methods::MCP_TOGGLE_SERVER,
        crate::handler::methods::MCP_USE_TOOL,
        crate::handler::methods::MCP_ACCESS_RESOURCE,
        crate::handler::methods::MCP_DELETE_SERVER,
        crate::handler::methods::MCP_UPDATE_TIMEOUT,
        crate::handler::methods::MCP_REFRESH_ALL,
        crate::handler::methods::MCP_TOGGLE_TOOL_ALWAYS_ALLOW,
        crate::handler::methods::MCP_TOGGLE_TOOL_ENABLED_FOR_PROMPT,
        // ── Settings ──
        crate::handler::methods::SETTINGS_UPDATE,
        crate::handler::methods::SETTINGS_SAVE_API_CONFIG,
        crate::handler::methods::SETTINGS_LOAD_API_CONFIG,
        crate::handler::methods::SETTINGS_LOAD_API_CONFIG_BY_ID,
        crate::handler::methods::SETTINGS_DELETE_API_CONFIG,
        crate::handler::methods::SETTINGS_LIST_API_CONFIGS,
        crate::handler::methods::SETTINGS_UPSERT_API_CONFIG,
        crate::handler::methods::SETTINGS_RENAME_API_CONFIG,
        crate::handler::methods::SETTINGS_CUSTOM_INSTRUCTIONS,
        crate::handler::methods::SETTINGS_UPDATE_PROMPT,
        crate::handler::methods::SETTINGS_COPY_SYSTEM_PROMPT,
        crate::handler::methods::SETTINGS_RESET_STATE,
        crate::handler::methods::SETTINGS_IMPORT_SETTINGS,
        crate::handler::methods::SETTINGS_EXPORT_SETTINGS,
        crate::handler::methods::SETTINGS_LOCK_API_CONFIG,
        crate::handler::methods::SETTINGS_TOGGLE_API_CONFIG_PIN,
        crate::handler::methods::SETTINGS_ENHANCEMENT_API_CONFIG_ID,
        crate::handler::methods::SETTINGS_AUTO_APPROVAL_ENABLED,
        crate::handler::methods::SETTINGS_DEBUG_SETTING,
        crate::handler::methods::SETTINGS_ALLOWED_COMMANDS,
        crate::handler::methods::SETTINGS_DENIED_COMMANDS,
        crate::handler::methods::SETTINGS_CONDENSING_PROMPT,
        crate::handler::methods::SETTINGS_SET_API_CONFIG_PASSWORD,
        crate::handler::methods::SETTINGS_HAS_OPENED_MODE_SELECTOR,
        crate::handler::methods::SETTINGS_TASK_SYNC_ENABLED,
        crate::handler::methods::SETTINGS_UPDATE_SETTINGS,
        crate::handler::methods::SETTINGS_UPDATE_VSCODE_SETTING,
        crate::handler::methods::SETTINGS_GET_VSCODE_SETTING,
        // ── Skills ──
        crate::handler::methods::SKILLS_LIST,
        crate::handler::methods::SKILLS_CREATE,
        crate::handler::methods::SKILLS_DELETE,
        crate::handler::methods::SKILLS_MOVE,
        crate::handler::methods::SKILLS_UPDATE_MODES,
        crate::handler::methods::SKILL_OPEN_FILE,
        // ── Mode ──
        crate::handler::methods::MODE_UPDATE_CUSTOM,
        crate::handler::methods::MODE_DELETE_CUSTOM,
        crate::handler::methods::MODE_EXPORT,
        crate::handler::methods::MODE_IMPORT,
        crate::handler::methods::MODE_SWITCH,
        crate::handler::methods::MODE_CHECK_RULES,
        crate::handler::methods::MODE_OPEN_SETTINGS,
        crate::handler::methods::MODE_SET_OPENAI_CUSTOM_MODEL_INFO,
        // ── Message ──
        crate::handler::methods::MESSAGE_DELETE,
        crate::handler::methods::MESSAGE_EDIT,
        crate::handler::methods::MESSAGE_QUEUE,
        crate::handler::methods::MESSAGE_DELETE_CONFIRM,
        crate::handler::methods::MESSAGE_EDIT_CONFIRM,
        crate::handler::methods::MESSAGE_EDIT_QUEUED,
        crate::handler::methods::MESSAGE_REMOVE_QUEUED,
        crate::handler::methods::MESSAGE_SUBMIT_EDITED,
        // ── Tools / Telemetry ──
        crate::handler::methods::TOOLS_REFRESH_CUSTOM,
        crate::handler::methods::TELEMETRY_SET_SETTING,
        // ── Marketplace ──
        crate::handler::methods::MARKETPLACE_INSTALL,
        crate::handler::methods::MARKETPLACE_REMOVE,
        crate::handler::methods::MARKETPLACE_INSTALL_WITH_PARAMS,
        crate::handler::methods::MARKETPLACE_FETCH_DATA,
        crate::handler::methods::MARKETPLACE_FILTER_ITEMS,
        crate::handler::methods::MARKETPLACE_BUTTON_CLICKED,
        crate::handler::methods::MARKETPLACE_CANCEL_INSTALL,
        // ── Worktree ──
        crate::handler::methods::WORKTREE_LIST,
        crate::handler::methods::WORKTREE_CREATE,
        crate::handler::methods::WORKTREE_DELETE,
        crate::handler::methods::WORKTREE_SWITCH,
        crate::handler::methods::WORKTREE_GET_BRANCHES,
        crate::handler::methods::WORKTREE_GET_DEFAULTS,
        crate::handler::methods::WORKTREE_GET_INCLUDE_STATUS,
        crate::handler::methods::WORKTREE_CHECK_BRANCH_INCLUDE,
        crate::handler::methods::WORKTREE_CREATE_INCLUDE,
        crate::handler::methods::WORKTREE_CHECKOUT_BRANCH,
        crate::handler::methods::WORKTREE_BROWSE_PATH,
        // ── TTS ──
        crate::handler::methods::TTS_PLAY,
        crate::handler::methods::TTS_STOP,
        crate::handler::methods::TTS_ENABLED,
        crate::handler::methods::TTS_SPEED,
        // ── Image ──
        crate::handler::methods::IMAGE_SAVE,
        crate::handler::methods::IMAGE_OPEN,
        // ── Model requests ──
        crate::handler::methods::MODELS_FLUSH_ROUTER,
        crate::handler::methods::MODELS_REQUEST_ROUTER,
        crate::handler::methods::MODELS_REQUEST_OPENAI,
        crate::handler::methods::MODELS_REQUEST_OLLAMA,
        crate::handler::methods::MODELS_REQUEST_LMSTUDIO,
        crate::handler::methods::MODELS_REQUEST_ROO,
        crate::handler::methods::MODELS_REQUEST_ROO_CREDIT,
        crate::handler::methods::MODELS_REQUEST_VSCODELM,
        // ── Mentions ──
        crate::handler::methods::MENTION_OPEN,
        crate::handler::methods::MENTION_RESOLVE,
        // ── Commands (slash) ──
        crate::handler::methods::COMMAND_REQUEST,
        crate::handler::methods::COMMAND_OPEN_FILE,
        crate::handler::methods::COMMAND_DELETE,
        crate::handler::methods::COMMAND_CREATE,
        // ── UI / VS Code-specific ──
        crate::handler::methods::WEBVIEW_DID_LAUNCH,
        crate::handler::methods::ANNOUNCEMENT_DID_SHOW,
        crate::handler::methods::IMAGES_SELECT,
        crate::handler::methods::IMAGES_DRAGGED,
        crate::handler::methods::PLAY_SOUND,
        crate::handler::methods::FILE_OPEN,
        crate::handler::methods::EXTERNAL_OPEN,
        crate::handler::methods::OPEN_KEYBOARD_SHORTCUTS,
        crate::handler::methods::OPEN_MCP_SETTINGS,
        crate::handler::methods::OPEN_PROJECT_MCP_SETTINGS,
        crate::handler::methods::FOCUS_PANEL,
        crate::handler::methods::TAB_SWITCH,
        crate::handler::methods::INSERT_TEXT,
        crate::handler::methods::MARKDOWN_PREVIEW,
        // ── Cloud ──
        crate::handler::methods::CLOUD_SIGN_IN,
        crate::handler::methods::CLOUD_SIGN_OUT,
        crate::handler::methods::CLOUD_MANUAL_URL,
        crate::handler::methods::CLOUD_BUTTON_CLICKED,
        crate::handler::methods::CLOUD_CLEAR_SKIP_MODEL,
        crate::handler::methods::CLOUD_SWITCH_ORG,
        crate::handler::methods::CODEX_SIGN_IN,
        crate::handler::methods::CODEX_SIGN_OUT,
        crate::handler::methods::CODEX_REQUEST_RATE_LIMITS,
        // ── Codebase Index ──
        crate::handler::methods::INDEX_ENABLED,
        crate::handler::methods::INDEX_REQUEST_STATUS,
        crate::handler::methods::INDEX_START,
        crate::handler::methods::INDEX_STOP,
        crate::handler::methods::INDEX_CLEAR,
        crate::handler::methods::INDEX_TOGGLE_WORKSPACE,
        crate::handler::methods::INDEX_SET_AUTO_ENABLE,
        crate::handler::methods::INDEX_SAVE_SETTINGS,
        crate::handler::methods::INDEX_REQUEST_SECRET_STATUS,
        // ── Upsell ──
        crate::handler::methods::UPSELL_DISMISS,
        crate::handler::methods::UPSELL_GET_DISMISSED,
        // ── Debug ──
        crate::handler::methods::DEBUG_API_HISTORY,
        crate::handler::methods::DEBUG_UI_HISTORY,
        crate::handler::methods::DEBUG_DOWNLOAD_DIAGNOSTICS,
        // ── Other ──
        crate::handler::methods::MDM_AUTH_NOTIFICATION,
        crate::handler::methods::IMAGE_GENERATION_SETTINGS,
    ]
}

/// Check if a method name is supported.
pub fn is_supported_method(method: &str) -> bool {
    supported_methods().contains(&method)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handler::Handler;
    use roo_app::{App, AppConfig};
    use roo_jsonrpc::types::Message;
    use serde_json::json;

    fn test_router() -> Router {
        let config = AppConfig {
            cwd: "/tmp/test".to_string(),
            mode: "code".to_string(),
            ..Default::default()
        };
        let handler = Handler::new(App::new(config));
        Router::new(handler)
    }

    #[tokio::test]
    async fn test_route_request() {
        let router = test_router();
        let request = Message::request(1, "ping", json!(null));
        let response = router.route(&request).await;
        assert_eq!(response.result.unwrap(), json!("pong"));
    }

    #[tokio::test]
    async fn test_route_batch() {
        let router = test_router();
        let messages = vec![
            Message::request(1, "initialize", json!(null)),
            Message::request(2, "ping", json!(null)),
            Message::request(3, "state/get", json!(null)),
        ];
        let responses = router.route_batch(&messages).await;
        assert_eq!(responses.len(), 3);
    }

    #[test]
    fn test_supported_methods() {
        let methods = supported_methods();
        assert!(methods.contains(&"initialize"));
        assert!(methods.contains(&"ping"));
        assert!(methods.contains(&"task/start"));
        assert!(methods.contains(&"state/get"));
    }

    #[test]
    fn test_is_supported_method() {
        assert!(is_supported_method("initialize"));
        assert!(is_supported_method("ping"));
        assert!(!is_supported_method("nonexistent"));
    }
}

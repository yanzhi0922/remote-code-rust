//! Global settings type definitions.
//!
//! Derived from `packages/types/src/global-settings.ts` (384 lines).
//! Defines 50+ settings fields for the Roo Code extension.

use serde::{Deserialize, Serialize};

/// Global settings for the Roo Code extension.
///
/// Source: `packages/types/src/global-settings.ts`
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalSettings {
    // --- Mode ---
    pub mode: Option<String>,

    // --- Auto-approval ---
    pub auto_approval_enabled: Option<bool>,
    pub auto_approval_max_requests: Option<u32>,
    pub auto_approval_max_error_count: Option<u32>,

    // --- Always allow ---
    pub always_allow_read_only: Option<bool>,
    pub always_allow_write: Option<bool>,
    pub always_allow_execute: Option<bool>,
    pub always_allow_mcp: Option<bool>,
    pub always_allow_mode_switch: Option<bool>,
    pub always_allow_subtasks: Option<bool>,
    pub always_allow_browser: Option<bool>,

    // --- Commands ---
    pub allowed_commands: Option<Vec<String>>,
    pub denied_commands: Option<Vec<String>>,
    pub command_execution_timeout: Option<u64>,
    pub command_timeout_allowlist: Option<Vec<String>>,

    // --- Custom instructions ---
    pub custom_instructions: Option<String>,

    // --- Task history ---
    pub task_history: Option<Vec<serde_json::Value>>,

    // --- Telemetry ---
    pub telemetry_setting: Option<String>,
    pub telemetry_key: Option<String>,

    // --- UI ---
    pub show_roo_mascot: Option<bool>,
    pub sound_enabled: Option<bool>,
    pub sound_volume: Option<f64>,
    pub max_open_tabs: Option<u32>,

    // --- TTS ---
    pub tts_enabled: Option<bool>,
    pub tts_speed: Option<f64>,

    // --- Code index ---
    pub code_index_enabled: Option<bool>,
    pub code_index_details: Option<serde_json::Value>,

    // --- Debug ---
    pub debug: Option<bool>,

    // --- Debug proxy ---
    pub debug_proxy_enabled: Option<bool>,
    pub debug_proxy_server_url: Option<String>,
    pub debug_proxy_tls_insecure: Option<bool>,

    // --- API ---
    pub api_request_timeout: Option<u64>,
    pub include_developer_docs: Option<bool>,

    // --- Misc ---
    pub prevent_completion_with_open_todos: Option<bool>,
    pub new_task_require_todos: Option<bool>,
    pub use_agent_rules: Option<bool>,
    pub custom_storage_path: Option<String>,
    pub auto_import_settings_path: Option<String>,
    pub maximum_indexed_files_for_file_search: Option<u32>,
    pub enable_code_actions: Option<bool>,
    pub vs_code_lm_model_selector: Option<serde_json::Value>,
    pub lock_api_config_across_modes: Option<bool>,
    pub pin_api_config: Option<bool>,
}

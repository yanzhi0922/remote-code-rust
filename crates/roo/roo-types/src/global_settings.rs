//! Global settings type definitions.
//!
//! Derived from `packages/types/src/global-settings.ts` (384 lines).
//! Defines 50+ settings fields for the Roo Code extension.

use serde::{Deserialize, Serialize};

use crate::mode::{CustomModePrompts, CustomSupportPrompts, ModeConfig};

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
    pub always_allow_read_only_outside_workspace: Option<bool>,
    pub always_allow_write: Option<bool>,
    pub always_allow_write_outside_workspace: Option<bool>,
    pub always_allow_write_protected: Option<bool>,
    pub always_allow_execute: Option<bool>,
    pub always_allow_mcp: Option<bool>,
    pub always_allow_mode_switch: Option<bool>,
    pub always_allow_subtasks: Option<bool>,
    pub always_allow_browser: Option<bool>,
    pub always_allow_followup_questions: Option<bool>,
    pub followup_auto_approve_timeout_ms: Option<u64>,
    pub request_delay_seconds: Option<u64>,
    pub allowed_max_requests: Option<u32>,
    pub allowed_max_cost: Option<f64>,

    // --- Commands ---
    pub allowed_commands: Option<Vec<String>>,
    pub denied_commands: Option<Vec<String>>,
    pub command_execution_timeout: Option<u64>,
    pub command_timeout_allowlist: Option<Vec<String>>,
    pub terminal_output_preview_size: Option<String>,
    pub terminal_shell_integration_timeout: Option<u64>,
    pub terminal_shell_integration_disabled: Option<bool>,
    pub terminal_command_delay: Option<u64>,
    pub execa_shell_path: Option<String>,

    // --- Custom instructions ---
    pub custom_instructions: Option<String>,
    pub custom_condensing_prompt: Option<String>,
    pub enable_subfolder_rules: Option<bool>,
    pub language: Option<String>,
    pub custom_modes: Option<Vec<ModeConfig>>,
    pub custom_mode_prompts: Option<CustomModePrompts>,
    pub custom_support_prompts: Option<CustomSupportPrompts>,
    pub disabled_tools: Option<Vec<String>>,

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
    pub mcp_enabled: Option<bool>,
    pub auto_condense_context: Option<bool>,
    pub auto_condense_context_percent: Option<u32>,
    pub include_current_time: Option<bool>,
    pub include_current_cost: Option<bool>,
    pub max_git_status_files: Option<u32>,
    pub include_diagnostic_messages: Option<bool>,
    pub max_diagnostic_messages: Option<u32>,
    pub enable_checkpoints: Option<bool>,
    pub checkpoint_timeout: Option<u32>,
    pub custom_storage_path: Option<String>,
    pub auto_import_settings_path: Option<String>,
    pub maximum_indexed_files_for_file_search: Option<u32>,
    pub enable_code_actions: Option<bool>,
    pub vs_code_lm_model_selector: Option<serde_json::Value>,
    pub lock_api_config_across_modes: Option<bool>,
    pub pin_api_config: Option<bool>,
    pub mode_api_configs: Option<std::collections::HashMap<String, String>>,
    pub enhancement_api_config_id: Option<String>,
    pub include_task_history_in_enhance: Option<bool>,
    pub history_preview_collapsed: Option<bool>,
    pub reasoning_block_collapsed: Option<bool>,
    pub enter_behavior: Option<String>,
    pub profile_thresholds: Option<std::collections::HashMap<String, f64>>,
    pub has_opened_mode_selector: Option<bool>,
    pub last_mode_export_path: Option<String>,
    pub last_mode_import_path: Option<String>,
    pub last_settings_export_path: Option<String>,
    pub last_task_export_path: Option<String>,
    pub last_image_save_path: Option<String>,
    pub worktree_auto_open_path: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deserializes_prompt_permission_and_mcp_settings_from_upstream_shape() {
        let settings: GlobalSettings = serde_json::from_value(json!({
            "mode": "code",
            "customInstructions": "Project-wide guidance",
            "enableSubfolderRules": true,
            "language": "pt-BR",
            "mcpEnabled": false,
            "alwaysAllowReadOnlyOutsideWorkspace": true,
            "alwaysAllowWriteProtected": false,
            "alwaysAllowFollowupQuestions": true,
            "followupAutoApproveTimeoutMs": 1200,
            "allowedMaxRequests": 5,
            "allowedMaxCost": 1.25,
            "disabledTools": ["execute_command"],
            "customModePrompts": {
                "code": {
                    "roleDefinition": "Override role",
                    "customInstructions": "Override instructions"
                }
            },
            "customSupportPrompts": {
                "enhance": "Improve this"
            },
            "customModes": [{
                "slug": "reviewer",
                "name": "Reviewer",
                "roleDefinition": "Review code",
                "groups": ["read"]
            }],
            "modeApiConfigs": {
                "code": "default"
            }
        }))
        .expect("upstream-compatible settings should deserialize");

        assert_eq!(settings.mode.as_deref(), Some("code"));
        assert_eq!(settings.language.as_deref(), Some("pt-BR"));
        assert_eq!(settings.mcp_enabled, Some(false));
        assert_eq!(settings.enable_subfolder_rules, Some(true));
        assert_eq!(
            settings.always_allow_read_only_outside_workspace,
            Some(true)
        );
        assert_eq!(settings.always_allow_followup_questions, Some(true));
        assert_eq!(settings.allowed_max_requests, Some(5));
        assert_eq!(
            settings.disabled_tools.as_ref().unwrap(),
            &vec!["execute_command".to_string()]
        );
        assert_eq!(settings.custom_modes.as_ref().map(Vec::len), Some(1));
        assert!(
            settings
                .custom_mode_prompts
                .as_ref()
                .and_then(|prompts| prompts.get("code"))
                .and_then(|component| component.as_ref())
                .and_then(|component| component.role_definition.as_deref())
                == Some("Override role")
        );
        assert_eq!(
            settings
                .custom_support_prompts
                .as_ref()
                .and_then(|prompts| prompts.get("enhance"))
                .and_then(|value| value.as_deref()),
            Some("Improve this")
        );
        assert_eq!(
            settings
                .mode_api_configs
                .as_ref()
                .and_then(|configs| configs.get("code"))
                .map(String::as_str),
            Some("default")
        );
    }

    #[test]
    fn serializes_new_settings_with_roo_camel_case_keys() {
        let settings = GlobalSettings {
            language: Some("zh-CN".to_string()),
            enable_subfolder_rules: Some(true),
            mcp_enabled: Some(true),
            always_allow_write_protected: Some(false),
            disabled_tools: Some(vec!["read_file".to_string()]),
            ..GlobalSettings::default()
        };

        let value = serde_json::to_value(settings).expect("settings should serialize");
        assert_eq!(value["language"], "zh-CN");
        assert_eq!(value["enableSubfolderRules"], true);
        assert_eq!(value["mcpEnabled"], true);
        assert_eq!(value["alwaysAllowWriteProtected"], false);
        assert_eq!(value["disabledTools"], json!(["read_file"]));
    }
}

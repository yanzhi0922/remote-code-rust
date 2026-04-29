//! Telemetry type definitions.
//!
//! Derived from `packages/types/src/telemetry.ts`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// TelemetrySetting
// ---------------------------------------------------------------------------

/// Telemetry preference setting.
///
/// Source: `packages/types/src/telemetry.ts` — `telemetrySettings`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TelemetrySetting {
    Unset,
    Enabled,
    Disabled,
}

// ---------------------------------------------------------------------------
// TelemetryEventName
// ---------------------------------------------------------------------------

/// All telemetry event names.
///
/// Source: `packages/types/src/telemetry.ts` — `TelemetryEventName`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TelemetryEventName {
    #[serde(rename = "Task Created")]
    TaskCreated,
    #[serde(rename = "Task Reopened")]
    TaskRestarted,
    #[serde(rename = "Task Completed")]
    TaskCompleted,
    #[serde(rename = "Task Message")]
    TaskMessage,
    #[serde(rename = "Conversation Message")]
    TaskConversationMessage,
    #[serde(rename = "LLM Completion")]
    LlmCompletion,
    #[serde(rename = "Mode Switched")]
    ModeSwitch,
    #[serde(rename = "Mode Selector Opened")]
    ModeSelectorOpened,
    #[serde(rename = "Tool Used")]
    ToolUsed,

    #[serde(rename = "Checkpoint Created")]
    CheckpointCreated,
    #[serde(rename = "Checkpoint Restored")]
    CheckpointRestored,
    #[serde(rename = "Checkpoint Diffed")]
    CheckpointDiffed,

    #[serde(rename = "Tab Shown")]
    TabShown,
    #[serde(rename = "Mode Setting Changed")]
    ModeSettingsChanged,
    #[serde(rename = "Custom Mode Created")]
    CustomModeCreated,

    #[serde(rename = "Context Condensed")]
    ContextCondensed,
    #[serde(rename = "Sliding Window Truncation")]
    SlidingWindowTruncation,

    #[serde(rename = "Code Action Used")]
    CodeActionUsed,
    #[serde(rename = "Prompt Enhanced")]
    PromptEnhanced,

    #[serde(rename = "Title Button Clicked")]
    TitleButtonClicked,

    #[serde(rename = "Authentication Initiated")]
    AuthenticationInitiated,

    #[serde(rename = "Marketplace Item Installed")]
    MarketplaceItemInstalled,
    #[serde(rename = "Marketplace Item Removed")]
    MarketplaceItemRemoved,
    #[serde(rename = "Marketplace Tab Viewed")]
    MarketplaceTabViewed,
    #[serde(rename = "Marketplace Install Button Clicked")]
    MarketplaceInstallButtonClicked,

    #[serde(rename = "Share Button Clicked")]
    ShareButtonClicked,
    #[serde(rename = "Share Organization Clicked")]
    ShareOrganizationClicked,
    #[serde(rename = "Share Public Clicked")]
    SharePublicClicked,
    #[serde(rename = "Share Connect To Cloud Clicked")]
    ShareConnectToCloudClicked,

    #[serde(rename = "Account Connect Clicked")]
    AccountConnectClicked,
    #[serde(rename = "Account Connect Success")]
    AccountConnectSuccess,
    #[serde(rename = "Account Logout Clicked")]
    AccountLogoutClicked,
    #[serde(rename = "Account Logout Success")]
    AccountLogoutSuccess,

    #[serde(rename = "Featured Provider Clicked")]
    FeaturedProviderClicked,

    #[serde(rename = "Upsell Dismissed")]
    UpsellDismissed,
    #[serde(rename = "Upsell Clicked")]
    UpsellClicked,

    #[serde(rename = "Schema Validation Error")]
    SchemaValidationError,
    #[serde(rename = "Diff Application Error")]
    DiffApplicationError,
    #[serde(rename = "Shell Integration Error")]
    ShellIntegrationError,
    #[serde(rename = "Consecutive Mistake Error")]
    ConsecutiveMistakeError,
    #[serde(rename = "Code Index Error")]
    CodeIndexError,
    #[serde(rename = "Telemetry Settings Changed")]
    TelemetrySettingsChanged,
    #[serde(rename = "Model Cache Empty Response")]
    ModelCacheEmptyResponse,
    #[serde(rename = "Read File Legacy Format Used")]
    ReadFileLegacyFormatUsed,
}

// ---------------------------------------------------------------------------
// AppProperties
// ---------------------------------------------------------------------------

/// Static app properties for telemetry.
///
/// Source: `packages/types/src/telemetry.ts` — `staticAppPropertiesSchema`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticAppProperties {
    pub app_name: String,
    pub app_version: String,
    pub vscode_version: String,
    pub platform: String,
    pub editor_name: String,
    pub hostname: Option<String>,
}

/// Dynamic app properties for telemetry.
///
/// Source: `packages/types/src/telemetry.ts` — `dynamicAppPropertiesSchema`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicAppProperties {
    pub language: String,
    pub mode: String,
}

/// Cloud app properties for telemetry.
///
/// Source: `packages/types/src/telemetry.ts` — `cloudAppPropertiesSchema`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudAppProperties {
    pub cloud_is_authenticated: Option<bool>,
}

/// Combined app properties.
///
/// Source: `packages/types/src/telemetry.ts` — `appPropertiesSchema`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppProperties {
    #[serde(flatten)]
    pub static_props: StaticAppProperties,
    pub language: String,
    pub mode: String,
    pub cloud_is_authenticated: Option<bool>,
}

// ---------------------------------------------------------------------------
// TaskProperties
// ---------------------------------------------------------------------------

/// Task properties for telemetry.
///
/// Source: `packages/types/src/telemetry.ts` — `taskPropertiesSchema`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProperties {
    pub task_id: Option<String>,
    pub parent_task_id: Option<String>,
    pub api_provider: Option<String>,
    pub model_id: Option<String>,
    pub diff_strategy: Option<String>,
    pub is_subtask: Option<bool>,
    pub todos: Option<TodoMetrics>,
}

/// Todo metrics for telemetry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TodoMetrics {
    pub total: u64,
    pub completed: u64,
    pub in_progress: u64,
    pub pending: u64,
}

// ---------------------------------------------------------------------------
// GitProperties
// ---------------------------------------------------------------------------

/// Git properties for telemetry.
///
/// Source: `packages/types/src/telemetry.ts` — `gitPropertiesSchema`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitProperties {
    pub repository_url: Option<String>,
    pub repository_name: Option<String>,
    pub default_branch: Option<String>,
}

// ---------------------------------------------------------------------------
// TelemetryProperties
// ---------------------------------------------------------------------------

/// Combined telemetry properties.
///
/// Source: `packages/types/src/telemetry.ts` — `telemetryPropertiesSchema`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryProperties {
    #[serde(flatten)]
    pub app: AppProperties,
    #[serde(flatten)]
    pub task: TaskProperties,
    #[serde(flatten)]
    pub git: GitProperties,
}

// ---------------------------------------------------------------------------
// TelemetryEvent
// ---------------------------------------------------------------------------

/// A telemetry event.
///
/// Source: `packages/types/src/telemetry.ts` — `TelemetryEvent`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEvent {
    pub event: TelemetryEventName,
    #[serde(default)]
    pub properties: Option<serde_json::Value>,
}

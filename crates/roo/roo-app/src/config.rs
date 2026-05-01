//! Application configuration.
//!
//! Source: `src/core/ClineProvider.ts` — constructor parameters and state initialization

use roo_types::provider_settings::ProviderSettings;

/// Configuration for creating an App instance.
///
/// Source: `src/core/ClineProvider.ts` — ClineProvider constructor options
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// The current working directory (project root).
    pub cwd: String,

    /// The global storage path for persistent data.
    pub global_storage_path: String,

    /// The current mode slug (e.g., "code", "architect", "ask").
    pub mode: String,

    /// The provider settings (API keys, model selection, etc.).
    pub provider_settings: ProviderSettings,

    /// Whether telemetry is enabled.
    pub telemetry_enabled: bool,

    /// The user's preferred language (e.g., "en", "zh-CN").
    pub language: Option<String>,

    /// Whether to enable checkpoints.
    pub checkpoints_enabled: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            cwd: std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string()),
            global_storage_path: String::new(),
            mode: "code".to_string(),
            provider_settings: ProviderSettings::default(),
            telemetry_enabled: false,
            language: None,
            checkpoints_enabled: true,
        }
    }
}

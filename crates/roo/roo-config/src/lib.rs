//! # roo-config — Configuration Management for Roo Code
//!
//! This crate provides configuration directory resolution, file system utilities,
//! configuration loading with merge support, settings import/export,
//! provider settings management, and settings migration.

pub mod auto_import_settings;
pub mod context_proxy;
pub mod error;
pub mod filesystem;
pub mod git_utils;
pub mod import_export;
pub mod loader;
pub mod migrate_settings;
pub mod network_proxy;
pub mod paths;
pub mod provider_settings_manager;
pub mod safe_write_json;

// Re-export key types and functions
pub use context_proxy::{
    ContextProxy, GLOBAL_SECRET_KEYS, GLOBAL_STATE_KEYS, InMemoryStateStore,
    PASS_THROUGH_STATE_KEYS, SECRET_STATE_KEYS, StateStore, is_pass_through_state_key,
};
pub use error::ConfigError;
pub use filesystem::{directory_exists, file_exists, read_file_if_exists};
pub use import_export::{
    ExportData, ImportExportError, ImportResult, ProviderProfiles, export_settings,
    import_settings_from_path, sanitize_provider_config,
};
pub use loader::{
    LoadedConfiguration, build_merged_content, load_configuration, load_roo_configuration,
};
pub use migrate_settings::{
    FileMigration, MigrationError, default_file_migrations, migrate_custom_modes_to_yaml,
    migrate_default_commands, migrate_settings,
};
pub use network_proxy::{NetworkProxy, ProxyConfig, ProxyProtocol, redact_proxy_url};
pub use paths::{
    discover_subfolder_roo_directories, get_agents_directories_for_cwd,
    get_all_roo_directories_for_cwd, get_global_agents_directory, get_global_roo_directory,
    get_project_agents_directory_for_cwd, get_project_roo_directory_for_cwd,
    get_roo_directories_for_cwd,
};
pub use provider_settings_manager::{
    MigrationState, ProviderProfiles as ProviderSettingsProfiles, ProviderSettingsError,
    ProviderSettingsManager, ProviderSettingsWithId,
};
pub use safe_write_json::{SafeWriteJsonError, SafeWriteJsonOptions, safe_write_json};

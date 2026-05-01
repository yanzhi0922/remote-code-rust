//! # rc-settings — Settings Schema, Validation, and Layered Loading
//!
//! Corresponds to `src/utils/settings/types.ts` (~1,187 lines).
//! Provides the full settings schema with typed structs, validation,
//! layered loading (global → project → local → managed), and merge logic.
//!
//! ## Settings Layers
//! 1. **Global** (`~/.claude/settings.json`)
//! 2. **Project** (`.claude/settings.json`)
//! 3. **Local** (`.claude/settings.local.json`)
//! 4. **Managed/Policy** (enterprise-managed settings)
//!
//! ## Example
//! ```ignore
//! use claude_settings::{Settings, load_settings_from_file};
//!
//! let settings = load_settings_from_file("settings.json")?;
//! println!("Model: {:?}", settings.model);
//! ```

pub mod attribution;
pub mod hooks;
pub mod loader;
pub mod mcp;
pub mod merge;
pub mod permissions;
pub mod provider;
pub mod sandbox;
pub mod types;
pub mod validation;
pub mod worktree;

pub use attribution::AttributionSettings;
pub use hooks::{
    AgentHookConfig, BashCommandHookConfig, HookCommandConfig, HookEntry, HookMatcherConfig,
    HookSettings, HookShellType, HttpHookConfig, PromptHookConfig,
};
pub use loader::{load_settings_from_file, load_settings_from_str};
pub use mcp::{AllowedMcpServerEntry, DeniedMcpServerEntry, McpServerEntryMatcher};
pub use merge::{SettingsLayer, merge_settings};
pub use permissions::PermissionSettings;
pub use provider::{ProviderConfig, ProviderType};
pub use sandbox::SandboxSettings;
pub use types::{CustomizationSurface, Settings, SpinnerConfig, SpinnerVerbMode};
pub use validation::validate_settings;
pub use worktree::WorktreeSettings;

//! # Roo Modes
//!
//! Higher-level mode operation logic for Roo Code.
//!
//! Provides mode selection, tool mapping, helper functions, and file restriction
//! checking. Derived from `src/shared/modes.ts`.

pub mod custom_modes_manager;
pub mod helpers;
pub mod restriction;
pub mod selection;
pub mod tools;

// Re-export key types and functions
pub use custom_modes_manager::{
    CACHE_TTL_MS, CustomModesManager, ExportResult, ExportedModeConfig, ImportResult,
    ROOMODES_FILENAME, RuleFile,
};
pub use helpers::{
    default_mode_slug, default_prompts, find_mode_by_slug, get_description, get_when_to_use,
    is_custom_mode,
};
pub use restriction::{FileRestrictionError, check_file_restriction};
pub use selection::{ModeSelection, get_mode_selection};
pub use tools::{get_group_name, get_tools_for_mode};

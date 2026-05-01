//! # Roo Tools
//!
//! Tool registration, definition, validation, and filtering for Roo Code.
//!
//! This crate implements the complete tool system including:
//! - **Tool definitions** in OpenAI Function Calling format
//! - **Tool group configuration** (TOOL_GROUPS, ALWAYS_AVAILABLE_TOOLS, TOOL_ALIASES)
//! - **Tool validation** for mode restrictions
//! - **Tool filtering** based on mode, experiments, and settings
//! - **Tool array building** for API requests
//! - **Repetition detection** to prevent infinite loops

pub mod build;
pub mod definition;
pub mod filter;
pub mod groups;
pub mod repetition;
pub mod tag_matcher;
pub mod validate;

// Re-export key types
pub use build::{BuildToolsOptions, BuildToolsResult, build_native_tools_array, build_native_tools_array_with_restrictions};
pub use definition::{NativeToolsOptions, ToolDefinition, find_tool_by_name, get_native_tools};
pub use filter::{FilterSettings, ModelToolInfo, filter_native_tools_for_mode, is_tool_allowed_in_mode};
pub use groups::{
    ToolGroupConfig, ALWAYS_AVAILABLE_TOOLS, TOOL_ALIASES, TOOL_DISPLAY_NAMES, TOOL_GROUPS,
    get_group_name, get_group_options, get_tools_for_mode, is_always_available, resolve_tool_alias,
};
pub use repetition::ToolRepetitionDetector;
pub use validate::{
    ToolValidationError, is_tool_allowed_for_mode, is_valid_tool_name, validate_tool_use,
};

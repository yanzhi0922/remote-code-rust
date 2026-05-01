//! # Roo Prompt
//!
//! System prompt generation for the Roo Code Rust project.
//!
//! This crate provides the system prompt builder and response formatting
//! utilities. All prompt text is derived directly from the TypeScript source
//! at `src/core/prompts/` and matches it verbatim.

pub mod sections;
pub mod system;
pub mod types;
pub mod responses;

// Re-export the main public API
pub use system::{build_system_prompt, generate_system_prompt, get_prompt_component};
pub use types::{FileEntry, SkillInfo, SystemPromptParams, SystemPromptSettings};
pub use responses::{
    create_pretty_patch,
    format_files_list,
    image_blocks,
    invalid_mcp_tool_argument_error,
    missing_tool_parameter_error,
    no_tools_used,
    roo_ignore_error,
    tool_approved_with_feedback,
    tool_denied,
    tool_denied_with_feedback,
    tool_error,
    tool_result,
    too_many_mistakes,
    unknown_mcp_server_error,
    unknown_mcp_tool_error,
    ContentBlock,
    ImageSource,
    ToolResult,
};

// Re-export section functions
pub use sections::{
    add_custom_instructions,
    get_capabilities_section,
    get_command_chain_operator,
    get_modes_section,
    get_objective_section,
    get_rules_section,
    get_shared_tool_use_section,
    get_skills_section,
    get_system_info_section,
    get_tool_use_guidelines_section,
    load_rule_files,
    markdown_formatting_section,
};

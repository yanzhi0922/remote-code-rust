//! Prompt sections module.
//!
//! Re-exports all section generators.
//! Source: `src/core/prompts/sections/index.ts`

pub mod capabilities;
pub mod custom_instructions;
pub mod markdown_formatting;
pub mod modes;
pub mod objective;
pub mod rules;
pub mod skills;
pub mod system_info;
pub mod tool_use;
pub mod tool_use_guidelines;

pub use capabilities::get_capabilities_section;
pub use custom_instructions::{add_custom_instructions, load_rule_files};
pub use markdown_formatting::markdown_formatting_section;
pub use modes::get_modes_section;
pub use objective::get_objective_section;
pub use rules::{get_command_chain_operator, get_rules_section};
pub use skills::get_skills_section;
pub use system_info::get_system_info_section;
pub use tool_use::get_shared_tool_use_section;
pub use tool_use_guidelines::get_tool_use_guidelines_section;

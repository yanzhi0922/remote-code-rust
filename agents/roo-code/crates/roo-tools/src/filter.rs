//! Tool filtering for mode restrictions.
//!
//! Corresponds to `src/core/prompts/tools/filter-tools-for-mode.ts`.

use std::collections::{HashMap, HashSet};

use roo_types::mode::ModeConfig;
use roo_types::tool::ToolName;

use crate::definition::ToolDefinition;
use crate::groups::{get_tools_for_mode, is_always_available, resolve_tool_alias, TOOL_GROUPS};
use crate::validate::is_tool_allowed_for_mode;

// ---------------------------------------------------------------------------
// FilterSettings
// ---------------------------------------------------------------------------

/// Settings that affect tool filtering.
#[derive(Debug, Clone, Default)]
pub struct FilterSettings {
    /// Whether the todo list feature is enabled.
    pub todo_list_enabled: bool,
    /// Tools that are explicitly disabled.
    pub disabled_tools: Vec<String>,
    /// Model-specific tool customization info.
    pub model_info: Option<ModelToolInfo>,
    /// Whether codebase search (code index) is available.
    pub codebase_search_enabled: bool,
    /// Whether MCP resources are available.
    pub mcp_resources_available: bool,
}

/// Model-specific tool customization.
#[derive(Debug, Clone, Default)]
pub struct ModelToolInfo {
    /// Tools to exclude from the allowed set.
    pub excluded_tools: Vec<String>,
    /// Tools to include (opt-in custom tools).
    pub included_tools: Vec<String>,
}

// ---------------------------------------------------------------------------
// filter_native_tools_for_mode
// ---------------------------------------------------------------------------

/// Filters native tools based on mode restrictions and model customization.
///
/// Source: `src/core/prompts/tools/filter-tools-for-mode.ts` — `filterNativeToolsForMode`
pub fn filter_native_tools_for_mode(
    tools: &[ToolDefinition],
    mode: Option<&str>,
    custom_modes: &[ModeConfig],
    experiments: Option<&HashMap<String, bool>>,
    settings: &FilterSettings,
) -> Vec<ToolDefinition> {
    let mode_slug = mode.unwrap_or("code");

    // Find mode configuration
    let mode_config = find_mode_by_slug(mode_slug, custom_modes)
        .or_else(|| find_mode_by_slug("code", custom_modes));

    let Some(mode_config) = mode_config else {
        tracing::warn!("No mode config found for {mode_slug}, returning empty tools");
        return vec![];
    };

    // Get all tools for this mode
    let all_tools_for_mode = get_tools_for_mode(&mode_config.groups);

    // Filter to only tools that pass permission checks
    let mut allowed_tool_names: HashSet<String> = all_tools_for_mode
        .iter()
        .filter(|tool| {
            is_tool_allowed_for_mode(
                tool.as_str(),
                mode_slug,
                custom_modes,
                None,
                None,
                experiments,
                None,
            )
        })
        .map(|t| t.as_str().to_string())
        .collect();

    // Apply model-specific tool customization
    if let Some(ref model_info) = settings.model_info {
        // Remove excluded tools
        for tool in &model_info.excluded_tools {
            let resolved = resolve_tool_alias(tool);
            allowed_tool_names.remove(&resolved);
        }

        // Add included tools only if they belong to an allowed group
        let allowed_groups: HashSet<roo_types::tool::ToolGroup> = mode_config
            .groups
            .iter()
            .map(|g| match g {
                roo_types::tool::GroupEntry::Plain(g) => *g,
                roo_types::tool::GroupEntry::WithOptions(g, _) => *g,
            })
            .collect();

        for tool in &model_info.included_tools {
            let resolved = resolve_tool_alias(tool);
            // Check if the tool belongs to an allowed group
            if let Some(group) = find_tool_group(&resolved) {
                if allowed_groups.contains(&group) {
                    allowed_tool_names.insert(resolved);
                }
            }
        }
    }

    // Conditionally exclude codebase_search if feature is disabled
    if !settings.codebase_search_enabled {
        allowed_tool_names.remove("codebase_search");
    }

    // Conditionally exclude update_todo_list if disabled
    if !settings.todo_list_enabled {
        allowed_tool_names.remove("update_todo_list");
    }

    // Conditionally exclude generate_image if experiment is not enabled
    if !experiments
        .and_then(|e| e.get("imageGeneration"))
        .copied()
        .unwrap_or(false)
    {
        allowed_tool_names.remove("generate_image");
    }

    // Conditionally exclude run_slash_command if experiment is not enabled
    if !experiments
        .and_then(|e| e.get("runSlashCommand"))
        .copied()
        .unwrap_or(false)
    {
        allowed_tool_names.remove("run_slash_command");
    }

    // Remove tools that are explicitly disabled
    for tool_name in &settings.disabled_tools {
        let resolved = resolve_tool_alias(tool_name);
        allowed_tool_names.remove(&resolved);
    }

    // Conditionally exclude access_mcp_resource if no MCP resources
    if !settings.mcp_resources_available {
        allowed_tool_names.remove("access_mcp_resource");
    }

    // Filter native tools based on allowed tool names
    tools
        .iter()
        .filter(|tool| allowed_tool_names.contains(&tool.name))
        .cloned()
        .collect()
}

/// Find which tool group a tool belongs to.
fn find_tool_group(tool_name: &str) -> Option<roo_types::tool::ToolGroup> {
    for (group, config) in TOOL_GROUPS.iter() {
        if config.tools.iter().any(|t| t.as_str() == tool_name)
            || config.custom_tools.iter().any(|t| t.as_str() == tool_name)
        {
            return Some(*group);
        }
    }
    None
}

/// Find a mode configuration by slug.
fn find_mode_by_slug(slug: &str, custom_modes: &[ModeConfig]) -> Option<ModeConfig> {
    custom_modes
        .iter()
        .find(|m| m.slug == slug)
        .cloned()
        .or_else(|| roo_types::mode::default_modes().into_iter().find(|m| m.slug == slug))
}

/// Checks if a specific tool is allowed in the current mode.
///
/// Source: `src/core/prompts/tools/filter-tools-for-mode.ts` — `isToolAllowedInMode`
pub fn is_tool_allowed_in_mode(
    tool_name: &str,
    mode: Option<&str>,
    custom_modes: &[ModeConfig],
    experiments: Option<&HashMap<String, bool>>,
    settings: &FilterSettings,
) -> bool {
    let mode_slug = mode.unwrap_or("code");

    // Check if it's an always-available tool
    if let Some(tool) = ToolName::all().iter().find(|t| t.as_str() == tool_name) {
        if is_always_available(tool) {
            // Still check conditional exclusions
            if tool_name == "update_todo_list" {
                return settings.todo_list_enabled;
            }
            if tool_name == "generate_image" {
                return experiments
                    .and_then(|e| e.get("imageGeneration"))
                    .copied()
                    .unwrap_or(false);
            }
            if tool_name == "run_slash_command" {
                return experiments
                    .and_then(|e| e.get("runSlashCommand"))
                    .copied()
                    .unwrap_or(false);
            }
            return true;
        }
    }

    // Resolve alias and check mode permissions
    let canonical_tool = resolve_tool_alias(tool_name);
    is_tool_allowed_for_mode(
        &canonical_tool,
        mode_slug,
        custom_modes,
        None,
        None,
        experiments,
        None,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_settings() -> FilterSettings {
        FilterSettings {
            todo_list_enabled: true,
            codebase_search_enabled: true,
            mcp_resources_available: true,
            ..Default::default()
        }
    }

    #[test]
    fn test_filter_code_mode() {
        let tools = crate::definition::get_native_tools(Default::default());
        let filtered = filter_native_tools_for_mode(
            &tools,
            Some("code"),
            &[],
            None,
            &default_settings(),
        );
        // Code mode should have read, edit, command tools
        let names: Vec<&str> = filtered.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"write_to_file"));
        assert!(names.contains(&"execute_command"));
        assert!(names.contains(&"apply_diff"));
        // Always available
        assert!(names.contains(&"ask_followup_question"));
        assert!(names.contains(&"attempt_completion"));
    }

    #[test]
    fn test_filter_architect_mode() {
        let tools = crate::definition::get_native_tools(Default::default());
        let filtered = filter_native_tools_for_mode(
            &tools,
            Some("architect"),
            &[],
            None,
            &default_settings(),
        );
        let names: Vec<&str> = filtered.iter().map(|t| t.name.as_str()).collect();
        // Architect mode has read and edit (for .md files), but not command
        assert!(names.contains(&"read_file"));
        // Architect has edit group (with .md restriction), so write_to_file is present
        assert!(names.contains(&"write_to_file"));
        assert!(!names.contains(&"execute_command"));
    }

    #[test]
    fn test_filter_ask_mode() {
        let tools = crate::definition::get_native_tools(Default::default());
        let filtered = filter_native_tools_for_mode(
            &tools,
            Some("ask"),
            &[],
            None,
            &default_settings(),
        );
        let names: Vec<&str> = filtered.iter().map(|t| t.name.as_str()).collect();
        // Ask mode has read + mcp groups + always-available tools
        assert!(names.contains(&"ask_followup_question"));
        assert!(names.contains(&"attempt_completion"));
        assert!(names.contains(&"read_file")); // Ask has read group
        assert!(!names.contains(&"execute_command"));
        assert!(!names.contains(&"write_to_file"));
    }

    #[test]
    fn test_filter_disabled_tools() {
        let tools = crate::definition::get_native_tools(Default::default());
        let settings = FilterSettings {
            disabled_tools: vec!["read_file".to_string()],
            ..default_settings()
        };
        let filtered =
            filter_native_tools_for_mode(&tools, Some("code"), &[], None, &settings);
        let names: Vec<&str> = filtered.iter().map(|t| t.name.as_str()).collect();
        assert!(!names.contains(&"read_file"));
    }

    #[test]
    fn test_filter_todo_list_disabled() {
        let tools = crate::definition::get_native_tools(Default::default());
        let settings = FilterSettings {
            todo_list_enabled: false,
            ..default_settings()
        };
        let filtered =
            filter_native_tools_for_mode(&tools, Some("code"), &[], None, &settings);
        let names: Vec<&str> = filtered.iter().map(|t| t.name.as_str()).collect();
        assert!(!names.contains(&"update_todo_list"));
    }

    #[test]
    fn test_filter_codebase_search_disabled() {
        let tools = crate::definition::get_native_tools(Default::default());
        let settings = FilterSettings {
            codebase_search_enabled: false,
            ..default_settings()
        };
        let filtered =
            filter_native_tools_for_mode(&tools, Some("code"), &[], None, &settings);
        let names: Vec<&str> = filtered.iter().map(|t| t.name.as_str()).collect();
        assert!(!names.contains(&"codebase_search"));
    }

    #[test]
    fn test_is_tool_allowed_in_mode() {
        let settings = default_settings();
        assert!(is_tool_allowed_in_mode(
            "ask_followup_question",
            Some("ask"),
            &[],
            None,
            &settings,
        ));
        assert!(is_tool_allowed_in_mode(
            "attempt_completion",
            Some("ask"),
            &[],
            None,
            &settings,
        ));
    }
}

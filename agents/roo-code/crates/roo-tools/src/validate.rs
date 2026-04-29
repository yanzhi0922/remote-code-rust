//! Tool validation logic.
//!
//! Corresponds to `src/core/tools/validateToolUse.ts`.

use std::collections::HashMap;

use roo_types::mode::ModeConfig;
use roo_types::tool::{ToolGroup, ToolName};

use crate::groups::{
    get_group_name, get_group_options, is_always_available, resolve_tool_alias, TOOL_GROUPS,
};

// ---------------------------------------------------------------------------
// Error types
// ---------------------------------------------------------------------------

/// Errors that can occur during tool validation.
#[derive(Debug, thiserror::Error)]
pub enum ToolValidationError {
    /// The tool name is not recognized.
    #[error("Unknown tool \"{0}\". This tool does not exist. Please use one of the available tools.")]
    UnknownTool(String),

    /// The tool is not allowed in the current mode.
    #[error("Tool \"{tool}\" is not allowed in {mode} mode.")]
    NotAllowedForMode {
        tool: String,
        mode: String,
    },

    /// The file path does not match the mode's file regex restriction.
    #[error("File path \"{file_path}\" does not match the allowed pattern \"{regex}\" for {mode_name} mode. {description}")]
    FileRestriction {
        mode_name: String,
        regex: String,
        description: String,
        file_path: String,
        tool: String,
    },
}

// ---------------------------------------------------------------------------
// Validation functions
// ---------------------------------------------------------------------------

/// Checks if a tool name is a valid, known tool.
/// This does NOT check if the tool is allowed for a specific mode,
/// only that the tool actually exists.
///
/// Source: `src/core/tools/validateToolUse.ts` — `isValidToolName`
pub fn is_valid_tool_name(
    tool_name: &str,
    experiments: Option<&HashMap<String, bool>>,
) -> bool {
    // Check if it's a valid static tool
    if ToolName::all().iter().any(|t| t.as_str() == tool_name) {
        return true;
    }

    // Check custom tools experiment
    if let Some(exp) = experiments {
        if exp.get("customTools").copied().unwrap_or(false) && tool_name == "custom_tool" {
            return true;
        }
    }

    // Check if it's a dynamic MCP tool (mcp_serverName_toolName format)
    if tool_name.starts_with("mcp_") {
        return true;
    }

    false
}

/// Validates that a tool can be used in the given mode.
///
/// Source: `src/core/tools/validateToolUse.ts` — `validateToolUse`
pub fn validate_tool_use(
    tool_name: &str,
    mode: &str,
    custom_modes: &[ModeConfig],
    tool_requirements: Option<&HashMap<String, bool>>,
    tool_params: Option<&serde_json::Value>,
    experiments: Option<&HashMap<String, bool>>,
) -> Result<(), ToolValidationError> {
    // First, check if the tool name is actually valid/known
    if !is_valid_tool_name(tool_name, experiments) {
        let valid_names: Vec<&str> = ToolName::all().iter().map(|t| t.as_str()).collect();
        return Err(ToolValidationError::UnknownTool(format!(
            "{tool_name}. Available tools: {}",
            valid_names.join(", ")
        )));
    }

    // Then check if the tool is allowed for the current mode
    if !is_tool_allowed_for_mode(
        tool_name,
        mode,
        custom_modes,
        tool_requirements,
        tool_params,
        experiments,
        None,
    ) {
        return Err(ToolValidationError::NotAllowedForMode {
            tool: tool_name.to_string(),
            mode: mode.to_string(),
        });
    }

    Ok(())
}

/// Edit operation parameters that indicate an actual edit is happening.
const EDIT_OPERATION_PARAMS: &[&str] = &[
    "diff",
    "content",
    "operations",
    "search",
    "replace",
    "args",
    "line",
    "patch",
    "old_string",
    "new_string",
];

/// Patch file markers used to identify file operations in apply_patch format.
const PATCH_FILE_MARKERS: &[&str] = &[
    "*** Add File: ",
    "*** Delete File: ",
    "*** Update File: ",
];

/// Extract file paths from apply_patch content.
fn extract_file_paths_from_patch(patch_content: &str) -> Vec<String> {
    let mut file_paths = Vec::new();
    for line in patch_content.lines() {
        for marker in PATCH_FILE_MARKERS {
            if let Some(rest) = line.strip_prefix(marker) {
                let path = rest.trim();
                if !path.is_empty() {
                    file_paths.push(path.to_string());
                }
                break;
            }
        }
    }
    file_paths
}

/// Check if a file path matches a regex pattern.
fn does_file_match_regex(file_path: &str, pattern: &str) -> bool {
    match regex::Regex::new(pattern) {
        Ok(re) => re.is_match(file_path),
        Err(e) => {
            tracing::error!("Invalid regex pattern: {pattern}: {e}");
            false
        }
    }
}

/// Checks if a tool is allowed for a specific mode.
///
/// Source: `src/core/tools/validateToolUse.ts` — `isToolAllowedForMode`
pub fn is_tool_allowed_for_mode(
    tool: &str,
    mode_slug: &str,
    custom_modes: &[ModeConfig],
    tool_requirements: Option<&HashMap<String, bool>>,
    tool_params: Option<&serde_json::Value>,
    _experiments: Option<&HashMap<String, bool>>,
    included_tools: Option<&[String]>,
) -> bool {
    // Resolve alias to canonical name
    let resolved_tool = resolve_tool_alias(tool);
    let resolved_included: Option<Vec<String>> =
        included_tools.map(|tools| tools.iter().map(|t| resolve_tool_alias(t)).collect());

    // Check tool requirements first — explicit disabling takes priority
    if let Some(reqs) = tool_requirements {
        if let Some(enabled) = reqs.get(tool) {
            if !enabled {
                return false;
            }
        }
        if let Some(enabled) = reqs.get(&resolved_tool) {
            if !enabled {
                return false;
            }
        }
    }

    // Always allow these tools (unless explicitly disabled above)
    if let Some(tool_name) = ToolName::all().iter().find(|t| t.as_str() == tool) {
        if is_always_available(tool_name) {
            return true;
        }
    }

    // Check if this is a dynamic MCP tool
    let is_dynamic_mcp_tool = tool.starts_with("mcp_") && tool != "mcp_server";

    // Find the mode configuration
    let mode_config = match find_mode_by_slug(mode_slug, custom_modes) {
        Some(m) => m,
        None => return false,
    };

    // Check if tool is in any of the mode's groups
    for group in &mode_config.groups {
        let group_name = get_group_name(group);
        let options = get_group_options(group);

        let group_config = match TOOL_GROUPS.get(&group_name) {
            Some(c) => c,
            None => continue,
        };

        // Check if this is a dynamic MCP tool and the mcp group is allowed
        if is_dynamic_mcp_tool && group_name == ToolGroup::Mcp {
            return true;
        }

        // Check if the tool is in the group's regular tools
        let is_regular_tool = group_config
            .tools
            .iter()
            .any(|t| t.as_str() == resolved_tool);

        // Check if the tool is a custom tool that has been explicitly included
        let is_custom_tool = group_config
            .custom_tools
            .iter()
            .any(|t| t.as_str() == resolved_tool)
            && resolved_included
                .as_ref()
                .map(|inc| inc.iter().any(|t| t == &resolved_tool))
                .unwrap_or(false);

        if !is_regular_tool && !is_custom_tool {
            continue;
        }

        // If there are no options, allow the tool
        let Some(opts) = options else {
            return true;
        };

        // For the edit group, check file regex if specified
        if group_name == ToolGroup::Edit {
            if let Some(ref file_regex) = opts.file_regex {
                if let Some(params) = tool_params {
                    let file_path = params
                        .get("path")
                        .or_else(|| params.get("file_path"))
                        .and_then(|v| v.as_str());

                    let is_edit_operation = EDIT_OPERATION_PARAMS
                        .iter()
                        .any(|param| params.get(param).is_some());

                    if let (Some(fp), true) = (file_path, is_edit_operation) {
                        if !does_file_match_regex(fp, file_regex) {
                            tracing::warn!(
                                "File restriction: {} does not match {}",
                                fp,
                                file_regex
                            );
                            return false;
                        }
                    }

                    // Handle apply_patch: extract file paths from patch content
                    if tool == "apply_patch" {
                        if let Some(patch) = params.get("patch").and_then(|v| v.as_str()) {
                            for patch_fp in extract_file_paths_from_patch(patch) {
                                if !does_file_match_regex(&patch_fp, file_regex) {
                                    tracing::warn!(
                                        "File restriction: {} does not match {}",
                                        patch_fp,
                                        file_regex
                                    );
                                    return false;
                                }
                            }
                        }
                    }
                }
            }
        }

        return true;
    }

    false
}

/// Find a mode configuration by slug.
fn find_mode_by_slug<'a>(
    slug: &str,
    custom_modes: &'a [ModeConfig],
) -> Option<ModeConfig> {
    // Check custom modes first
    if let Some(mode) = custom_modes.iter().find(|m| m.slug == slug) {
        return Some(mode.clone());
    }
    // Check built-in modes
    roo_types::mode::default_modes()
        .into_iter()
        .find(|m| m.slug == slug)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_valid_tool_name_known_tools() {
        assert!(is_valid_tool_name("execute_command", None));
        assert!(is_valid_tool_name("read_file", None));
        assert!(is_valid_tool_name("write_to_file", None));
        assert!(is_valid_tool_name("ask_followup_question", None));
        assert!(is_valid_tool_name("attempt_completion", None));
    }

    #[test]
    fn test_is_valid_tool_name_mcp_tools() {
        assert!(is_valid_tool_name("mcp_server_tool", None));
        assert!(is_valid_tool_name("mcp_weather_getForecast", None));
    }

    #[test]
    fn test_is_valid_tool_name_unknown() {
        assert!(!is_valid_tool_name("unknown_tool", None));
        assert!(!is_valid_tool_name("nonexistent", None));
    }

    #[test]
    fn test_validate_tool_use_allowed() {
        // Code mode has read, edit, command groups
        let result = validate_tool_use(
            "read_file",
            "code",
            &[],
            None,
            None,
            None,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_tool_use_unknown_tool() {
        let result = validate_tool_use(
            "nonexistent_tool",
            "code",
            &[],
            None,
            None,
            None,
        );
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolValidationError::UnknownTool(_)));
    }

    #[test]
    fn test_validate_tool_use_not_allowed_for_mode() {
        // Ask mode should not have execute_command
        let result = validate_tool_use(
            "execute_command",
            "ask",
            &[],
            None,
            None,
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_always_available_tools_pass() {
        let result = validate_tool_use(
            "ask_followup_question",
            "ask",
            &[],
            None,
            None,
            None,
        );
        assert!(result.is_ok());

        let result = validate_tool_use(
            "attempt_completion",
            "ask",
            &[],
            None,
            None,
            None,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_tool_requirements_disable() {
        let mut reqs = HashMap::new();
        reqs.insert("ask_followup_question".to_string(), false);

        assert!(!is_tool_allowed_for_mode(
            "ask_followup_question",
            "code",
            &[],
            Some(&reqs),
            None,
            None,
            None,
        ));
    }

    #[test]
    fn test_tool_requirements_enable() {
        let mut reqs = HashMap::new();
        reqs.insert("read_file".to_string(), true);

        assert!(is_tool_allowed_for_mode(
            "read_file",
            "code",
            &[],
            Some(&reqs),
            None,
            None,
            None,
        ));
    }

    #[test]
    fn test_dynamic_mcp_tool_allowed() {
        assert!(is_tool_allowed_for_mode(
            "mcp_server_tool",
            "code",
            &[],
            None,
            None,
            None,
            None,
        ));
    }

    #[test]
    fn test_extract_file_paths_from_patch() {
        let patch = "*** Begin Patch\n*** Add File: hello.txt\n+Hello world\n*** Update File: src/app.py\n@@ def greet():\n-print(\"Hi\")\n+print(\"Hello!\")\n*** End Patch";
        let paths = extract_file_paths_from_patch(patch);
        assert_eq!(paths, vec!["hello.txt", "src/app.py"]);
    }

    #[test]
    fn test_code_mode_allows_read_edit_command() {
        assert!(is_tool_allowed_for_mode("read_file", "code", &[], None, None, None, None));
        assert!(is_tool_allowed_for_mode("write_to_file", "code", &[], None, None, None, None));
        assert!(is_tool_allowed_for_mode("execute_command", "code", &[], None, None, None, None));
        assert!(is_tool_allowed_for_mode("apply_diff", "code", &[], None, None, None, None));
    }

    #[test]
    fn test_architect_mode_allows_read_and_md_edit_but_not_command() {
        assert!(is_tool_allowed_for_mode("read_file", "architect", &[], None, None, None, None));
        // Architect has edit group with file_regex "\.md$", so write_to_file is allowed
        // but only for .md files. Without file params, it's still allowed by group membership.
        assert!(is_tool_allowed_for_mode("write_to_file", "architect", &[], None, None, None, None));
        assert!(!is_tool_allowed_for_mode("execute_command", "architect", &[], None, None, None, None));
    }
}

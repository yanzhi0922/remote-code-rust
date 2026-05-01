//! Build the complete tools array for native protocol requests.
//!
//! Corresponds to `src/core/task/build-tools.ts`.

use std::collections::HashMap;

use roo_types::mode::ModeConfig;

use crate::definition::{get_native_tools, NativeToolsOptions, ToolDefinition};
use crate::filter::{filter_native_tools_for_mode, FilterSettings};

// ---------------------------------------------------------------------------
// BuildToolsOptions
// ---------------------------------------------------------------------------

/// Options for building the tools array.
#[derive(Debug, Clone)]
pub struct BuildToolsOptions {
    /// Current working directory.
    pub cwd: String,
    /// Current mode slug.
    pub mode: Option<String>,
    /// Custom mode configurations.
    pub custom_modes: Vec<ModeConfig>,
    /// Experiment flags.
    pub experiments: HashMap<String, bool>,
    /// Tools that are explicitly disabled.
    pub disabled_tools: Vec<String>,
    /// Whether to include all tools with restrictions.
    pub include_all_tools_with_restrictions: bool,
    /// Whether the todo list feature is enabled.
    pub todo_list_enabled: bool,
    /// Whether the model supports images.
    pub supports_images: bool,
    /// Whether codebase search is available.
    pub codebase_search_enabled: bool,
    /// Whether MCP resources are available.
    pub mcp_resources_available: bool,
}

impl Default for BuildToolsOptions {
    fn default() -> Self {
        Self {
            cwd: String::new(),
            mode: None,
            custom_modes: vec![],
            experiments: HashMap::new(),
            disabled_tools: vec![],
            include_all_tools_with_restrictions: false,
            todo_list_enabled: true,
            supports_images: false,
            codebase_search_enabled: false,
            mcp_resources_available: false,
        }
    }
}

// ---------------------------------------------------------------------------
// BuildToolsResult
// ---------------------------------------------------------------------------

/// Result of building the tools array.
#[derive(Debug, Clone)]
pub struct BuildToolsResult {
    /// The tools to pass to the model.
    pub tools: Vec<ToolDefinition>,
    /// The names of tools that are allowed based on mode restrictions.
    /// Only populated when `include_all_tools_with_restrictions` is true.
    pub allowed_function_names: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// Build functions
// ---------------------------------------------------------------------------

/// Builds the complete tools array for native protocol requests.
/// Combines native tools filtered by mode restrictions.
///
/// Source: `src/core/task/build-tools.ts` — `buildNativeToolsArray`
pub fn build_native_tools_array(options: BuildToolsOptions) -> Vec<ToolDefinition> {
    let result = build_native_tools_array_with_restrictions(options);
    result.tools
}

/// Builds the complete tools array with optional mode restrictions.
///
/// Source: `src/core/task/build-tools.ts` — `buildNativeToolsArrayWithRestrictions`
pub fn build_native_tools_array_with_restrictions(options: BuildToolsOptions) -> BuildToolsResult {
    let filter_settings = FilterSettings {
        todo_list_enabled: options.todo_list_enabled,
        disabled_tools: options.disabled_tools.clone(),
        codebase_search_enabled: options.codebase_search_enabled,
        mcp_resources_available: options.mcp_resources_available,
        ..Default::default()
    };

    // Build native tools with dynamic read_file tool based on settings
    let native_tools = get_native_tools(NativeToolsOptions {
        supports_images: options.supports_images,
    });

    // Filter native tools based on mode restrictions
    let filtered_native_tools = filter_native_tools_for_mode(
        &native_tools,
        options.mode.as_deref(),
        &options.custom_modes,
        Some(&options.experiments),
        &filter_settings,
    );

    if options.include_all_tools_with_restrictions {
        // Return ALL tools but provide allowed names based on mode filtering
        let allowed_function_names: Vec<String> = filtered_native_tools
            .iter()
            .map(|t| crate::groups::resolve_tool_alias(&t.name))
            .collect();

        BuildToolsResult {
            tools: native_tools,
            allowed_function_names: Some(allowed_function_names),
        }
    } else {
        BuildToolsResult {
            tools: filtered_native_tools,
            allowed_function_names: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_options() -> BuildToolsOptions {
        BuildToolsOptions {
            cwd: "/test".into(),
            mode: Some("code".into()),
            codebase_search_enabled: true,
            mcp_resources_available: true,
            ..Default::default()
        }
    }

    #[test]
    fn test_build_native_tools_array_code_mode() {
        let tools = build_native_tools_array(default_options());
        assert!(!tools.is_empty());
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"write_to_file"));
        assert!(names.contains(&"execute_command"));
    }

    #[test]
    fn test_build_native_tools_array_ask_mode() {
        let opts = BuildToolsOptions {
            mode: Some("ask".into()),
            ..default_options()
        };
        let tools = build_native_tools_array(opts);
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(names.contains(&"ask_followup_question"));
        assert!(names.contains(&"attempt_completion"));
        assert!(!names.contains(&"execute_command"));
    }

    #[test]
    fn test_build_with_restrictions() {
        let opts = BuildToolsOptions {
            include_all_tools_with_restrictions: true,
            ..default_options()
        };
        let result = build_native_tools_array_with_restrictions(opts);
        // Should return ALL tools
        assert_eq!(result.tools.len(), 21);
        // But have restricted allowed names
        assert!(result.allowed_function_names.is_some());
        let allowed = result.allowed_function_names.unwrap();
        assert!(allowed.contains(&"read_file".to_string()));
    }

    #[test]
    fn test_build_with_disabled_tools() {
        let opts = BuildToolsOptions {
            disabled_tools: vec!["execute_command".to_string()],
            ..default_options()
        };
        let tools = build_native_tools_array(opts);
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(!names.contains(&"execute_command"));
    }

    #[test]
    fn test_build_with_todo_disabled() {
        let opts = BuildToolsOptions {
            todo_list_enabled: false,
            ..default_options()
        };
        let tools = build_native_tools_array(opts);
        let names: Vec<&str> = tools.iter().map(|t| t.name.as_str()).collect();
        assert!(!names.contains(&"update_todo_list"));
    }
}

//! Tool group configuration.
//!
//! Corresponds to `src/shared/tools.ts` — `TOOL_GROUPS`, `ALWAYS_AVAILABLE_TOOLS`,
//! `TOOL_ALIASES`, `TOOL_DISPLAY_NAMES`.

use std::collections::HashMap;
use std::sync::LazyLock;

use roo_types::tool::{ToolGroup, ToolName};

// ---------------------------------------------------------------------------
// ToolGroupConfig
// ---------------------------------------------------------------------------

/// Configuration for a single tool group.
///
/// Source: `src/shared/tools.ts` — `ToolGroupConfig`
#[derive(Debug, Clone)]
pub struct ToolGroupConfig {
    /// Tools that are always available in this group.
    pub tools: Vec<ToolName>,
    /// Whether this group is always available (shouldn't show in prompts view).
    pub always_available: bool,
    /// Opt-in only tools — only available when explicitly included via model's `includedTools`.
    pub custom_tools: Vec<ToolName>,
}

// ---------------------------------------------------------------------------
// TOOL_GROUPS
// ---------------------------------------------------------------------------

/// All tool groups with their configurations.
///
/// Source: `src/shared/tools.ts` — `TOOL_GROUPS`
pub static TOOL_GROUPS: LazyLock<HashMap<ToolGroup, ToolGroupConfig>> = LazyLock::new(|| {
    let mut m = HashMap::new();

    m.insert(
        ToolGroup::Read,
        ToolGroupConfig {
            tools: vec![
                ToolName::ReadFile,
                ToolName::SearchFiles,
                ToolName::ListFiles,
                ToolName::CodebaseSearch,
            ],
            always_available: false,
            custom_tools: vec![],
        },
    );

    m.insert(
        ToolGroup::Edit,
        ToolGroupConfig {
            tools: vec![
                ToolName::ApplyDiff,
                ToolName::WriteToFile,
                ToolName::GenerateImage,
            ],
            always_available: false,
            custom_tools: vec![
                ToolName::Edit,
                ToolName::SearchReplace,
                ToolName::EditFile,
                ToolName::ApplyPatch,
            ],
        },
    );

    m.insert(
        ToolGroup::Command,
        ToolGroupConfig {
            tools: vec![ToolName::ExecuteCommand, ToolName::ReadCommandOutput],
            always_available: false,
            custom_tools: vec![],
        },
    );

    m.insert(
        ToolGroup::Mcp,
        ToolGroupConfig {
            tools: vec![ToolName::UseMcpTool, ToolName::AccessMcpResource],
            always_available: false,
            custom_tools: vec![],
        },
    );

    m.insert(
        ToolGroup::Modes,
        ToolGroupConfig {
            tools: vec![ToolName::SwitchMode, ToolName::NewTask],
            always_available: true,
            custom_tools: vec![],
        },
    );

    m
});

// ---------------------------------------------------------------------------
// ALWAYS_AVAILABLE_TOOLS
// ---------------------------------------------------------------------------

/// Tools that are always available to all modes.
///
/// Source: `src/shared/tools.ts` — `ALWAYS_AVAILABLE_TOOLS`
pub static ALWAYS_AVAILABLE_TOOLS: [ToolName; 7] = [
    ToolName::AskFollowupQuestion,
    ToolName::AttemptCompletion,
    ToolName::SwitchMode,
    ToolName::NewTask,
    ToolName::UpdateTodoList,
    ToolName::RunSlashCommand,
    ToolName::Skill,
];

// ---------------------------------------------------------------------------
// TOOL_ALIASES
// ---------------------------------------------------------------------------

/// Central registry of tool aliases.
/// Maps alias name → canonical tool name.
///
/// Source: `src/shared/tools.ts` — `TOOL_ALIASES`
pub static TOOL_ALIASES: LazyLock<HashMap<&'static str, ToolName>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert("write_file", ToolName::WriteToFile);
    m.insert("search_and_replace", ToolName::Edit);
    m
});

// ---------------------------------------------------------------------------
// TOOL_DISPLAY_NAMES
// ---------------------------------------------------------------------------

/// Human-readable display names for each tool.
///
/// Source: `src/shared/tools.ts` — `TOOL_DISPLAY_NAMES`
pub static TOOL_DISPLAY_NAMES: LazyLock<HashMap<ToolName, &'static str>> = LazyLock::new(|| {
    let mut m = HashMap::new();
    m.insert(ToolName::ExecuteCommand, "run commands");
    m.insert(ToolName::ReadFile, "read files");
    m.insert(ToolName::ReadCommandOutput, "read command output");
    m.insert(ToolName::WriteToFile, "write files");
    m.insert(ToolName::ApplyDiff, "apply changes");
    m.insert(ToolName::Edit, "edit files");
    m.insert(ToolName::SearchAndReplace, "apply changes using search and replace");
    m.insert(ToolName::SearchReplace, "apply single search and replace");
    m.insert(ToolName::EditFile, "edit files using search and replace");
    m.insert(ToolName::ApplyPatch, "apply patches using codex format");
    m.insert(ToolName::SearchFiles, "search files");
    m.insert(ToolName::ListFiles, "list files");
    m.insert(ToolName::UseMcpTool, "use mcp tools");
    m.insert(ToolName::AccessMcpResource, "access mcp resources");
    m.insert(ToolName::AskFollowupQuestion, "ask questions");
    m.insert(ToolName::AttemptCompletion, "complete tasks");
    m.insert(ToolName::SwitchMode, "switch modes");
    m.insert(ToolName::NewTask, "create new task");
    m.insert(ToolName::CodebaseSearch, "codebase search");
    m.insert(ToolName::UpdateTodoList, "update todo list");
    m.insert(ToolName::RunSlashCommand, "run slash command");
    m.insert(ToolName::Skill, "load skill");
    m.insert(ToolName::GenerateImage, "generate images");
    m.insert(ToolName::CustomTool, "use custom tools");
    m
});

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Resolve a tool name alias to its canonical name.
/// If the tool name is an alias, returns the canonical tool name string.
/// If it's already a canonical name or unknown, returns as-is.
pub fn resolve_tool_alias(tool_name: &str) -> String {
    match TOOL_ALIASES.get(tool_name) {
        Some(canonical) => canonical.as_str().to_string(),
        None => tool_name.to_string(),
    }
}

/// Check if a tool name is in the always-available list.
pub fn is_always_available(tool_name: &ToolName) -> bool {
    ALWAYS_AVAILABLE_TOOLS.contains(tool_name)
}

/// Get all tools for a mode based on its group entries.
/// Returns tool names including always-available tools.
///
/// Source: `src/shared/modes.ts` — `getToolsForMode`
pub fn get_tools_for_mode(groups: &[roo_types::tool::GroupEntry]) -> Vec<ToolName> {
    let mut tools = std::collections::HashSet::new();

    for group_entry in groups {
        let group_name = match group_entry {
            roo_types::tool::GroupEntry::Plain(g) => *g,
            roo_types::tool::GroupEntry::WithOptions(g, _) => *g,
        };

        if let Some(config) = TOOL_GROUPS.get(&group_name) {
            for tool in &config.tools {
                tools.insert(*tool);
            }
        }
    }

    // Always add required tools
    for tool in &ALWAYS_AVAILABLE_TOOLS {
        tools.insert(*tool);
    }

    tools.into_iter().collect()
}

/// Get the group name from a group entry.
pub fn get_group_name(group_entry: &roo_types::tool::GroupEntry) -> ToolGroup {
    match group_entry {
        roo_types::tool::GroupEntry::Plain(g) => *g,
        roo_types::tool::GroupEntry::WithOptions(g, _) => *g,
    }
}

/// Get the group options from a group entry.
pub fn get_group_options(
    group_entry: &roo_types::tool::GroupEntry,
) -> Option<&roo_types::tool::GroupOptions> {
    match group_entry {
        roo_types::tool::GroupEntry::Plain(_) => None,
        roo_types::tool::GroupEntry::WithOptions(_, opts) => Some(opts),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_groups_read() {
        let read_group = TOOL_GROUPS.get(&ToolGroup::Read).unwrap();
        assert_eq!(
            read_group.tools,
            vec![
                ToolName::ReadFile,
                ToolName::SearchFiles,
                ToolName::ListFiles,
                ToolName::CodebaseSearch,
            ]
        );
        assert!(!read_group.always_available);
        assert!(read_group.custom_tools.is_empty());
    }

    #[test]
    fn test_tool_groups_edit() {
        let edit_group = TOOL_GROUPS.get(&ToolGroup::Edit).unwrap();
        assert_eq!(
            edit_group.tools,
            vec![ToolName::ApplyDiff, ToolName::WriteToFile, ToolName::GenerateImage,]
        );
        assert_eq!(
            edit_group.custom_tools,
            vec![
                ToolName::Edit,
                ToolName::SearchReplace,
                ToolName::EditFile,
                ToolName::ApplyPatch,
            ]
        );
    }

    #[test]
    fn test_tool_groups_modes_always_available() {
        let modes_group = TOOL_GROUPS.get(&ToolGroup::Modes).unwrap();
        assert!(modes_group.always_available);
    }

    #[test]
    fn test_always_available_tools() {
        assert!(is_always_available(&ToolName::AskFollowupQuestion));
        assert!(is_always_available(&ToolName::AttemptCompletion));
        assert!(is_always_available(&ToolName::SwitchMode));
        assert!(is_always_available(&ToolName::NewTask));
        assert!(is_always_available(&ToolName::UpdateTodoList));
        assert!(is_always_available(&ToolName::RunSlashCommand));
        assert!(is_always_available(&ToolName::Skill));

        // These should NOT be always available
        assert!(!is_always_available(&ToolName::ExecuteCommand));
        assert!(!is_always_available(&ToolName::ReadFile));
        assert!(!is_always_available(&ToolName::WriteToFile));
    }

    #[test]
    fn test_tool_aliases() {
        assert_eq!(resolve_tool_alias("write_file"), "write_to_file");
        assert_eq!(resolve_tool_alias("search_and_replace"), "edit");
        // Non-alias returns as-is
        assert_eq!(resolve_tool_alias("read_file"), "read_file");
        assert_eq!(resolve_tool_alias("execute_command"), "execute_command");
    }

    #[test]
    fn test_tool_display_names() {
        assert_eq!(TOOL_DISPLAY_NAMES.get(&ToolName::ExecuteCommand), Some(&"run commands"));
        assert_eq!(TOOL_DISPLAY_NAMES.get(&ToolName::ReadFile), Some(&"read files"));
        assert_eq!(TOOL_DISPLAY_NAMES.get(&ToolName::WriteToFile), Some(&"write files"));
        assert_eq!(TOOL_DISPLAY_NAMES.get(&ToolName::CustomTool), Some(&"use custom tools"));
    }

    #[test]
    fn test_all_tool_groups_present() {
        assert!(TOOL_GROUPS.contains_key(&ToolGroup::Read));
        assert!(TOOL_GROUPS.contains_key(&ToolGroup::Edit));
        assert!(TOOL_GROUPS.contains_key(&ToolGroup::Command));
        assert!(TOOL_GROUPS.contains_key(&ToolGroup::Mcp));
        assert!(TOOL_GROUPS.contains_key(&ToolGroup::Modes));
    }

    #[test]
    fn test_get_tools_for_mode() {
        use roo_types::tool::GroupEntry;
        let groups = vec![
            GroupEntry::Plain(ToolGroup::Read),
            GroupEntry::Plain(ToolGroup::Edit),
        ];
        let tools = get_tools_for_mode(&groups);

        // Should include read group tools
        assert!(tools.contains(&ToolName::ReadFile));
        assert!(tools.contains(&ToolName::SearchFiles));
        assert!(tools.contains(&ToolName::ListFiles));
        assert!(tools.contains(&ToolName::CodebaseSearch));

        // Should include edit group tools
        assert!(tools.contains(&ToolName::ApplyDiff));
        assert!(tools.contains(&ToolName::WriteToFile));
        assert!(tools.contains(&ToolName::GenerateImage));

        // Should include always-available tools
        assert!(tools.contains(&ToolName::AskFollowupQuestion));
        assert!(tools.contains(&ToolName::AttemptCompletion));
        assert!(tools.contains(&ToolName::SwitchMode));
    }
}

//! Mode-to-tool mapping functions.
//!
//! Source: `src/shared/modes.ts` — `getGroupName`, `getToolsForMode`

use roo_tools::groups::{get_group_name as inner_get_group_name, get_tools_for_mode as inner_get_tools_for_mode};
use roo_types::tool::{GroupEntry, ToolGroup, ToolName};

// ---------------------------------------------------------------------------
// getGroupName
// ---------------------------------------------------------------------------

/// Extract the `ToolGroup` from a `GroupEntry`.
///
/// Delegates to [`roo_tools::groups::get_group_name`].
///
/// Source: `src/shared/modes.ts` — `getGroupName`
pub fn get_group_name(group: &GroupEntry) -> ToolGroup {
    inner_get_group_name(group)
}

// ---------------------------------------------------------------------------
// getToolsForMode
// ---------------------------------------------------------------------------

/// Get all tool name strings for a mode based on its group entries,
/// including always-available tools.
///
/// Returns `Vec<String>` (tool name strings), unlike the lower-level
/// [`roo_tools::groups::get_tools_for_mode`] which returns `Vec<ToolName>`.
///
/// Source: `src/shared/modes.ts` — `getToolsForMode`
pub fn get_tools_for_mode(groups: &[GroupEntry]) -> Vec<String> {
    inner_get_tools_for_mode(groups)
        .into_iter()
        .map(|t: ToolName| t.as_str().to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_group_name_plain() {
        let entry = GroupEntry::Plain(ToolGroup::Read);
        assert_eq!(get_group_name(&entry), ToolGroup::Read);
    }

    #[test]
    fn test_get_group_name_with_options() {
        let entry = GroupEntry::WithOptions(
            ToolGroup::Edit,
            roo_types::tool::GroupOptions {
                file_regex: Some("\\.md$".into()),
                description: Some("Markdown only".into()),
            },
        );
        assert_eq!(get_group_name(&entry), ToolGroup::Edit);
    }

    #[test]
    fn test_get_tools_for_mode_returns_strings() {
        let groups = vec![
            GroupEntry::Plain(ToolGroup::Read),
            GroupEntry::Plain(ToolGroup::Edit),
        ];
        let tools = get_tools_for_mode(&groups);
        // Should contain string tool names
        assert!(tools.contains(&"read_file".to_string()));
        assert!(tools.contains(&"apply_diff".to_string()));
        // Should contain always-available tools
        assert!(tools.contains(&"ask_followup_question".to_string()));
        assert!(tools.contains(&"attempt_completion".to_string()));
    }

    #[test]
    fn test_get_tools_for_mode_empty_groups() {
        let tools = get_tools_for_mode(&[]);
        // Should still contain always-available tools
        assert!(tools.contains(&"ask_followup_question".to_string()));
        assert!(tools.contains(&"attempt_completion".to_string()));
        assert!(tools.contains(&"switch_mode".to_string()));
        assert!(tools.contains(&"new_task".to_string()));
    }

    #[test]
    fn test_get_tools_for_mode_deduplicates() {
        let groups = vec![
            GroupEntry::Plain(ToolGroup::Read),
            GroupEntry::Plain(ToolGroup::Read),
        ];
        let tools = get_tools_for_mode(&groups);
        // read_file should appear only once
        assert_eq!(
            tools.iter().filter(|t| **t == "read_file").count(),
            1
        );
    }
}

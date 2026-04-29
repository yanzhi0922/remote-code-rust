//! Tool classification for auto-approval.
//!
//! Mirrors `tools.ts` — classifies tool actions as read-only or write.

use crate::types::ToolAction;

/// Tool names that are considered read-only (do not modify files).
const READ_ONLY_TOOL_NAMES: &[&str] = &[
    "readFile",
    "listFiles",
    "listFilesTopLevel",
    "listFilesRecursive",
    "searchFiles",
    "codebaseSearch",
    "runSlashCommand",
];

/// Tool names that are considered write operations (modify files).
const WRITE_TOOL_NAMES: &[&str] = &[
    "editedExistingFile",
    "appliedDiff",
    "newFileCreated",
    "generateImage",
];

/// Returns `true` if the tool action is a read-only operation.
///
/// Mirrors `isReadOnlyToolAction` from `tools.ts`.
pub fn is_read_only_tool_action(tool: &ToolAction) -> bool {
    READ_ONLY_TOOL_NAMES.contains(&tool.tool_name())
}

/// Returns `true` if the tool action is a write operation.
///
/// Mirrors `isWriteToolAction` from `tools.ts`.
pub fn is_write_tool_action(tool: &ToolAction) -> bool {
    WRITE_TOOL_NAMES.contains(&tool.tool_name())
}

/// Returns `true` if the given tool name is a read-only tool.
pub fn is_read_only_tool_name(name: &str) -> bool {
    READ_ONLY_TOOL_NAMES.contains(&name)
}

/// Returns `true` if the given tool name is a write tool.
pub fn is_write_tool_name(name: &str) -> bool {
    WRITE_TOOL_NAMES.contains(&name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_only_tools() {
        for name in READ_ONLY_TOOL_NAMES {
            assert!(is_read_only_tool_name(name), "{name} should be read-only");
        }
    }

    #[test]
    fn test_write_tools() {
        for name in WRITE_TOOL_NAMES {
            assert!(is_write_tool_name(name), "{name} should be write");
        }
    }

    #[test]
    fn test_read_only_tool_action() {
        let tool = ToolAction::ReadFile {
            is_outside_workspace: false,
        };
        assert!(is_read_only_tool_action(&tool));

        let tool = ToolAction::SearchFiles {
            is_outside_workspace: true,
        };
        assert!(is_read_only_tool_action(&tool));
    }

    #[test]
    fn test_write_tool_action() {
        let tool = ToolAction::EditedExistingFile {
            is_outside_workspace: false,
        };
        assert!(is_write_tool_action(&tool));

        let tool = ToolAction::NewFileCreated {
            is_outside_workspace: false,
        };
        assert!(is_write_tool_action(&tool));
    }

    #[test]
    fn test_non_read_only_non_write() {
        assert!(!is_read_only_tool_name("editedExistingFile"));
        assert!(!is_write_tool_name("readFile"));
        assert!(!is_read_only_tool_name("updateTodoList"));
        assert!(!is_write_tool_name("updateTodoList"));
    }
}

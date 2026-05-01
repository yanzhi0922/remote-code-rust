//! Helper functions for miscellaneous tools.

use crate::types::{MiscToolError, TodoItem, TodoStatus};

/// Parse a markdown checklist into todo items.
///
/// Supports formats:
/// - `[x] task text` → Completed
/// - `[ ] task text` → Pending
/// - `[-] task text` → InProgress
pub fn parse_markdown_checklist(markdown: &str) -> Vec<TodoItem> {
    let mut items = Vec::new();

    for line in markdown.lines() {
        let trimmed = line.trim();

        // Try to match checkbox pattern
        if let Some(rest) = trimmed.strip_prefix("[x]") {
            items.push(TodoItem {
                status: TodoStatus::Completed,
                text: rest.trim().to_string(),
            });
        } else if let Some(rest) = trimmed.strip_prefix("[-]") {
            items.push(TodoItem {
                status: TodoStatus::InProgress,
                text: rest.trim().to_string(),
            });
        } else if let Some(rest) = trimmed.strip_prefix("[ ]") {
            items.push(TodoItem {
                status: TodoStatus::Pending,
                text: rest.trim().to_string(),
            });
        }
        // Skip lines that don't match
    }

    items
}

/// Format follow-up suggestions as a numbered list.
pub fn format_followup_suggestions(suggestions: &[String]) -> String {
    suggestions
        .iter()
        .enumerate()
        .map(|(i, s)| format!("{}. {s}", i + 1))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Validate a completion result.
pub fn validate_completion_result(result: &str) -> Result<(), MiscToolError> {
    if result.trim().is_empty() {
        return Err(MiscToolError::Validation(
            "completion result must not be empty".to_string(),
        ));
    }
    Ok(())
}

/// Validate a skill name.
pub fn validate_skill_name(name: &str) -> Result<(), MiscToolError> {
    if name.trim().is_empty() {
        return Err(MiscToolError::InvalidSkill(
            "skill name must not be empty".to_string(),
        ));
    }

    // Skill names should be alphanumeric with hyphens/underscores
    for ch in name.chars() {
        if !ch.is_alphanumeric() && ch != '-' && ch != '_' {
            return Err(MiscToolError::InvalidSkill(format!(
                "skill name contains invalid character: '{ch}'"
            )));
        }
    }

    Ok(())
}

/// Serialize todo items back to markdown format.
pub fn serialize_todo_items(items: &[TodoItem]) -> String {
    items
        .iter()
        .map(|item| format!("{} {}", item.status.to_checkbox(), item.text))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- parse_markdown_checklist tests ----

    #[test]
    fn test_parse_complete_checklist() {
        let md = "[x] task 1\n[ ] task 2\n[-] task 3\n[x] task 4";
        let items = parse_markdown_checklist(md);
        assert_eq!(items.len(), 4);
        assert_eq!(items[0].status, TodoStatus::Completed);
        assert_eq!(items[1].status, TodoStatus::Pending);
        assert_eq!(items[2].status, TodoStatus::InProgress);
        assert_eq!(items[3].status, TodoStatus::Completed);
        assert_eq!(items[0].text, "task 1");
    }

    #[test]
    fn test_parse_empty() {
        let items = parse_markdown_checklist("");
        assert!(items.is_empty());
    }

    #[test]
    fn test_parse_invalid_format() {
        let md = "not a checkbox\nalso not\n- still not";
        let items = parse_markdown_checklist(md);
        assert!(items.is_empty());
    }

    #[test]
    fn test_parse_mixed() {
        let md = "[x] done\nsome text\n[ ] pending\nmore text";
        let items = parse_markdown_checklist(md);
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn test_parse_with_leading_whitespace() {
        let md = "  [x] indented\n\t[ ] tabbed";
        let items = parse_markdown_checklist(md);
        assert_eq!(items.len(), 2);
    }

    // ---- format_followup_suggestions tests ----

    #[test]
    fn test_format_suggestions() {
        let suggestions = vec![
            "Option A".to_string(),
            "Option B".to_string(),
            "Option C".to_string(),
        ];
        let result = format_followup_suggestions(&suggestions);
        assert!(result.contains("1. Option A"));
        assert!(result.contains("2. Option B"));
        assert!(result.contains("3. Option C"));
    }

    #[test]
    fn test_format_empty_suggestions() {
        let result = format_followup_suggestions(&[]);
        assert!(result.is_empty());
    }

    // ---- validate_completion_result tests ----

    #[test]
    fn test_validate_empty_result() {
        assert!(validate_completion_result("").is_err());
    }

    #[test]
    fn test_validate_whitespace_result() {
        assert!(validate_completion_result("   ").is_err());
    }

    #[test]
    fn test_validate_valid_result() {
        assert!(validate_completion_result("Task completed successfully").is_ok());
    }

    // ---- validate_skill_name tests ----

    #[test]
    fn test_validate_empty_skill() {
        assert!(validate_skill_name("").is_err());
    }

    #[test]
    fn test_validate_valid_skill() {
        assert!(validate_skill_name("my-skill").is_ok());
        assert!(validate_skill_name("my_skill").is_ok());
        assert!(validate_skill_name("myskill123").is_ok());
    }

    #[test]
    fn test_validate_invalid_skill() {
        assert!(validate_skill_name("my skill").is_err());
        assert!(validate_skill_name("skill@name").is_err());
    }

    // ---- TodoStatus tests ----

    #[test]
    fn test_todo_status_checkbox() {
        assert_eq!(TodoStatus::Pending.to_checkbox(), "[ ]");
        assert_eq!(TodoStatus::InProgress.to_checkbox(), "[-]");
        assert_eq!(TodoStatus::Completed.to_checkbox(), "[x]");
    }

    #[test]
    fn test_todo_status_from_checkbox() {
        assert_eq!(TodoStatus::from_checkbox(true), TodoStatus::Completed);
        assert_eq!(TodoStatus::from_checkbox(false), TodoStatus::Pending);
    }

    // ---- serialize_todo_items tests ----

    #[test]
    fn test_serialize_todo_items() {
        let items = vec![
            TodoItem { status: TodoStatus::Completed, text: "done".to_string() },
            TodoItem { status: TodoStatus::Pending, text: "todo".to_string() },
        ];
        let result = serialize_todo_items(&items);
        assert!(result.contains("[x] done"));
        assert!(result.contains("[ ] todo"));
    }

    // ---- MiscToolError tests ----

    #[test]
    fn test_misc_tool_error_display() {
        let err = MiscToolError::Validation("bad input".to_string());
        assert_eq!(format!("{err}"), "Validation error: bad input");
    }

    // ---- CompletionResult tests ----

    #[test]
    fn test_completion_result_serde() {
        let r = crate::types::CompletionResult {
            result: "Done!".to_string(),
            has_command: false,
            attempt_completion_result: None,
            todo_warning: None,
        };
        let json = serde_json::to_string(&r).unwrap();
        let parsed: crate::types::CompletionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.result, "Done!");
        assert!(!parsed.has_command);
    }

    // ---- FollowupResult tests ----

    #[test]
    fn test_followup_result_serde() {
        let r = crate::types::FollowupResult {
            question: "What next?".to_string(),
            suggestions: vec!["A".to_string(), "B".to_string()],
        };
        let json = serde_json::to_string(&r).unwrap();
        let parsed: crate::types::FollowupResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.suggestions.len(), 2);
    }

    // ---- SkillResult tests ----

    #[test]
    fn test_skill_result_serde() {
        let r = crate::types::SkillResult {
            skill_name: "my-skill".to_string(),
            args: Some("arg1".to_string()),
            is_valid: true,
            content: None,
        };
        let json = serde_json::to_string(&r).unwrap();
        let parsed: crate::types::SkillResult = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_valid);
    }
}

//! Todo reminder section formatting.
//!
//! Ported from `src/core/environment/reminder.ts`.

use crate::types::TodoItemInput;

/// Map a raw status string to its display label.
fn display_status(status: &str) -> &str {
    match status {
        "pending" => "Pending",
        "in_progress" => "In Progress",
        "completed" => "Completed",
        other => other,
    }
}

/// Escape characters that would break a markdown table cell.
fn escape_table_cell(s: &str) -> String {
    s.replace('\\', "\\\\").replace('|', "\\|")
}

/// Format the reminders section as a markdown block.
///
/// Returns a string identical to the TypeScript `formatReminderSection`.
///
/// - `None` or empty list → creation prompt
/// - Non-empty list → numbered markdown table
pub fn format_reminder_section(todo_list: Option<&[TodoItemInput]>) -> String {
    match todo_list {
        None | Some([]) => {
            "You have not created a todo list yet. Create one with `update_todo_list` if your task is complicated or involves multiple steps.".to_string()
        }
        Some(items) => {
            let mut lines: Vec<String> = Vec::with_capacity(items.len() + 10);
            lines.push("====".to_string());
            lines.push(String::new());
            lines.push("REMINDERS".to_string());
            lines.push(String::new());
            lines.push(
                "Below is your current list of reminders for this task. Keep them updated as you progress.".to_string(),
            );
            lines.push(String::new());

            lines.push("| # | Content | Status |".to_string());
            lines.push("|---|---------|--------|".to_string());

            for (idx, item) in items.iter().enumerate() {
                let escaped = escape_table_cell(&item.content);
                let status = display_status(&item.status);
                lines.push(format!("| {} | {} | {} |", idx + 1, escaped, status));
            }

            lines.push(String::new());
            lines.push(String::new());
            lines.push(
                "IMPORTANT: When task status changes, remember to call the `update_todo_list` tool to update your progress.".to_string(),
            );
            lines.push(String::new());

            lines.join("\n")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_none_todo_list() {
        let result = format_reminder_section(None);
        assert!(result.contains("You have not created a todo list yet"));
        assert!(result.contains("update_todo_list"));
    }

    #[test]
    fn test_empty_todo_list() {
        let items: Vec<TodoItemInput> = vec![];
        let result = format_reminder_section(Some(&items));
        assert!(result.contains("You have not created a todo list yet"));
    }

    #[test]
    fn test_single_item() {
        let items = vec![TodoItemInput {
            content: "Implement feature X".to_string(),
            status: "pending".to_string(),
        }];
        let result = format_reminder_section(Some(&items));
        assert!(result.contains("REMINDERS"));
        assert!(result.contains("| # | Content | Status |"));
        assert!(result.contains("| 1 | Implement feature X | Pending |"));
        assert!(result.contains("IMPORTANT"));
    }

    #[test]
    fn test_multiple_items_with_statuses() {
        let items = vec![
            TodoItemInput {
                content: "Task A".to_string(),
                status: "completed".to_string(),
            },
            TodoItemInput {
                content: "Task B".to_string(),
                status: "in_progress".to_string(),
            },
            TodoItemInput {
                content: "Task C".to_string(),
                status: "pending".to_string(),
            },
        ];
        let result = format_reminder_section(Some(&items));
        assert!(result.contains("| 1 | Task A | Completed |"));
        assert!(result.contains("| 2 | Task B | In Progress |"));
        assert!(result.contains("| 3 | Task C | Pending |"));
    }

    #[test]
    fn test_escape_backslash() {
        let items = vec![TodoItemInput {
            content: "Path C:\\Users\\test".to_string(),
            status: "pending".to_string(),
        }];
        let result = format_reminder_section(Some(&items));
        assert!(result.contains(r"C:\\Users\\test"));
    }

    #[test]
    fn test_escape_pipe() {
        let items = vec![TodoItemInput {
            content: "Use option | verbose".to_string(),
            status: "pending".to_string(),
        }];
        let result = format_reminder_section(Some(&items));
        assert!(result.contains(r"Use option \| verbose"));
    }

    #[test]
    fn test_unknown_status_passthrough() {
        let items = vec![TodoItemInput {
            content: "Task".to_string(),
            status: "custom_status".to_string(),
        }];
        let result = format_reminder_section(Some(&items));
        assert!(result.contains("| 1 | Task | custom_status |"));
    }

    #[test]
    fn test_reminder_section_structure() {
        let items = vec![TodoItemInput {
            content: "Do something".to_string(),
            status: "pending".to_string(),
        }];
        let result = format_reminder_section(Some(&items));

        // Verify the exact structure matches TS output
        assert!(result.starts_with("====\n"));
        assert!(result.contains("\nREMINDERS\n"));
        assert!(result.contains("\n\nIMPORTANT:"));
        assert!(result.ends_with('\n'));
    }
}

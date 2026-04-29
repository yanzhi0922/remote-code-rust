//! update_todo_list tool implementation.

use crate::helpers::*;
use crate::types::*;
use roo_types::tool::UpdateTodoListParams;

/// Validate update_todo_list parameters.
pub fn validate_update_todo_params(params: &UpdateTodoListParams) -> Result<(), MiscToolError> {
    if params.todos.trim().is_empty() {
        return Err(MiscToolError::Validation(
            "todos must not be empty".to_string(),
        ));
    }
    Ok(())
}

/// Process an update_todo_list request.
///
/// Parses the markdown checklist and returns structured todo items.
pub fn process_update_todo(params: &UpdateTodoListParams) -> Result<Vec<TodoItem>, MiscToolError> {
    validate_update_todo_params(params)?;

    let items = parse_markdown_checklist(&params.todos);

    if items.is_empty() {
        return Err(MiscToolError::Parse(
            "no valid checklist items found in input".to_string(),
        ));
    }

    Ok(items)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_empty_todos() {
        let params = UpdateTodoListParams {
            todos: "".to_string(),
        };
        assert!(validate_update_todo_params(&params).is_err());
    }

    #[test]
    fn test_validate_valid_todos() {
        let params = UpdateTodoListParams {
            todos: "[x] done\n[ ] pending".to_string(),
        };
        assert!(validate_update_todo_params(&params).is_ok());
    }

    #[test]
    fn test_process_update_todo() {
        let params = UpdateTodoListParams {
            todos: "[x] task 1\n[ ] task 2\n[-] task 3".to_string(),
        };
        let items = process_update_todo(&params).unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].status, TodoStatus::Completed);
        assert_eq!(items[1].status, TodoStatus::Pending);
        assert_eq!(items[2].status, TodoStatus::InProgress);
    }

    #[test]
    fn test_process_update_todo_no_valid_items() {
        let params = UpdateTodoListParams {
            todos: "not a checkbox\nalso not".to_string(),
        };
        assert!(process_update_todo(&params).is_err());
    }

    #[test]
    fn test_process_update_todo_mixed() {
        let params = UpdateTodoListParams {
            todos: "[x] done\nplain text\n[ ] pending".to_string(),
        };
        let items = process_update_todo(&params).unwrap();
        assert_eq!(items.len(), 2);
    }
}

//! new_task tool implementation.

use crate::helpers::*;
use crate::types::*;
use roo_types::tool::NewTaskParams;

/// Validate new_task parameters.
pub fn validate_new_task_params(params: &NewTaskParams) -> Result<(), ModeToolError> {
    validate_mode_slug(&params.mode)?;

    if params.message.trim().is_empty() {
        return Err(ModeToolError::Validation(
            "message must not be empty".to_string(),
        ));
    }

    Ok(())
}

/// Process a new_task request.
pub fn process_new_task(params: &NewTaskParams) -> Result<NewTaskResult, ModeToolError> {
    validate_new_task_params(params)?;

    let processed_message = process_task_message(&params.message);

    let todos = params.todos.as_ref().map(|t| parse_todos_string(t));

    Ok(NewTaskResult {
        mode: params.mode.clone(),
        message: processed_message,
        todos,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_valid_params() {
        let params = NewTaskParams {
            mode: "code".to_string(),
            message: "implement feature".to_string(),
            todos: None,
        };
        assert!(validate_new_task_params(&params).is_ok());
    }

    #[test]
    fn test_validate_invalid_mode() {
        let params = NewTaskParams {
            mode: "bad-mode".to_string(),
            message: "do something".to_string(),
            todos: None,
        };
        assert!(validate_new_task_params(&params).is_err());
    }

    #[test]
    fn test_validate_empty_message() {
        let params = NewTaskParams {
            mode: "code".to_string(),
            message: "".to_string(),
            todos: None,
        };
        assert!(validate_new_task_params(&params).is_err());
    }

    #[test]
    fn test_process_new_task() {
        let params = NewTaskParams {
            mode: "code".to_string(),
            message: "implement feature".to_string(),
            todos: Some("[ ] task1\n[x] task2".to_string()),
        };
        let result = process_new_task(&params).unwrap();
        assert_eq!(result.mode, "code");
        assert_eq!(result.message, "implement feature");
        assert!(result.todos.is_some());
    }

    #[test]
    fn test_process_new_task_with_escapes() {
        let params = NewTaskParams {
            mode: "ask".to_string(),
            message: "line1\\nline2".to_string(),
            todos: None,
        };
        let result = process_new_task(&params).unwrap();
        assert_eq!(result.message, "line1\nline2");
    }

    #[test]
    fn test_process_new_task_no_todos() {
        let params = NewTaskParams {
            mode: "architect".to_string(),
            message: "design system".to_string(),
            todos: None,
        };
        let result = process_new_task(&params).unwrap();
        assert!(result.todos.is_none());
    }
}

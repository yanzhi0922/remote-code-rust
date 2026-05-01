//! attempt_completion tool implementation.
//!
//! Aligned with TS `AttemptCompletionTool.ts`:
//! - Validates the `result` parameter is non-empty.
//! - Checks `did_tool_fail_in_current_turn` and blocks completion if true.
//! - Checks for incomplete todos and optionally blocks (configurable).
//! - Populates `attempt_completion_result` with text and images.
//! - Emits a telemetry event stub for task completion.

use crate::helpers::*;
use crate::types::*;
use roo_types::tool::AttemptCompletionParams;

/// Validate attempt_completion parameters.
pub fn validate_attempt_completion_params(params: &AttemptCompletionParams) -> Result<(), MiscToolError> {
    validate_completion_result(&params.result)
}

/// Check if any tool failed in the current turn.
///
/// Matches TS: `if (task.didToolFailInCurrentTurn) { ... block completion }`
/// Returns an error message if a tool failed, preventing completion.
pub fn check_tool_failure_guard(did_tool_fail: bool) -> Result<(), MiscToolError> {
    if did_tool_fail {
        return Err(MiscToolError::Validation(
            "Cannot complete task because a tool failed in the current turn. \
             Please fix the error before attempting completion."
                .to_string(),
        ));
    }
    Ok(())
}

/// Check for incomplete todos.
///
/// In the TS source, `preventCompletionWithOpenTodos` can block completion entirely.
/// When `block_on_incomplete` is true, returns an error that prevents completion.
/// When false, returns a warning that the caller can display.
///
/// - `block_on_incomplete`: Whether to block completion (matches TS `preventCompletionWithOpenTodos`).
/// - Returns `Err` if blocking and there are incomplete todos.
/// - Returns `Ok(Some(warning))` if not blocking but there are incomplete todos.
/// - Returns `Ok(None)` if all todos are complete.
pub fn check_todo_guard(
    todos: &[TodoItem],
    block_on_incomplete: bool,
) -> Result<Option<String>, MiscToolError> {
    let incomplete: Vec<&TodoItem> = todos
        .iter()
        .filter(|t| t.status != TodoStatus::Completed)
        .collect();

    if incomplete.is_empty() {
        return Ok(None);
    }

    let pending = incomplete
        .iter()
        .filter(|t| t.status == TodoStatus::Pending)
        .count();
    let in_progress = incomplete
        .iter()
        .filter(|t| t.status == TodoStatus::InProgress)
        .count();

    if block_on_incomplete {
        // Matches TS: pushToolResult(formatResponse.toolError(
        //   "Cannot complete task while there are incomplete todos. ..."))
        return Err(MiscToolError::Validation(
            "Cannot complete task while there are incomplete todos. \
             Please finish all todos before attempting completion."
                .to_string(),
        ));
    }

    Ok(Some(format!(
        "Warning: There are incomplete todo items ({} pending, {} in progress). \
         Consider completing all todos before finishing.",
        pending, in_progress
    )))
}

/// Emit a telemetry event for task completion.
///
/// In the TS source, this calls `TelemetryService.instance.captureTaskCompleted(task.taskId)`.
/// H4: Now integrated with `roo-telemetry` crate. Creates a telemetry service
/// and captures the task completed event.
pub fn emit_task_completed_event(task_id: &str) {
    let service = roo_telemetry::service::TelemetryService::new();
    service.capture_task_completed(task_id);
}

/// Process an attempt_completion request.
///
/// Validates parameters, checks tool failure guard and todo guard,
/// and builds the result.
///
/// # Arguments
/// * `params` — The completion parameters (result text, optional command).
/// * `todos` — Current todo list items.
/// * `did_tool_fail` — Whether any tool failed in the current turn (matches TS `didToolFailInCurrentTurn`).
/// * `block_on_incomplete_todos` — Whether to block completion with incomplete todos (matches TS `preventCompletionWithOpenTodos`).
pub fn process_attempt_completion(
    params: &AttemptCompletionParams,
    todos: &[TodoItem],
    did_tool_fail: bool,
    block_on_incomplete_todos: bool,
) -> Result<CompletionResult, MiscToolError> {
    validate_attempt_completion_params(params)?;
    check_tool_failure_guard(did_tool_fail)?;
    let todo_warning = check_todo_guard(todos, block_on_incomplete_todos)?;

    let attempt_completion_result = Some(CompletionResultData {
        text: params.result.clone(),
        images: Vec::new(),
    });

    Ok(CompletionResult {
        result: params.result.clone(),
        has_command: params.command.is_some(),
        attempt_completion_result,
        todo_warning,
    })
}

/// Process an attempt_completion request with images.
///
/// This variant allows passing images that were collected during the
/// completion interaction (e.g., from user feedback).
pub fn process_attempt_completion_with_images(
    params: &AttemptCompletionParams,
    todos: &[TodoItem],
    images: Vec<String>,
    did_tool_fail: bool,
    block_on_incomplete_todos: bool,
) -> Result<CompletionResult, MiscToolError> {
    validate_attempt_completion_params(params)?;
    check_tool_failure_guard(did_tool_fail)?;
    let todo_warning = check_todo_guard(todos, block_on_incomplete_todos)?;

    let attempt_completion_result = Some(CompletionResultData {
        text: params.result.clone(),
        images,
    });

    Ok(CompletionResult {
        result: params.result.clone(),
        has_command: params.command.is_some(),
        attempt_completion_result,
        todo_warning,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_empty_result() {
        let params = AttemptCompletionParams {
            result: "".to_string(),
            command: None,
        };
        assert!(validate_attempt_completion_params(&params).is_err());
    }

    #[test]
    fn test_validate_valid_result() {
        let params = AttemptCompletionParams {
            result: "Task done!".to_string(),
            command: None,
        };
        assert!(validate_attempt_completion_params(&params).is_ok());
    }

    #[test]
    fn test_process_completion() {
        let params = AttemptCompletionParams {
            result: "All good".to_string(),
            command: None,
        };
        let result = process_attempt_completion(&params, &[], false, false).unwrap();
        assert_eq!(result.result, "All good");
        assert!(!result.has_command);
        assert!(result.attempt_completion_result.is_some());
        assert!(result.todo_warning.is_none());
    }

    #[test]
    fn test_process_completion_with_command() {
        let params = AttemptCompletionParams {
            result: "Deployed".to_string(),
            command: Some("npm start".to_string()),
        };
        let result = process_attempt_completion(&params, &[], false, false).unwrap();
        assert!(result.has_command);
    }

    // --- Tool failure guard tests ---

    #[test]
    fn test_tool_failure_guard_no_failure() {
        assert!(check_tool_failure_guard(false).is_ok());
    }

    #[test]
    fn test_tool_failure_guard_blocks_on_failure() {
        let err = check_tool_failure_guard(true).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("tool failed"));
        assert!(msg.contains("current turn"));
    }

    #[test]
    fn test_process_completion_blocked_by_tool_failure() {
        let params = AttemptCompletionParams {
            result: "Done".to_string(),
            command: None,
        };
        let err = process_attempt_completion(&params, &[], true, false).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("tool failed"));
    }

    // --- Todo guard tests ---

    #[test]
    fn test_todo_guard_no_todos() {
        let warning = check_todo_guard(&[], false).unwrap();
        assert!(warning.is_none());
    }

    #[test]
    fn test_todo_guard_all_complete() {
        let todos = vec![
            TodoItem {
                status: TodoStatus::Completed,
                text: "task 1".to_string(),
            },
            TodoItem {
                status: TodoStatus::Completed,
                text: "task 2".to_string(),
            },
        ];
        let warning = check_todo_guard(&todos, false).unwrap();
        assert!(warning.is_none());
    }

    #[test]
    fn test_todo_guard_with_incomplete_todos() {
        let todos = vec![
            TodoItem {
                status: TodoStatus::Completed,
                text: "done task".to_string(),
            },
            TodoItem {
                status: TodoStatus::Pending,
                text: "pending task".to_string(),
            },
            TodoItem {
                status: TodoStatus::InProgress,
                text: "in-progress task".to_string(),
            },
        ];
        let warning = check_todo_guard(&todos, false).unwrap();
        assert!(warning.is_some());
        let msg = warning.unwrap();
        assert!(msg.contains("1 pending"));
        assert!(msg.contains("1 in progress"));
        assert!(msg.contains("incomplete todo items"));
    }

    #[test]
    fn test_todo_guard_only_pending() {
        let todos = vec![TodoItem {
            status: TodoStatus::Pending,
            text: "not done".to_string(),
        }];
        let warning = check_todo_guard(&todos, false).unwrap().unwrap();
        assert!(msg_contains(&warning, "1 pending"));
        assert!(msg_contains(&warning, "0 in progress"));
    }

    #[test]
    fn test_todo_guard_blocks_when_configured() {
        // Matches TS: preventCompletionWithOpenTodos = true
        let todos = vec![TodoItem {
            status: TodoStatus::Pending,
            text: "unfinished".to_string(),
        }];
        let err = check_todo_guard(&todos, true).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("incomplete todos"));
        assert!(msg.contains("Cannot complete"));
    }

    #[test]
    fn test_todo_guard_no_block_when_all_complete() {
        let todos = vec![TodoItem {
            status: TodoStatus::Completed,
            text: "done".to_string(),
        }];
        // Even with block_on_incomplete=true, all complete → no error
        assert!(check_todo_guard(&todos, true).unwrap().is_none());
    }

    // --- Result data tests ---

    #[test]
    fn test_attempt_completion_result_data() {
        let params = AttemptCompletionParams {
            result: "Finished!".to_string(),
            command: None,
        };
        let result = process_attempt_completion(&params, &[], false, false).unwrap();
        let data = result.attempt_completion_result.unwrap();
        assert_eq!(data.text, "Finished!");
        assert!(data.images.is_empty());
    }

    #[test]
    fn test_attempt_completion_with_images() {
        let params = AttemptCompletionParams {
            result: "See screenshot".to_string(),
            command: None,
        };
        let images = vec!["data:image/png;base64,abc".to_string()];
        let result =
            process_attempt_completion_with_images(&params, &[], images.clone(), false, false).unwrap();
        let data = result.attempt_completion_result.unwrap();
        assert_eq!(data.images, images);
    }

    #[test]
    fn test_attempt_completion_with_todos_and_images() {
        let params = AttemptCompletionParams {
            result: "Done with warnings".to_string(),
            command: Some("echo done".to_string()),
        };
        let todos = vec![TodoItem {
            status: TodoStatus::InProgress,
            text: "still working".to_string(),
        }];
        let images = vec!["data:image/png;base64,xyz".to_string()];
        let result =
            process_attempt_completion_with_images(&params, &todos, images, false, false).unwrap();

        assert!(result.has_command);
        assert!(result.todo_warning.is_some());
        assert!(result.todo_warning.as_ref().unwrap().contains("0 pending"));
        assert!(result.todo_warning.as_ref().unwrap().contains("1 in progress"));
        let data = result.attempt_completion_result.unwrap();
        assert_eq!(data.images.len(), 1);
    }

    #[test]
    fn test_attempt_completion_blocked_by_incomplete_todos() {
        let params = AttemptCompletionParams {
            result: "Done".to_string(),
            command: None,
        };
        let todos = vec![TodoItem {
            status: TodoStatus::Pending,
            text: "not done".to_string(),
        }];
        let err = process_attempt_completion(&params, &todos, false, true).unwrap_err();
        let msg = format!("{}", err);
        assert!(msg.contains("incomplete todos"));
    }

    #[test]
    fn test_emit_task_completed_event() {
        // Should not panic; just a stub that logs.
        emit_task_completed_event("test-task-123");
    }

    /// Helper to check if a string contains a substring.
    fn msg_contains(s: &str, sub: &str) -> bool {
        s.contains(sub)
    }
}

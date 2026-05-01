//! Helper functions for mode tools.

use crate::types::{ModeToolError, VALID_MODE_SLUGS};

/// Validate a mode slug against the known valid modes.
pub fn validate_mode_slug(slug: &str) -> Result<(), ModeToolError> {
    if slug.trim().is_empty() {
        return Err(ModeToolError::InvalidMode(
            "mode slug must not be empty".to_string(),
        ));
    }

    if !VALID_MODE_SLUGS.contains(&slug) {
        return Err(ModeToolError::InvalidMode(format!(
            "unknown mode: '{slug}'. Valid modes: [{}]",
            VALID_MODE_SLUGS.join(", ")
        )));
    }

    Ok(())
}

/// Check if the requested mode is the same as the current mode.
pub fn is_same_mode(requested: &str, current: &str) -> bool {
    requested == current
}

/// Process a task message by handling escape sequences.
///
/// Converts `\n` literal strings to actual newlines.
pub fn process_task_message(message: &str) -> String {
    message.replace("\\n", "\n").replace("\\t", "\t")
}

/// Parse todo items from a markdown string.
///
/// Returns the cleaned todo string.
pub fn parse_todos_string(todos: &str) -> String {
    todos.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- validate_mode_slug tests ----

    #[test]
    fn test_valid_code_mode() {
        assert!(validate_mode_slug("code").is_ok());
    }

    #[test]
    fn test_valid_architect_mode() {
        assert!(validate_mode_slug("architect").is_ok());
    }

    #[test]
    fn test_valid_ask_mode() {
        assert!(validate_mode_slug("ask").is_ok());
    }

    #[test]
    fn test_valid_debug_mode() {
        assert!(validate_mode_slug("debug").is_ok());
    }

    #[test]
    fn test_valid_orchestrator_mode() {
        assert!(validate_mode_slug("orchestrator").is_ok());
    }

    #[test]
    fn test_invalid_empty() {
        assert!(validate_mode_slug("").is_err());
    }

    #[test]
    fn test_invalid_unknown() {
        assert!(validate_mode_slug("unknown-mode").is_err());
    }

    #[test]
    fn test_invalid_whitespace() {
        assert!(validate_mode_slug("   ").is_err());
    }

    // ---- is_same_mode tests ----

    #[test]
    fn test_same_mode() {
        assert!(is_same_mode("code", "code"));
    }

    #[test]
    fn test_different_mode() {
        assert!(!is_same_mode("code", "architect"));
    }

    // ---- process_task_message tests ----

    #[test]
    fn test_process_message_no_escapes() {
        assert_eq!(process_task_message("hello world"), "hello world");
    }

    #[test]
    fn test_process_message_newline_escape() {
        assert_eq!(process_task_message("line1\\nline2"), "line1\nline2");
    }

    #[test]
    fn test_process_message_tab_escape() {
        assert_eq!(process_task_message("col1\\tcol2"), "col1\tcol2");
    }

    #[test]
    fn test_process_message_multiple_escapes() {
        assert_eq!(
            process_task_message("a\\nb\\tc"),
            "a\nb\tc"
        );
    }

    // ---- ModeSwitchResult tests ----

    #[test]
    fn test_mode_switch_result_serde() {
        let r = crate::types::ModeSwitchResult {
            mode_slug: "code".to_string(),
            reason: Some("need to code".to_string()),
            is_same_mode: false,
        };
        let json = serde_json::to_string(&r).unwrap();
        let parsed: crate::types::ModeSwitchResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.mode_slug, "code");
        assert!(!parsed.is_same_mode);
    }

    // ---- NewTaskResult tests ----

    #[test]
    fn test_new_task_result_serde() {
        let r = crate::types::NewTaskResult {
            mode: "code".to_string(),
            message: "implement feature".to_string(),
            todos: Some("[ ] task1\n[x] task2".to_string()),
        };
        let json = serde_json::to_string(&r).unwrap();
        let parsed: crate::types::NewTaskResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.mode, "code");
        assert!(parsed.todos.is_some());
    }

    // ---- ModeValidation tests ----

    #[test]
    fn test_mode_validation_valid() {
        let v = crate::types::ModeValidation {
            mode_slug: "code".to_string(),
            is_valid: true,
            error: None,
        };
        assert!(v.is_valid);
    }

    // ---- ModeToolError tests ----

    #[test]
    fn test_mode_tool_error_display() {
        let err = ModeToolError::InvalidMode("bad-mode".to_string());
        assert!(format!("{err}").contains("bad-mode"));
    }
}

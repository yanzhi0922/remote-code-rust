//! Plan mode tools: enter_plan_mode, exit_plan_mode.
//!
//! Provides tools for switching between plan mode (read-only, no modifications)
//! and normal execution mode. Plan mode allows the agent to analyze and plan
//! without making any changes to the file system.

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use super::ToolExecutionContext;

/// Plan mode state tracked in the execution context.
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(Default)]
pub enum PlanModeState {
    /// Normal execution mode — tools can make modifications.
    #[default]
    Normal,
    /// Plan mode — only read-only tools are allowed.
    Plan {
        /// The objective for the plan.
        objective: String,
    },
}

impl PlanModeState {
    /// Check if currently in plan mode.
    #[must_use]
    pub fn is_plan_mode(&self) -> bool {
        matches!(self, Self::Plan { .. })
    }
}


/// Enter plan mode.
///
/// Switches to read-only mode where only non-destructive tools are allowed.
/// The agent should analyze the codebase and create a plan without making changes.
///
/// # Errors
/// Returns an error if the objective is missing.
pub fn enter_plan_mode(input: &Value, _context: &ToolExecutionContext) -> Result<String> {
    let objective = input["objective"]
        .as_str()
        .ok_or_else(|| anyhow!("objective is required for plan mode"))?;

    if objective.trim().is_empty() {
        return Err(anyhow!("objective cannot be empty"));
    }

    let plan_id = format!("plan-{}", uuid::Uuid::new_v4().as_simple());

    let constraints = input["constraints"]
        .as_str()
        .unwrap_or("No specific constraints.");

    let scope = input["scope"]
        .as_str()
        .unwrap_or("full project");

    Ok(json!({
        "type": "enter_plan_mode",
        "plan_id": plan_id,
        "objective": objective,
        "scope": scope,
        "constraints": constraints,
        "message": format!("Entering plan mode. Objective: {objective}"),
        "allowed_operations": [
            "read_file",
            "list_directory",
            "search_text",
            "grep",
            "glob",
            "web_fetch",
            "web_search",
            "ask_user",
            "tool_search"
        ],
        "blocked_operations": [
            "write_file",
            "edit_file",
            "replace_in_file",
            "bash_command",
            "notebook_edit"
        ],
        "note": "In plan mode, tools are read-only. No modifications will be made."
    })
    .to_string())
}

/// Exit plan mode and resume normal execution.
///
/// Optionally includes a plan summary that was created during plan mode.
///
/// # Errors
/// Returns an error if the plan summary is invalid.
pub fn exit_plan_mode(input: &Value, _context: &ToolExecutionContext) -> Result<String> {
    let plan_summary = input["plan_summary"].as_str().unwrap_or("");
    let plan_id = input["plan_id"]
        .as_str()
        .unwrap_or("unknown");

    let steps_planned = input["steps_planned"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(json!({
        "type": "exit_plan_mode",
        "plan_id": plan_id,
        "plan_summary": plan_summary,
        "steps_planned": steps_planned,
        "message": "Exiting plan mode. Resuming normal execution.",
        "note": "All tools are now available for execution."
    })
    .to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::path::PathBuf;

    fn test_context() -> ToolExecutionContext {
        ToolExecutionContext {
            cwd: PathBuf::from("/tmp"),
            timeout_ms: 30_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Arc::new(std::sync::Mutex::new(
                rc_core::task_stack::TaskStack::default(),
            )),
        }
    }

    #[test]
    fn plan_mode_state_default_is_normal() {
        assert_eq!(PlanModeState::default(), PlanModeState::Normal);
    }

    #[test]
    fn plan_mode_state_normal_is_not_plan() {
        assert!(!PlanModeState::Normal.is_plan_mode());
    }

    #[test]
    fn plan_mode_state_plan_is_plan() {
        let state = PlanModeState::Plan {
            objective: "test".to_string(),
        };
        assert!(state.is_plan_mode());
    }

    #[test]
    fn enter_plan_mode_requires_objective() {
        let input = json!({});
        let context = test_context();
        let result = enter_plan_mode(&input, &context);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("objective"));
    }

    #[test]
    fn enter_plan_mode_rejects_empty_objective() {
        let input = json!({"objective": ""});
        let context = test_context();
        let result = enter_plan_mode(&input, &context);
        assert!(result.is_err());
    }

    #[test]
    fn enter_plan_mode_returns_plan_id() {
        let input = json!({"objective": "Refactor the codebase"});
        let context = test_context();
        let result = enter_plan_mode(&input, &context).unwrap();
        let parsed: Value = serde_json::from_str(&result).expect("valid json");
        assert!(parsed["plan_id"].as_str().unwrap().starts_with("plan-"));
        assert_eq!(parsed["type"], "enter_plan_mode");
    }

    #[test]
    fn enter_plan_mode_lists_allowed_operations() {
        let input = json!({"objective": "Test objective"});
        let context = test_context();
        let result = enter_plan_mode(&input, &context).unwrap();
        let parsed: Value = serde_json::from_str(&result).expect("valid json");
        let allowed = parsed["allowed_operations"].as_array().expect("allowed array");
        assert!(allowed.len() > 0);
        assert!(allowed.iter().any(|v| v == "read_file"));
    }

    #[test]
    fn enter_plan_mode_lists_blocked_operations() {
        let input = json!({"objective": "Test objective"});
        let context = test_context();
        let result = enter_plan_mode(&input, &context).unwrap();
        let parsed: Value = serde_json::from_str(&result).expect("valid json");
        let blocked = parsed["blocked_operations"].as_array().expect("blocked array");
        assert!(blocked.iter().any(|v| v == "write_file"));
        assert!(blocked.iter().any(|v| v == "bash_command"));
    }

    #[test]
    fn enter_plan_mode_with_constraints() {
        let input = json!({
            "objective": "Refactor",
            "constraints": "No breaking changes",
            "scope": "crates/rc-core"
        });
        let context = test_context();
        let result = enter_plan_mode(&input, &context).unwrap();
        let parsed: Value = serde_json::from_str(&result).expect("valid json");
        assert_eq!(parsed["constraints"], "No breaking changes");
        assert_eq!(parsed["scope"], "crates/rc-core");
    }

    #[test]
    fn exit_plan_mode_returns_normal_mode() {
        let input = json!({});
        let context = test_context();
        let result = exit_plan_mode(&input, &context).unwrap();
        let parsed: Value = serde_json::from_str(&result).expect("valid json");
        assert_eq!(parsed["type"], "exit_plan_mode");
        assert_eq!(parsed["message"], "Exiting plan mode. Resuming normal execution.");
    }

    #[test]
    fn exit_plan_mode_with_plan_summary() {
        let input = json!({
            "plan_summary": "Refactor the module structure",
            "plan_id": "plan-123",
            "steps_planned": ["Step 1: Analyze", "Step 2: Refactor"]
        });
        let context = test_context();
        let result = exit_plan_mode(&input, &context).unwrap();
        let parsed: Value = serde_json::from_str(&result).expect("valid json");
        assert_eq!(parsed["plan_summary"], "Refactor the module structure");
        assert_eq!(parsed["plan_id"], "plan-123");
        let steps = parsed["steps_planned"].as_array().expect("steps array");
        assert_eq!(steps.len(), 2);
    }

    #[test]
    fn exit_plan_mode_without_summary() {
        let input = json!({});
        let context = test_context();
        let result = exit_plan_mode(&input, &context).unwrap();
        let parsed: Value = serde_json::from_str(&result).expect("valid json");
        assert_eq!(parsed["plan_summary"], "");
        assert_eq!(parsed["steps_planned"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn enter_plan_mode_objective_preserved_in_output() {
        let input = json!({"objective": "Fix all bugs in the auth module"});
        let context = test_context();
        let result = enter_plan_mode(&input, &context).unwrap();
        let parsed: Value = serde_json::from_str(&result).expect("valid json");
        assert_eq!(parsed["objective"], "Fix all bugs in the auth module");
    }
}

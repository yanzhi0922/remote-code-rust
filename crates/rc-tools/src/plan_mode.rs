//! Plan mode runtime integration and tool guarding.

use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};
use once_cell::sync::Lazy;
use rc_core::{PermissionMode, ToolCall, ToolResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{ToolExecutionContext, ToolSpec};

static PLAN_MODE_RUNTIME: Lazy<Mutex<Option<Arc<dyn PlanModeRuntime>>>> =
    Lazy::new(|| Mutex::new(None));

const PLAN_MODE_SAFE_TOOLS: &[&str] = &[
    "ask_user",
    "brief",
    "config_read",
    "ctx_inspect",
    "exit_plan_mode",
    "glob",
    "grep",
    "list_directory",
    "list_worktrees",
    "lsp",
    "memory_read",
    "mcp_server_list",
    "mcp_list_resources",
    "mcp_resource_read",
    "read_file",
    "review_artifact",
    "search_text",
    "snip",
    "task_get",
    "task_list",
    "tool_search",
    "verify_plan",
    "web_browser",
    "web_fetch",
    "web_search",
];

const PLAN_MODE_DENIED_READ_CLASS_TOOLS: &[&str] = &[
    "daemon",
    "mcp_auth",
    "mcp_call",
    "remote_trigger",
    "send_message",
    "skill_discover",
    "skill_execute",
    "sleep",
    "synthetic_output",
    "task_create",
    "task_stop",
    "task_update",
    "team_create",
    "team_delete",
    "team_status",
    "terminal_capture",
    "todo_write",
    "tungsten",
    "workflow",
];

/// Runtime snapshot used to make tool-gating decisions.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlanModeRuntimeSnapshot {
    #[serde(default)]
    pub permission_mode: PermissionMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan_file_path: Option<PathBuf>,
}

/// Host-owned runtime seam for real plan-mode state transitions.
pub trait PlanModeRuntime: Send + Sync {
    fn enter_plan_mode(&self, objective: &str) -> Result<String>;
    fn exit_plan_mode(
        &self,
        plan_summary: Option<&str>,
        steps_planned: &[String],
    ) -> Result<String>;
    fn snapshot(&self) -> PlanModeRuntimeSnapshot;

    fn persist_plan_snapshot(&self) -> Result<()> {
        Ok(())
    }
}

/// Configure the active process-scoped plan-mode runtime.
///
/// This mirrors the existing process-scoped tool runtime policy: the host
/// configures a single session runtime before entering the main prompt loop.
pub fn configure_plan_mode_runtime(runtime: Option<Arc<dyn PlanModeRuntime>>) -> Result<()> {
    let mut slot = PLAN_MODE_RUNTIME
        .lock()
        .map_err(|_| anyhow!("plan mode runtime lock poisoned"))?;
    *slot = runtime;
    Ok(())
}

fn current_runtime() -> Option<Arc<dyn PlanModeRuntime>> {
    PLAN_MODE_RUNTIME
        .lock()
        .ok()
        .and_then(|runtime| runtime.clone())
}

pub(crate) fn persist_plan_snapshot_if_active() -> Result<()> {
    if let Some(runtime) = current_runtime() {
        runtime.persist_plan_snapshot()?;
    }
    Ok(())
}

/// Enter plan mode through the host runtime when available.
///
/// # Errors
/// Returns an error if the objective is missing or empty.
pub fn enter_plan_mode(input: &Value, _context: &ToolExecutionContext) -> Result<String> {
    let objective = input["objective"]
        .as_str()
        .ok_or_else(|| anyhow!("objective is required for plan mode"))?
        .trim();

    if objective.is_empty() {
        return Err(anyhow!("objective cannot be empty"));
    }

    if let Some(runtime) = current_runtime() {
        return runtime.enter_plan_mode(objective);
    }

    Ok(format!(
        "Entered plan mode.\n\nObjective: {objective}\n\nPlan mode is active. Stay read-only, inspect the codebase, and design an implementation approach before exiting plan mode."
    ))
}

/// Exit plan mode through the host runtime when available.
///
/// # Errors
/// Returns an error if the host runtime rejects the transition.
pub fn exit_plan_mode(input: &Value, _context: &ToolExecutionContext) -> Result<String> {
    let plan_summary = input.get("plan_summary").and_then(Value::as_str);
    let steps_planned = input["steps_planned"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    if let Some(runtime) = current_runtime() {
        return runtime.exit_plan_mode(plan_summary, &steps_planned);
    }

    Ok("Exited plan mode. You can now proceed with implementation.".to_owned())
}

/// Inject the current plan content into any `exit_plan_mode` tool call before
/// it is persisted or executed. This mirrors the upstream runtime's
/// normalized-tool-input behavior closely enough for resume recovery.
pub fn normalize_exit_plan_mode_tool_calls(tool_calls: &mut [ToolCall]) {
    let Some(plan_file_path) = current_plan_file_path() else {
        return;
    };
    let plan_file_path_string = plan_file_path.display().to_string();
    let plan_content = fs::read_to_string(&plan_file_path).ok();

    for call in tool_calls
        .iter_mut()
        .filter(|call| call.name == "exit_plan_mode")
    {
        if !call.input.is_object() {
            call.input = Value::Object(serde_json::Map::new());
        }
        let Some(input) = call.input.as_object_mut() else {
            continue;
        };
        input
            .entry("plan_file_path".to_owned())
            .or_insert_with(|| Value::String(plan_file_path_string.clone()));
        if let Some(plan_content) = plan_content.as_ref() {
            input
                .entry("plan".to_owned())
                .or_insert_with(|| Value::String(plan_content.clone()));
        }
    }
}

/// Apply plan-mode write restrictions before any tool executes.
#[must_use]
pub fn plan_mode_guard(
    spec: &ToolSpec,
    call: &ToolCall,
    context: &ToolExecutionContext,
    mode: Option<PermissionMode>,
) -> Option<ToolResult> {
    if mode != Some(PermissionMode::Plan) {
        return None;
    }

    if is_plan_file_edit(spec.name.as_str(), call, context)
        || is_safe_plan_mode_tool(spec.name.as_str())
    {
        return None;
    }

    Some(ToolResult {
        content: blocked_tool_message(spec.name.as_str(), current_plan_file_path()),
        is_error: true,
        content_blocks: Vec::new(),
    })
}

fn is_safe_plan_mode_tool(tool_name: &str) -> bool {
    PLAN_MODE_SAFE_TOOLS.contains(&tool_name)
        || (is_read_class_tool(tool_name)
            && !PLAN_MODE_DENIED_READ_CLASS_TOOLS.contains(&tool_name))
}

fn is_read_class_tool(tool_name: &str) -> bool {
    matches!(
        rc_permissions::classify_tool(tool_name),
        rc_permissions::PermissionClass::Read
    )
}

fn is_plan_file_edit(tool_name: &str, call: &ToolCall, context: &ToolExecutionContext) -> bool {
    if !matches!(tool_name, "write_file" | "replace_in_file" | "edit_file") {
        return false;
    }

    let Some(path) = call
        .input
        .get("path")
        .or_else(|| call.input.get("file_path"))
        .and_then(Value::as_str)
    else {
        return false;
    };

    let Some(plan_file_path) = current_plan_file_path() else {
        return false;
    };

    normalize_joined_path(path, &context.cwd) == normalize_path(plan_file_path)
}

pub(crate) fn current_plan_file_path() -> Option<PathBuf> {
    current_runtime().and_then(|runtime| runtime.snapshot().plan_file_path)
}

fn blocked_tool_message(tool_name: &str, plan_file_path: Option<PathBuf>) -> String {
    let plan_file_hint = plan_file_path
        .map(|path| {
            format!(
                "\n\nThe only file you may edit right now is:\n{}",
                path.display()
            )
        })
        .unwrap_or_default();

    format!(
        "Plan mode is active. `{tool_name}` is not allowed right now.\n\nUse read-only tools to inspect the project, update the plan file as needed, and call `exit_plan_mode` when the plan is ready.{plan_file_hint}"
    )
}

fn normalize_joined_path(path: &str, cwd: &Path) -> PathBuf {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        normalize_path(candidate)
    } else {
        normalize_path(cwd.join(candidate))
    }
}

fn normalize_path(path: impl Into<PathBuf>) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.into().components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use serde_json::json;

    static PLAN_MODE_TEST_MUTEX: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    #[derive(Debug)]
    struct StubPlanRuntime {
        plan_file_path: Option<PathBuf>,
    }

    impl PlanModeRuntime for StubPlanRuntime {
        fn enter_plan_mode(&self, objective: &str) -> Result<String> {
            Ok(format!(
                "Entered plan mode for `{objective}`.\nPlan file: {}",
                self.plan_file_path
                    .as_ref()
                    .expect("plan file path")
                    .display()
            ))
        }

        fn exit_plan_mode(
            &self,
            plan_summary: Option<&str>,
            _steps_planned: &[String],
        ) -> Result<String> {
            Ok(format!(
                "Exited plan mode.\nSummary: {}",
                plan_summary.unwrap_or("(none)")
            ))
        }

        fn snapshot(&self) -> PlanModeRuntimeSnapshot {
            PlanModeRuntimeSnapshot {
                permission_mode: PermissionMode::Plan,
                plan_file_path: self.plan_file_path.clone(),
            }
        }
    }

    fn test_context() -> ToolExecutionContext {
        ToolExecutionContext {
            cwd: PathBuf::from("/tmp/workspace"),
            timeout_ms: 30_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Arc::new(std::sync::Mutex::new(
                rc_core::task_stack::TaskStack::default(),
            )),
        }
    }

    #[test]
    fn enter_plan_mode_requires_objective() {
        let _guard = PLAN_MODE_TEST_MUTEX.lock().expect("test mutex");
        let input = json!({});
        let result = enter_plan_mode(&input, &test_context());
        assert!(result.is_err());
    }

    #[test]
    fn enter_plan_mode_uses_runtime_when_configured() {
        let _guard = PLAN_MODE_TEST_MUTEX.lock().expect("test mutex");
        configure_plan_mode_runtime(Some(Arc::new(StubPlanRuntime {
            plan_file_path: Some(PathBuf::from(
                "/tmp/workspace/.remote-code-rust/plans/demo.md",
            )),
        })))
        .expect("configure");

        let result = enter_plan_mode(&json!({"objective": "Refactor auth"}), &test_context())
            .expect("enter plan mode");
        assert!(result.contains("Plan file:"));

        configure_plan_mode_runtime(None).expect("clear runtime");
    }

    #[test]
    fn plan_mode_guard_allows_only_plan_file_edits() {
        let _guard = PLAN_MODE_TEST_MUTEX.lock().expect("test mutex");
        configure_plan_mode_runtime(Some(Arc::new(StubPlanRuntime {
            plan_file_path: Some(PathBuf::from(
                "/tmp/workspace/.remote-code-rust/plans/demo.md",
            )),
        })))
        .expect("configure");

        let spec = ToolSpec {
            name: "write_file".to_owned(),
            protocol_name: "WriteFile".to_owned(),
            permission_tool_name: "Edit".to_owned(),
            description: "write".to_owned(),
            requires_permission: true,
            input_schema: Value::Null,
        };
        let allowed = plan_mode_guard(
            &spec,
            &ToolCall {
                id: "tool-1".to_owned(),
                name: "write_file".to_owned(),
                input: json!({"path": ".remote-code-rust/plans/demo.md", "content": "plan"}),
            },
            &test_context(),
            Some(PermissionMode::Plan),
        );
        assert!(allowed.is_none());

        let blocked = plan_mode_guard(
            &spec,
            &ToolCall {
                id: "tool-2".to_owned(),
                name: "write_file".to_owned(),
                input: json!({"path": "src/main.rs", "content": "oops"}),
            },
            &test_context(),
            Some(PermissionMode::Plan),
        )
        .expect("blocked result");
        assert!(blocked.is_error);
        assert!(blocked.content.contains("only file you may edit"));

        configure_plan_mode_runtime(None).expect("clear runtime");
    }

    #[test]
    fn plan_mode_guard_blocks_todo_write_even_though_it_is_permissionless() {
        let result = plan_mode_guard(
            &ToolSpec {
                name: "todo_write".to_owned(),
                protocol_name: "TodoWrite".to_owned(),
                permission_tool_name: "TodoWrite".to_owned(),
                description: "write todos".to_owned(),
                requires_permission: false,
                input_schema: Value::Null,
            },
            &ToolCall {
                id: "tool-3".to_owned(),
                name: "todo_write".to_owned(),
                input: json!({"todos": []}),
            },
            &test_context(),
            Some(PermissionMode::Plan),
        )
        .expect("blocked");
        assert!(result.is_error);
        assert!(result.content.contains("Plan mode is active"));
    }

    #[test]
    fn normalize_exit_plan_mode_tool_calls_injects_plan_context() {
        let _guard = PLAN_MODE_TEST_MUTEX.lock().expect("test mutex");
        let tempdir = tempfile::tempdir().expect("tempdir");
        let plan_path = tempdir.path().join("demo-plan.md");
        fs::write(&plan_path, "# Plan\n- inspect\n").expect("write plan");
        configure_plan_mode_runtime(Some(Arc::new(StubPlanRuntime {
            plan_file_path: Some(plan_path.clone()),
        })))
        .expect("configure");

        let mut tool_calls = vec![ToolCall {
            id: "tool-1".to_owned(),
            name: "exit_plan_mode".to_owned(),
            input: json!({}),
        }];
        normalize_exit_plan_mode_tool_calls(&mut tool_calls);

        assert_eq!(
            tool_calls[0].input["plan_file_path"].as_str(),
            Some(plan_path.to_string_lossy().as_ref())
        );
        assert_eq!(
            tool_calls[0].input["plan"].as_str(),
            Some("# Plan\n- inspect\n")
        );

        configure_plan_mode_runtime(None).expect("clear runtime");
    }
}

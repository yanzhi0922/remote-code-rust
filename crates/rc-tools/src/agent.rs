//! Agent, send_message, and plan-mode tool implementations.

use std::collections::BTreeSet;
use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use directories::BaseDirs;
use rc_agents::AgentDefinition;
use rc_agents::loader::load_all_agents;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use rc_core::{SubAgentCompletion, SubAgentExecutionRequest};
use rc_permissions::{PermissionBroker, StaticPermissionBroker};

use super::ToolExecutionContext;
use crate::delegate::{DelegationConfig, DelegationContext, DelegationEngine};
use crate::tasks::{
    TaskKind, TaskStatus, allocate_task_id, finish_tracked_task, start_tracked_task,
};
use crate::{ToolSpec, runtime_provider_tool_specs};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DelegateProgressEvent {
    SubtaskStarted {
        task_id: String,
        parent_task_id: Option<String>,
        description: String,
        depth: u32,
    },
    SubtaskProgress {
        task_id: String,
        turn: u32,
        max_turns: u32,
        summary: String,
    },
    SubtaskCompleted {
        task_id: String,
        success: bool,
        output_preview: String,
        turns_used: u32,
    },
    BatchProgress {
        total: usize,
        completed: usize,
        running: usize,
    },
}

#[must_use]
pub fn parse_delegate_progress_event(message: &str) -> Option<DelegateProgressEvent> {
    serde_json::from_str(message).ok()
}

#[must_use]
pub fn render_delegate_progress_event(event: &DelegateProgressEvent) -> String {
    match event {
        DelegateProgressEvent::SubtaskStarted {
            task_id,
            description,
            depth,
            ..
        } => {
            let indent = "  ".repeat(*depth as usize);
            format!("{indent}🔹 [{task_id}] Started: {description}")
        }
        DelegateProgressEvent::SubtaskProgress {
            task_id,
            turn,
            summary,
            ..
        } => format!("  ⏳ [{task_id}] Turn {turn}: {summary}"),
        DelegateProgressEvent::SubtaskCompleted {
            task_id,
            success,
            turns_used,
            ..
        } => {
            let icon = if *success { "✅" } else { "❌" };
            format!("  {icon} [{task_id}] Completed ({turns_used} turns)")
        }
        DelegateProgressEvent::BatchProgress {
            completed, total, ..
        } => format!("  📊 Batch progress: {completed}/{total}"),
    }
}

/// Returns a boxed, `Send` future to break the recursive async chain:
/// `agent_tool → execute_tool_call → delegate_single → execute_tool_call → agent_tool`.
pub(crate) fn agent_tool<'a>(
    input: &'a Value,
    context: &'a ToolExecutionContext,
) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
    Box::pin(async move { agent_tool_inner(input, context).await })
}

#[derive(Debug, Clone, Deserialize)]
struct AgentToolInput {
    prompt: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    subagent_type: Option<String>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    tools: Vec<String>,
    #[serde(default)]
    mode: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    tasks: Vec<String>,
}

async fn agent_tool_inner(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let parsed: AgentToolInput = serde_json::from_value(input.clone())
        .map_err(|error| anyhow!("invalid agent tool input: {error}"))?;

    let mode = parsed.mode.as_deref().unwrap_or("single");

    // Resolve the sub-agent completion provider.
    let sub_agent = match &context.sub_agent {
        Some(provider) => provider.clone(),
        None => {
            let prompt = parsed.prompt.clone();
            return Ok(json!({
                "type": "sub_agent_request",
                "prompt": prompt,
                "description": parsed.description.clone(),
                "subagent_type": parsed.subagent_type.clone(),
                "model": parsed.model.clone(),
                "allowed_tools": parsed.tools.clone(),
                "message": format!(
                    "Sub-agent task: {}. [No provider available for sub-agent execution]",
                    parsed.prompt
                ),
            })
            .to_string());
        }
    };

    match mode {
        "batch" => run_batch_delegation(input, context, sub_agent, &parsed.tools).await,
        _ if sub_agent.supports_agent_execution() => {
            run_resolved_agent_execution(&parsed, context, sub_agent).await
        }
        _ => run_single_delegation(&parsed.prompt, context, sub_agent, &parsed.tools).await,
    }
}

async fn run_resolved_agent_execution(
    input: &AgentToolInput,
    context: &ToolExecutionContext,
    sub_agent: Arc<dyn SubAgentCompletion>,
) -> Result<String> {
    let definition = resolve_agent_definition(input.subagent_type.as_deref(), &context.cwd)?;
    let allowed_tools = resolve_agent_allowed_tools(&definition, &input.tools).await?;
    let title = input
        .description
        .clone()
        .unwrap_or_else(|| truncate_str(&input.prompt, 80));
    let (task_id, parent_task_id, depth) = start_agent_tracking(context, &title)?;
    emit_delegate_event(
        context,
        DelegateProgressEvent::SubtaskStarted {
            task_id: task_id.clone(),
            parent_task_id: parent_task_id.clone(),
            description: title,
            depth,
        },
    );

    let result = sub_agent
        .execute_agent(SubAgentExecutionRequest {
            agent_type: definition.agent_type.clone(),
            task: input.prompt.clone(),
            description: input.description.clone(),
            context: Vec::new(),
            system_prompt: definition.system_prompt.clone(),
            model: input.model.clone().or_else(|| definition.model.clone()),
            max_turns: definition.max_turns,
            allowed_tools,
            working_dir: context.cwd.clone(),
        })
        .await;

    match result {
        Ok(result) => {
            let output = if result.success {
                result.output
            } else {
                format!(
                    "Sub-agent failed after {} turns: {}",
                    result.turns, result.output
                )
            };
            finish_tracked_task(
                &task_id,
                if result.success {
                    TaskStatus::Completed
                } else {
                    TaskStatus::Failed
                },
                Some(&truncate_str(&output, 200)),
                &output,
                Some(result.turns),
            )?;
            emit_delegate_event(
                context,
                DelegateProgressEvent::SubtaskCompleted {
                    task_id,
                    success: result.success,
                    output_preview: truncate_str(&output, 200),
                    turns_used: result.turns,
                },
            );
            Ok(output)
        }
        Err(error) => {
            let output = error.to_string();
            finish_tracked_task(
                &task_id,
                TaskStatus::Failed,
                Some(&truncate_str(&output, 200)),
                &output,
                None,
            )?;
            emit_delegate_event(
                context,
                DelegateProgressEvent::SubtaskCompleted {
                    task_id,
                    success: false,
                    output_preview: truncate_str(&output, 200),
                    turns_used: 0,
                },
            );
            Err(error)
        }
    }
}

/// Delegate a single task using the [`DelegationEngine`].
///
/// Uses the [`TaskStack`] from the execution context to track delegation
/// depth and enforce nesting limits.
async fn run_single_delegation(
    prompt: &str,
    context: &ToolExecutionContext,
    sub_agent: Arc<dyn SubAgentCompletion>,
    allowed_tools: &[String],
) -> Result<String> {
    // Determine current delegation depth from the task stack.
    let depth = {
        let stack = context.task_stack.lock().expect("task_stack lock poisoned");
        stack.depth()
    };

    let engine = DelegationEngine::new(DelegationConfig::default());

    let broker: Arc<dyn PermissionBroker> = Arc::new(StaticPermissionBroker::new(true));

    let delegation_ctx = DelegationContext {
        task: prompt.to_owned(),
        cwd: context.cwd.clone(),
        parent_conversation: Vec::new(),
        depth,
        task_metadata: None,
        allowed_tools: allowed_tools.to_vec(),
        tool_context: context.clone(),
        broker,
    };

    // Build a progress callback that prints to the frontend.
    let progress_cb = build_progress_callback(context);

    let result = engine
        .delegate_single(delegation_ctx, sub_agent, progress_cb)
        .await?;

    if result.success {
        Ok(result.output)
    } else {
        Ok(format!(
            "Sub-agent failed after {} turns: {}",
            result.turns_used, result.output
        ))
    }
}

/// Delegate multiple tasks in parallel using [`DelegationEngine::delegate_batch`].
async fn run_batch_delegation(
    input: &Value,
    context: &ToolExecutionContext,
    sub_agent: Arc<dyn SubAgentCompletion>,
    allowed_tools: &[String],
) -> Result<String> {
    let tasks: Vec<String> = input
        .get("tasks")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    if tasks.is_empty() {
        return Err(anyhow!("batch mode requires a non-empty 'tasks' array"));
    }

    let engine = DelegationEngine::new(DelegationConfig::default());
    let progress_cb = build_progress_callback(context);

    let broker: Arc<dyn PermissionBroker> = Arc::new(StaticPermissionBroker::new(true));

    let (batch_depth, parent_task_id) = {
        let stack = context.task_stack.lock().expect("task_stack lock poisoned");
        if let Some(frame) = stack.current() {
            (frame.depth.saturating_add(1), Some(frame.task_id.clone()))
        } else {
            (0, None)
        }
    };

    let results: Vec<crate::delegate::DelegationResult> = engine
        .delegate_batch(
            &tasks,
            sub_agent,
            &context.cwd,
            allowed_tools,
            batch_depth,
            parent_task_id,
            progress_cb,
            context.clone(),
            broker,
        )
        .await?;

    // Format results as a summary JSON.
    let summary: Vec<Value> = results
        .iter()
        .map(|r| {
            json!({
                "task": r.task,
                "success": r.success,
                "turns_used": r.turns_used,
                "output_preview": truncate_str(&r.output, 200),
            })
        })
        .collect();

    let succeeded = results.iter().filter(|r| r.success).count();
    Ok(json!({
        "type": "batch_delegation_result",
        "total": results.len(),
        "succeeded": succeeded,
        "failed": results.len() - succeeded,
        "results": summary,
    })
    .to_string())
}

/// Build an optional progress callback from the tool execution context.
///
/// Wraps the frontend-provided callback to format progress events as
/// human-readable strings suitable for the [`DelegationEngine`].
fn build_progress_callback(
    context: &ToolExecutionContext,
) -> Option<Arc<dyn Fn(rc_ui_bridge::UiEvent) + Send + Sync>> {
    context.progress_cb.as_ref().map(|cb| {
        let cb = cb.clone();
        Arc::new(move |event: rc_ui_bridge::UiEvent| {
            if let Some(serialized) = serialize_delegate_event(&event) {
                cb(&serialized);
            }
        }) as Arc<dyn Fn(rc_ui_bridge::UiEvent) + Send + Sync>
    })
}

fn serialize_delegate_event(event: &rc_ui_bridge::UiEvent) -> Option<String> {
    let envelope = match event {
        rc_ui_bridge::UiEvent::SubtaskStarted {
            task_id,
            parent_task_id,
            description,
            depth,
        } => DelegateProgressEvent::SubtaskStarted {
            task_id: task_id.clone(),
            parent_task_id: parent_task_id.clone(),
            description: description.clone(),
            depth: *depth,
        },
        rc_ui_bridge::UiEvent::SubtaskProgress {
            task_id,
            turn,
            max_turns,
            summary,
        } => DelegateProgressEvent::SubtaskProgress {
            task_id: task_id.clone(),
            turn: *turn,
            max_turns: *max_turns,
            summary: summary.clone(),
        },
        rc_ui_bridge::UiEvent::SubtaskCompleted {
            task_id,
            success,
            output_preview,
            turns_used,
        } => DelegateProgressEvent::SubtaskCompleted {
            task_id: task_id.clone(),
            success: *success,
            output_preview: output_preview.clone(),
            turns_used: *turns_used,
        },
        rc_ui_bridge::UiEvent::BatchProgress {
            total,
            completed,
            running,
        } => DelegateProgressEvent::BatchProgress {
            total: *total,
            completed: *completed,
            running: *running,
        },
        _ => return None,
    };
    serde_json::to_string(&envelope).ok()
}

fn emit_delegate_event(context: &ToolExecutionContext, event: DelegateProgressEvent) {
    if let Some(cb) = context.progress_cb.as_ref()
        && let Ok(serialized) = serde_json::to_string(&event)
    {
        cb(&serialized);
    }
}

fn resolve_agent_definition(subagent_type: Option<&str>, cwd: &Path) -> Result<AgentDefinition> {
    let user_agents_dir =
        BaseDirs::new().map(|base| base.home_dir().join(".claude").join("agents"));
    let project_agents_dir = cwd.join(".claude").join("agents");
    resolve_agent_definition_from_dirs(
        subagent_type,
        user_agents_dir.as_deref(),
        Some(project_agents_dir.as_path()),
    )
}

fn resolve_agent_definition_from_dirs(
    subagent_type: Option<&str>,
    user_dir: Option<&Path>,
    project_dir: Option<&Path>,
) -> Result<AgentDefinition> {
    let requested_type = subagent_type.unwrap_or("general-purpose");
    let definitions = load_all_agents(user_dir, project_dir);
    if let Some(definition) = definitions
        .active_agents
        .into_iter()
        .find(|definition| definition.agent_type == requested_type)
    {
        return Ok(definition);
    }

    let mut available_agents = definitions
        .all_agents
        .into_iter()
        .map(|definition| definition.agent_type)
        .collect::<Vec<_>>();
    available_agents.sort();
    available_agents.dedup();

    let mut error = format!(
        "unknown subagent_type `{requested_type}`; available agents: {}",
        available_agents.join(", ")
    );
    if !definitions.failed_files.is_empty() {
        let failures = definitions
            .failed_files
            .into_iter()
            .map(|(path, reason)| format!("{path}: {reason}"))
            .collect::<Vec<_>>()
            .join("; ");
        error.push_str(&format!(". failed agent files: {failures}"));
    }

    Err(anyhow!(error))
}

async fn resolve_agent_allowed_tools(
    definition: &AgentDefinition,
    requested_tools: &[String],
) -> Result<Vec<String>> {
    let specs = runtime_provider_tool_specs().await;
    let filtered_by_definition = apply_agent_tool_allowlist(&specs, &definition.tools, true)?;
    let filtered_by_request = if requested_tools.is_empty() {
        filtered_by_definition
    } else {
        apply_agent_tool_allowlist(&filtered_by_definition, requested_tools, false)?
    };

    let denied = collect_matching_tool_names(&filtered_by_request, &definition.disallowed_tools);
    let mut selected = filtered_by_request
        .into_iter()
        .filter(|spec| !denied.contains(&spec.name))
        .map(|spec| spec.name)
        .collect::<Vec<_>>();
    selected.sort();
    selected.dedup();
    Ok(selected)
}

fn apply_agent_tool_allowlist(
    specs: &[ToolSpec],
    allowlist: &[String],
    wildcard_on_empty: bool,
) -> Result<Vec<ToolSpec>> {
    if allowlist.is_empty() && wildcard_on_empty {
        return Ok(specs.to_vec());
    }
    if allowlist.len() == 1 && allowlist[0] == "*" {
        return Ok(specs.to_vec());
    }

    let matched = collect_matching_tool_names(specs, allowlist);
    let unknown = allowlist
        .iter()
        .filter(|requested| !matches_any_tool_alias(specs, requested))
        .cloned()
        .collect::<Vec<_>>();
    if !unknown.is_empty() {
        return Err(anyhow!(
            "unknown agent tool name(s): {}",
            unknown.join(", ")
        ));
    }

    Ok(specs
        .iter()
        .filter(|spec| matched.contains(&spec.name))
        .cloned()
        .collect())
}

fn collect_matching_tool_names(specs: &[ToolSpec], requested: &[String]) -> BTreeSet<String> {
    let requested = requested
        .iter()
        .map(|tool| tool.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    specs
        .iter()
        .filter(|spec| {
            tool_aliases(spec)
                .iter()
                .any(|alias| requested.contains(&alias.to_ascii_lowercase()))
        })
        .map(|spec| spec.name.clone())
        .collect()
}

fn matches_any_tool_alias(specs: &[ToolSpec], requested: &str) -> bool {
    let requested = requested.to_ascii_lowercase();
    specs.iter().any(|spec| {
        tool_aliases(spec)
            .iter()
            .any(|alias| alias.eq_ignore_ascii_case(&requested))
    })
}

fn tool_aliases(spec: &ToolSpec) -> BTreeSet<String> {
    let mut aliases = BTreeSet::from([
        spec.name.clone(),
        spec.protocol_name.clone(),
        spec.permission_tool_name.clone(),
    ]);
    match spec.name.as_str() {
        "read_file" => {
            aliases.insert("Read".to_owned());
        }
        "write_file" => {
            aliases.insert("Write".to_owned());
        }
        "edit_file" | "replace_in_file" => {
            aliases.insert("Edit".to_owned());
        }
        "bash_command" => {
            aliases.insert("Bash".to_owned());
        }
        "glob" => {
            aliases.insert("Glob".to_owned());
        }
        "grep" => {
            aliases.insert("Grep".to_owned());
        }
        "agent" => {
            aliases.insert("Agent".to_owned());
        }
        "send_message" => {
            aliases.insert("SendMessage".to_owned());
        }
        _ => {}
    }
    aliases
}

fn start_agent_tracking(
    context: &ToolExecutionContext,
    title: &str,
) -> Result<(String, Option<String>, u32)> {
    let (parent_task_id, depth) = {
        let stack = context.task_stack.lock().expect("task_stack lock poisoned");
        if let Some(frame) = stack.current() {
            (Some(frame.task_id.clone()), frame.depth.saturating_add(1))
        } else {
            (None, 0)
        }
    };
    let task_id = allocate_task_id();
    start_tracked_task(
        task_id.clone(),
        title,
        parent_task_id.clone(),
        depth,
        TaskKind::Delegation,
        Some("started"),
    )?;
    Ok((task_id, parent_task_id, depth))
}

/// Truncate a string to `max_bytes` bytes, respecting UTF-8 boundaries.
fn truncate_str(text: &str, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        text.to_owned()
    } else {
        let boundary = text
            .char_indices()
            .take_while(|(i, _)| *i < max_bytes)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(max_bytes.min(text.len()));
        format!("{}...", &text[..boundary])
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex as StdMutex};

    use rc_core::{ProviderResponse, UsageSummary};
    use tempfile::tempdir;

    #[derive(Clone)]
    struct RecordingAgentRuntime {
        requests: Arc<StdMutex<Vec<SubAgentExecutionRequest>>>,
        result: rc_core::SubAgentExecutionResult,
    }

    #[async_trait::async_trait]
    impl SubAgentCompletion for RecordingAgentRuntime {
        async fn complete(
            &self,
            _conversation: &[rc_core::ConversationEntry],
        ) -> Result<ProviderResponse> {
            panic!("complete() should not be used when execute_agent is supported")
        }

        fn supports_agent_execution(&self) -> bool {
            true
        }

        async fn execute_agent(
            &self,
            request: SubAgentExecutionRequest,
        ) -> Result<rc_core::SubAgentExecutionResult> {
            self.requests.lock().expect("requests lock").push(request);
            Ok(self.result.clone())
        }
    }

    fn test_context_with_cwd(
        cwd: PathBuf,
        sub_agent: Option<Arc<dyn SubAgentCompletion>>,
    ) -> ToolExecutionContext {
        ToolExecutionContext {
            cwd,
            timeout_ms: 5_000,
            sub_agent,
            progress_cb: None,
            task_stack: Default::default(),
        }
    }

    fn test_context(sub_agent: Option<Arc<dyn SubAgentCompletion>>) -> ToolExecutionContext {
        let tempdir = tempdir().expect("tempdir");
        test_context_with_cwd(PathBuf::from(tempdir.path()), sub_agent)
    }

    #[test]
    fn format_started_event() {
        let event = DelegateProgressEvent::SubtaskStarted {
            task_id: "t1".into(),
            parent_task_id: Some("root".into()),
            description: "fix bug".into(),
            depth: 1,
        };
        assert!(render_delegate_progress_event(&event).contains("Started"));
    }

    #[test]
    fn format_completed_event() {
        let event = DelegateProgressEvent::SubtaskCompleted {
            task_id: "t1".into(),
            success: true,
            turns_used: 3,
            output_preview: "done".into(),
        };
        let label = render_delegate_progress_event(&event);
        assert!(label.contains("✅"));
        assert!(label.contains("3 turns"));
    }

    #[test]
    fn format_batch_progress_event() {
        let event = DelegateProgressEvent::BatchProgress {
            completed: 2,
            total: 5,
            running: 1,
        };
        let label = render_delegate_progress_event(&event);
        assert!(label.contains("2/5"));
    }

    #[test]
    fn delegate_progress_round_trips_json() {
        let event = DelegateProgressEvent::SubtaskStarted {
            task_id: "t1".into(),
            parent_task_id: Some("root".into()),
            description: "fix bug".into(),
            depth: 1,
        };
        let json = serde_json::to_string(&event).expect("serialize event");
        let parsed = parse_delegate_progress_event(&json).expect("parse event");
        match parsed {
            DelegateProgressEvent::SubtaskStarted { parent_task_id, .. } => {
                assert_eq!(parent_task_id.as_deref(), Some("root"));
            }
            _ => panic!("expected started event"),
        }
    }

    #[tokio::test]
    async fn resolved_agent_execution_routes_verification_agent() {
        let requests = Arc::new(StdMutex::new(Vec::new()));
        let runtime: Arc<dyn SubAgentCompletion> = Arc::new(RecordingAgentRuntime {
            requests: Arc::clone(&requests),
            result: rc_core::SubAgentExecutionResult {
                output: "verified".to_owned(),
                success: true,
                turns: 4,
                usage: UsageSummary::default(),
            },
        });
        let context = test_context(Some(runtime));

        let result = agent_tool_inner(
            &json!({
                "prompt": "Review the recent Rust refactor for regressions.",
                "description": "Verify refactor",
                "subagent_type": "verification"
            }),
            &context,
        )
        .await
        .expect("agent tool should succeed");

        assert_eq!(result, "verified");
        let requests = requests.lock().expect("requests lock");
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(request.agent_type, "verification");
        assert_eq!(request.max_turns, 200);
        assert!(
            request
                .system_prompt
                .as_deref()
                .unwrap_or_default()
                .contains("verification specialist")
        );
        assert!(request.allowed_tools.contains(&"read_file".to_owned()));
        assert!(!request.allowed_tools.contains(&"write_file".to_owned()));
        assert!(!request.allowed_tools.contains(&"edit_file".to_owned()));
        assert!(
            !request
                .allowed_tools
                .contains(&"replace_in_file".to_owned())
        );
        assert!(!request.allowed_tools.contains(&"agent".to_owned()));
    }

    #[tokio::test]
    async fn resolved_agent_execution_defaults_to_general_purpose_agent() {
        let requests = Arc::new(StdMutex::new(Vec::new()));
        let runtime: Arc<dyn SubAgentCompletion> = Arc::new(RecordingAgentRuntime {
            requests: Arc::clone(&requests),
            result: rc_core::SubAgentExecutionResult {
                output: "implemented".to_owned(),
                success: true,
                turns: 2,
                usage: UsageSummary::default(),
            },
        });
        let context = test_context(Some(runtime));

        let result = agent_tool_inner(
            &json!({
                "prompt": "Investigate the code path and make the required change."
            }),
            &context,
        )
        .await
        .expect("agent tool should succeed");

        assert_eq!(result, "implemented");
        let requests = requests.lock().expect("requests lock");
        let request = &requests[0];
        assert_eq!(request.agent_type, "general-purpose");
        assert_eq!(request.max_turns, 200);
        assert!(
            request
                .system_prompt
                .as_deref()
                .unwrap_or_default()
                .contains("Complete the task fully")
        );
        assert!(request.allowed_tools.contains(&"agent".to_owned()));
        assert!(request.allowed_tools.contains(&"write_file".to_owned()));
    }

    #[tokio::test]
    async fn unknown_subagent_type_is_rejected() {
        let requests = Arc::new(StdMutex::new(Vec::new()));
        let runtime: Arc<dyn SubAgentCompletion> = Arc::new(RecordingAgentRuntime {
            requests,
            result: rc_core::SubAgentExecutionResult {
                output: String::new(),
                success: true,
                turns: 1,
                usage: UsageSummary::default(),
            },
        });
        let context = test_context(Some(runtime));

        let error = agent_tool_inner(
            &json!({
                "prompt": "Do work.",
                "subagent_type": "unknown-agent"
            }),
            &context,
        )
        .await
        .expect_err("unknown agent type should fail");

        assert!(error.to_string().contains("unknown subagent_type"));
    }

    #[test]
    fn resolve_agent_definition_prefers_project_override_over_user() {
        let temp = tempdir().expect("tempdir");
        let user_dir = temp.path().join("user-agents");
        let project_dir = temp.path().join("project-agents");
        std::fs::create_dir_all(&user_dir).expect("user agents dir");
        std::fs::create_dir_all(&project_dir).expect("project agents dir");
        std::fs::write(
            user_dir.join("reviewer.md"),
            "---\ndescription: User reviewer\ntools: [Read]\n---\nUse the user reviewer prompt.\n",
        )
        .expect("write user reviewer");
        std::fs::write(
            project_dir.join("reviewer.md"),
            "---\ndescription: Project reviewer\ntools: [Read, Grep]\n---\nUse the project reviewer prompt.\n",
        )
        .expect("write project reviewer");

        let definition = resolve_agent_definition_from_dirs(
            Some("reviewer"),
            Some(&user_dir),
            Some(&project_dir),
        )
        .expect("project override should resolve");

        assert_eq!(definition.agent_type, "reviewer");
        assert_eq!(definition.when_to_use, "Project reviewer");
        assert_eq!(definition.tools, vec!["Read", "Grep"]);
        assert_eq!(
            definition.system_prompt.as_deref(),
            Some("Use the project reviewer prompt.")
        );
        assert_eq!(definition.source, rc_agents::AgentSource::Project);
    }

    #[tokio::test]
    async fn resolved_agent_execution_loads_project_agent_definition() {
        let temp = tempdir().expect("tempdir");
        let project_agents_dir = temp.path().join(".claude").join("agents");
        std::fs::create_dir_all(&project_agents_dir).expect("project agents dir");
        std::fs::write(
            project_agents_dir.join("reviewer.md"),
            "---\ndescription: Project reviewer\ntools: [Read]\nmodel: inherit\n---\nUse the project reviewer prompt.\n",
        )
        .expect("write project reviewer");

        let requests = Arc::new(StdMutex::new(Vec::new()));
        let runtime: Arc<dyn SubAgentCompletion> = Arc::new(RecordingAgentRuntime {
            requests: Arc::clone(&requests),
            result: rc_core::SubAgentExecutionResult {
                output: "reviewed".to_owned(),
                success: true,
                turns: 5,
                usage: UsageSummary::default(),
            },
        });
        let context = test_context_with_cwd(temp.path().to_path_buf(), Some(runtime));

        let result = agent_tool_inner(
            &json!({
                "prompt": "Review the custom project agent path.",
                "description": "Project review",
                "subagent_type": "reviewer"
            }),
            &context,
        )
        .await
        .expect("custom project agent should succeed");

        assert_eq!(result, "reviewed");
        let requests = requests.lock().expect("requests lock");
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert_eq!(request.agent_type, "reviewer");
        assert_eq!(request.model.as_deref(), Some("inherit"));
        assert!(
            request
                .system_prompt
                .as_deref()
                .unwrap_or_default()
                .contains("project reviewer prompt")
        );
        assert!(request.allowed_tools.contains(&"read_file".to_owned()));
        assert!(!request.allowed_tools.contains(&"write_file".to_owned()));
    }
}

//! Agent, send_message, and plan-mode tool implementations.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use rc_core::SubAgentCompletion;
use rc_permissions::{PermissionBroker, StaticPermissionBroker};

use super::ToolExecutionContext;
use crate::delegate::{DelegationConfig, DelegationContext, DelegationEngine};

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

async fn agent_tool_inner(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let prompt = input
        .get("prompt")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("agent tool requires a prompt"))?;

    // Parse optional mode ("single" | "batch") and tasks array.
    let mode = input
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("single");
    let allowed_tools: Vec<String> = input
        .get("tools")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();

    // Resolve the sub-agent completion provider.
    let sub_agent = match &context.sub_agent {
        Some(provider) => provider.clone(),
        None => {
            return Ok(json!({
                "type": "sub_agent_request",
                "prompt": prompt,
                "allowed_tools": allowed_tools,
                "message": format!(
                    "Sub-agent task: {}. [No provider available for sub-agent execution]",
                    prompt
                ),
            })
            .to_string());
        }
    };

    match mode {
        "batch" => run_batch_delegation(input, context, sub_agent, &allowed_tools).await,
        _ => run_single_delegation(prompt, context, sub_agent, &allowed_tools).await,
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

pub(crate) fn send_message(input: &Value) -> Result<String> {
    let recipient = input["recipient"]
        .as_str()
        .ok_or_else(|| anyhow!("recipient is required"))?;
    let message = input["message"]
        .as_str()
        .ok_or_else(|| anyhow!("message is required"))?;

    // Simplified implementation: return a JSON structure for the conversation
    // loop to handle actual message delivery via AgentScheduler.
    Ok(json!({
        "type": "agent_message",
        "recipient": recipient,
        "message": message,
        "status": "queued",
        "note": "Message queued for delivery. Actual delivery requires AgentScheduler context."
    })
    .to_string())
}

pub(crate) fn enter_plan_mode(input: &Value) -> Result<String> {
    let objective = input["objective"]
        .as_str()
        .ok_or_else(|| anyhow!("objective is required"))?;

    Ok(json!({
        "type": "enter_plan_mode",
        "objective": objective,
        "message": format!("Entering plan mode. Objective: {objective}"),
        "note": "In plan mode, tools are read-only. No modifications will be made."
    })
    .to_string())
}

pub(crate) fn exit_plan_mode(_input: &Value) -> Result<String> {
    Ok(json!({
        "type": "exit_plan_mode",
        "message": "Exiting plan mode. Resuming normal execution."
    })
    .to_string())
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
}

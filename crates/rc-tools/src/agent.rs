//! Agent, send_message, and plan-mode tool implementations.

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use super::{ToolExecutionContext, execute_tool_call};

pub(crate) async fn agent_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let prompt = input
        .get("prompt")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("agent tool requires a prompt"))?;
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

    // If no sub-agent completion provider is available, fall back to delegation JSON.
    let sub_agent = match &context.sub_agent {
        Some(provider) => provider,
        None => {
            let response = json!({
                "type": "sub_agent_request",
                "prompt": prompt,
                "allowed_tools": allowed_tools,
                "message": format!(
                    "Sub-agent task: {}. [No provider available for sub-agent execution]",
                    prompt
                ),
            });
            return Ok(response.to_string());
        }
    };

    // Create a sub-conversation with a system prompt and the user task.
    let mut sub_conversation = vec![
        rc_core::ConversationEntry::system(
            "You are a sub-agent. Complete the task concisely and return the result.",
        ),
        rc_core::ConversationEntry::user(prompt),
    ];

    // Execute the sub-agent loop with a maximum of 5 turns.
    let max_turns = 5;
    let timeout = std::time::Duration::from_secs(60);

    for turn in 0..max_turns {
        let response = match tokio::time::timeout(timeout, sub_agent.complete(&sub_conversation)).await {
            Ok(Ok(response)) => response,
            Ok(Err(error)) => {
                return Ok(format!(
                    "Sub-agent error on turn {}: {error}",
                    turn + 1
                ));
            }
            Err(_) => {
                return Ok(format!(
                    "Sub-agent timed out after {}s on turn {}.",
                    timeout.as_secs(),
                    turn + 1
                ));
            }
        };

        let assistant_text = response.text.clone();
        sub_conversation.push(rc_core::ConversationEntry::assistant(&assistant_text));

        // If no tool calls, the sub-agent is done.
        if response.tool_calls.is_empty() {
            return Ok(assistant_text);
        }

        // Execute tool calls within the sub-agent context.
        for tool_call in &response.tool_calls {
            let tool_name = &tool_call.name;

            // Check if the tool is in the allowed list.
            if !allowed_tools.is_empty() && !allowed_tools.contains(tool_name) {
                sub_conversation.push(rc_core::ConversationEntry::tool(
                    &tool_call.id,
                    tool_name,
                    "Tool not allowed in sub-agent context",
                    true,
                ));
                continue;
            }

            // Execute the tool call with bypass permissions.
            let broker = rc_permissions::StaticPermissionBroker::new(
                rc_core::PermissionMode::BypassPermissions,
            );
            let result = Box::pin(execute_tool_call(tool_call, context, &broker)).await;

            match result {
                Ok(tool_result) => {
                    let truncated = if tool_result.content.len() > 5000 {
                        format!("{}...[truncated]", &tool_result.content[..5000])
                    } else {
                        tool_result.content
                    };
                    sub_conversation.push(rc_core::ConversationEntry::tool(
                        &tool_call.id,
                        tool_name,
                        &truncated,
                        tool_result.is_error,
                    ));
                }
                Err(error) => {
                    sub_conversation.push(rc_core::ConversationEntry::tool(
                        &tool_call.id,
                        tool_name,
                        format!("Error: {error}"),
                        true,
                    ));
                }
            }
        }
    }

    // Return the last assistant message (or a summary if we ran out of turns).
    let final_response = sub_conversation
        .last()
        .map(|entry| entry.text.clone())
        .unwrap_or_default();

    if final_response.is_empty() {
        Ok(format!(
            "Sub-agent completed {} turns without a final text response.",
            max_turns
        ))
    } else {
        Ok(final_response)
    }
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

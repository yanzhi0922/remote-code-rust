//! System tools: todo_write, config_read, sleep, snip, tool_search,
//! verify_plan, terminal_capture, monitor, brief, ctx_inspect, list_peers.

use std::process::Stdio;

use anyhow::{Context, Result, anyhow};
use claude_core::ToolResult;
use serde_json::{Value, json};
use tokio::io::AsyncReadExt;
use tokio::process::Command;

use super::{
    ToolExecutionContext, runtime_provider_tool_specs, runtime_tool_search_candidate_specs,
};
use crate::search::ToolSearchEngine;

pub(crate) fn todo_write(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let todos = input
        .get("todos")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("todo_write requires a todos array"))?;
    let mut todo_items = Vec::new();
    for todo in todos {
        let id = todo.get("id").and_then(Value::as_str).map(|s| s.to_owned());
        let content = todo
            .get("content")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("each todo must have content"))?;
        let status = todo
            .get("status")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("each todo must have a status"))?;
        if !["pending", "in_progress", "completed"].contains(&status) {
            return Err(anyhow!(
                "invalid todo status '{}': must be pending, in_progress, or completed",
                status
            ));
        }
        let priority = todo.get("priority").and_then(Value::as_str);
        if let Some(p) = &priority
            && !["high", "medium", "low"].contains(p)
        {
            return Err(anyhow!(
                "invalid todo priority '{}': must be high, medium, or low",
                p
            ));
        }
        let mut item = json!({
            "content": content,
            "status": status,
        });
        if let Some(id) = id {
            item["id"] = json!(id);
        }
        if let Some(priority) = priority {
            item["priority"] = json!(priority);
        }
        todo_items.push(item);
    }
    let todos_dir = context.cwd.join(".remote-code-rust");
    std::fs::create_dir_all(&todos_dir)?;
    let todos_path = todos_dir.join("todos.json");
    let content = serde_json::to_string_pretty(&todo_items)?;
    std::fs::write(&todos_path, content)?;
    Ok(format!("Updated {} todo items.", todo_items.len()))
}

pub(crate) fn config_read(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let action = input
        .get("action")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("config requires an action (get or set)"))?;
    let key = input
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("config requires a key"))?;
    let config_dir = context.cwd.join(".remote-code-rust");
    let config_path = config_dir.join("config.json");
    match action {
        "get" => {
            if !config_path.exists() {
                return Ok(json!({key: null}).to_string());
            }
            let content =
                std::fs::read_to_string(&config_path).context("failed to read config file")?;
            let config: Value =
                serde_json::from_str(&content).context("failed to parse config file")?;
            let value = config.get(key).cloned().unwrap_or(Value::Null);
            Ok(json!({key: value}).to_string())
        }
        "set" => {
            let value = input
                .get("value")
                .ok_or_else(|| anyhow!("config set requires a value"))?;
            std::fs::create_dir_all(&config_dir)?;
            let mut config: Value = if config_path.exists() {
                let content = std::fs::read_to_string(&config_path)?;
                serde_json::from_str(&content)?
            } else {
                json!({})
            };
            if let Some(obj) = config.as_object_mut() {
                obj.insert(key.to_owned(), value.clone());
            }
            let content = serde_json::to_string_pretty(&config)?;
            std::fs::write(&config_path, content)?;
            Ok(format!("Set {} in config.", key))
        }
        _ => Err(anyhow!("config action must be 'get' or 'set'")),
    }
}

pub(crate) async fn sleep_tool(input: &Value) -> Result<String> {
    let seconds = input["seconds"]
        .as_u64()
        .ok_or_else(|| anyhow!("seconds is required"))?
        .min(30);

    tokio::time::sleep(std::time::Duration::from_secs(seconds)).await;

    Ok(format!("Slept for {seconds} second(s)."))
}

pub(crate) fn snip_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let content = input["content"]
        .as_str()
        .ok_or_else(|| anyhow!("content is required"))?;
    let label = input["label"].as_str().unwrap_or("snippet");

    let snippets_dir = context.cwd.join(".remote-code-rust").join("snippets");
    std::fs::create_dir_all(&snippets_dir)?;

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let safe_label = label.replace([' ', '/', '\\', ':'], "_");
    let filename = format!("{safe_label}_{timestamp}.txt");
    let filepath = snippets_dir.join(&filename);

    std::fs::write(&filepath, content)?;

    Ok(format!(
        "Snippet saved to .remote-code-rust/snippets/{filename}"
    ))
}

fn build_runtime_tool_search_index(specs: &[crate::ToolSpec]) -> ToolSearchEngine {
    let mut engine = ToolSearchEngine::new();
    for spec in specs {
        let search_terms = spec.tool_search_terms();
        let tags = search_terms.iter().map(String::as_str).collect::<Vec<_>>();
        engine.add_tool(spec.provider_wire_name(), &spec.description, &tags);
    }
    engine
}

fn select_requested_tool_specs<'a>(
    specs: &'a [crate::ToolSpec],
    query: &str,
    max_results: usize,
) -> (Vec<&'a crate::ToolSpec>, Vec<String>) {
    let requested = query
        .strip_prefix("select:")
        .unwrap_or(query)
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .collect::<Vec<_>>();

    let mut selected = Vec::new();
    let mut missing = Vec::new();
    let mut seen = std::collections::BTreeSet::new();

    for requested_name in requested {
        let matched = specs
            .iter()
            .find(|spec| spec.matches_tool_name(requested_name));
        if let Some(spec) = matched {
            if seen.insert(spec.name.clone()) {
                selected.push(spec);
            }
        } else {
            missing.push(requested_name.to_owned());
        }

        if selected.len() >= max_results {
            break;
        }
    }

    (selected, missing)
}

fn tool_search_result_payload(
    query: &str,
    total_deferred_tools: usize,
    matches: &[String],
    missing: &[String],
) -> Value {
    let mut data = json!({
        "matches": matches,
        "query": query,
        "total_deferred_tools": total_deferred_tools,
    });
    if !missing.is_empty()
        && let Some(object) = data.as_object_mut()
    {
        object.insert("missing".to_owned(), json!(missing));
    }

    json!({ "data": data })
}

fn tool_search_result(
    query: &str,
    total_deferred_tools: usize,
    matches: Vec<String>,
    missing: Vec<String>,
) -> ToolResult {
    if matches.is_empty() {
        let mut message = "No matching deferred tools found".to_owned();
        if !missing.is_empty() {
            message.push_str(&format!(
                ". Missing requested tools: {}",
                missing.join(", ")
            ));
        }
        return ToolResult {
            content: message,
            is_error: false,
            content_blocks: Vec::new(),
            follow_up_user_blocks: Vec::new(),
        };
    }

    ToolResult {
        content: tool_search_result_payload(query, total_deferred_tools, &matches, &missing)
            .to_string(),
        is_error: false,
        content_blocks: matches
            .into_iter()
            .map(|tool_name| {
                json!({
                    "type": "tool_reference",
                    "tool_name": tool_name,
                })
            })
            .collect(),
        follow_up_user_blocks: Vec::new(),
    }
}

pub(crate) async fn tool_search_tool(input: &Value) -> Result<ToolResult> {
    let query = input
        .get("query")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("query is required"))?;
    let max_results = input
        .get("max_results")
        .and_then(Value::as_u64)
        .unwrap_or(5) as usize;

    let specs = runtime_tool_search_candidate_specs().await;
    if query.trim_start().starts_with("select:") {
        let (selected, missing) = select_requested_tool_specs(&specs, query, max_results);
        let matches = selected
            .into_iter()
            .map(|spec| spec.provider_wire_name().to_owned())
            .collect::<Vec<_>>();
        return Ok(tool_search_result(query, specs.len(), matches, missing));
    }

    let engine = build_runtime_tool_search_index(&specs);
    let results = engine.search(query, max_results);

    if results.is_empty() {
        Ok(tool_search_result(
            query,
            specs.len(),
            Vec::new(),
            Vec::new(),
        ))
    } else {
        let matches = results
            .iter()
            .map(|result| result.name.clone())
            .collect::<Vec<_>>();
        Ok(tool_search_result(query, specs.len(), matches, Vec::new()))
    }
}

pub(crate) fn verify_plan_tool(input: &Value) -> Result<String> {
    let plan = input
        .get("plan")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("plan is required (array of strings)"))?;
    let completed = input
        .get("completed")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("completed is required (array of booleans)"))?;
    if plan.len() != completed.len() {
        return Err(anyhow!(
            "plan and completed arrays must have the same length ({} vs {})",
            plan.len(),
            completed.len()
        ));
    }
    let mut incomplete = Vec::new();
    let mut done_count = 0usize;
    for (item, is_done) in plan.iter().zip(completed.iter()) {
        let desc = item.as_str().unwrap_or("(invalid)");
        let done = is_done.as_bool().unwrap_or(false);
        if done {
            done_count += 1;
        } else {
            incomplete.push(desc.to_owned());
        }
    }
    let total = plan.len();
    Ok(json!({
        "total_items": total,
        "completed": done_count,
        "incomplete": incomplete,
        "progress_pct": if total > 0 { done_count * 100 / total } else { 100 },
    })
    .to_string())
}

pub(crate) async fn terminal_capture_tool(
    input: &Value,
    context: &ToolExecutionContext,
) -> Result<String> {
    let command = input
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("command is required"))?;
    let mut process = if cfg!(windows) {
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-Command", command]);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.args(["-lc", command]);
        cmd
    };
    process.current_dir(&context.cwd);
    process.stdout(Stdio::piped());
    process.stderr(Stdio::piped());

    let mut child = process.spawn().context("failed to spawn command")?;
    let future = async {
        let mut stdout = String::new();
        let mut stderr = String::new();
        if let Some(mut stream) = child.stdout.take() {
            let _ = stream.read_to_string(&mut stdout).await;
        }
        if let Some(mut stream) = child.stderr.take() {
            let _ = stream.read_to_string(&mut stderr).await;
        }
        let status = child.wait().await?;
        Ok::<_, anyhow::Error>((status.code(), stdout, stderr))
    };
    let (exit_code, stdout, stderr) =
        tokio::time::timeout(std::time::Duration::from_millis(context.timeout_ms), future)
            .await
            .map_err(|_| anyhow!("command timed out after {}ms", context.timeout_ms))??;

    Ok(json!({
        "command": command,
        "exit_code": exit_code,
        "stdout": stdout.trim_end(),
        "stderr": stderr.trim_end(),
    })
    .to_string())
}

pub(crate) fn monitor_tool(input: &Value) -> Result<String> {
    let target = input["target"]
        .as_str()
        .ok_or_else(|| anyhow!("target is required (agents, tasks, or sessions)"))?;
    let interval_ms = input
        .get("interval_ms")
        .and_then(Value::as_u64)
        .unwrap_or(1000);

    let snapshot = match target {
        "agents" => {
            // Query running processes for agent-like activity.
            let mut agent_processes = Vec::new();
            if cfg!(windows) {
                if let Ok(output) = std::process::Command::new("tasklist")
                    .args(["/FO", "CSV", "/NH"])
                    .output()
                {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    for line in stdout.lines() {
                        let parts: Vec<&str> = line.split("\",\"").collect();
                        if parts.len() >= 2 {
                            let name = parts[0].trim_matches('"');
                            if name.to_lowercase().contains("remote-code")
                                || name.to_lowercase().contains("node")
                                || name.to_lowercase().contains("python")
                            {
                                agent_processes.push(json!({
                                    "name": name,
                                    "pid": parts[1].trim_matches('"').parse::<u64>().unwrap_or(0),
                                }));
                            }
                        }
                    }
                }
            } else if let Ok(output) = std::process::Command::new("ps").args(["aux"]).output() {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines().take(50) {
                    if line.contains("remote-code") || line.contains("agent") {
                        agent_processes.push(json!({ "entry": line.trim() }));
                    }
                }
            }
            json!({
                "target": "agents",
                "interval_ms": interval_ms,
                "processes": agent_processes,
                "count": agent_processes.len(),
            })
        }
        "tasks" => {
            // Read tasks from the task file if it exists.
            let tasks_dir = std::env::temp_dir().join("remote-code-rust").join("tasks");
            let mut task_list = Vec::new();
            if tasks_dir.exists()
                && let Ok(entries) = std::fs::read_dir(&tasks_dir)
            {
                for entry in entries.flatten() {
                    if let Some(name) = entry.file_name().to_str()
                        && name.ends_with(".json")
                        && let Ok(content) = std::fs::read_to_string(entry.path())
                        && let Ok(task) = serde_json::from_str::<Value>(&content)
                    {
                        task_list.push(task);
                    }
                }
            }
            json!({
                "target": "tasks",
                "interval_ms": interval_ms,
                "tasks": task_list,
                "count": task_list.len(),
            })
        }
        "sessions" => {
            // List session files in the .remote-code-rust directory.
            let mut session_list = Vec::new();
            let sessions_dir = std::env::temp_dir()
                .join("remote-code-rust")
                .join("sessions");
            if sessions_dir.exists()
                && let Ok(entries) = std::fs::read_dir(&sessions_dir)
            {
                for entry in entries.flatten() {
                    if let Some(name) = entry.file_name().to_str()
                        && name.ends_with(".json")
                    {
                        session_list.push(json!({ "id": name.trim_end_matches(".json") }));
                    }
                }
            }
            json!({
                "target": "sessions",
                "interval_ms": interval_ms,
                "sessions": session_list,
                "count": session_list.len(),
            })
        }
        _ => return Err(anyhow!("target must be 'agents', 'tasks', or 'sessions'")),
    };
    Ok(snapshot.to_string())
}

pub(crate) fn brief_tool(input: &Value) -> Result<String> {
    let content = input["content"]
        .as_str()
        .ok_or_else(|| anyhow!("content is required"))?;
    let max_length = input
        .get("max_length")
        .and_then(Value::as_u64)
        .unwrap_or(500) as usize;

    if content.len() <= max_length {
        return Ok(content.to_owned());
    }

    let truncated: String = content.chars().take(max_length).collect();
    Ok(format!(
        "{}\n\n[...truncated from {} to {} chars]",
        truncated,
        content.len(),
        max_length
    ))
}

pub(crate) async fn ctx_inspect_tool(input: &Value) -> Result<String> {
    let action = input["action"]
        .as_str()
        .ok_or_else(|| anyhow!("action is required (tokens, messages, or tools)"))?;

    let specs = runtime_provider_tool_specs().await;
    match action {
        // NOTE: Token and message counts are placeholder implementations.
        // They require access to the conversation context and tokenizer which
        // are not available at the tool execution layer. The "unavailable"
        // status makes this explicit rather than returning misleading values.
        "tokens" => Ok(json!({
            "estimated_tokens": "unavailable",
            "note": "Token counting is not available in this context. It requires a model-specific tokenizer and conversation state."
        })
        .to_string()),
        "messages" => Ok(json!({
            "message_count": "unavailable",
            "note": "Message count is not available in this context. It requires conversation state from the active session."
        })
        .to_string()),
        "tools" => Ok(json!({
            "total_tools": specs.len(),
            "tools": specs.iter().map(|s| &s.name).collect::<Vec<_>>(),
        })
        .to_string()),
        _ => Err(anyhow!("action must be 'tokens', 'messages', or 'tools'")),
    }
}

pub(crate) async fn list_peers_tool(input: &Value) -> Result<String> {
    super::team_runtime::list_peers(input).await
}

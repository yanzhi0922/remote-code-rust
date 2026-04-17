//! System tools: todo_write, config_read, sleep, snip, tool_search,
//! verify_plan, terminal_capture, monitor, brief, ctx_inspect, list_peers.

use std::process::Stdio;

use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};
use tokio::io::AsyncReadExt;
use tokio::process::Command;

use super::{ToolExecutionContext, ToolRegistry, runtime_builtin_tool_specs};

pub(crate) fn todo_write(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let todos = input
        .get("todos")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("todo_write requires a todos array"))?;
    let mut todo_items = Vec::new();
    for todo in todos {
        let id = todo
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("each todo must have an id"))?;
        let text = todo
            .get("text")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("each todo must have text"))?;
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
        todo_items.push(json!({
            "id": id,
            "text": text,
            "status": status,
        }));
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

pub(crate) fn tool_search_tool(input: &Value) -> Result<String> {
    let query = input
        .get("query")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("query is required"))?;
    let max_results = input
        .get("max_results")
        .and_then(Value::as_u64)
        .unwrap_or(5) as usize;

    // Use BM25 search engine for relevance-ranked results.
    let registry = ToolRegistry::new();
    let results = registry.search(query, max_results);

    if results.is_empty() {
        // Fallback: return all tools with a note
        let specs = runtime_builtin_tool_specs();
        let matches: Vec<Value> = specs
            .iter()
            .take(max_results)
            .map(|spec| {
                json!({
                    "name": spec.name,
                    "protocol_name": spec.protocol_name,
                    "description": spec.description,
                })
            })
            .collect();
        Ok(json!({
            "query": query,
            "results": matches,
            "note": "No BM25 matches found. Showing first available tools."
        })
        .to_string())
    } else {
        let matches: Vec<Value> = results
            .iter()
            .map(|r| {
                json!({
                    "name": r.name,
                    "score": format!("{:.4}", r.score),
                    "description": r.description,
                })
            })
            .collect();
        Ok(json!({
            "query": query,
            "results": matches,
        })
        .to_string())
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
            } else if let Ok(output) = std::process::Command::new("ps")
                .args(["aux"])
                .output()
            {
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
            if tasks_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&tasks_dir) {
                    for entry in entries.flatten() {
                        if let Some(name) = entry.file_name().to_str() {
                            if name.ends_with(".json") {
                                if let Ok(content) = std::fs::read_to_string(entry.path()) {
                                    if let Ok(task) = serde_json::from_str::<Value>(&content) {
                                        task_list.push(task);
                                    }
                                }
                            }
                        }
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
            let sessions_dir = std::env::temp_dir().join("remote-code-rust").join("sessions");
            if sessions_dir.exists() {
                if let Ok(entries) = std::fs::read_dir(&sessions_dir) {
                    for entry in entries.flatten() {
                        if let Some(name) = entry.file_name().to_str() {
                            if name.ends_with(".json") {
                                session_list.push(json!({ "id": name.trim_end_matches(".json") }));
                            }
                        }
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

pub(crate) fn ctx_inspect_tool(input: &Value) -> Result<String> {
    let action = input["action"]
        .as_str()
        .ok_or_else(|| anyhow!("action is required (tokens, messages, or tools)"))?;

    let specs = runtime_builtin_tool_specs();
    match action {
        "tokens" => Ok(json!({
            "estimated_tokens": "N/A (requires tokenizer)",
            "note": "Token counting requires a model-specific tokenizer."
        })
        .to_string()),
        "messages" => Ok(json!({
            "message_count": 0,
            "note": "Message count requires conversation context."
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

pub(crate) fn list_peers_tool() -> Result<String> {
    // Check for active swarm/team peers in the state directory.
    let state_dir = std::env::temp_dir().join("remote-code-rust").join("teams");
    let mut peers = Vec::new();
    if state_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&state_dir) {
            for entry in entries.flatten() {
                if let Some(name) = entry.file_name().to_str() {
                    if name.ends_with(".json") {
                        if let Ok(content) = std::fs::read_to_string(entry.path()) {
                            if let Ok(team) = serde_json::from_str::<Value>(&content) {
                                if let Some(members) = team.get("agents").and_then(Value::as_array)
                                {
                                    for member in members {
                                        peers.push(json!({
                                            "name": member.get("name").and_then(Value::as_str).unwrap_or("unknown"),
                                            "role": member.get("role").and_then(Value::as_str).unwrap_or("worker"),
                                            "team": team.get("objective").and_then(Value::as_str).unwrap_or("unknown"),
                                        }));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    if peers.is_empty() {
        Ok(json!({
            "peers": [],
            "message": "No peers registered in current context. Use team_create to create a team."
        })
        .to_string())
    } else {
        Ok(json!({
            "peers": peers,
            "count": peers.len(),
        })
        .to_string())
    }
}

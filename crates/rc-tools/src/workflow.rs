//! Workflow, cron scheduling, and daemon management tools.

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use super::ToolExecutionContext;

pub(crate) fn schedule_cron_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let action = input["action"].as_str().unwrap_or("create");

    let crons_dir = context.cwd.join(".remote-code-rust");
    std::fs::create_dir_all(&crons_dir)?;
    let crons_path = crons_dir.join("crons.json");

    let mut crons: Vec<Value> = if crons_path.exists() {
        let content = std::fs::read_to_string(&crons_path)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        Vec::new()
    };

    match action {
        "create" | "add" => {
            let schedule = input["schedule"]
                .as_str()
                .ok_or_else(|| anyhow!("schedule is required (cron expression)"))?;
            let command = input["command"]
                .as_str()
                .ok_or_else(|| anyhow!("command is required"))?;
            let description = input["description"].as_str().unwrap_or("");

            let entry = json!({
                "id": format!("cron-{}", crons.len() + 1),
                "schedule": schedule,
                "command": command,
                "description": description,
                "created_at": std::time::SystemTime::now()
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            });
            crons.push(entry);

            let content = serde_json::to_string_pretty(&crons)?;
            std::fs::write(&crons_path, content)?;

            Ok(format!(
                "Cron job saved: '{}' → {}",
                schedule, command
            ))
        }
        "list" => {
            Ok(json!({
                "crons": crons,
                "count": crons.len(),
            }).to_string())
        }
        "delete" | "remove" => {
            let id = input["id"]
                .as_str()
                .or_else(|| input["schedule"].as_str())
                .ok_or_else(|| anyhow!("id or schedule is required for delete"))?;

            let before = crons.len();
            crons.retain(|c| {
                c["id"].as_str() != Some(id) && c["schedule"].as_str() != Some(id)
            });

            if crons.len() < before {
                let content = serde_json::to_string_pretty(&crons)?;
                std::fs::write(&crons_path, content)?;
                Ok("Cron job deleted.".to_string())
            } else {
                Ok(format!("Cron job '{id}' not found."))
            }
        }
        _ => Err(anyhow!("action must be 'create', 'list', or 'delete'")),
    }
}

pub(crate) fn workflow_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let action = input["action"]
        .as_str()
        .ok_or_else(|| anyhow!("action is required (create, run, or status)"))?;
    let name = input["name"]
        .as_str()
        .ok_or_else(|| anyhow!("name is required"))?;

    let wf_dir = context.cwd.join(".remote-code-rust");
    std::fs::create_dir_all(&wf_dir)?;
    let wf_path = wf_dir.join("workflows.json");

    let mut workflows: Vec<Value> = if wf_path.exists() {
        let content = std::fs::read_to_string(&wf_path)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        Vec::new()
    };

    match action {
        "create" => {
            let steps: Vec<String> = input
                .get("steps")
                .and_then(Value::as_array)
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            let description = input["description"].as_str().unwrap_or("");
            let entry = json!({
                "name": name,
                "description": description,
                "steps": steps,
                "status": "created",
                "created_at": std::time::SystemTime::now()
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            });
            workflows.push(entry);
            let content = serde_json::to_string_pretty(&workflows)?;
            std::fs::write(&wf_path, content)?;
            Ok(format!("Workflow '{name}' created with {} steps.", steps.len()))
        }
        "run" => {
            let wf = workflows
                .iter()
                .find(|w| w["name"].as_str() == Some(name))
                .cloned();
            match wf {
                Some(w) => {
                    let steps = w["steps"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|v| v.as_str().map(String::from))
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default();

                    // Execute each step sequentially.
                    let mut results = Vec::new();
                    let mut all_success = true;
                    for (i, step) in steps.iter().enumerate() {
                        let output = std::process::Command::new(if cfg!(windows) { "cmd" } else { "sh" })
                            .arg(if cfg!(windows) { "/C" } else { "-c" })
                            .arg(step)
                            .current_dir(&context.cwd)
                            .output();

                        let result = match output {
                            Ok(out) => {
                                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                                let success = out.status.success();
                                if !success {
                                    all_success = false;
                                }
                                json!({
                                    "step": i + 1,
                                    "command": step,
                                    "success": success,
                                    "stdout": stdout.chars().take(2000).collect::<String>(),
                                    "stderr": stderr.chars().take(1000).collect::<String>(),
                                })
                            }
                            Err(e) => {
                                all_success = false;
                                json!({
                                    "step": i + 1,
                                    "command": step,
                                    "success": false,
                                    "error": e.to_string(),
                                })
                            }
                        };
                        results.push(result);
                    }

                    // Update workflow status.
                    let wf_mut = workflows
                        .iter_mut()
                        .find(|w| w["name"].as_str() == Some(name));
                    if let Some(w) = wf_mut {
                        w["status"] = if all_success { json!("completed") } else { json!("failed") };
                        w["last_run"] = json!(std::time::SystemTime::now()
                            .duration_since(std::time::SystemTime::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0));
                        let content = serde_json::to_string_pretty(&workflows)?;
                        std::fs::write(&wf_path, content)?;
                    }

                    Ok(json!({
                        "workflow": name,
                        "status": if all_success { "completed" } else { "failed" },
                        "steps_executed": results.len(),
                        "results": results,
                    }).to_string())
                }
                None => Err(anyhow!("workflow '{name}' not found")),
            }
        }
        "status" => {
            let wf = workflows.iter().find(|w| w["name"].as_str() == Some(name));
            match wf {
                Some(w) => Ok(serde_json::to_string_pretty(w)?),
                None => Ok(json!({
                    "name": name,
                    "status": "not_found",
                    "message": format!("Workflow '{name}' does not exist.")
                })
                .to_string()),
            }
        }
        "list" => {
            let names: Vec<Value> = workflows
                .iter()
                .map(|w| json!({
                    "name": w["name"],
                    "status": w["status"],
                    "steps": w["steps"].as_array().map(|a| a.len()).unwrap_or(0),
                }))
                .collect();
            Ok(json!({
                "workflows": names,
                "count": names.len(),
            }).to_string())
        }
        "delete" => {
            let before = workflows.len();
            workflows.retain(|w| w["name"].as_str() != Some(name));
            if workflows.len() < before {
                let content = serde_json::to_string_pretty(&workflows)?;
                std::fs::write(&wf_path, content)?;
                Ok(format!("Workflow '{name}' deleted."))
            } else {
                Ok(format!("Workflow '{name}' not found."))
            }
        }
        _ => Err(anyhow!("action must be 'create', 'run', 'status', 'list', or 'delete'")),
    }
}

pub(crate) fn daemon_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let action = input["action"]
        .as_str()
        .ok_or_else(|| anyhow!("action is required (start, stop, status, list, restart, or logs)"))?;

    let daemon_dir = context.cwd.join(".remote-code-rust");
    std::fs::create_dir_all(&daemon_dir)?;
    let daemon_path = daemon_dir.join("daemons.json");

    let mut daemons: Vec<Value> = if daemon_path.exists() {
        let content = std::fs::read_to_string(&daemon_path)?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        Vec::new()
    };

    match action {
        "start" => {
            let command = input["command"]
                .as_str()
                .ok_or_else(|| anyhow!("command is required for start action"))?;

            // Create log files for stdout/stderr.
            let daemon_id = format!("daemon-{}", daemons.len() + 1);
            let stdout_path = daemon_dir.join(format!("{daemon_id}.stdout.log"));
            let stderr_path = daemon_dir.join(format!("{daemon_id}.stderr.log"));

            let stdout_file = std::fs::File::create(&stdout_path)?;
            let stderr_file = std::fs::File::create(&stderr_path)?;

            // Spawn the process in the background.
            let shell = if cfg!(windows) { "cmd" } else { "sh" };
            let flag = if cfg!(windows) { "/C" } else { "-c" };
            let child = std::process::Command::new(shell)
                .arg(flag)
                .arg(command)
                .current_dir(&context.cwd)
                .stdout(std::process::Stdio::from(stdout_file))
                .stderr(std::process::Stdio::from(stderr_file))
                .spawn();

            let (pid, status) = match child {
                Ok(c) => {
                    let pid = c.id();
                    // We cannot hold the Child across the match because we need to
                    // store data in the JSON. The process will continue running in
                    // the background after the Child handle is dropped (on Unix it
                    // is reparented to init; on Windows it continues independently).
                    drop(c);
                    (Some(pid as u64), "running")
                }
                Err(_) => (None, "failed_to_start"),
            };

            let now = std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);

            let entry = json!({
                "id": daemon_id,
                "command": command,
                "status": status,
                "pid": pid,
                "started_at": now,
                "stdout_log": stdout_path.to_string_lossy(),
                "stderr_log": stderr_path.to_string_lossy(),
            });
            daemons.push(entry);
            let content = serde_json::to_string_pretty(&daemons)?;
            std::fs::write(&daemon_path, content)?;

            if let Some(p) = pid {
                Ok(format!("Daemon '{daemon_id}' started: {command} (pid={p})"))
            } else {
                Ok(format!("Daemon '{daemon_id}' failed to start: {command}"))
            }
        }
        "stop" => {
            let id = input["id"].as_str();
            let count_before = daemons.len();

            daemons.retain(|d| {
                let should_remove = if let Some(id_val) = id {
                    d["id"].as_str() == Some(id_val) || d["command"].as_str() == Some(id_val)
                } else {
                    // Stop all: remove everything.
                    true
                };

                // Try to kill the process if we have a PID.
                if should_remove
                    && let Some(pid) = d["pid"].as_u64()
                {
                    let _ = kill_process(pid as u32);
                }
                !should_remove
            });

            let stopped = count_before - daemons.len();
            let content = serde_json::to_string_pretty(&daemons)?;
            std::fs::write(&daemon_path, content)?;
            Ok(format!("Stopped {stopped} daemon(s)."))
        }
        "status" => {
            let id = input["id"].as_str();
            match id {
                Some(id_val) => {
                    let daemon = daemons.iter().find(|d| {
                        d["id"].as_str() == Some(id_val) || d["command"].as_str() == Some(id_val)
                    });
                    match daemon {
                        Some(d) => {
                            // Check if the process is still alive.
                            let mut d = d.clone();
                            if let Some(pid) = d["pid"].as_u64()
                                && !is_process_alive(pid as u32)
                            {
                                d["status"] = json!("stopped");
                            }
                            Ok(serde_json::to_string_pretty(&d)?)
                        }
                        None => Ok(json!({
                            "id": id_val,
                            "status": "not_found",
                        }).to_string()),
                    }
                }
                None => Ok(serde_json::to_string_pretty(&daemons)?),
            }
        }
        "list" => {
            let summary: Vec<Value> = daemons.iter().map(|d| {
                let mut s = json!({
                    "id": d["id"],
                    "command": d["command"],
                    "status": d["status"],
                    "pid": d["pid"],
                });
                // Check liveness.
                if let Some(pid) = d["pid"].as_u64()
                    && !is_process_alive(pid as u32)
                {
                    s["status"] = json!("stopped");
                }
                s
            }).collect();
            Ok(json!({
                "daemons": summary,
                "count": summary.len(),
            }).to_string())
        }
        "restart" => {
            let id = input["id"]
                .as_str()
                .ok_or_else(|| anyhow!("id is required for restart action"))?;

            let daemon = daemons.iter().find(|d| {
                d["id"].as_str() == Some(id) || d["command"].as_str() == Some(id)
            }).cloned();

            match daemon {
                Some(d) => {
                    let command = d["command"].as_str().unwrap_or("").to_string();
                    // Stop the old one.
                    if let Some(pid) = d["pid"].as_u64() {
                        let _ = kill_process(pid as u32);
                    }
                    // Remove old entry.
                    daemons.retain(|e| e["id"].as_str() != d["id"].as_str());
                    let content = serde_json::to_string_pretty(&daemons)?;
                    std::fs::write(&daemon_path, content)?;
                    // Start a new one with the same command.
                    drop(d);
                    let restart_input = json!({
                        "action": "start",
                        "command": command,
                    });
                    daemon_tool(&restart_input, context)
                }
                None => Err(anyhow!("daemon '{id}' not found")),
            }
        }
        "logs" => {
            let id = input["id"]
                .as_str()
                .ok_or_else(|| anyhow!("id is required for logs action"))?;
            let lines = input["lines"].as_u64().unwrap_or(50) as usize;

            let daemon = daemons.iter().find(|d| {
                d["id"].as_str() == Some(id) || d["command"].as_str() == Some(id)
            });

            match daemon {
                Some(d) => {
                    let stdout_log = d["stdout_log"].as_str().unwrap_or("");
                    let stderr_log = d["stderr_log"].as_str().unwrap_or("");

                    let stdout_content = if std::path::Path::new(stdout_log).exists() {
                        read_last_n_lines(stdout_log, lines)
                    } else {
                        String::new()
                    };
                    let stderr_content = if std::path::Path::new(stderr_log).exists() {
                        read_last_n_lines(stderr_log, lines)
                    } else {
                        String::new()
                    };

                    Ok(json!({
                        "id": id,
                        "stdout": stdout_content,
                        "stderr": stderr_content,
                    }).to_string())
                }
                None => Err(anyhow!("daemon '{id}' not found")),
            }
        }
        _ => Err(anyhow!("action must be 'start', 'stop', 'status', 'list', 'restart', or 'logs'")),
    }
}

/// Try to kill a process by PID (cross-platform best-effort).
pub(crate) fn kill_process(pid: u32) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        std::process::Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .output()?;
    }
    #[cfg(not(windows))]
    {
        // Use the POSIX kill command.
        let _ = std::process::Command::new("kill")
            .arg(pid.to_string())
            .output();
    }
    Ok(())
}

/// Check if a process is still alive (cross-platform best-effort).
pub(crate) fn is_process_alive(pid: u32) -> bool {
    #[cfg(windows)]
    {
        let output = std::process::Command::new("tasklist")
            .args(["/FI", &format!("PID eq {pid}"), "/NH"])
            .output();
        match output {
            Ok(out) => {
                let text = String::from_utf8_lossy(&out.stdout);
                text.contains(&pid.to_string())
            }
            Err(_) => false,
        }
    }
    #[cfg(not(windows))]
    {
        // kill -0 checks existence without sending a signal.
        let output = std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .output();
        match output {
            Ok(out) => out.status.success(),
            Err(_) => false,
        }
    }
}

/// Read the last N lines from a file.
pub(crate) fn read_last_n_lines(path: &str, n: usize) -> String {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return String::new(),
    };
    let lines: Vec<&str> = content.lines().collect();
    let start = if lines.len() > n { lines.len() - n } else { 0 };
    lines[start..].join("\n")
}

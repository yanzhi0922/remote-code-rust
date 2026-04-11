//! Miscellaneous tools: ask_user, lsp_tool, notebook_edit, skill_discover,
//! team_create/status, remote_trigger, tungsten, overflow_test, synthetic_output,
//! skill_execute, voice_input.

use std::process::Stdio;

use anyhow::{Result, anyhow, Context};
use serde_json::{Value, json};
use tokio::io::AsyncReadExt;
use tokio::process::Command;

use super::ToolExecutionContext;

pub(crate) fn ask_user(input: &Value, _context: &ToolExecutionContext) -> Result<String> {
    let question = input
        .get("question")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("ask_user requires a question"))?;
    let suggestions: Vec<String> = input
        .get("suggestions")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    Ok(json!({
        "type": "ask_user",
        "question": question,
        "suggestions": suggestions,
        "message": "Waiting for user input. In headless mode, please provide the answer via the input stream."
    })
    .to_string())
}

pub(crate) async fn lsp_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let action = input["action"]
        .as_str()
        .ok_or_else(|| anyhow!("action is required"))?;
    let file_path = input["file_path"]
        .as_str()
        .ok_or_else(|| anyhow!("file_path is required"))?;

    let client = super::lsp::LspClient::new(&context.cwd);

    match action {
        "definitions" => {
            let symbol = input["symbol"]
                .as_str()
                .ok_or_else(|| anyhow!("symbol is required for definitions action"))?;
            let locations = client.find_definitions(symbol, Some(file_path))?;
            if locations.is_empty() {
                Ok(format!("No definitions found for '{symbol}'."))
            } else {
                Ok(super::lsp::format_locations(&locations))
            }
        }
        "references" => {
            let symbol = input["symbol"]
                .as_str()
                .ok_or_else(|| anyhow!("symbol is required for references action"))?;
            let locations = client.find_references(symbol)?;
            if locations.is_empty() {
                Ok(format!("No references found for '{symbol}'."))
            } else {
                Ok(super::lsp::format_locations(&locations))
            }
        }
        "hover" => {
            let symbol = input["symbol"]
                .as_str()
                .ok_or_else(|| anyhow!("symbol is required for hover action"))?;
            client.hover(file_path, symbol)
        }
        "completion" => {
            let line = input.get("line").and_then(Value::as_u64).unwrap_or(1);
            let column = input.get("column").and_then(Value::as_u64).unwrap_or(1);
            let suggestions = client.completion(file_path, line as u32, column as u32)?;
            Ok(super::lsp::format_completions(&suggestions))
        }
        "diagnostics" => {
            let diagnostics = client.diagnostics(file_path).await?;
            let result = super::lsp::format_diagnostics(&diagnostics);
            // Limit output size.
            Ok(result.chars().take(10_000).collect())
        }
        _ => Err(anyhow!("Unknown LSP action: {action}")),
    }
}

pub(crate) fn notebook_edit(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let path = input["path"]
        .as_str()
        .ok_or_else(|| anyhow!("path is required"))?;
    let cell_index = input["cell_index"]
        .as_u64()
        .ok_or_else(|| anyhow!("cell_index is required"))? as usize;
    let new_source = input["new_source"]
        .as_str()
        .ok_or_else(|| anyhow!("new_source is required"))?;

    let target = super::file_ops::resolve_workspace_path(&context.cwd, Some(path))?;
    let content = std::fs::read_to_string(&target)
        .with_context(|| format!("failed to read notebook {}", target.display()))?;
    let mut notebook: Value = serde_json::from_str(&content)
        .with_context(|| format!("failed to parse notebook {}", target.display()))?;

    let cells = notebook
        .get_mut("cells")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| anyhow!("notebook has no cells array"))?;

    if cell_index >= cells.len() {
        return Err(anyhow!(
            "cell_index {} out of range ({} cells)",
            cell_index,
            cells.len()
        ));
    }

    let cell = &mut cells[cell_index];

    // Update cell_type if provided
    if let Some(cell_type) = input["cell_type"].as_str() {
        cell["cell_type"] = json!(cell_type);
    }

    // Update source – store as a single string (valid in nbformat)
    cell["source"] = json!(new_source);

    // Clear outputs for code cells
    if cell
        .get("cell_type")
        .and_then(Value::as_str)
        .is_some_and(|ct| ct == "code")
    {
        cell["outputs"] = json!([]);
        cell["execution_count"] = Value::Null;
    }

    let output = serde_json::to_string_pretty(&notebook)?;
    std::fs::write(&target, output)?;

    Ok(format!(
        "Updated cell {} in {}",
        cell_index,
        target.display()
    ))
}

pub(crate) fn skill_discover(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    // Search common skill locations
    let search_dirs = [
        context.cwd.join(".roo"),
        context.cwd.join(".remote-code-rust"),
    ];

    let mut all_skills = Vec::new();
    for dir in &search_dirs {
        if dir.exists() {
            match rc_skills::discover_skills(dir) {
                Ok(skills) => {
                    for skill in skills {
                        all_skills.push(json!({
                            "slug": skill.metadata.slug,
                            "title": skill.metadata.title,
                            "summary": skill.metadata.summary,
                            "path": skill.metadata.path,
                            "tools": skill.metadata.tools,
                            "triggers": skill.metadata.triggers,
                        }));
                    }
                }
                Err(e) => {
                    all_skills.push(json!({
                        "error": format!("Error scanning {}: {e}", dir.display())
                    }));
                }
            }
        }
    }

    // Also search the workspace root itself
    if let Ok(skills) = rc_skills::discover_skills(&context.cwd) {
        for skill in skills {
            all_skills.push(json!({
                "slug": skill.metadata.slug,
                "title": skill.metadata.title,
                "summary": skill.metadata.summary,
                "path": skill.metadata.path,
                "tools": skill.metadata.tools,
                "triggers": skill.metadata.triggers,
            }));
        }
    }

    // Suppress unused variable warning for input
    let _ = input;

    if all_skills.is_empty() {
        Ok("No skills found in the current workspace.".to_owned())
    } else {
        Ok(serde_json::to_string_pretty(&all_skills)?)
    }
}

pub(crate) fn team_create_tool(input: &Value) -> Result<String> {
    let objective = input
        .get("objective")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("objective is required"))?;
    let lead = input
        .get("lead")
        .and_then(Value::as_str)
        .unwrap_or("lead");
    let mut scheduler = rc_agents::AgentScheduler::new(lead, objective);
    if let Some(agents) = input.get("agents").and_then(Value::as_array) {
        for agent_def in agents {
            let name = agent_def
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("agent");
            let role = agent_def
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or("worker");
            let agent = rc_agents::AgentIdentity::new(name, role);
            scheduler.register_agent(agent);
        }
    }
    let report = scheduler.team_status();
    Ok(serde_json::to_string_pretty(&report)?)
}

pub(crate) fn team_status_tool() -> Result<String> {
    // Return a placeholder status indicating no active team in the current context.
    Ok(json!({
        "type": "team_status",
        "message": "No active team in current tool context. Use team_create to create a team.",
        "note": "Team management requires AgentScheduler context in the conversation loop."
    })
    .to_string())
}

pub(crate) async fn remote_trigger_tool(input: &Value) -> Result<String> {
    let url = input["url"]
        .as_str()
        .ok_or_else(|| anyhow!("url is required"))?;
    let event = input["event"]
        .as_str()
        .ok_or_else(|| anyhow!("event is required"))?;
    let payload = input.get("payload").cloned().unwrap_or(json!({}));

    let body = json!({
        "event": event,
        "payload": payload,
    });

    let client = reqwest::Client::new();
    let response = client
        .post(url)
        .json(&body)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .context("failed to send remote trigger")?;

    let status = response.status();
    let response_text = response
        .text()
        .await
        .context("failed to read trigger response")?;

    Ok(json!({
        "url": url,
        "event": event,
        "http_status": status.as_u16(),
        "response": response_text.chars().take(5000).collect::<String>(),
    })
    .to_string())
}

pub(crate) async fn tungsten_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let action = input["action"]
        .as_str()
        .ok_or_else(|| anyhow!("action is required (compile, run, or test)"))?;
    let target = input["target"]
        .as_str()
        .ok_or_else(|| anyhow!("target is required"))?;

    // Detect project type by checking for marker files.
    let is_rust = context.cwd.join("Cargo.toml").exists()
        || context.cwd.join(target).join("Cargo.toml").exists();
    let is_node = context.cwd.join("package.json").exists()
        || context.cwd.join(target).join("package.json").exists();
    let is_python = context.cwd.join("setup.py").exists()
        || context.cwd.join("pyproject.toml").exists()
        || context.cwd.join(target).join("setup.py").exists();

    let command = match action {
        "compile" => {
            if is_rust {
                format!("cargo build --manifest-path {target}/Cargo.toml 2>&1 || cargo build 2>&1")
            } else if is_node {
                "npm run build 2>&1".to_owned()
            } else if is_python {
                "python -m py_compile . 2>&1".to_owned()
            } else {
                return Ok("Unable to detect project type. No Cargo.toml, package.json, or setup.py found.".to_owned());
            }
        }
        "run" => {
            if is_rust {
                format!("cargo run --manifest-path {target}/Cargo.toml 2>&1 || cargo run 2>&1")
            } else if is_node {
                "npm start 2>&1".to_owned()
            } else if is_python {
                "python main.py 2>&1".to_owned()
            } else {
                return Ok("Unable to detect project type.".to_owned());
            }
        }
        "test" => {
            if is_rust {
                format!("cargo test --manifest-path {target}/Cargo.toml 2>&1 || cargo test 2>&1")
            } else if is_node {
                "npm test 2>&1".to_owned()
            } else if is_python {
                "python -m pytest 2>&1".to_owned()
            } else {
                return Ok("Unable to detect project type.".to_owned());
            }
        }
        _ => return Err(anyhow!("action must be 'compile', 'run', or 'test'")),
    };

    let mut process = if cfg!(windows) {
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-Command", &command]);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.args(["-lc", &command]);
        cmd
    };
    process.current_dir(&context.cwd);
    process.stdout(Stdio::piped());
    process.stderr(Stdio::piped());

    let mut child = process.spawn().context("failed to spawn tungsten command")?;
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
        Ok::<_, anyhow::Error>((status.success(), stdout, stderr))
    };
    let (success, stdout, stderr) =
        tokio::time::timeout(std::time::Duration::from_millis(context.timeout_ms), future)
            .await
            .map_err(|_| anyhow!("tungsten command timed out"))??;

    let mut parts = Vec::new();
    if !stdout.trim().is_empty() {
        parts.push(stdout.trim_end().to_owned());
    }
    if !stderr.trim().is_empty() {
        parts.push(format!("stderr:\n{}", stderr.trim_end()));
    }
    if !success {
        parts.push("exit_status: failed".to_owned());
    }
    Ok(if parts.is_empty() {
        "Command completed with no output.".to_owned()
    } else {
        parts.join("\n\n")
    })
}

pub(crate) fn overflow_test_tool(input: &Value) -> Result<String> {
    let scenario = input["scenario"]
        .as_str()
        .ok_or_else(|| anyhow!("scenario is required (large_output, many_messages, or deep_recursion)"))?;

    match scenario {
        "large_output" => {
            let data: String = (0..10_000)
                .map(|i| format!("Line {i}: This is test output data for overflow testing.\n"))
                .collect();
            Ok(json!({
                "scenario": "large_output",
                "size_chars": data.len(),
                "size_lines": 10_000,
                "data_preview": data.chars().take(500).collect::<String>(),
            })
            .to_string())
        }
        "many_messages" => {
            let messages: Vec<Value> = (0..100)
                .map(|i| {
                    json!({
                        "id": i,
                        "role": if i % 2 == 0 { "user" } else { "assistant" },
                        "content": format!("Message {i}: Test content for context overflow testing."),
                    })
                })
                .collect();
            Ok(json!({
                "scenario": "many_messages",
                "count": messages.len(),
                "messages": messages,
            })
            .to_string())
        }
        "deep_recursion" => {
            let depth = 50;
            let mut nested = json!("leaf");
            for _ in 0..depth {
                nested = json!({ "child": nested });
            }
            Ok(json!({
                "scenario": "deep_recursion",
                "depth": depth,
                "structure": nested,
            })
            .to_string())
        }
        _ => Err(anyhow!(
            "scenario must be 'large_output', 'many_messages', or 'deep_recursion'"
        )),
    }
}

pub(crate) fn synthetic_output_tool(input: &Value) -> Result<String> {
    let output_type = input["type"]
        .as_str()
        .ok_or_else(|| anyhow!("type is required (json, csv, markdown, or text)"))?;
    let rows = input.get("rows").and_then(Value::as_u64).unwrap_or(10) as usize;

    match output_type {
        "json" => {
            let data: Vec<Value> = (0..rows)
                .map(|i| {
                    json!({
                        "id": i,
                        "name": format!("item_{i}"),
                        "value": i * 10,
                        "active": i % 2 == 0,
                    })
                })
                .collect();
            Ok(serde_json::to_string_pretty(&data)?)
        }
        "csv" => {
            let mut lines = vec!["id,name,value,active".to_owned()];
            for i in 0..rows {
                lines.push(format!("{i},item_{i},{},{}", i * 10, i % 2 == 0));
            }
            Ok(lines.join("\n"))
        }
        "markdown" => {
            let mut md = String::from("# Synthetic Report\n\n");
            md.push_str("| id | name | value | active |\n");
            md.push_str("|----|------|-------|--------|\n");
            for i in 0..rows {
                md.push_str(&format!("| {i} | item_{i} | {} | {} |\n", i * 10, i % 2 == 0));
            }
            Ok(md)
        }
        "text" => {
            let lines: Vec<String> = (0..rows)
                .map(|i| format!("Row {i}: name=item_{i}, value={}, active={}", i * 10, i % 2 == 0))
                .collect();
            Ok(lines.join("\n"))
        }
        _ => Err(anyhow!(
            "type must be 'json', 'csv', 'markdown', or 'text'"
        )),
    }
}

/// Load and return a skill's instructions by slug.
///
/// Searches the workspace skill directories for a matching skill and returns
/// its full content (instructions) for the agent to follow.
pub(crate) fn skill_execute_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let slug = input["slug"]
        .as_str()
        .ok_or_else(|| anyhow!("slug is required"))?;
    let arguments = input.get("arguments").cloned().unwrap_or(json!({}));

    let search_dirs = [
        context.cwd.join(".roo"),
        context.cwd.join(".remote-code-rust"),
        context.cwd.clone(),
    ];

    for dir in &search_dirs {
        if !dir.exists() {
            continue;
        }
        if let Ok(skills) = rc_skills::discover_skills(dir) {
            for skill in skills {
                if skill.metadata.slug == slug {
                    let summary = skill.metadata.summary.as_deref().unwrap_or("(no summary)");
                    let mut output = format!(
                        "# Skill: {} ({})\n\n{}\n\n",
                        skill.metadata.title,
                        skill.metadata.slug,
                        summary
                    );
                    if !skill.instructions.is_empty() {
                        output.push_str(&skill.instructions);
                    }
                    if !arguments.is_null() && !arguments.as_object().is_none_or(|o| o.is_empty())
                    {
                        output.push_str(&format!(
                            "\n\n## Arguments\n```json\n{}\n```",
                            serde_json::to_string_pretty(&arguments)?
                        ));
                    }
                    return Ok(output);
                }
            }
        }
    }

    Err(anyhow!(
        "Skill '{slug}' not found. Use skill_discover to list available skills."
    ))
}

pub(crate) fn voice_input_tool(input: &Value) -> Result<String> {
    let duration_secs = input.get("duration_secs").and_then(Value::as_u64).unwrap_or(5);
    let language = input["language"].as_str().unwrap_or("en");

    // Try to record audio using sox/rec/ffmpeg and transcribe with whisper.
    let temp_dir = std::env::temp_dir();
    let wav_path = temp_dir.join("remote-code-voice.wav");

    // Attempt recording with sox (rec command) or ffmpeg.
    let record_result = if cfg!(windows) {
        // On Windows, try ffmpeg.
        std::process::Command::new("ffmpeg")
            .args([
                "-y", "-f", "dshow", "-i", "audio=microphone",
                "-t", &duration_secs.to_string(),
                "-ar", "16000", "-ac", "1",
                wav_path.to_str().unwrap_or(""),
            ])
            .output()
    } else {
        // On Unix, try rec (sox) first, then ffmpeg.
        let sox_result = std::process::Command::new("rec")
            .args([
                "-r", "16000", "-c", "1",
                wav_path.to_str().unwrap_or(""),
                "trim", "0", &duration_secs.to_string(),
            ])
            .output();
        match sox_result {
            Ok(out) if out.status.success() => Ok(out),
            _ => {
                std::process::Command::new("ffmpeg")
                    .args([
                        "-y", "-f", "alsa", "-i", "default",
                        "-t", &duration_secs.to_string(),
                        "-ar", "16000", "-ac", "1",
                        wav_path.to_str().unwrap_or(""),
                    ])
                    .output()
            }
        }
    };

    let recorded = matches!(record_result, Ok(out) if out.status.success() && wav_path.exists());

    if !recorded {
        return Ok(json!({
            "type": "voice_input",
            "duration_secs": duration_secs,
            "status": "recording_failed",
            "message": "Voice recording failed. Install sox (rec) or ffmpeg with audio support.",
            "hint": "Windows: install ffmpeg. macOS: brew install sox. Linux: apt install sox.",
        }).to_string());
    }

    // Try to transcribe with whisper CLI.
    let whisper_result = std::process::Command::new("whisper")
        .args([
            wav_path.to_str().unwrap_or(""),
            "--model", "base",
            "--language", language,
            "--output_format", "txt",
            "--output_dir", temp_dir.to_str().unwrap_or(""),
        ])
        .output();

    let transcription = match whisper_result {
        Ok(out) if out.status.success() => {
            let txt_path = temp_dir.join("remote-code-voice.txt");
            if txt_path.exists() {
                std::fs::read_to_string(&txt_path).unwrap_or_default()
            } else {
                String::from("(transcription file not found)")
            }
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            format!("(whisper error: {})", stderr.chars().take(200).collect::<String>())
        }
        Err(e) => format!("(whisper not available: {e})"),
    };

    // Clean up temp files.
    let _ = std::fs::remove_file(&wav_path);
    let _ = std::fs::remove_file(temp_dir.join("remote-code-voice.txt"));

    Ok(json!({
        "type": "voice_input",
        "duration_secs": duration_secs,
        "language": language,
        "status": "success",
        "transcription": transcription.trim(),
    }).to_string())
}

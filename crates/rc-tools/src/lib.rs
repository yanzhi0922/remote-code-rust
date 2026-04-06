use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, Result, anyhow};
use ignore::WalkBuilder;
use rc_core::{ToolCall, ToolResult};
use rc_permissions::{PermissionBroker, PermissionRequest, auto_allows, classify_tool};
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use walkdir::WalkDir;

const IGNORED_DIRS: &[&str] = &[
    ".git",
    ".hg",
    ".svn",
    "node_modules",
    "dist",
    "coverage",
    ".next",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub protocol_name: String,
    pub permission_tool_name: String,
    pub description: String,
    pub requires_permission: bool,
    pub input_schema: Value,
}

impl ToolSpec {
    pub fn to_openai_schema(&self) -> Value {
        json!({
            "type": "function",
            "function": {
                "name": self.name,
                "description": self.description,
                "parameters": self.input_schema,
            }
        })
    }

    pub fn to_anthropic_schema(&self) -> Value {
        json!({
            "name": self.name,
            "description": self.description,
            "input_schema": self.input_schema,
        })
    }
}

#[derive(Debug, Clone)]
pub struct ToolExecutionContext {
    pub cwd: PathBuf,
    pub timeout_ms: u64,
}

pub fn builtin_tool_specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            name: "list_directory".to_owned(),
            protocol_name: "ListDirectory".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: "List files and directories relative to the current workspace.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "recursive": {"type": "boolean"},
                    "max_entries": {"type": "integer", "minimum": 1, "maximum": 500}
                },
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "read_file".to_owned(),
            protocol_name: "ReadFile".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: "Read a UTF-8 text file from the current workspace.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "start_line": {"type": "integer", "minimum": 1},
                    "end_line": {"type": "integer", "minimum": 1},
                    "max_chars": {"type": "integer", "minimum": 1, "maximum": 50000}
                },
                "required": ["path"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "search_text".to_owned(),
            protocol_name: "SearchText".to_owned(),
            permission_tool_name: "Read".to_owned(),
            description: "Search files for a text pattern or regular expression.".to_owned(),
            requires_permission: false,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string"},
                    "path": {"type": "string"},
                    "max_matches": {"type": "integer", "minimum": 1, "maximum": 200}
                },
                "required": ["pattern"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "write_file".to_owned(),
            protocol_name: "WriteFile".to_owned(),
            permission_tool_name: "Edit".to_owned(),
            description: "Create or overwrite a text file.".to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "content": {"type": "string"},
                    "append": {"type": "boolean"}
                },
                "required": ["path", "content"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "replace_in_file".to_owned(),
            protocol_name: "ReplaceInFile".to_owned(),
            permission_tool_name: "Edit".to_owned(),
            description: "Replace text in an existing file.".to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "search": {"type": "string"},
                    "replace": {"type": "string"},
                    "all": {"type": "boolean"}
                },
                "required": ["path", "search", "replace"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "edit_file".to_owned(),
            protocol_name: "EditFile".to_owned(),
            permission_tool_name: "Edit".to_owned(),
            description: "Apply ordered search/replace edits to a text file.".to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "edits": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "search": {"type": "string"},
                                "replace": {"type": "string"},
                                "all": {"type": "boolean"}
                            },
                            "required": ["search", "replace"],
                            "additionalProperties": false
                        }
                    },
                    "create_if_missing": {"type": "boolean"}
                },
                "required": ["path", "edits"],
                "additionalProperties": false,
            }),
        },
        ToolSpec {
            name: "bash_command".to_owned(),
            protocol_name: "Bash".to_owned(),
            permission_tool_name: "Bash".to_owned(),
            description: "Run a shell command in the current workspace.".to_owned(),
            requires_permission: true,
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string"},
                    "timeout_ms": {"type": "integer", "minimum": 1000, "maximum": 600000}
                },
                "required": ["command"],
                "additionalProperties": false,
            }),
        },
    ]
}

pub async fn execute_tool_call(
    call: &ToolCall,
    context: &ToolExecutionContext,
    broker: &dyn PermissionBroker,
) -> Result<ToolResult> {
    let spec = builtin_tool_specs()
        .into_iter()
        .find(|spec| spec.name == call.name)
        .ok_or_else(|| anyhow!("unknown tool {}", call.name))?;

    if spec.requires_permission && !auto_allows(broker.mode(), classify_tool(&spec.name)) {
        let decision = broker
            .decide(PermissionRequest {
                tool_name: spec.name.clone(),
                tool_use_id: call.id.clone(),
                title: format!("Allow {}", spec.protocol_name),
                description: spec.description.clone(),
                input: call.input.clone(),
                blocked_path: call
                    .input
                    .get("path")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
            })
            .await;
        if !decision.allowed {
            return Ok(ToolResult {
                content: decision
                    .message
                    .unwrap_or_else(|| format!("Permission denied for {}.", spec.name)),
                is_error: true,
            });
        }
    }

    let result = match spec.name.as_str() {
        "list_directory" => list_directory(&call.input, context),
        "read_file" => read_file(&call.input, context),
        "search_text" => search_text(&call.input, context),
        "write_file" => write_file(&call.input, context),
        "replace_in_file" => replace_in_file(&call.input, context),
        "edit_file" => edit_file(&call.input, context),
        "bash_command" => bash_command(&call.input, context).await,
        _ => Err(anyhow!("unsupported tool {}", spec.name)),
    };

    match result {
        Ok(content) => Ok(ToolResult {
            content,
            is_error: false,
        }),
        Err(error) => Ok(ToolResult {
            content: error.to_string(),
            is_error: true,
        }),
    }
}

fn resolve_workspace_path(cwd: &Path, maybe_relative: Option<&str>) -> Result<PathBuf> {
    let candidate = match maybe_relative {
        Some(path) if !path.trim().is_empty() => cwd.join(path),
        _ => cwd.to_path_buf(),
    };
    let canonical_cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    let canonical_candidate = candidate.canonicalize().unwrap_or(candidate.clone());
    if !canonical_candidate.starts_with(&canonical_cwd) {
        return Err(anyhow!(
            "path {} escapes the workspace {}",
            candidate.display(),
            cwd.display()
        ));
    }
    Ok(candidate)
}

fn list_directory(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let target = resolve_workspace_path(&context.cwd, input.get("path").and_then(Value::as_str))?;
    let recursive = input
        .get("recursive")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let max_entries = input
        .get("max_entries")
        .and_then(Value::as_u64)
        .unwrap_or(200) as usize;
    let mut builder = WalkBuilder::new(&target);
    builder.hidden(false);
    if !recursive {
        builder.max_depth(Some(1));
    }
    let mut lines = Vec::new();
    for entry in builder.build().take(max_entries) {
        let entry = entry?;
        let path = entry.path();
        if path == target {
            continue;
        }
        if path.components().any(|component| {
            IGNORED_DIRS.contains(&component.as_os_str().to_string_lossy().as_ref())
        }) {
            continue;
        }
        let relative = path.strip_prefix(&context.cwd).unwrap_or(path);
        let marker = if entry
            .file_type()
            .is_some_and(|file_type| file_type.is_dir())
        {
            "dir"
        } else {
            "file"
        };
        lines.push(format!("[{marker}] {}", relative.display()));
    }
    if lines.is_empty() {
        Ok("No files matched.".to_owned())
    } else {
        Ok(lines.join("\n"))
    }
}

fn read_file(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let path = input
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("read_file requires a path"))?;
    let target = resolve_workspace_path(&context.cwd, Some(path))?;
    let contents = std::fs::read_to_string(&target)
        .with_context(|| format!("failed to read {}", target.display()))?;
    let start_line = input.get("start_line").and_then(Value::as_u64).unwrap_or(1) as usize;
    let end_line = input
        .get("end_line")
        .and_then(Value::as_u64)
        .unwrap_or(usize::MAX as u64) as usize;
    let max_chars = input
        .get("max_chars")
        .and_then(Value::as_u64)
        .unwrap_or(50_000) as usize;
    let selected = contents
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let line_number = index + 1;
            if line_number < start_line || line_number > end_line {
                None
            } else {
                Some(format!("{:>4} {line}", line_number))
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    Ok(selected.chars().take(max_chars).collect())
}

fn search_text(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let pattern = input
        .get("pattern")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("search_text requires a pattern"))?;
    let target = resolve_workspace_path(&context.cwd, input.get("path").and_then(Value::as_str))?;
    let regex = Regex::new(pattern).or_else(|_| Regex::new(&regex::escape(pattern)))?;
    let max_matches = input
        .get("max_matches")
        .and_then(Value::as_u64)
        .unwrap_or(50) as usize;
    let mut matches = Vec::new();
    for entry in WalkDir::new(&target).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().components().any(|component| {
            IGNORED_DIRS.contains(&component.as_os_str().to_string_lossy().as_ref())
        }) {
            continue;
        }
        let Ok(contents) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        for (index, line) in contents.lines().enumerate() {
            if regex.is_match(line) {
                let relative = entry
                    .path()
                    .strip_prefix(&context.cwd)
                    .unwrap_or(entry.path());
                matches.push(format!(
                    "{}:{}:{}",
                    relative.display(),
                    index + 1,
                    line.trim()
                ));
                if matches.len() >= max_matches {
                    return Ok(matches.join("\n"));
                }
            }
        }
    }
    if matches.is_empty() {
        Ok("No matches found.".to_owned())
    } else {
        Ok(matches.join("\n"))
    }
}

fn write_file(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let path = input
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("write_file requires a path"))?;
    let content = input
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("write_file requires content"))?;
    let append = input
        .get("append")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let target = resolve_workspace_path(&context.cwd, Some(path))?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if append {
        use std::io::Write as _;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&target)?;
        file.write_all(content.as_bytes())?;
    } else {
        std::fs::write(&target, content)?;
    }
    Ok(format!("Wrote {}", target.display()))
}

fn replace_in_file(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let path = input
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("replace_in_file requires a path"))?;
    let search = input
        .get("search")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("replace_in_file requires search text"))?;
    let replace = input
        .get("replace")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("replace_in_file requires replacement text"))?;
    let replace_all = input.get("all").and_then(Value::as_bool).unwrap_or(false);
    let target = resolve_workspace_path(&context.cwd, Some(path))?;
    let original = std::fs::read_to_string(&target)?;
    let updated = if replace_all {
        original.replace(search, replace)
    } else {
        original.replacen(search, replace, 1)
    };
    std::fs::write(&target, updated)?;
    Ok(format!("Updated {}", target.display()))
}

fn edit_file(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let path = input
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("edit_file requires a path"))?;
    let target = resolve_workspace_path(&context.cwd, Some(path))?;
    let edits = input
        .get("edits")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("edit_file requires edits"))?;
    let create_if_missing = input
        .get("create_if_missing")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut content = if target.exists() {
        std::fs::read_to_string(&target)?
    } else if create_if_missing {
        String::new()
    } else {
        return Err(anyhow!("{} does not exist", target.display()));
    };
    for edit in edits {
        let search = edit
            .get("search")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("edit is missing search"))?;
        let replace = edit
            .get("replace")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("edit is missing replace"))?;
        let replace_all = edit.get("all").and_then(Value::as_bool).unwrap_or(false);
        if search.is_empty() && create_if_missing && content.is_empty() {
            content = replace.to_owned();
            continue;
        }
        content = if replace_all {
            content.replace(search, replace)
        } else {
            content.replacen(search, replace, 1)
        };
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&target, content)?;
    Ok(format!(
        "Applied {} edits to {}",
        edits.len(),
        target.display()
    ))
}

async fn bash_command(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let command = input
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("bash_command requires a command"))?;
    let timeout_ms = input
        .get("timeout_ms")
        .and_then(Value::as_u64)
        .unwrap_or(context.timeout_ms)
        .clamp(1_000, 600_000);
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

    let mut child = process.spawn().context("failed to spawn shell command")?;
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
        tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), future)
            .await
            .map_err(|_| anyhow!("command timed out after {timeout_ms}ms"))??;
    let mut sections = Vec::new();
    if !stdout.trim().is_empty() {
        sections.push(format!("stdout:\n{}", stdout.trim_end()));
    }
    if !stderr.trim().is_empty() {
        sections.push(format!("stderr:\n{}", stderr.trim_end()));
    }
    if !success {
        sections.push("exit_status: failed".to_owned());
    }
    Ok(if sections.is_empty() {
        "command completed with no output".to_owned()
    } else {
        sections.join("\n\n")
    })
}

#[cfg(test)]
mod tests {
    use super::{ToolExecutionContext, builtin_tool_specs, execute_tool_call};
    use rc_core::{PermissionMode, ToolCall};
    use rc_permissions::StaticPermissionBroker;
    use serde_json::json;
    use tempfile::tempdir;

    #[tokio::test]
    async fn read_and_search_tools_work() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let file = tempdir.path().join("notes.txt");
        if let Err(error) = std::fs::write(&file, "hello\nremote code\n") {
            panic!("failed to seed file: {error}");
        }
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 5_000,
        };
        let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);

        let read = execute_tool_call(
            &ToolCall {
                id: "1".to_owned(),
                name: "read_file".to_owned(),
                input: json!({"path":"notes.txt"}),
            },
            &context,
            &broker,
        )
        .await;
        assert!(read.is_ok());
        let read = read.unwrap_or_else(|error| panic!("read failed: {error}"));
        assert!(read.content.contains("remote code"));

        let search = execute_tool_call(
            &ToolCall {
                id: "2".to_owned(),
                name: "search_text".to_owned(),
                input: json!({"pattern":"remote","path":"."}),
            },
            &context,
            &broker,
        )
        .await;
        assert!(search.is_ok());
        let search = search.unwrap_or_else(|error| panic!("search failed: {error}"));
        assert!(search.content.contains("notes.txt:2"));

        assert!(
            builtin_tool_specs()
                .iter()
                .any(|spec| spec.protocol_name == "Bash")
        );
    }
}

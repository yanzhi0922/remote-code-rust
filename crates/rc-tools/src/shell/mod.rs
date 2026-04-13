pub mod backgrounding;
pub mod git_safety;
pub mod output;
pub mod path_validation;
pub mod readonly;
pub mod semantics;

use std::process::Stdio;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::io::AsyncReadExt;
use tokio::process::Command;

use crate::ToolExecutionContext;
use crate::tasks;

use self::output::{ShellOutputSummary, format_shell_result, persist_shell_output, truncate_output};
use self::path_validation::resolve_working_dir;
use self::readonly::ShellKind;
use self::semantics::{ShellCommandSemantic, analyze_command};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellExecutionPolicy {
    pub block_inline_cwd: bool,
    pub allow_background: bool,
    pub block_destructive_git: bool,
    pub max_capture_chars: usize,
    pub output_dir: Option<std::path::PathBuf>,
}

impl Default for ShellExecutionPolicy {
    fn default() -> Self {
        Self {
            block_inline_cwd: true,
            allow_background: true,
            block_destructive_git: true,
            max_capture_chars: 16_000,
            output_dir: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ShellExecutionRequest {
    pub kind: ShellKind,
    pub command: String,
    pub description: Option<String>,
    pub cwd: std::path::PathBuf,
    pub timeout_ms: u64,
    pub background: bool,
}

#[derive(Debug)]
struct ShellExecutionOutcome {
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    timed_out: bool,
}

pub async fn execute_shell_command(
    kind: ShellKind,
    input: &Value,
    context: &ToolExecutionContext,
    policy: &ShellExecutionPolicy,
) -> Result<String> {
    let command = input
        .get("command")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("command is required"))?
        .trim()
        .to_owned();
    if command.is_empty() {
        return Err(anyhow!("command must not be empty"));
    }

    let request = ShellExecutionRequest {
        kind,
        command: command.clone(),
        description: input
            .get("description")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
        cwd: resolve_working_dir(
            &context.cwd,
            input.get("cwd").and_then(Value::as_str),
        )?,
        timeout_ms: input
            .get("timeout_ms")
            .and_then(Value::as_u64)
            .unwrap_or(context.timeout_ms)
            .clamp(1_000, 600_000),
        background: input
            .get("background")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    };
    let analysis = analyze_command(kind, &request.command, request.background);

    if policy.block_inline_cwd && analysis.changes_directory && input.get("cwd").is_none() {
        return Err(anyhow!(
            "inline directory changes are blocked; pass the target via the cwd field instead"
        ));
    }
    if policy.block_destructive_git && analysis.destructive_git {
        return Err(anyhow!(
            "destructive git commands are blocked by the shell safety policy"
        ));
    }
    if matches!(analysis.semantic, ShellCommandSemantic::Dangerous) {
        return Err(anyhow!("dangerous shell command blocked by safety policy"));
    }
    if analysis.background && !policy.allow_background {
        return Err(anyhow!("background shell commands are disabled by policy"));
    }

    if analysis.background {
        execute_background(request, analysis, policy).await
    } else {
        execute_foreground(request, analysis, policy).await
    }
}

async fn execute_foreground(
    request: ShellExecutionRequest,
    analysis: self::semantics::ShellCommandAnalysis,
    policy: &ShellExecutionPolicy,
) -> Result<String> {
    let mut process = build_process(request.kind, &request.command);
    process.current_dir(&request.cwd);
    process.stdout(Stdio::piped());
    process.stderr(Stdio::piped());

    let child = process.spawn().context("failed to spawn shell command")?;
    let outcome = capture_process_output(child, request.timeout_ms).await?;

    let file_stem = shell_file_stem();
    let stdout = truncate_output(&outcome.stdout, policy.max_capture_chars);
    let stderr = truncate_output(&outcome.stderr, policy.max_capture_chars);
    let artifact_contents = build_artifact_contents(
        &request.command,
        request.description.as_deref(),
        &request.cwd,
        outcome.exit_code,
        outcome.timed_out,
        &stdout,
        &stderr,
    );
    let artifact_path = persist_shell_output(
        policy.output_dir.as_deref(),
        &file_stem,
        &artifact_contents,
    )?;
    let summary = ShellOutputSummary {
        exit_code: outcome.exit_code,
        stdout,
        stderr,
        timed_out: outcome.timed_out,
        artifact_path,
    };
    let rendered = format_shell_result(
        &request.command,
        request.description.as_deref(),
        &request.cwd,
        &analysis,
        &summary,
    );
    if outcome.timed_out {
        Err(anyhow!(rendered))
    } else {
        Ok(rendered)
    }
}

async fn execute_background(
    request: ShellExecutionRequest,
    analysis: self::semantics::ShellCommandAnalysis,
    policy: &ShellExecutionPolicy,
) -> Result<String> {
    let task = tasks::create_background_task(&format!(
        "{} ({:?})",
        request
            .description
            .as_deref()
            .unwrap_or(&request.command)
            .chars()
            .take(80)
            .collect::<String>(),
        request.kind
    ))?;
    tasks::mark_task_running(&task.id, Some("Background shell command started."))?;

    let task_id = task.id.clone();
    let task_id_for_task = task_id.clone();
    let semantic = format!("{:?}", analysis.semantic).to_ascii_lowercase();
    let analysis_for_task = analysis.clone();
    let response_description = request.description.clone();
    let output_dir = policy.output_dir.clone();
    let max_capture_chars = policy.max_capture_chars;
    tokio::spawn(async move {
        let outcome = async {
            let mut process = build_process(request.kind, &request.command);
            process.current_dir(&request.cwd);
            process.stdout(Stdio::piped());
            process.stderr(Stdio::piped());

            let child = process
                .spawn()
                .context("failed to spawn background shell command")?;
            capture_process_output(child, request.timeout_ms).await
        }
        .await;

        match outcome {
            Ok(outcome) => {
                let stdout = truncate_output(&outcome.stdout, max_capture_chars);
                let stderr = truncate_output(&outcome.stderr, max_capture_chars);
                let file_stem = format!("task-{}", task_id_for_task);
                let artifact_contents = build_artifact_contents(
                    &request.command,
                    request.description.as_deref(),
                    &request.cwd,
                    outcome.exit_code,
                    outcome.timed_out,
                    &stdout,
                    &stderr,
                );
                let artifact_path = persist_shell_output(
                    output_dir.as_deref(),
                    &file_stem,
                    &artifact_contents,
                )
                .ok()
                .flatten();
                let summary = ShellOutputSummary {
                    exit_code: outcome.exit_code,
                    stdout,
                    stderr,
                    timed_out: outcome.timed_out,
                    artifact_path,
                };
                let rendered = format_shell_result(
                    &request.command,
                    request.description.as_deref(),
                    &request.cwd,
                    &analysis_for_task,
                    &summary,
                );
                let status = if outcome.timed_out {
                    tasks::TaskStatus::Failed
                } else if outcome.exit_code == Some(0) {
                    tasks::TaskStatus::Completed
                } else {
                    tasks::TaskStatus::Failed
                };
                let _ = tasks::finish_background_task(&task_id_for_task, status, &rendered);
            }
            Err(error) => {
                let _ = tasks::finish_background_task(
                    &task_id_for_task,
                    tasks::TaskStatus::Failed,
                    &format!("Background shell command failed: {error}"),
                );
            }
        }
    });

    Ok(json!({
        "task_id": task_id,
        "status": "running",
        "semantic": semantic,
        "description": response_description,
        "message": "Background shell command started."
    })
    .to_string())
}

async fn capture_process_output(
    mut child: tokio::process::Child,
    timeout_ms: u64,
) -> Result<ShellExecutionOutcome> {
    let stdout_task = child.stdout.take().map(|mut stream| {
        tokio::spawn(async move {
            let mut stdout = String::new();
            let _ = stream.read_to_string(&mut stdout).await;
            stdout
        })
    });
    let stderr_task = child.stderr.take().map(|mut stream| {
        tokio::spawn(async move {
            let mut stderr = String::new();
            let _ = stream.read_to_string(&mut stderr).await;
            stderr
        })
    });

    let (status, timed_out) =
        match tokio::time::timeout(Duration::from_millis(timeout_ms), child.wait()).await {
            Ok(status) => (Some(status?), false),
            Err(_) => {
                let _ = child.kill().await;
                let status = child.wait().await.ok();
                (status, true)
            }
        };

    let stdout = match stdout_task {
        Some(task) => task.await.unwrap_or_default(),
        None => String::new(),
    };
    let stderr = match stderr_task {
        Some(task) => task.await.unwrap_or_default(),
        None => String::new(),
    };

    Ok(ShellExecutionOutcome {
        exit_code: status.and_then(|status| status.code()),
        stdout,
        stderr,
        timed_out,
    })
}

fn build_process(kind: ShellKind, command: &str) -> Command {
    match kind {
        ShellKind::Bash => {
            if cfg!(windows) {
                let mut cmd = Command::new(crate::command::which_powershell());
                cmd.args(["-NoProfile", "-NonInteractive", "-Command", command]);
                cmd.env("PS_OUTPUT_ENCODING", "utf8");
                cmd
            } else {
                let mut cmd = Command::new("sh");
                cmd.args(["-lc", command]);
                cmd
            }
        }
        ShellKind::PowerShell => {
            let mut cmd = Command::new(crate::command::which_powershell());
            cmd.args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                command,
            ]);
            cmd.env("PS_OUTPUT_ENCODING", "utf8");
            cmd
        }
    }
}

fn shell_file_stem() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    format!("shell-{millis}")
}

fn build_artifact_contents(
    command: &str,
    description: Option<&str>,
    cwd: &std::path::Path,
    exit_code: Option<i32>,
    timed_out: bool,
    stdout: &str,
    stderr: &str,
) -> String {
    format!(
        "command: {command}\ndescription: {}\ncwd: {}\nexit_code: {}\ntimed_out: {timed_out}\n\nstdout:\n{}\n\nstderr:\n{}\n",
        description.unwrap_or("(none)"),
        cwd.display(),
        exit_code.map_or_else(|| "none".to_owned(), |code| code.to_string()),
        stdout.trim_end(),
        stderr.trim_end()
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use tempfile::tempdir;

    use super::{ShellExecutionPolicy, execute_shell_command};
    use crate::ToolExecutionContext;
    use crate::shell::readonly::ShellKind;

    fn test_shell_kind() -> ShellKind {
        if cfg!(windows) {
            ShellKind::PowerShell
        } else {
            ShellKind::Bash
        }
    }

    fn timeout_command() -> &'static str {
        if cfg!(windows) {
            "Start-Sleep -Seconds 2"
        } else {
            "sleep 2"
        }
    }

    fn echo_command() -> &'static str {
        if cfg!(windows) {
            "Write-Output 'hello'"
        } else {
            "printf 'hello'"
        }
    }

    #[tokio::test]
    async fn timed_out_commands_return_rich_error_and_artifact() {
        let tempdir = tempdir().expect("tempdir");
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 250,
            ..ToolExecutionContext::default()
        };
        let policy = ShellExecutionPolicy {
            output_dir: Some(tempdir.path().join("shell")),
            ..ShellExecutionPolicy::default()
        };

        let result = execute_shell_command(
            test_shell_kind(),
            &json!({
                "command": timeout_command(),
                "description": "timeout test",
                "timeout_ms": 250
            }),
            &context,
            &policy,
        )
        .await;

        let error = result.expect_err("command should time out");
        let rendered = error.to_string();
        assert!(rendered.contains("timed_out: true"));
        assert!(rendered.contains("description: timeout test"));
        assert!(tempdir.path().join("shell").exists());
    }

    #[tokio::test]
    async fn descriptions_are_included_in_success_output() {
        let tempdir = tempdir().expect("tempdir");
        let context = ToolExecutionContext {
            cwd: tempdir.path().to_path_buf(),
            timeout_ms: 2_000,
            ..ToolExecutionContext::default()
        };
        let policy = ShellExecutionPolicy::default();

        let result = execute_shell_command(
            test_shell_kind(),
            &json!({
                "command": echo_command(),
                "description": "print a greeting"
            }),
            &context,
            &policy,
        )
        .await
        .expect("command should succeed");

        assert!(result.contains("description: print a greeting"));
        assert!(result.contains("stdout:"));
    }
}

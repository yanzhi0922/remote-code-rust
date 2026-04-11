//! Command-line tool implementations: bash_command, powershell, repl.

use std::path::PathBuf;
use std::process::Stdio;

use anyhow::{Result, anyhow, Context};
use serde_json::Value;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

use super::ToolExecutionContext;

pub(crate) async fn bash_command(input: &Value, context: &ToolExecutionContext) -> Result<String> {
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

pub(crate) async fn powershell_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let command = input["command"]
        .as_str()
        .ok_or_else(|| anyhow!("command is required"))?;
    let timeout_ms = input
        .get("timeout_ms")
        .and_then(Value::as_u64)
        .unwrap_or(context.timeout_ms)
        .clamp(1_000, 600_000);
    let working_dir = input["cwd"]
        .as_str()
        .map(PathBuf::from)
        .unwrap_or_else(|| context.cwd.clone());

    if !cfg!(windows) {
        return Ok(
            "PowerShell is only available on Windows. Use bash_command instead.".to_owned(),
        );
    }

    // Try pwsh (PowerShell 7+) first, then fall back to powershell (5.1).
    let pwsh_path = which_powershell();
    let mut process = Command::new(&pwsh_path);
    process.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-Command",
        command,
    ]);
    process.current_dir(&working_dir);
    process.stdout(Stdio::piped());
    process.stderr(Stdio::piped());

    // Set UTF-8 output encoding for proper character handling.
    process.env("PS_OUTPUT_ENCODING", "utf8");

    let mut child = process
        .spawn()
        .with_context(|| format!("failed to spawn {pwsh_path}"))?;
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
        tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), future)
            .await
            .map_err(|_| anyhow!("powershell timed out after {timeout_ms}ms"))??;

    let success = exit_code.unwrap_or(1) == 0;
    let mut parts = Vec::new();
    if !stdout.trim().is_empty() {
        parts.push(format!("stdout:\n{}", stdout.trim_end()));
    }
    if !stderr.trim().is_empty() {
        parts.push(format!("stderr:\n{}", stderr.trim_end()));
    }
    if !success {
        parts.push(format!("exit_status: {} (failed)", exit_code.unwrap_or(-1)));
    }
    Ok(if parts.is_empty() {
        "command completed with no output".to_owned()
    } else {
        parts.join("\n\n")
    })
}

/// Find the best available PowerShell executable.
///
/// Prefers `pwsh` (PowerShell 7+, cross-platform) over `powershell`
/// (Windows PowerShell 5.1) for better compatibility and features.
pub(crate) fn which_powershell() -> String {
    // Try pwsh first (PowerShell 7+).
    let pwsh_candidates = ["pwsh", "pwsh.exe"];
    for candidate in &pwsh_candidates {
        if let Ok(output) = std::process::Command::new(candidate)
            .arg("-Version")
            .output()
            && output.status.success()
        {
            return candidate.to_string();
        }
    }
    // Fall back to Windows PowerShell.
    "powershell".to_string()
}

pub(crate) async fn repl_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let language = input["language"]
        .as_str()
        .ok_or_else(|| anyhow!("language is required (python, node, or rust)"))?;
    let code = input["code"]
        .as_str()
        .ok_or_else(|| anyhow!("code is required"))?;

    let (interpreter, flag) = match language {
        "python" => ("python", "-c"),
        "node" => ("node", "-e"),
        "rust" => {
            // For rust, write a temp file and compile/run it.
            let tmp_dir = context.cwd.join(".remote-code-rust").join("tmp");
            std::fs::create_dir_all(&tmp_dir)?;
            let src_path = tmp_dir.join("repl_tmp.rs");
            std::fs::write(&src_path, code)?;
            let output = std::process::Command::new("rustc")
                .args(["--edition", "2021", "-o"])
                .arg(tmp_dir.join("repl_tmp"))
                .arg(&src_path)
                .output()?;
            if !output.status.success() {
                return Ok(format!(
                    "Compile error:\n{}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }
            let run_output = std::process::Command::new(tmp_dir.join("repl_tmp"))
                .current_dir(&context.cwd)
                .output()?;
            return Ok(String::from_utf8_lossy(&run_output.stdout).to_string());
        }
        _ => return Err(anyhow!("unsupported language '{language}'. Use python, node, or rust.")),
    };

    let mut cmd = Command::new(interpreter);
    cmd.arg(flag).arg(code);
    cmd.current_dir(&context.cwd);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().context(format!("failed to spawn {interpreter}"))?;
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
            .map_err(|_| anyhow!("REPL execution timed out"))??;

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
        "No output.".to_owned()
    } else {
        parts.join("\n\n")
    })
}

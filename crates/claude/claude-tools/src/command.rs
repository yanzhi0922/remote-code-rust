//! Command-line tool implementations: bash_command, powershell, repl.

use std::process::Stdio;
use std::sync::OnceLock;

use anyhow::{Context, Result, anyhow};
use serde_json::Value;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

use super::shell::powershell_security::{PowerShellSecurityResult, powershell_command_is_safe};
use super::shell::readonly::ShellKind;
use super::{ToolExecutionContext, current_tool_runtime_policy};

pub(crate) async fn bash_command(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let policy = current_tool_runtime_policy().shell_policy;
    super::shell::execute_shell_command(ShellKind::Bash, input, context, &policy).await
}

pub(crate) async fn powershell_tool(
    input: &Value,
    context: &ToolExecutionContext,
) -> Result<String> {
    if !cfg!(windows) {
        return Ok("PowerShell is only available on Windows. Use bash_command instead.".to_owned());
    }

    // Run PowerShell-specific security analysis
    if let Some(command) = input.get("command").and_then(Value::as_str) {
        match powershell_command_is_safe(command) {
            PowerShellSecurityResult::Ask(reason) => {
                return Err(anyhow!(
                    "PowerShell command blocked by security policy: {reason}"
                ));
            }
            PowerShellSecurityResult::Passthrough | PowerShellSecurityResult::Allow => {}
        }
    }

    let policy = current_tool_runtime_policy().shell_policy;
    super::shell::execute_shell_command(ShellKind::PowerShell, input, context, &policy).await
}

/// Find the best available PowerShell executable.
///
/// Prefers `pwsh` (PowerShell 7+, cross-platform) over `powershell`
/// (Windows PowerShell 5.1) for better compatibility and features.
/// Result is cached after the first detection to avoid repeated blocking
/// `std::process::Command` calls in async contexts.
static POWERSHELL_PATH: OnceLock<String> = OnceLock::new();

pub(crate) fn which_powershell() -> &'static str {
    POWERSHELL_PATH.get_or_init(|| {
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
        "powershell".to_owned()
    })
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
            // WARNING: The Rust REPL compiles and executes native code with no
            // sandboxing.  This means arbitrary code can access the filesystem,
            // network, and OS resources.  A future improvement should sandbox
            // execution (e.g. via WASM, a container, or seccomp).
            let tmp_dir = context.cwd.join(".remote-code-rust").join("tmp");
            let src_path = tmp_dir.join("repl_tmp.rs");
            {
                let tmp_dir = tmp_dir.clone();
                let src_path = src_path.clone();
                let code = code.to_owned();
                tokio::task::spawn_blocking(move || -> std::io::Result<()> {
                    std::fs::create_dir_all(&tmp_dir)?;
                    std::fs::write(&src_path, &code)?;
                    Ok(())
                })
                .await
                .map_err(|e| anyhow!("Rust REPL setup task join error: {e}"))?
                .map_err(|e| anyhow!("Rust REPL setup failed: {e}"))?;
            }

            // Use tokio::process::Command to avoid blocking the runtime
            let compile_output = Command::new("rustc")
                .args(["--edition", "2021", "-o"])
                .arg(tmp_dir.join("repl_tmp"))
                .arg(&src_path)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await?;
            if !compile_output.status.success() {
                return Ok(format!(
                    "Compile error:\n{}",
                    String::from_utf8_lossy(&compile_output.stderr)
                ));
            }

            // Wrap the compiled binary execution in a 60-second timeout so a
            // long-running or hung program cannot stall the REPL indefinitely.
            let run_result = tokio::time::timeout(
                std::time::Duration::from_secs(60),
                Command::new(tmp_dir.join("repl_tmp"))
                    .current_dir(&context.cwd)
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .output(),
            )
            .await;

            match run_result {
                Ok(Ok(output)) => {
                    return Ok(String::from_utf8_lossy(&output.stdout).to_string());
                }
                Ok(Err(e)) => {
                    return Ok(format!("Execution error: {e}"));
                }
                Err(_) => {
                    return Ok("Rust REPL execution timed out after 60 seconds.".to_owned());
                }
            }
        }
        _ => {
            return Err(anyhow!(
                "unsupported language '{language}'. Use python, node, or rust."
            ));
        }
    };

    let mut cmd = Command::new(interpreter);
    cmd.arg(flag).arg(code);
    cmd.current_dir(&context.cwd);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .context(format!("failed to spawn {interpreter}"))?;
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

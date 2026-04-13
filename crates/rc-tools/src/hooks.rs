//! Command hook execution (shell commands triggered by lifecycle events).

use std::process::Stdio;

use anyhow::{Context, Result, anyhow};
use rc_core::HookShell;
use tokio::io::AsyncReadExt;
use tokio::process::Command;

use super::{CommandHookExecutionRequest, CommandHookExecutionResult};

pub async fn execute_command_hook(
    request: &CommandHookExecutionRequest,
) -> Result<CommandHookExecutionResult> {
    let shell = request.shell.unwrap_or_else(default_hook_shell);
    let timeout_secs = request.timeout_secs.unwrap_or(15).max(1);
    let mut process = build_shell_command(shell, &request.command);
    process.current_dir(&request.cwd);
    process.stdin(Stdio::piped());
    process.stdout(Stdio::piped());
    process.stderr(Stdio::piped());

    let mut child = process.spawn().context("failed to spawn command hook")?;
    if let Some(mut stdin) = child.stdin.take() {
        let input = serde_json::to_vec(&request.input)?;
        tokio::spawn(async move {
            let _ = tokio::io::AsyncWriteExt::write_all(&mut stdin, &input).await;
        });
    }

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
        tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), future)
            .await
            .map_err(|_| anyhow!("command hook timed out after {timeout_secs}s"))??;

    Ok(CommandHookExecutionResult {
        event: request.event,
        command: request.command.clone(),
        shell,
        exit_code,
        stdout,
        stderr,
    })
}

pub(crate) fn build_shell_command(shell: HookShell, command: &str) -> Command {
    match shell {
        HookShell::PowerShell => {
            let mut cmd = Command::new("powershell");
            cmd.args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                command,
            ]);
            cmd
        }
        HookShell::Bash => {
            #[cfg(windows)]
            {
                let mut cmd = Command::new("bash");
                cmd.args(["-lc", command]);
                cmd
            }
            #[cfg(not(windows))]
            {
                let mut cmd = Command::new("sh");
                cmd.args(["-lc", command]);
                cmd
            }
        }
    }
}

pub(crate) fn default_hook_shell() -> HookShell {
    if cfg!(windows) {
        HookShell::PowerShell
    } else {
        HookShell::Bash
    }
}

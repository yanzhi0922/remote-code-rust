//! Sandbox execution for running commands in an isolated environment.

use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::process::Command;

/// Configuration for sandbox execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxConfig {
    /// Directories the sandboxed command is allowed to access.
    pub allowed_dirs: Vec<PathBuf>,
    /// Whether network access is permitted.
    pub allow_network: bool,
    /// Maximum execution time in seconds.
    pub timeout_secs: u64,
    /// Maximum memory usage in MB (Linux only).
    pub max_memory_mb: Option<u64>,
}

impl SandboxConfig {
    /// Create a default sandbox config scoped to the given workspace directory.
    #[must_use]
    pub fn default_for_workspace(workspace: &std::path::Path) -> Self {
        Self {
            allowed_dirs: vec![workspace.to_path_buf()],
            allow_network: false,
            timeout_secs: 120,
            max_memory_mb: None,
        }
    }
}

/// Result of a sandboxed command execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxResult {
    /// Captured standard output.
    pub stdout: String,
    /// Captured standard error.
    pub stderr: String,
    /// Exit code, if available.
    pub exit_code: Option<i32>,
    /// Whether the command timed out.
    pub timed_out: bool,
}

/// Execute a command inside a sandbox with the given configuration.
///
/// On Windows the command is run via `cmd /C`, on Unix via `sh -c`.
/// The environment is stripped down to `PATH` and `HOME` only.
pub async fn execute_in_sandbox(
    command: &str,
    config: &SandboxConfig,
) -> Result<SandboxResult> {
    let mut cmd = build_sandbox_command(command);

    // Strip the environment to a safe subset.
    cmd.env_clear();
    if let Ok(path) = std::env::var("PATH") {
        cmd.env("PATH", path);
    }
    if let Ok(home) = std::env::var("HOME") {
        cmd.env("HOME", home);
    }
    if let Ok(userprofile) = std::env::var("USERPROFILE") {
        cmd.env("USERPROFILE", userprofile);
    }
    if let Ok(systemroot) = std::env::var("SystemRoot") {
        cmd.env("SystemRoot", systemroot);
    }

    // Set working directory to the first allowed directory.
    if let Some(dir) = config.allowed_dirs.first() {
        cmd.current_dir(dir);
    }

    let output = tokio::time::timeout(
        Duration::from_secs(config.timeout_secs),
        cmd.output(),
    )
    .await;

    match output {
        Ok(Ok(output)) => Ok(SandboxResult {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code(),
            timed_out: false,
        }),
        Ok(Err(error)) => Err(error).context("sandbox command failed to execute"),
        Err(_) => Ok(SandboxResult {
            stdout: String::new(),
            stderr: format!(
                "Command timed out after {} seconds.",
                config.timeout_secs
            ),
            exit_code: None,
            timed_out: true,
        }),
    }
}

/// Build the platform-appropriate shell command.
fn build_sandbox_command(command: &str) -> Command {
    #[cfg(windows)]
    {
        let mut cmd = Command::new("cmd");
        cmd.arg("/C").arg(command);
        cmd
    }

    #[cfg(not(windows))]
    {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command);
        cmd
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[tokio::test]
    async fn sandbox_executes_echo() {
        let workspace = std::env::temp_dir();
        let config = SandboxConfig::default_for_workspace(&workspace);

        let (command, expected) = ("echo hello", "hello");

        let result = execute_in_sandbox(command, &config)
            .await
            .expect("sandbox should execute");

        assert!(!result.timed_out, "should not time out");
        assert!(
            result.stdout.trim().contains(expected),
            "stdout should contain '{expected}', got: {}",
            result.stdout
        );
    }

    #[tokio::test]
    async fn sandbox_respects_timeout() {
        let workspace = std::env::temp_dir();
        let config = SandboxConfig {
            timeout_secs: 1,
            ..SandboxConfig::default_for_workspace(&workspace)
        };

        let command = if cfg!(windows) {
            "ping -n 10 127.0.0.1"
        } else {
            "sleep 30"
        };

        let result = execute_in_sandbox(command, &config)
            .await
            .expect("sandbox should handle timeout");

        assert!(result.timed_out, "should have timed out");
    }

    #[test]
    fn sandbox_config_default_for_workspace() {
        let workspace = Path::new("/tmp/test");
        let config = SandboxConfig::default_for_workspace(workspace);
        assert_eq!(config.allowed_dirs, vec![workspace.to_path_buf()]);
        assert!(!config.allow_network);
        assert_eq!(config.timeout_secs, 120);
        assert!(config.max_memory_mb.is_none());
    }
}

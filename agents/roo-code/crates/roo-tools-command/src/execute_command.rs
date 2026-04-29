//! Execute command tool implementation.
//!
//! Provides both parameter validation helpers (used by tests and the dispatcher)
//! and the real command execution logic that integrates with [`roo_terminal`].

use crate::helpers::*;
use crate::types::*;
use roo_terminal::registry::TerminalRegistry;
use roo_terminal::terminal::RooTerminal;
use roo_terminal::types::NoopCallbacks;
use roo_types::tool::ExecuteCommandParams;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

/// Unescape HTML entities in a command string.
///
/// Matches TS: `const canonicalCommand = unescapeHtmlEntities(command)`
/// Some LLMs (non-Claude) may output HTML entities like `<` instead of `<`
/// in their tool call parameters.
pub fn unescape_command(command: &str) -> String {
    command
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
}

// ---------------------------------------------------------------------------
// Validation & preparation (kept for backward-compat)
// ---------------------------------------------------------------------------

/// Validate execute_command parameters.
pub fn validate_execute_command_params(params: &ExecuteCommandParams) -> Result<(), CommandToolError> {
    if params.command.trim().is_empty() {
        return Err(CommandToolError::InvalidCommand(
            "command must not be empty".to_string(),
        ));
    }

    Ok(())
}

/// Generate an artifact ID for a command execution.
pub fn generate_artifact_id(command: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    command.hash(&mut hasher);
    let hash = hasher.finish();
    format!("cmd-{hash:x}.txt")
}

/// Process command execution parameters and return a structured result.
///
/// Note: The actual command execution is handled by [`execute_command`].
/// This function only validates parameters and prepares the result structure.
pub fn prepare_command_execution(
    params: &ExecuteCommandParams,
    timeout: Option<u64>,
) -> Result<PreparedCommand, CommandToolError> {
    validate_execute_command_params(params)?;
    let resolved_timeout = resolve_timeout(timeout)?;
    let artifact_id = generate_artifact_id(&params.command);

    Ok(PreparedCommand {
        command: params.command.clone(),
        timeout_secs: resolved_timeout,
        artifact_id,
    })
}

/// A prepared command ready for execution.
#[derive(Debug, Clone)]
pub struct PreparedCommand {
    pub command: String,
    pub timeout_secs: u64,
    pub artifact_id: String,
}

// ---------------------------------------------------------------------------
// Real execution
// ---------------------------------------------------------------------------

/// Result of a real command execution via [`TerminalRegistry`].
#[derive(Debug, Clone)]
pub struct ExecuteCommandResult {
    /// Combined output text (stdout + stderr, possibly truncated).
    pub output: String,
    /// Process exit code, if available.
    pub exit_code: Option<i32>,
    /// Artifact ID when output was persisted to disk.
    pub artifact_id: Option<String>,
    /// Whether the command timed out.
    pub was_timed_out: bool,
}

/// Execute a command via the [`TerminalRegistry`].
///
/// This is the core function that:
/// 1. Validates the command
/// 2. Resolves the working directory
/// 3. Creates/reuses a terminal from the registry
/// 4. Runs the command with optional timeout
/// 5. Persists large output to disk and returns a truncated preview
pub async fn execute_command(
    command: &str,
    cwd: Option<&Path>,
    timeout_ms: Option<u64>,
    registry: Arc<TerminalRegistry>,
    working_dir: &Path,
    output_dir: Option<&Path>,
    max_preview_lines: usize,
) -> Result<ExecuteCommandResult, String> {
    // 1. Validate command non-empty
    if command.trim().is_empty() {
        return Err("Command cannot be empty".to_string());
    }

    // 2. Resolve working directory
    let resolved_cwd = match cwd {
        Some(c) if !c.as_os_str().is_empty() => {
            if c.is_absolute() {
                c.to_path_buf()
            } else {
                working_dir.join(c)
            }
        }
        _ => working_dir.to_path_buf(),
    };

    // 3. Ensure cwd exists
    if !resolved_cwd.exists() {
        return Err(format!(
            "Working directory does not exist: '{}'. \
             Please verify the path is correct and the directory exists. \
             Command: '{}'",
            resolved_cwd.display(),
            command
        ));
    }

    // 4. Create a terminal via the registry
    let terminal_id = registry.create_terminal(&resolved_cwd).await;
    let terminal = registry
        .get_terminal(terminal_id)
        .await
        .ok_or("Terminal not found after creation")?;

    // 5. Generate artifact_id for potential output persistence
    let artifact_id = generate_artifact_id(command);

    // 6. Execute the command (with timeout)
    let timeout_duration = timeout_ms
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_secs(DEFAULT_TIMEOUT_SECS));

    let result = {
        let guard = terminal.lock().await;
        let callbacks = NoopCallbacks;
        match tokio::time::timeout(timeout_duration, guard.run_command(command, &callbacks)).await {
            Ok(Ok(cmd_result)) => cmd_result,
            Ok(Err(e)) => return Err(format!(
                "Command execution failed for '{}': {}. \
                 Working directory: '{}'. \
                 Please check that the command is valid and try again.",
                command, e, resolved_cwd.display()
            )),
            Err(_) => {
                // Timeout
                return Ok(ExecuteCommandResult {
                    output: format!(
                        "Command timed out after {}ms",
                        timeout_duration.as_millis()
                    ),
                    exit_code: None,
                    artifact_id: None,
                    was_timed_out: true,
                });
            }
        }
    };

    // 7. Process output — persist to disk if large
    let full_output = result.full_output();
    let line_count = full_output.lines().count();

    let (output, artifact_id_result) = if line_count > max_preview_lines {
        if let Some(dir) = output_dir {
            // Persist full output to disk
            let cmd_output_dir = dir.join("command-output");
            if let Err(e) = tokio::fs::create_dir_all(&cmd_output_dir).await {
                return Err(format!(
                    "Failed to create output directory '{}': {}. \
                     Command: '{}'",
                    cmd_output_dir.display(), e, command
                ));
            }
            let file_path = cmd_output_dir.join(&artifact_id);
            if let Err(e) = tokio::fs::write(&file_path, &full_output).await {
                return Err(format!(
                    "Failed to persist command output to '{}': {}. \
                     Command: '{}'",
                    file_path.display(), e, command
                ));
            }

            // Build truncated preview
            let preview: String = full_output
                .lines()
                .take(max_preview_lines)
                .collect::<Vec<&str>>()
                .join("\n");
            let preview_text = format!(
                "[OUTPUT TRUNCATED - Full output saved to artifact: {}]\n{}",
                artifact_id, preview
            );
            (preview_text, Some(artifact_id))
        } else {
            // No output dir — just truncate in memory
            let (truncated, _) = format_command_output(&full_output, MAX_OUTPUT_SIZE);
            (truncated, None)
        }
    } else {
        (full_output, None)
    };

    // 8. Format final result (matching TS format)
    let exit_status = crate::helpers::format_exit_status(result.exit_code);

    let final_output = if output.is_empty() {
        format!("Command executed in terminal within working directory '{}'. {}", resolved_cwd.display(), exit_status)
    } else {
        format!("Command executed in terminal within working directory '{}'. {}\nOutput:\n{}", resolved_cwd.display(), exit_status, output)
    };

    Ok(ExecuteCommandResult {
        output: final_output,
        exit_code: result.exit_code,
        artifact_id: artifact_id_result,
        was_timed_out: false,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_empty_command() {
        let params = ExecuteCommandParams {
            command: "".to_string(),
        };
        assert!(validate_execute_command_params(&params).is_err());
    }

    #[test]
    fn test_validate_whitespace_command() {
        let params = ExecuteCommandParams {
            command: "   ".to_string(),
        };
        assert!(validate_execute_command_params(&params).is_err());
    }

    #[test]
    fn test_validate_valid_command() {
        let params = ExecuteCommandParams {
            command: "echo hello".to_string(),
        };
        assert!(validate_execute_command_params(&params).is_ok());
    }

    #[test]
    fn test_generate_artifact_id() {
        let id1 = generate_artifact_id("echo hello");
        let id2 = generate_artifact_id("echo hello");
        let id3 = generate_artifact_id("echo world");

        assert!(id1.starts_with("cmd-"));
        assert!(id1.ends_with(".txt"));
        assert_eq!(id1, id2); // Same command = same ID
        assert_ne!(id1, id3); // Different command = different ID
    }

    #[test]
    fn test_prepare_command() {
        let params = ExecuteCommandParams {
            command: "cargo build".to_string(),
        };
        let prepared = prepare_command_execution(&params, Some(60)).unwrap();
        assert_eq!(prepared.command, "cargo build");
        assert_eq!(prepared.timeout_secs, 60);
        assert!(!prepared.artifact_id.is_empty());
    }

    #[test]
    fn test_prepare_command_default_timeout() {
        let params = ExecuteCommandParams {
            command: "echo hi".to_string(),
        };
        let prepared = prepare_command_execution(&params, None).unwrap();
        assert_eq!(prepared.timeout_secs, DEFAULT_TIMEOUT_SECS);
    }

    #[test]
    fn test_unescape_command() {
        assert_eq!(unescape_command("echo &"), "echo &");
        assert_eq!(unescape_command("echo <"), "echo <");
        assert_eq!(unescape_command("echo >"), "echo >");
        assert_eq!(unescape_command("echo &quot;"), "echo \"");
        assert_eq!(unescape_command("echo '"), "echo '");
        assert_eq!(unescape_command("echo '"), "echo '");
        assert_eq!(unescape_command("echo hello"), "echo hello");
        assert_eq!(
            unescape_command("<div>hello&world</div>"),
            "<div>hello&world</div>"
        );
    }

    #[tokio::test]
    async fn test_execute_command_simple() {
        let registry = Arc::new(TerminalRegistry::new());
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        let result = execute_command(
            "echo hello",
            None,
            Some(5000),
            registry,
            dir.path(),
            Some(dir.path()),
            50,
        )
        .await
        .expect("command should succeed");

        assert!(!result.was_timed_out);
        assert_eq!(result.exit_code, Some(0));
        assert!(
            result.output.contains("hello"),
            "expected 'hello' in output, got: {}",
            result.output
        );
    }

    #[tokio::test]
    async fn test_execute_command_empty_fails() {
        let registry = Arc::new(TerminalRegistry::new());
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        let result = execute_command(
            "",
            None,
            None,
            registry,
            dir.path(),
            None,
            50,
        )
        .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_command_nonexistent_cwd() {
        let registry = Arc::new(TerminalRegistry::new());
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let nonexistent = dir.path().join("does-not-exist");

        let result = execute_command(
            "echo hi",
            Some(&nonexistent),
            None,
            registry,
            dir.path(),
            None,
            50,
        )
        .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    #[tokio::test]
    async fn test_execute_command_failing_command() {
        let registry = Arc::new(TerminalRegistry::new());
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        // Use a command that will fail (exit non-zero)
        let result = execute_command(
            "exit 42",
            None,
            Some(5000),
            registry,
            dir.path(),
            None,
            50,
        )
        .await
        .expect("command should complete");

        assert!(!result.was_timed_out);
        assert_eq!(result.exit_code, Some(42));
    }

    #[tokio::test]
    async fn test_execute_command_output_truncation() {
        let registry = Arc::new(TerminalRegistry::new());
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        // Generate many lines of output (more than max_preview_lines=5)
        let result = execute_command(
            // Print 10 lines
            if cfg!(windows) {
                "for /L %i in (1,1,10) do @echo line%i"
            } else {
                "for i in $(seq 1 10); do echo \"line$i\"; done"
            },
            None,
            Some(10000),
            registry,
            dir.path(),
            Some(dir.path()),
            5, // max_preview_lines = 5
        )
        .await
        .expect("command should succeed");

        assert!(result.artifact_id.is_some(), "output should be truncated and persisted");
        assert!(result.output.contains("OUTPUT TRUNCATED"), "should contain truncation notice");
    }

    // --- Detailed error message tests ---

    #[tokio::test]
    async fn test_execute_command_nonexistent_cwd_detailed_error() {
        let registry = Arc::new(TerminalRegistry::new());
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        let result = execute_command(
            "echo hello",
            Some(std::path::Path::new("/nonexistent/path/that/does/not/exist")),
            Some(5000),
            registry,
            dir.path(),
            Some(dir.path()),
            100,
        )
        .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("does not exist"), "Error should mention missing path");
        assert!(err.contains("echo hello"), "Error should include the command");
        assert!(err.contains("verify the path"), "Error should suggest checking path");
    }

    #[test]
    fn test_prepare_command_validation_detailed_error() {
        let params = ExecuteCommandParams {
            command: "".to_string(),
        };
        let result = validate_execute_command_params(&params);
        assert!(result.is_err());
    }
}

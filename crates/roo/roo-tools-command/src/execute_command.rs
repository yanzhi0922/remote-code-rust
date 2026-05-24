//! Execute command tool implementation.
//!
//! Provides both parameter validation helpers (used by tests and the dispatcher)
//! and the real command execution logic that integrates with [`roo_terminal`].
//!
//! # Improvements over basic execution
//!
//! 1. **RooIgnore command validation** – commands that read blocked files are
//!    rejected before spawning a process (matches TS `validateCommand`).
//! 2. **Dual timeout (agent + user)** – an *agent* timeout sends a partial-
//!    output notification and lets the command keep running in the background;
//!    a *user* timeout hard-kills the process.
//! 3. **Output streaming** – an optional callback is invoked for every output
//!    line, throttled to avoid overwhelming the terminal / UI.

use crate::helpers::*;
use crate::types::*;
use roo_ignore::RooIgnoreController;
use roo_terminal::registry::TerminalRegistry;
use roo_terminal::terminal::RooTerminal;
use roo_terminal::types::{CommandResult, TerminalCallbacks};
use roo_types::tool::ExecuteCommandParams;

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

/// Unescape HTML entities in a command string.
///
/// Matches TS: `const canonicalCommand = unescapeHtmlEntities(command)`
/// Some LLMs (non-Claude) may output HTML entities like `<` instead of `<`
/// in their tool call parameters.
///
/// Handles:
/// - Named entities: `&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;`, `&#39;`
/// - Hex numeric references: `&#x2F;`, `&#X3C;` etc.
/// - Decimal numeric references: `&#60;`, `&#47;` etc.
pub fn unescape_command(command: &str) -> String {
    let mut result = command
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'");

    // Handle hex numeric references: &#xHH; or &#XHH;
    let hex_re = regex::Regex::new(r"&#[xX]([0-9a-fA-F]+);").unwrap(); // SAFE: invariant guaranteed by construction
    let hex_result = hex_re.replace_all(&result, |caps: &regex::Captures| {
        if let Ok(code) = u32::from_str_radix(&caps[1], 16) {
            char::from_u32(code).map_or_else(|| caps[0].to_string(), |c| c.to_string())
        } else {
            caps[0].to_string()
        }
    });
    result = hex_result.into_owned();

    // Handle decimal numeric references: &#NN;
    let dec_re = regex::Regex::new(r"&#(\d+);").unwrap(); // SAFE: invariant guaranteed by construction
    let dec_result = dec_re.replace_all(&result, |caps: &regex::Captures| {
        if let Ok(code) = caps[1].parse::<u32>() {
            char::from_u32(code).map_or_else(|| caps[0].to_string(), |c| c.to_string())
        } else {
            caps[0].to_string()
        }
    });
    result = dec_result.into_owned();

    result
}

// ---------------------------------------------------------------------------
// Validation & preparation (kept for backward-compat)
// ---------------------------------------------------------------------------

/// Validate execute_command parameters.
pub fn validate_execute_command_params(
    params: &ExecuteCommandParams,
) -> Result<(), CommandToolError> {
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
// Streaming callback
// ---------------------------------------------------------------------------

/// Callback trait for streaming command output.
///
/// Implementations receive each output line as it is produced by the
/// running command. The throttling logic (see
/// [`ExecuteCommandOptions::stream_throttle_ms`]) ensures the callback is
/// invoked at most once every `stream_throttle_ms` milliseconds.
pub trait CommandOutputStreamer: Send + Sync {
    /// Called with accumulated (possibly compressed) output text.
    ///
    /// `partial` is `true` while the command is still running and `false`
    /// on the final invocation.
    fn on_output(&self, text: &str, partial: bool);
}

/// A no-op streamer that discards all output.
pub struct NoopStreamer;

impl CommandOutputStreamer for NoopStreamer {
    fn on_output(&self, _text: &str, _partial: bool) {}
}

/// A simple streamer that prints each update to stdout.
pub struct StdoutStreamer {
    /// Set to `true` to prefix each chunk with `[cmd] `.
    pub prefix: bool,
}

impl CommandOutputStreamer for StdoutStreamer {
    fn on_output(&self, text: &str, partial: bool) {
        let tag = if partial { "(running)" } else { "(done)" };
        if self.prefix {
            eprintln!("[cmd][{tag}] {text}");
        } else {
            eprintln!("[{tag}] {text}");
        }
    }
}

// ---------------------------------------------------------------------------
// Internal streaming callbacks adapter
// ---------------------------------------------------------------------------

/// Adapter that bridges [`TerminalCallbacks`] to both a
/// [`CommandOutputStreamer`] and optional RooIgnore validation.
struct StreamingCallbacks<'a> {
    streamer: &'a dyn CommandOutputStreamer,
    throttle_ms: u64,
    /// Accumulated output (bounded to ~100 KB to avoid unbounded growth).
    accumulated: std::sync::Mutex<String>,
    /// Timestamp (ms via [`Instant`]) of the last stream emission.
    last_emit: std::sync::Mutex<std::time::Instant>,
}

impl<'a> StreamingCallbacks<'a> {
    fn new(streamer: &'a dyn CommandOutputStreamer, throttle_ms: u64) -> Self {
        Self {
            streamer,
            throttle_ms,
            accumulated: std::sync::Mutex::new(String::new()),
            last_emit: std::sync::Mutex::new(
                std::time::Instant::now() - Duration::from_millis(throttle_ms + 1),
            ),
        }
    }

    /// Emit throttled partial output.
    fn maybe_emit(&self) {
        let mut last = self.last_emit.lock().unwrap_or_else(|e| e.into_inner());
        let elapsed = last.elapsed().as_millis() as u64;
        if elapsed >= self.throttle_ms {
            let acc = self.accumulated.lock().unwrap_or_else(|e| e.into_inner());
            if !acc.is_empty() {
                self.streamer.on_output(&acc, true);
            }
            *last = std::time::Instant::now();
        }
    }
}

impl TerminalCallbacks for StreamingCallbacks<'_> {
    fn on_line(&self, line: &str) {
        const MAX_ACCUMULATED: usize = 100_000;
        let mut acc = self.accumulated.lock().unwrap_or_else(|e| e.into_inner());
        acc.push_str(line);
        acc.push('\n');
        if acc.len() > MAX_ACCUMULATED {
            let keep = acc.len().saturating_sub(MAX_ACCUMULATED);
            acc.drain(0..keep);
        }
        drop(acc);
        self.maybe_emit();
    }

    fn on_completed(&self, result: &CommandResult) {
        // Final emit (non-partial).
        let acc = self.accumulated.lock().unwrap_or_else(|e| e.into_inner());
        let final_output = if result.full_output().len() > acc.len() {
            result.full_output()
        } else {
            acc.clone()
        };
        self.streamer.on_output(&final_output, false);
    }

    fn on_shell_execution_started(&self, _pid: u32) {}
    fn on_shell_execution_complete(&self, _details: &roo_terminal::types::ShellExecutionDetails) {}
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Configuration for [`execute_command`].
#[derive(Debug, Clone)]
pub struct ExecuteCommandOptions {
    /// Agent-specified timeout in **milliseconds**.
    ///
    /// When this fires the command transitions to "background" mode:
    /// a partial-output notification is emitted and the caller receives the
    /// output collected so far, but the child process continues running.
    ///
    /// Set to `None` or `0` to disable.
    pub agent_timeout_ms: Option<u64>,

    /// User-configured hard timeout in **milliseconds**.
    ///
    /// When this fires the child process is killed (SIGTERM / SIGKILL on
    /// Unix, `TerminateProcess` on Windows) and the caller receives a
    /// timeout error.
    ///
    /// Set to `None` or `0` to disable.
    pub user_timeout_ms: Option<u64>,

    /// Throttle interval for the output streamer in milliseconds.
    ///
    /// The streamer callback is invoked at most once every
    /// `stream_throttle_ms` ms. Defaults to 100 ms (matching the TS
    /// default of ~150 ms but slightly more aggressive for CLI use).
    pub stream_throttle_ms: u64,
}

impl Default for ExecuteCommandOptions {
    fn default() -> Self {
        Self {
            agent_timeout_ms: None,
            user_timeout_ms: None,
            stream_throttle_ms: 100,
        }
    }
}

// ---------------------------------------------------------------------------
// Result
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

// ---------------------------------------------------------------------------
// Dual-timeout helper
// ---------------------------------------------------------------------------

/// Outcome of the dual-timeout wrapper.
enum DualTimeoutResult {
    /// Command finished (success or non-zero exit).
    Completed(CommandResult),
    /// The user (hard) timeout fired – process should be killed.
    UserTimedOut,
    /// The command future itself returned an error (e.g. spawn failure).
    Failed(roo_terminal::terminal::TerminalError),
}

/// Drive a command future with optional agent and user timeouts.
///
/// * **No agent timeout**: equivalent to `tokio::time::timeout(user, future)`.
/// * **Agent timeout fires**: emit a partial-output notification and keep
///   waiting for the command to complete or the user timeout, whichever comes
///   first.
/// * **User timeout fires**: return [`DualTimeoutResult::UserTimedOut`].
async fn run_with_dual_timeout<F>(
    cmd_future: F,
    agent_timeout: Option<Duration>,
    user_timeout: Duration,
    streamer: &dyn CommandOutputStreamer,
) -> DualTimeoutResult
where
    F: std::future::Future<Output = Result<CommandResult, roo_terminal::terminal::TerminalError>>,
{
    match agent_timeout {
        None => {
            // Simple single-timeout path.
            match tokio::time::timeout(user_timeout, cmd_future).await {
                Ok(Ok(result)) => DualTimeoutResult::Completed(result),
                Ok(Err(e)) => DualTimeoutResult::Failed(e),
                Err(_) => DualTimeoutResult::UserTimedOut,
            }
        }
        Some(at) => {
            // Phase 1: race command vs agent timeout.
            // We use a pin to make the future `Unpin`-safe for `select!`.
            let cmd_future = Box::pin(cmd_future);
            let mut cmd_future = cmd_future;

            // Check if agent timeout already exceeds user timeout.
            if at >= user_timeout {
                // Agent timeout is moot — just use user timeout.
                return match tokio::time::timeout(user_timeout, cmd_future).await {
                    Ok(Ok(result)) => DualTimeoutResult::Completed(result),
                    Ok(Err(e)) => DualTimeoutResult::Failed(e),
                    Err(_) => DualTimeoutResult::UserTimedOut,
                };
            }

            // Phase 1: race command completion vs agent timeout.
            let phase1 = tokio::select! {
                result = &mut cmd_future => {
                    // Command completed before agent timeout.
                    return match result {
                        Ok(cmd_result) => DualTimeoutResult::Completed(cmd_result),
                        Err(e) => DualTimeoutResult::Failed(e),
                    };
                }
                _ = tokio::time::sleep(at) => {
                    // Agent timeout fired.
                    at
                }
            };

            let agent_elapsed = phase1;

            // If we get here the agent timeout fired.  Emit a warning.
            streamer.on_output(
                &format!(
                    "[Agent timeout reached after {}ms – command continues in background]",
                    at.as_millis()
                ),
                true,
            );

            // Phase 2: wait for command completion or remaining user timeout.
            let remaining = user_timeout.saturating_sub(agent_elapsed);
            match tokio::time::timeout(remaining, cmd_future).await {
                Ok(Ok(result)) => DualTimeoutResult::Completed(result),
                Ok(Err(e)) => DualTimeoutResult::Failed(e),
                Err(_) => DualTimeoutResult::UserTimedOut,
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Real execution
// ---------------------------------------------------------------------------

/// Execute a command via the [`TerminalRegistry`].
///
/// This is the core function that:
/// 1. Validates the command (including roo-ignore checks)
/// 2. Resolves the working directory
/// 3. Creates/reuses a terminal from the registry
/// 4. Runs the command with dual timeout (agent + user)
/// 5. Streams output via optional callback
/// 6. Persists large output to disk and returns a truncated preview
pub async fn execute_command(
    command: &str,
    cwd: Option<&Path>,
    timeout_ms: Option<u64>,
    registry: Arc<TerminalRegistry>,
    working_dir: &Path,
    output_dir: Option<&Path>,
    max_preview_lines: usize,
) -> Result<ExecuteCommandResult, String> {
    execute_command_with_opts(
        command,
        cwd,
        timeout_ms,
        registry,
        working_dir,
        output_dir,
        max_preview_lines,
        &NoopStreamer,
        ExecuteCommandOptions::default(),
        None,
    )
    .await
}

/// Execute a command with all extensions enabled.
///
/// See [`execute_command`] for the minimal API. This version adds:
/// - `streamer` – callback for streaming output lines
/// - `opts` – dual-timeout and throttle configuration
/// - `roo_ignore` – optional [`RooIgnoreController`] for command validation
pub async fn execute_command_with_opts(
    command: &str,
    cwd: Option<&Path>,
    timeout_ms: Option<u64>,
    registry: Arc<TerminalRegistry>,
    working_dir: &Path,
    output_dir: Option<&Path>,
    max_preview_lines: usize,
    streamer: &dyn CommandOutputStreamer,
    opts: ExecuteCommandOptions,
    roo_ignore: Option<&RooIgnoreController>,
) -> Result<ExecuteCommandResult, String> {
    // --- 0. Unescape HTML entities in command -------------------------
    // TS: `const canonicalCommand = unescapeHtmlEntities(command)`
    // Some LLMs (non-Claude) may output HTML entities like &lt; instead of <
    // in their tool call parameters. This must happen BEFORE any validation.
    let command = unescape_command(command);

    // --- 1. Validate command non-empty --------------------------------
    if command.trim().is_empty() {
        return Err("Command cannot be empty".to_string());
    }

    // --- 2. RooIgnore command validation ------------------------------
    if let Some(controller) = roo_ignore
        && let Some(blocked_path) = controller.validate_command(&command)
    {
        return Ok(ExecuteCommandResult {
            output: format!(
                "Command blocked by .rooignore rules: '{}' accesses a restricted file. \
                     Please check your .rooignore configuration.",
                blocked_path
            ),
            exit_code: None,
            artifact_id: None,
            was_timed_out: false,
        });
    }

    // --- 3. Resolve working directory ---------------------------------
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

    // 4. Ensure cwd exists
    if !resolved_cwd.exists() {
        return Err(format!(
            "Working directory does not exist: '{}'. \
             Please verify the path is correct and the directory exists. \
             Command: '{}'",
            resolved_cwd.display(),
            command
        ));
    }

    // 5. Create a terminal via the registry
    let terminal_id = registry.create_terminal(&resolved_cwd).await;
    let terminal = registry
        .get_terminal(terminal_id)
        .await
        .ok_or("Terminal not found after creation")?;

    // 6. Generate artifact_id for potential output persistence
    let artifact_id = generate_artifact_id(&command);

    // --- 7. Dual-timeout execution ------------------------------------
    let callbacks = StreamingCallbacks::new(streamer, opts.stream_throttle_ms);
    let agent_timeout = opts
        .agent_timeout_ms
        .filter(|&t| t > 0)
        .map(Duration::from_millis);
    let user_timeout = opts
        .user_timeout_ms
        .filter(|&t| t > 0)
        .map(Duration::from_millis);

    // Fall back to the legacy `timeout_ms` parameter when the opts don't
    // specify a user timeout.
    let legacy_timeout = timeout_ms
        .filter(|_| user_timeout.is_none())
        .map(Duration::from_millis);

    let effective_user_timeout = user_timeout
        .or(legacy_timeout)
        .unwrap_or_else(|| Duration::from_secs(DEFAULT_TIMEOUT_SECS));

    // NOTE: The terminal MutexGuard is held across the `run_with_dual_timeout`
    // await below. This is intentional and safe: `run_command` borrows `&self`
    // from the guard, so the guard must outlive the future. The lock is
    // per-terminal (not a global lock), so holding it only blocks other
    // operations on this specific terminal — which is correct since a terminal
    // can only run one command at a time.
    let result = {
        let guard = terminal.lock().await;
        let cmd_future = guard.run_command(&command, &callbacks);

        // We wrap the command future in a helper that respects both timeouts.
        let result =
            run_with_dual_timeout(cmd_future, agent_timeout, effective_user_timeout, streamer)
                .await;

        match result {
            DualTimeoutResult::Completed(cmd_result) => cmd_result,
            DualTimeoutResult::UserTimedOut => {
                return Ok(ExecuteCommandResult {
                    output: format!(
                        "Command timed out after {}ms",
                        effective_user_timeout.as_millis()
                    ),
                    exit_code: None,
                    artifact_id: None,
                    was_timed_out: true,
                });
            }
            DualTimeoutResult::Failed(e) => {
                return Err(format!(
                    "Command execution failed for '{}': {}. \
                     Working directory: '{}'. \
                     Please check that the command is valid and try again.",
                    command,
                    e,
                    resolved_cwd.display()
                ));
            }
        }
    };

    // 8. Process output — persist to disk if large
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
                    cmd_output_dir.display(),
                    e,
                    command
                ));
            }
            let file_path = cmd_output_dir.join(&artifact_id);
            if let Err(e) = tokio::fs::write(&file_path, &full_output).await {
                return Err(format!(
                    "Failed to persist command output to '{}': {}. \
                     Command: '{}'",
                    file_path.display(),
                    e,
                    command
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

    // 9. Format final result (matching TS format)
    let exit_status = crate::helpers::format_exit_status(result.exit_code);

    let final_output = if output.is_empty() {
        format!(
            "Command executed in terminal within working directory '{}'. {}",
            resolved_cwd.display(),
            exit_status
        )
    } else {
        format!(
            "Command executed in terminal within working directory '{}'. {}\nOutput:\n{}",
            resolved_cwd.display(),
            exit_status,
            output
        )
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

    #[test]
    fn test_unescape_command_hex_numeric() {
        // &#x2F; → /
        assert_eq!(unescape_command("echo &#x2F;usr&#x2F;bin"), "echo /usr/bin");
        // &#x3C; → <  (uppercase X)
        assert_eq!(unescape_command("&#X3C;tag&#X3E;"), "<tag>");
    }

    #[test]
    fn test_unescape_command_decimal_numeric() {
        // &#60; → <, &#62; → >
        assert_eq!(unescape_command("&#60;div&#62;"), "<div>");
        // &#47; → /
        assert_eq!(unescape_command("a&#47;b"), "a/b");
    }

    #[test]
    fn test_unescape_command_invalid_numeric() {
        // Invalid hex should be left unchanged
        assert_eq!(unescape_command("&#xGG;"), "&#xGG;");
        // Invalid decimal should be left unchanged
        assert_eq!(unescape_command("&#abc;"), "&#abc;");
    }

    // --- RooIgnore command validation tests ---

    #[tokio::test]
    async fn test_execute_command_rooignore_blocks_reading() {
        let mut controller = RooIgnoreController::new("/project");
        controller.load_patterns("secret.txt");

        let registry = Arc::new(TerminalRegistry::new());
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        let result = execute_command_with_opts(
            "cat secret.txt",
            None,
            Some(5000),
            registry,
            dir.path(),
            Some(dir.path()),
            50,
            &NoopStreamer,
            ExecuteCommandOptions::default(),
            Some(&controller),
        )
        .await
        .expect("should return Ok (not an error)");

        assert!(
            result.output.contains("blocked by .rooignore"),
            "expected rooignore block message, got: {}",
            result.output
        );
    }

    #[tokio::test]
    async fn test_execute_command_rooignore_allows_non_reading() {
        let mut controller = RooIgnoreController::new("/project");
        controller.load_patterns("secret.txt");

        let registry = Arc::new(TerminalRegistry::new());
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        let result = execute_command_with_opts(
            "echo secret.txt",
            None,
            Some(5000),
            registry,
            dir.path(),
            None,
            50,
            &NoopStreamer,
            ExecuteCommandOptions::default(),
            Some(&controller),
        )
        .await
        .expect("command should succeed");

        assert!(
            !result.output.contains("blocked by .rooignore"),
            "echo should not be blocked, got: {}",
            result.output
        );
    }

    #[tokio::test]
    async fn test_execute_command_rooignore_none_allows_all() {
        let registry = Arc::new(TerminalRegistry::new());
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        let result = execute_command_with_opts(
            "cat anything.txt",
            None,
            Some(5000),
            registry,
            dir.path(),
            None,
            50,
            &NoopStreamer,
            ExecuteCommandOptions::default(),
            None, // No roo-ignore controller
        )
        .await
        .expect("command should succeed");

        assert!(
            !result.output.contains("blocked by .rooignore"),
            "without controller, nothing should be blocked, got: {}",
            result.output
        );
    }

    // --- Streaming callback tests ---

    /// A streamer that records all calls.
    struct RecordingStreamer {
        calls: std::sync::Mutex<Vec<(String, bool)>>,
    }

    impl RecordingStreamer {
        fn new() -> Self {
            Self {
                calls: std::sync::Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<(String, bool)> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl CommandOutputStreamer for RecordingStreamer {
        fn on_output(&self, text: &str, partial: bool) {
            self.calls.lock().unwrap().push((text.to_string(), partial));
        }
    }

    #[tokio::test]
    async fn test_execute_command_streaming_receives_output() {
        let registry = Arc::new(TerminalRegistry::new());
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let streamer = RecordingStreamer::new();

        let result = execute_command_with_opts(
            "echo hello_stream",
            None,
            Some(5000),
            registry,
            dir.path(),
            None,
            50,
            &streamer,
            ExecuteCommandOptions {
                stream_throttle_ms: 0, // no throttling for test
                ..Default::default()
            },
            None,
        )
        .await
        .expect("command should succeed");

        assert!(!result.was_timed_out);

        let calls = streamer.calls();
        // Should have at least one non-partial (final) call
        let final_calls: Vec<_> = calls.iter().filter(|(_, p)| !p).collect();
        assert!(
            !final_calls.is_empty(),
            "expected at least one final (partial=false) streaming call, got: {:?}",
            calls
        );
        // The final output should contain the command output
        assert!(
            final_calls
                .iter()
                .any(|(text, _)| text.contains("hello_stream")),
            "expected 'hello_stream' in streaming output, got: {:?}",
            calls
        );
    }

    // --- Dual timeout tests ---

    #[tokio::test]
    async fn test_execute_command_user_timeout() {
        let registry = Arc::new(TerminalRegistry::new());
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        // Use a command that sleeps for a long time
        let sleep_cmd = if cfg!(windows) {
            "ping -n 30 127.0.0.1 > NUL"
        } else {
            "sleep 30"
        };

        let result = execute_command_with_opts(
            sleep_cmd,
            None,
            None,
            registry,
            dir.path(),
            None,
            50,
            &NoopStreamer,
            ExecuteCommandOptions {
                user_timeout_ms: Some(500), // 500ms user timeout
                ..Default::default()
            },
            None,
        )
        .await
        .expect("should return Ok");

        assert!(result.was_timed_out, "expected timeout, got: {:?}", result);
    }

    // --- Original tests preserved ---

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

        let result = execute_command("", None, None, registry, dir.path(), None, 50).await;

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
        let result = execute_command("exit 42", None, Some(5000), registry, dir.path(), None, 50)
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

        assert!(
            result.artifact_id.is_some(),
            "output should be truncated and persisted"
        );
        assert!(
            result.output.contains("OUTPUT TRUNCATED"),
            "should contain truncation notice"
        );
    }

    // --- Detailed error message tests ---

    #[tokio::test]
    async fn test_execute_command_nonexistent_cwd_detailed_error() {
        let registry = Arc::new(TerminalRegistry::new());
        let dir = tempfile::tempdir().expect("failed to create temp dir");

        let result = execute_command(
            "echo hello",
            Some(std::path::Path::new(
                "/nonexistent/path/that/does/not/exist",
            )),
            Some(5000),
            registry,
            dir.path(),
            Some(dir.path()),
            100,
        )
        .await;

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.contains("does not exist"),
            "Error should mention missing path"
        );
        assert!(
            err.contains("echo hello"),
            "Error should include the command"
        );
        assert!(
            err.contains("verify the path"),
            "Error should suggest checking path"
        );
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

//! Terminal abstraction and default implementation.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::types::{CommandResult, ShellExecutionDetails, TerminalCallbacks, TerminalId};

/// Error type for terminal operations.
#[derive(Debug, thiserror::Error)]
pub enum TerminalError {
    /// The terminal has been closed and cannot execute commands.
    #[error("terminal {id} is closed")]
    Closed { id: TerminalId },

    /// The terminal is busy executing another command.
    #[error("terminal {id} is busy")]
    Busy { id: TerminalId },

    /// Failed to spawn the command process.
    #[error("failed to spawn command '{command}': {reason}")]
    SpawnFailed { command: String, reason: String },

    /// The command execution timed out.
    #[error("command '{command}' timed out after {timeout_ms}ms")]
    Timeout { command: String, timeout_ms: u64 },

    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Trait representing a terminal that can execute commands.
///
/// This is the core abstraction for terminal operations, providing a unified
/// interface regardless of the underlying implementation (VS Code terminal,
/// system shell, etc.).
pub trait RooTerminal: Send + Sync {
    /// Get the terminal's unique identifier.
    fn get_id(&self) -> TerminalId;

    /// Execute a command in this terminal.
    ///
    /// Returns a [`CommandResult`] with the output and exit code.
    fn run_command(
        &self,
        command: &str,
        callbacks: &dyn TerminalCallbacks,
    ) -> impl std::future::Future<Output = Result<CommandResult, TerminalError>> + Send;

    /// Check if the terminal is currently busy executing a command.
    fn is_busy(&self) -> bool;

    /// Check if the terminal has been closed.
    fn is_closed(&self) -> bool;

    /// Get the current working directory of the terminal.
    fn get_cwd(&self) -> &Path;

    /// Close the terminal, releasing any resources.
    fn close(&mut self);
}

/// Default terminal implementation using `tokio::process::Command`.
///
/// This implementation spawns shell processes directly on the host system,
/// providing cross-platform command execution without any VS Code dependency.
#[derive(Debug)]
pub struct DefaultTerminal {
    /// Unique identifier for this terminal.
    id: TerminalId,
    /// The initial working directory.
    cwd: PathBuf,
    /// Whether the terminal is currently busy.
    busy: bool,
    /// Whether the terminal has been closed.
    closed: bool,
    /// Optional task ID this terminal is associated with.
    /// Corresponds to TS: `taskId?: string`
    task_id: Option<String>,
    /// Unretrieved output buffer.
    /// Corresponds to TS: `unretrievedOutput`
    unretrieved_output: String,
}

impl DefaultTerminal {
    /// Create a new `DefaultTerminal` with the given ID and working directory.
    pub fn new(id: TerminalId, cwd: impl Into<PathBuf>) -> Self {
        Self {
            id,
            cwd: cwd.into(),
            busy: false,
            closed: false,
            task_id: None,
            unretrieved_output: String::new(),
        }
    }

    /// Get the terminal's unique ID.
    pub fn get_id(&self) -> TerminalId {
        self.id
    }

    /// Get the working directory.
    pub fn get_cwd(&self) -> &Path {
        &self.cwd
    }

    /// Get the task ID associated with this terminal, if any.
    /// Corresponds to TS: `terminal.taskId`
    pub fn task_id(&self) -> Option<&str> {
        self.task_id.as_deref()
    }

    /// Set the task ID for this terminal.
    pub fn set_task_id(&mut self, task_id: String) {
        if task_id.is_empty() {
            self.task_id = None;
        } else {
            self.task_id = Some(task_id);
        }
    }

    /// Get unretrieved output from this terminal.
    /// Corresponds to TS: `getUnretrievedOutput()`
    pub fn get_unretrieved_output(&mut self) -> String {
        let output = self.unretrieved_output.clone();
        self.unretrieved_output.clear();
        output
    }

    /// Append to unretrieved output buffer.
    pub fn append_unretrieved_output(&mut self, text: &str) {
        self.unretrieved_output.push_str(text);
    }

    /// Build the shell command for the current platform using tokio.
    fn build_shell_command(
        command: &str,
        cwd: &Path,
        env: &HashMap<String, String>,
    ) -> tokio::process::Command {
        use std::process::Stdio;

        let mut cmd = if cfg!(target_os = "windows") {
            let mut c = tokio::process::Command::new("cmd");
            c.args(["/C", command]);
            c
        } else {
            let mut c = tokio::process::Command::new("sh");
            c.args(["-c", command]);
            c
        };
        cmd.current_dir(cwd)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in env {
            cmd.env(key, value);
        }
        cmd
    }
}

impl RooTerminal for DefaultTerminal {
    fn get_id(&self) -> TerminalId {
        self.id
    }

    async fn run_command(
        &self,
        command: &str,
        callbacks: &dyn TerminalCallbacks,
    ) -> Result<CommandResult, TerminalError> {
        if self.closed {
            return Err(TerminalError::Closed { id: self.id });
        }

        let env = get_env();
        let mut cmd = Self::build_shell_command(command, &self.cwd, &env);

        let child = cmd.spawn().map_err(|e| TerminalError::SpawnFailed {
            command: command.to_string(),
            reason: e.to_string(),
        })?;

        // Get the PID before waiting
        let pid = child.id().unwrap_or(0);
        callbacks.on_shell_execution_started(pid);

        let output = child.wait_with_output().await?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        // Call on_line for each line of stdout
        for line in stdout.lines() {
            callbacks.on_line(line);
        }

        let exit_code = output.status.code();
        let result = CommandResult::new(exit_code, stdout.clone(), stderr.clone());

        callbacks.on_completed(&result);

        let details = ShellExecutionDetails::new(exit_code, None);
        callbacks.on_shell_execution_complete(&details);

        Ok(result)
    }

    fn is_busy(&self) -> bool {
        self.busy
    }

    fn is_closed(&self) -> bool {
        self.closed
    }

    fn get_cwd(&self) -> &Path {
        &self.cwd
    }

    fn close(&mut self) {
        self.closed = true;
        self.busy = false;
    }
}

/// Returns the default environment variables for terminal operations.
///
/// These environment variables are set to ensure consistent behavior
/// across different shell environments:
/// - `ROO_ACTIVE=true` — Marks that a Roo terminal session is active.
/// - `PAGER=cat` — Ensures paged output is not split (Unix only).
/// - `VTE_VERSION=0` — Disables VTE-specific shell integration features.
pub fn get_env() -> HashMap<String, String> {
    let mut env = HashMap::new();
    env.insert("ROO_ACTIVE".to_string(), "true".to_string());

    if cfg!(windows) {
        // On Windows, PAGER is typically not needed
    } else {
        env.insert("PAGER".to_string(), "cat".to_string());
    }

    env.insert("VTE_VERSION".to_string(), "0".to_string());
    env
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// A test callback that records all events.
    #[derive(Debug, Default)]
    struct TestCallbacks {
        lines: Arc<Mutex<Vec<String>>>,
        completed: Arc<Mutex<Vec<CommandResult>>>,
        started_pids: Arc<Mutex<Vec<u32>>>,
        completed_details: Arc<Mutex<Vec<ShellExecutionDetails>>>,
    }

    impl TestCallbacks {
        fn new() -> Self {
            Self::default()
        }
    }

    impl TerminalCallbacks for TestCallbacks {
        fn on_line(&self, line: &str) {
            self.lines.lock().unwrap().push(line.to_string());
        }
        fn on_completed(&self, result: &CommandResult) {
            self.completed.lock().unwrap().push(result.clone());
        }
        fn on_shell_execution_started(&self, pid: u32) {
            self.started_pids.lock().unwrap().push(pid);
        }
        fn on_shell_execution_complete(&self, details: &ShellExecutionDetails) {
            self.completed_details.lock().unwrap().push(details.clone());
        }
    }

    #[test]
    fn test_default_terminal_new() {
        let terminal = DefaultTerminal::new(TerminalId::new(1), "/tmp");
        assert_eq!(terminal.get_id(), TerminalId::new(1));
        assert_eq!(terminal.get_cwd(), Path::new("/tmp"));
        assert!(!terminal.is_busy());
        assert!(!terminal.is_closed());
    }

    #[test]
    fn test_default_terminal_close() {
        let mut terminal = DefaultTerminal::new(TerminalId::new(2), "/tmp");
        assert!(!terminal.is_closed());
        terminal.close();
        assert!(terminal.is_closed());
        assert!(!terminal.is_busy());
    }

    #[tokio::test]
    async fn test_default_terminal_run_closed() {
        let mut terminal = DefaultTerminal::new(TerminalId::new(3), "/tmp");
        terminal.close();
        let callbacks = crate::types::NoopCallbacks;
        let result = terminal.run_command("echo hello", &callbacks).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            TerminalError::Closed { id } => assert_eq!(id, TerminalId::new(3)),
            e => panic!("Expected Closed error, got: {e}"),
        }
    }

    #[tokio::test]
    async fn test_default_terminal_run_simple_command() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let terminal = DefaultTerminal::new(TerminalId::new(4), dir.path());

        let callbacks = TestCallbacks::new();
        let result = terminal.run_command("echo hello world", &callbacks).await;

        assert!(result.is_ok());
        let cmd_result = result.unwrap();
        assert!(cmd_result.is_success());
        // On Windows, cmd /C echo may output to console buffer rather than stdout.
        // Check full output (stdout + stderr) for the expected text.
        let full = cmd_result.full_output();
        assert!(
            full.contains("hello world"),
            "expected 'hello world' in output, got stdout={:?} stderr={:?}",
            cmd_result.stdout,
            cmd_result.stderr
        );

        // Check callbacks were called
        assert!(!callbacks.started_pids.lock().unwrap().is_empty());
        assert!(!callbacks.completed.lock().unwrap().is_empty());
        assert!(!callbacks.completed_details.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_default_terminal_run_failing_command() {
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let terminal = DefaultTerminal::new(TerminalId::new(5), dir.path());

        let callbacks = TestCallbacks::new();
        let result = terminal
            .run_command(
                if cfg!(windows) {
                    "exit /b 42"
                } else {
                    "exit 42"
                },
                &callbacks,
            )
            .await;

        assert!(result.is_ok());
        let cmd_result = result.unwrap();
        assert!(!cmd_result.is_success());
        assert_eq!(cmd_result.exit_code, Some(42));
    }

    #[test]
    fn test_get_env() {
        let env = get_env();
        assert_eq!(env.get("ROO_ACTIVE").unwrap(), "true");
        assert_eq!(env.get("VTE_VERSION").unwrap(), "0");

        #[cfg(not(windows))]
        {
            assert_eq!(env.get("PAGER").unwrap(), "cat");
        }
        #[cfg(windows)]
        {
            assert!(env.get("PAGER").is_none());
        }
    }

    #[test]
    fn test_terminal_error_display() {
        let err = TerminalError::Closed {
            id: TerminalId::new(42),
        };
        assert!(err.to_string().contains("42"));
        assert!(err.to_string().contains("closed"));

        let err = TerminalError::Busy {
            id: TerminalId::new(1),
        };
        assert!(err.to_string().contains("busy"));

        let err = TerminalError::SpawnFailed {
            command: "test".into(),
            reason: "not found".into(),
        };
        assert!(err.to_string().contains("test"));
    }
}

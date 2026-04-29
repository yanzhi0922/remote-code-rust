//! Terminal process management with state machine.

use std::sync::Arc;

use tokio::sync::Mutex;

use crate::types::{CommandResult, ShellExecutionDetails};

/// The state of a terminal process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProcessState {
    /// The process has been created but not yet started.
    Pending,
    /// The process is currently running.
    Running,
    /// The process completed successfully.
    Completed,
    /// The process failed (error or non-zero exit).
    Failed,
}

impl std::fmt::Display for ProcessState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessState::Pending => write!(f, "pending"),
            ProcessState::Running => write!(f, "running"),
            ProcessState::Completed => write!(f, "completed"),
            ProcessState::Failed => write!(f, "failed"),
        }
    }
}

impl Default for ProcessState {
    fn default() -> Self {
        ProcessState::Pending
    }
}

/// A managed terminal process that tracks command execution state.
///
/// `TerminalProcess` implements a state machine with the following transitions:
/// - `Pending` → `Running` (when the command starts executing)
/// - `Running` → `Completed` (when the command finishes successfully)
/// - `Running` → `Failed` (when the command fails or times out)
/// - `Pending` → `Failed` (when the command fails to start)
#[derive(Debug)]
pub struct TerminalProcess {
    /// The command being executed.
    command: String,
    /// The current state of the process.
    state: ProcessState,
    /// The accumulated stdout output.
    output: String,
    /// The accumulated stderr output.
    stderr_output: String,
    /// The process ID, if available.
    pid: Option<u32>,
    /// The exit code, if the process has completed.
    exit_code: Option<i32>,
    /// The signal that terminated the process, if any.
    signal: Option<i32>,
}

impl TerminalProcess {
    /// Create a new `TerminalProcess` for the given command.
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            state: ProcessState::Pending,
            output: String::new(),
            stderr_output: String::new(),
            pid: None,
            exit_code: None,
            signal: None,
        }
    }

    /// Returns the command being executed.
    pub fn command(&self) -> &str {
        &self.command
    }

    /// Returns the current process state.
    pub fn state(&self) -> ProcessState {
        self.state
    }

    /// Returns the accumulated stdout output.
    pub fn output(&self) -> &str {
        &self.output
    }

    /// Returns the accumulated stderr output.
    pub fn stderr_output(&self) -> &str {
        &self.stderr_output
    }

    /// Returns the process ID, if available.
    pub fn pid(&self) -> Option<u32> {
        self.pid
    }

    /// Returns the exit code, if the process has completed.
    pub fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }

    /// Returns `true` if the process is currently running.
    pub fn is_running(&self) -> bool {
        self.state == ProcessState::Running
    }

    /// Returns `true` if the process has completed (either successfully or with failure).
    pub fn is_finished(&self) -> bool {
        matches!(self.state, ProcessState::Completed | ProcessState::Failed)
    }

    /// Transition to the `Running` state and set the process ID.
    ///
    /// # Panics
    /// Panics if the current state is not `Pending`.
    pub fn start(&mut self, pid: u32) {
        assert_eq!(
            self.state,
            ProcessState::Pending,
            "Cannot start a process that is not in Pending state (current: {})",
            self.state
        );
        self.state = ProcessState::Running;
        self.pid = Some(pid);
    }

    /// Append a line of stdout output.
    ///
    /// Only appends if the process is in `Running` state.
    pub fn append_output(&mut self, line: &str) {
        if self.state == ProcessState::Running {
            if !self.output.is_empty() {
                self.output.push('\n');
            }
            self.output.push_str(line);
        }
    }

    /// Append a line of stderr output.
    ///
    /// Only appends if the process is in `Running` state.
    pub fn append_stderr(&mut self, line: &str) {
        if self.state == ProcessState::Running {
            if !self.stderr_output.is_empty() {
                self.stderr_output.push('\n');
            }
            self.stderr_output.push_str(line);
        }
    }

    /// Transition to the `Completed` state with the given exit code.
    ///
    /// # Panics
    /// Panics if the current state is not `Running`.
    pub fn complete(&mut self, exit_code: i32) {
        assert_eq!(
            self.state,
            ProcessState::Running,
            "Cannot complete a process that is not in Running state (current: {})",
            self.state
        );
        self.state = ProcessState::Completed;
        self.exit_code = Some(exit_code);
    }

    /// Transition to the `Failed` state with an optional exit code and signal.
    ///
    /// Can be called from either `Pending` or `Running` state.
    pub fn fail(&mut self, exit_code: Option<i32>, signal: Option<i32>) {
        assert!(
            matches!(self.state, ProcessState::Pending | ProcessState::Running),
            "Cannot fail a process that is already in {} state",
            self.state
        );
        self.state = ProcessState::Failed;
        self.exit_code = exit_code;
        self.signal = signal;
    }

    /// Build a [`CommandResult`] from the current process state.
    pub fn to_command_result(&self) -> CommandResult {
        CommandResult::new(self.exit_code, self.output.clone(), self.stderr_output.clone())
    }

    /// Build [`ShellExecutionDetails`] from the current process state.
    pub fn to_execution_details(&self) -> ShellExecutionDetails {
        ShellExecutionDetails::new(self.exit_code, self.signal)
    }
}

/// A thread-safe wrapper around `TerminalProcess` for concurrent access.
#[derive(Debug, Clone)]
pub struct SharedTerminalProcess {
    inner: Arc<Mutex<TerminalProcess>>,
}

impl SharedTerminalProcess {
    /// Create a new shared terminal process.
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(TerminalProcess::new(command))),
        }
    }

    /// Get the command being executed.
    pub async fn command(&self) -> String {
        let guard = self.inner.lock().await;
        guard.command.clone()
    }

    /// Get the current process state.
    pub async fn state(&self) -> ProcessState {
        let guard = self.inner.lock().await;
        guard.state
    }

    /// Get the accumulated stdout output.
    pub async fn output(&self) -> String {
        let guard = self.inner.lock().await;
        guard.output.clone()
    }

    /// Get the accumulated stderr output.
    pub async fn stderr_output(&self) -> String {
        let guard = self.inner.lock().await;
        guard.stderr_output.clone()
    }

    /// Get the process ID.
    pub async fn pid(&self) -> Option<u32> {
        let guard = self.inner.lock().await;
        guard.pid
    }

    /// Check if the process is running.
    pub async fn is_running(&self) -> bool {
        let guard = self.inner.lock().await;
        guard.is_running()
    }

    /// Check if the process has finished.
    pub async fn is_finished(&self) -> bool {
        let guard = self.inner.lock().await;
        guard.is_finished()
    }

    /// Start the process with the given PID.
    pub async fn start(&self, pid: u32) {
        let mut guard = self.inner.lock().await;
        guard.start(pid);
    }

    /// Append a line of stdout output.
    pub async fn append_output(&self, line: &str) {
        let mut guard = self.inner.lock().await;
        guard.append_output(line);
    }

    /// Append a line of stderr output.
    pub async fn append_stderr(&self, line: &str) {
        let mut guard = self.inner.lock().await;
        guard.append_stderr(line);
    }

    /// Complete the process with the given exit code.
    pub async fn complete(&self, exit_code: i32) {
        let mut guard = self.inner.lock().await;
        guard.complete(exit_code);
    }

    /// Fail the process.
    pub async fn fail(&self, exit_code: Option<i32>, signal: Option<i32>) {
        let mut guard = self.inner.lock().await;
        guard.fail(exit_code, signal);
    }

    /// Build a `CommandResult` from the current state.
    pub async fn to_command_result(&self) -> CommandResult {
        let guard = self.inner.lock().await;
        guard.to_command_result()
    }

    /// Build `ShellExecutionDetails` from the current state.
    pub async fn to_execution_details(&self) -> ShellExecutionDetails {
        let guard = self.inner.lock().await;
        guard.to_execution_details()
    }

    /// Get a cloned Arc to the inner Mutex (for advanced use cases).
    pub fn inner(&self) -> Arc<Mutex<TerminalProcess>> {
        Arc::clone(&self.inner)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_state_default() {
        assert_eq!(ProcessState::default(), ProcessState::Pending);
    }

    #[test]
    fn test_process_state_display() {
        assert_eq!(ProcessState::Pending.to_string(), "pending");
        assert_eq!(ProcessState::Running.to_string(), "running");
        assert_eq!(ProcessState::Completed.to_string(), "completed");
        assert_eq!(ProcessState::Failed.to_string(), "failed");
    }

    #[test]
    fn test_terminal_process_new() {
        let proc = TerminalProcess::new("echo hello");
        assert_eq!(proc.command(), "echo hello");
        assert_eq!(proc.state(), ProcessState::Pending);
        assert!(proc.output().is_empty());
        assert!(proc.stderr_output().is_empty());
        assert!(proc.pid().is_none());
        assert!(proc.exit_code().is_none());
        assert!(!proc.is_running());
        assert!(!proc.is_finished());
    }

    #[test]
    fn test_process_lifecycle_success() {
        let mut proc = TerminalProcess::new("ls");
        assert_eq!(proc.state(), ProcessState::Pending);

        proc.start(1234);
        assert_eq!(proc.state(), ProcessState::Running);
        assert_eq!(proc.pid(), Some(1234));
        assert!(proc.is_running());
        assert!(!proc.is_finished());

        proc.append_output("file1.txt");
        proc.append_output("file2.txt");
        assert_eq!(proc.output(), "file1.txt\nfile2.txt");

        proc.complete(0);
        assert_eq!(proc.state(), ProcessState::Completed);
        assert!(!proc.is_running());
        assert!(proc.is_finished());
        assert_eq!(proc.exit_code(), Some(0));
    }

    #[test]
    fn test_process_lifecycle_failure() {
        let mut proc = TerminalProcess::new("false");
        proc.start(5678);
        proc.append_stderr("error message");
        proc.complete(1);

        assert_eq!(proc.state(), ProcessState::Completed);
        assert_eq!(proc.exit_code(), Some(1));
        assert_eq!(proc.stderr_output(), "error message");
    }

    #[test]
    fn test_process_fail_from_pending() {
        let mut proc = TerminalProcess::new("nonexistent");
        proc.fail(None, None);
        assert_eq!(proc.state(), ProcessState::Failed);
        assert!(proc.is_finished());
    }

    #[test]
    fn test_process_fail_from_running() {
        let mut proc = TerminalProcess::new("sleep 10");
        proc.start(9999);
        proc.fail(None, Some(9)); // SIGKILL
        assert_eq!(proc.state(), ProcessState::Failed);
        assert_eq!(proc.signal, Some(9));
    }

    #[test]
    fn test_process_to_command_result() {
        let mut proc = TerminalProcess::new("echo test");
        proc.start(100);
        proc.append_output("test");
        proc.complete(0);

        let result = proc.to_command_result();
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.stdout, "test");
        assert!(result.is_success());
    }

    #[test]
    fn test_process_to_execution_details() {
        let mut proc = TerminalProcess::new("cmd");
        proc.start(200);
        proc.complete(0);

        let details = proc.to_execution_details();
        assert!(details.is_success());
        assert_eq!(details.exit_code, Some(0));
        assert!(details.signal.is_none());
    }

    #[test]
    fn test_process_append_output_only_when_running() {
        let mut proc = TerminalProcess::new("cmd");
        // Not running yet, should not append
        proc.append_output("should not appear");
        assert!(proc.output().is_empty());

        proc.start(300);
        proc.append_output("should appear");
        assert_eq!(proc.output(), "should appear");
    }

    #[test]
    fn test_process_append_stderr_only_when_running() {
        let mut proc = TerminalProcess::new("cmd");
        proc.append_stderr("should not appear");
        assert!(proc.stderr_output().is_empty());

        proc.start(400);
        proc.append_stderr("error");
        assert_eq!(proc.stderr_output(), "error");
    }

    #[tokio::test]
    async fn test_shared_terminal_process() {
        let shared = SharedTerminalProcess::new("echo hello");
        assert_eq!(shared.command().await, "echo hello");
        assert_eq!(shared.state().await, ProcessState::Pending);

        shared.start(42).await;
        assert_eq!(shared.state().await, ProcessState::Running);
        assert_eq!(shared.pid().await, Some(42));
        assert!(shared.is_running().await);

        shared.append_output("line1").await;
        shared.append_output("line2").await;
        assert_eq!(shared.output().await, "line1\nline2");

        shared.complete(0).await;
        assert!(shared.is_finished().await);

        let result = shared.to_command_result().await;
        assert!(result.is_success());
    }
}

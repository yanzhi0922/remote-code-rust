//! Core type definitions for the terminal integration layer.

use std::fmt;

/// Unique identifier for a terminal instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TerminalId(pub u32);

impl TerminalId {
    /// Create a new terminal ID.
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Get the raw u32 value.
    pub fn as_u32(self) -> u32 {
        self.0
    }
}

impl fmt::Display for TerminalId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TerminalId({})", self.0)
    }
}

impl From<u32> for TerminalId {
    fn from(val: u32) -> Self {
        Self(val)
    }
}

impl From<TerminalId> for u32 {
    fn from(id: TerminalId) -> Self {
        id.0
    }
}

/// The current state of a terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerminalState {
    /// Terminal is idle and ready to accept commands.
    Idle,
    /// Terminal is currently executing a command.
    Busy,
    /// Terminal has been closed and can no longer execute commands.
    Closed,
}

impl fmt::Display for TerminalState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TerminalState::Idle => write!(f, "idle"),
            TerminalState::Busy => write!(f, "busy"),
            TerminalState::Closed => write!(f, "closed"),
        }
    }
}

impl Default for TerminalState {
    fn default() -> Self {
        TerminalState::Idle
    }
}

/// The result of a completed command execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandResult {
    /// The exit code of the process. `None` if the process was terminated by a signal.
    pub exit_code: Option<i32>,
    /// The captured stdout output.
    pub stdout: String,
    /// The captured stderr output.
    pub stderr: String,
}

impl CommandResult {
    /// Create a new command result.
    pub fn new(exit_code: Option<i32>, stdout: String, stderr: String) -> Self {
        Self {
            exit_code,
            stdout,
            stderr,
        }
    }

    /// Create a successful command result (exit code 0).
    pub fn success(stdout: String, stderr: String) -> Self {
        Self {
            exit_code: Some(0),
            stdout,
            stderr,
        }
    }

    /// Create a failed command result with a non-zero exit code.
    pub fn failure(code: i32, stdout: String, stderr: String) -> Self {
        Self {
            exit_code: Some(code),
            stdout,
            stderr,
        }
    }

    /// Returns `true` if the command exited successfully (exit code 0).
    pub fn is_success(&self) -> bool {
        self.exit_code == Some(0)
    }

    /// Returns the full output combining stdout and stderr.
    pub fn full_output(&self) -> String {
        if self.stderr.is_empty() {
            self.stdout.clone()
        } else if self.stdout.is_empty() {
            self.stderr.clone()
        } else {
            format!("{}\n{}", self.stdout, self.stderr)
        }
    }
}

impl Default for CommandResult {
    fn default() -> Self {
        Self {
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
        }
    }
}

/// Details about a completed shell execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellExecutionDetails {
    /// The exit code of the shell process, if available.
    pub exit_code: Option<i32>,
    /// The signal that terminated the process, if any.
    pub signal: Option<i32>,
}

impl ShellExecutionDetails {
    /// Create new shell execution details.
    pub fn new(exit_code: Option<i32>, signal: Option<i32>) -> Self {
        Self { exit_code, signal }
    }

    /// Create execution details for a successful execution.
    pub fn success() -> Self {
        Self {
            exit_code: Some(0),
            signal: None,
        }
    }

    /// Create execution details from a signal termination.
    pub fn from_signal(signal: i32) -> Self {
        Self {
            exit_code: None,
            signal: Some(signal),
        }
    }

    /// Returns `true` if the execution completed successfully.
    pub fn is_success(&self) -> bool {
        self.exit_code == Some(0)
    }
}

/// Callbacks for terminal process events.
///
/// Implement this trait to receive notifications about command execution lifecycle.
pub trait TerminalCallbacks: Send + Sync {
    /// Called for each line of output produced by the running command.
    fn on_line(&self, line: &str);

    /// Called when the command has completed with the full output.
    fn on_completed(&self, result: &CommandResult);

    /// Called when the shell execution starts, providing the process ID.
    fn on_shell_execution_started(&self, pid: u32);

    /// Called when the shell execution completes with exit details.
    fn on_shell_execution_complete(&self, details: &ShellExecutionDetails);
}

/// A no-op implementation of [`TerminalCallbacks`] that discards all events.
pub struct NoopCallbacks;

impl TerminalCallbacks for NoopCallbacks {
    fn on_line(&self, _line: &str) {}
    fn on_completed(&self, _result: &CommandResult) {}
    fn on_shell_execution_started(&self, _pid: u32) {}
    fn on_shell_execution_complete(&self, _details: &ShellExecutionDetails) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_id_new() {
        let id = TerminalId::new(42);
        assert_eq!(id.as_u32(), 42);
    }

    #[test]
    fn test_terminal_id_display() {
        let id = TerminalId::new(7);
        assert_eq!(format!("{id}"), "TerminalId(7)");
    }

    #[test]
    fn test_terminal_id_from_u32() {
        let id: TerminalId = 99u32.into();
        assert_eq!(id, TerminalId(99));
    }

    #[test]
    fn test_terminal_id_into_u32() {
        let id = TerminalId(55);
        let val: u32 = id.into();
        assert_eq!(val, 55);
    }

    #[test]
    fn test_terminal_id_ordering() {
        let a = TerminalId(1);
        let b = TerminalId(2);
        assert!(a < b);
        assert_eq!(a, TerminalId::new(1));
    }

    #[test]
    fn test_terminal_state_default() {
        assert_eq!(TerminalState::default(), TerminalState::Idle);
    }

    #[test]
    fn test_terminal_state_display() {
        assert_eq!(TerminalState::Idle.to_string(), "idle");
        assert_eq!(TerminalState::Busy.to_string(), "busy");
        assert_eq!(TerminalState::Closed.to_string(), "closed");
    }

    #[test]
    fn test_command_result_success() {
        let result = CommandResult::success("hello".into(), String::new());
        assert!(result.is_success());
        assert_eq!(result.exit_code, Some(0));
        assert_eq!(result.stdout, "hello");
    }

    #[test]
    fn test_command_result_failure() {
        let result = CommandResult::failure(1, String::new(), "error".into());
        assert!(!result.is_success());
        assert_eq!(result.exit_code, Some(1));
        assert_eq!(result.stderr, "error");
    }

    #[test]
    fn test_command_result_full_output() {
        let result = CommandResult::success("out".into(), "err".into());
        assert_eq!(result.full_output(), "out\nerr");

        let result = CommandResult::success("out".into(), String::new());
        assert_eq!(result.full_output(), "out");

        let result = CommandResult::success(String::new(), "err".into());
        assert_eq!(result.full_output(), "err");
    }

    #[test]
    fn test_command_result_default() {
        let result = CommandResult::default();
        assert!(result.exit_code.is_none());
        assert!(result.stdout.is_empty());
        assert!(result.stderr.is_empty());
    }

    #[test]
    fn test_shell_execution_details_success() {
        let details = ShellExecutionDetails::success();
        assert!(details.is_success());
        assert_eq!(details.exit_code, Some(0));
        assert!(details.signal.is_none());
    }

    #[test]
    fn test_shell_execution_details_from_signal() {
        let details = ShellExecutionDetails::from_signal(9);
        assert!(!details.is_success());
        assert!(details.exit_code.is_none());
        assert_eq!(details.signal, Some(9));
    }

    #[test]
    fn test_shell_execution_details_new() {
        let details = ShellExecutionDetails::new(Some(0), None);
        assert!(details.is_success());
    }
}

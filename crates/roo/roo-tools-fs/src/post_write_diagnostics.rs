//! Post-write diagnostics for detecting errors after file modifications.
//!
//! In the TS version, VSCode's `vscode.languages.getDiagnostics()` is used to
//! check for new problems after file writes. For the CLI Rust port, we provide
//! a simplified approach:
//!
//! - **Rust files** (`.rs`): Runs `cargo check --message-format=short 2>&1`
//!   and captures error-level diagnostics.
//! - **Other files**: Returns a "not supported" result (skipped).
//!
//! Adapted from `.research/Roo-Code/src/integrations/diagnostics/index.ts`.

use std::path::Path;
use std::process::Command;

use crate::types::{DiagnosticsResult, MAX_DIAGNOSTIC_MESSAGES};

/// Run post-write diagnostics on a file.
///
/// For Rust files, this runs `cargo check` and captures error messages.
/// For other file types, returns a skipped result.
///
/// # Arguments
/// * `path` - The file path that was written/edited.
/// * `cwd` - The current working directory (used as the working dir for commands).
///
/// # Returns
/// A `DiagnosticsResult` containing any error messages found.
pub fn post_write_diagnostics(path: &Path, cwd: &Path) -> DiagnosticsResult {
    let path_str = match path.to_str() {
        Some(s) => s.to_string(),
        None => return DiagnosticsResult::skipped("<invalid-path>"),
    };

    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    match extension {
        "rs" => run_cargo_check_diagnostics(&path_str, cwd),
        _ => DiagnosticsResult::skipped(&path_str),
    }
}

/// Run `cargo check --message-format=short` for a Rust file and collect error messages.
///
/// Only collects error-level diagnostics (not warnings), matching the TS behavior
/// where only `DiagnosticSeverity.Error` is included.
fn run_cargo_check_diagnostics(path: &str, cwd: &Path) -> DiagnosticsResult {
    let output = match Command::new("cargo")
        .args(["check", "--message-format=short"])
        .current_dir(cwd)
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            // cargo not found or other execution error — don't fail the write
            return DiagnosticsResult::with_messages(
                path,
                vec![format!("[roo] Could not run cargo check: {e}")],
            );
        }
    };

    // `--message-format=short` outputs to stderr in the format:
    //   src/main.rs:10:5: error[E0425]: cannot find value `x` in this scope
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut messages = Vec::new();

    for line in stderr.lines() {
        if messages.len() >= MAX_DIAGNOSTIC_MESSAGES {
            let remaining = stderr.lines().count() - messages.len();
            messages.push(format!(
                "... {} more diagnostics omitted to prevent context overflow",
                remaining
            ));
            break;
        }

        // Only include error lines (contain "error" indicator)
        if is_error_line(line) {
            messages.push(format!("- {}", line.trim()));
        }
    }

    DiagnosticsResult::with_messages(path, messages)
}

/// Check if a `cargo check` output line represents an error.
fn is_error_line(line: &str) -> bool {
    let lower = line.to_lowercase();
    // Match patterns like:
    //   "error[E0425]: ..."
    //   "error: could not compile ..."
    // But NOT "warning: ..."
    if lower.starts_with("warning") {
        return false;
    }
    lower.contains("error")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_post_write_diagnostics_rust_file() {
        // For a Rust file, it should attempt cargo check (result depends on env)
        let dir = tempfile::tempdir().unwrap();
        let rs_file = dir.path().join("src/main.rs");
        let result = post_write_diagnostics(&rs_file, dir.path());
        assert!(result.ran);
        assert_eq!(result.path, rs_file.to_str().unwrap());
    }

    #[test]
    fn test_post_write_diagnostics_non_rust_file() {
        let dir = tempfile::tempdir().unwrap();
        let ts_file = dir.path().join("src/index.ts");
        let result = post_write_diagnostics(&ts_file, dir.path());
        assert!(!result.ran);
        assert!(!result.has_problems());
    }

    #[test]
    fn test_post_write_diagnostics_python_file() {
        let dir = tempfile::tempdir().unwrap();
        let py_file = dir.path().join("src/main.py");
        let result = post_write_diagnostics(&py_file, dir.path());
        assert!(!result.ran);
    }

    #[test]
    fn test_diagnostics_result_skipped() {
        let result = DiagnosticsResult::skipped("test.ts");
        assert!(!result.ran);
        assert!(!result.has_problems());
        assert!(result.to_problems_string().is_empty());
    }

    #[test]
    fn test_diagnostics_result_with_messages() {
        let result =
            DiagnosticsResult::with_messages("test.rs", vec!["error: something broke".to_string()]);
        assert!(result.ran);
        assert!(result.has_problems());
        assert!(
            result
                .to_problems_string()
                .contains("New problems detected")
        );
        assert!(result.to_problems_string().contains("something broke"));
    }

    #[test]
    fn test_diagnostics_result_no_messages() {
        let result = DiagnosticsResult::with_messages("test.rs", vec![]);
        assert!(result.ran);
        assert!(!result.has_problems());
        assert!(result.to_problems_string().is_empty());
    }

    #[test]
    fn test_is_error_line() {
        assert!(is_error_line(
            "src/main.rs:10:5: error[E0425]: cannot find value"
        ));
        assert!(is_error_line("error: could not compile `foo`"));
        assert!(!is_error_line("warning: unused variable: `x`"));
        assert!(!is_error_line("    Checking foo v0.1.0"));
        assert!(is_error_line("error: aborting due to 3 previous errors"));
    }

    #[test]
    fn test_max_diagnostic_messages_limit() {
        // Verify the constant is reasonable
        assert!(MAX_DIAGNOSTIC_MESSAGES > 0);
        assert!(MAX_DIAGNOSTIC_MESSAGES <= 100);
    }
}

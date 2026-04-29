//! Helper functions for command tools.

use crate::types::CommandToolError;
use regex::Regex;

/// Validate an artifact ID string.
///
/// Artifact IDs must:
/// - Not be empty
/// - Not contain path traversal (..)
/// - Not contain path separators (/ or \)
/// - Only contain alphanumeric chars, hyphens, underscores, dots
/// - Be within the max length
pub fn validate_artifact_id(id: &str) -> Result<(), CommandToolError> {
    if id.is_empty() {
        return Err(CommandToolError::InvalidArtifactId(
            "artifact ID must not be empty".to_string(),
        ));
    }

    if id.len() > crate::types::MAX_ARTIFACT_ID_LENGTH {
        return Err(CommandToolError::InvalidArtifactId(format!(
            "artifact ID too long: {} chars (max {})",
            id.len(),
            crate::types::MAX_ARTIFACT_ID_LENGTH
        )));
    }

    if id.contains("..") {
        return Err(CommandToolError::InvalidArtifactId(
            "artifact ID must not contain '..'".to_string(),
        ));
    }

    if id.contains('/') || id.contains('\\') {
        return Err(CommandToolError::InvalidArtifactId(
            "artifact ID must not contain path separators".to_string(),
        ));
    }

    // Only allow alphanumeric, hyphens, underscores, dots
    for ch in id.chars() {
        if !ch.is_alphanumeric() && ch != '-' && ch != '_' && ch != '.' {
            return Err(CommandToolError::InvalidArtifactId(format!(
                "artifact ID contains invalid character: '{ch}'"
            )));
        }
    }

    Ok(())
}

/// Resolve a timeout value in seconds.
///
/// Returns the timeout in seconds, using the default if None.
/// Returns an error for zero or negative values.
pub fn resolve_timeout(timeout: Option<u64>) -> Result<u64, CommandToolError> {
    match timeout {
        Some(0) => Err(CommandToolError::InvalidTimeout(
            "timeout must be > 0".to_string(),
        )),
        Some(secs) => Ok(secs),
        None => Ok(crate::types::DEFAULT_TIMEOUT_SECS),
    }
}

/// Format command output with optional truncation.
///
/// If the output exceeds max_bytes, it is truncated and a truncation
/// notice is appended.
pub fn format_command_output(output: &str, max_bytes: usize) -> (String, bool) {
    if output.len() <= max_bytes {
        return (output.to_string(), false);
    }

    let truncated = &output[..max_bytes];
    let notice = format!(
        "\n\n[OUTPUT TRUNCATED - Full output saved to artifact: cmd-XXXX.txt] ({} total bytes)",
        output.len()
    );
    (format!("{truncated}{notice}"), true)
}

/// Format bytes to human-readable string.
///
/// Matches TS `formatBytes()`:
/// - `< 1024` → `{}B`
/// - `< 1024*1024` → `{:.1}KB`
/// - `>= 1024*1024` → `{:.1}MB`
pub fn format_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{}B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Format persisted output result for tool response when output was truncated.
///
/// Matches TS `formatPersistedOutput()`:
/// ```text
/// Command executed in '{workingDir}'. {exitStatus}
///
/// Output ({sizeStr}) persisted. Artifact ID: {artifactId}
///
/// Preview:
/// {preview}
///
/// Use read_command_output tool to view full output if needed.
/// ```
pub fn format_persisted_output(
    working_dir: &str,
    exit_code: Option<i32>,
    total_bytes: usize,
    artifact_id: &str,
    preview: &str,
) -> String {
    let exit_status = format_exit_status(exit_code);
    let size_str = format_bytes(total_bytes);

    // Matches TS formatPersistedOutput exactly:
    // [
    //   `Command executed in '${workingDir}'. ${exitStatus}`,
    //   "",
    //   `Output (${sizeStr}) persisted. Artifact ID: ${artifactId}`,
    //   "",
    //   "Preview:",
    //   result.preview,
    //   "",
    //   "Use read_command_output tool to view full output if needed.",
    // ].join("\n")
    vec![
        format!("Command executed in '{}'. {}", working_dir, exit_status),
        String::new(),
        format!("Output ({}) persisted. Artifact ID: {}", size_str, artifact_id),
        String::new(),
        "Preview:".to_string(),
        preview.to_string(),
        String::new(),
        "Use read_command_output tool to view full output if needed.".to_string(),
    ]
    .join("\n")
}

/// Format exit status from exit code.
///
/// Matches TS `formatExitStatus()`:
/// - `None` → "Exit code: <undefined, notify user>"
/// - `Some(0)` → "Exit code: 0"
/// - `Some(non-zero)` → "Command execution was not successful, inspect the cause and adjust as needed.\nExit code: {code}"
pub fn format_exit_status(exit_code: Option<i32>) -> String {
    match exit_code {
        None => "Exit code: <undefined, notify user>".to_string(),
        Some(0) => "Exit code: 0".to_string(),
        Some(code) => format!(
            "Command execution was not successful, inspect the cause and adjust as needed.\nExit code: {}",
            code
        ),
    }
}

/// Filter output lines by a search pattern (regex or literal).
///
/// Returns the matching lines and the count of matches.
pub fn filter_output_by_search(
    output: &str,
    pattern: &str,
    is_regex: bool,
) -> Result<(String, usize), CommandToolError> {
    if pattern.is_empty() {
        return Ok((output.to_string(), output.lines().count()));
    }

    let re = if is_regex {
        Regex::new(pattern)
            .map_err(|e| CommandToolError::InvalidRegex(format!("Invalid regex: {e}")))?
    } else {
        // Escape literal pattern for regex
        let escaped = regex::escape(pattern);
        Regex::new(&escaped)
            .map_err(|e| CommandToolError::InvalidRegex(format!("Invalid regex: {e}")))?
    };

    let mut matched = Vec::new();
    for line in output.lines() {
        if re.is_match(line) {
            matched.push(line);
        }
    }

    let count = matched.len();
    let result = matched.join("\n");

    Ok((result, count))
}

/// Paginate output content by byte offset and limit.
///
/// Returns the sliced content and whether there's more data.
pub fn paginate_output(content: &str, offset: u64, limit: u64) -> (String, bool) {
    let bytes = content.as_bytes();
    let total = bytes.len();

    if offset as usize >= total {
        return (String::new(), false);
    }

    let start = offset as usize;
    let end = std::cmp::min(start + limit as usize, total);
    let has_more = end < total;

    // Try to slice at a valid UTF-8 boundary
    let sliced = String::from_utf8_lossy(&bytes[start..end]).into_owned();

    (sliced, has_more)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- validate_artifact_id tests ----

    #[test]
    fn test_valid_artifact_id() {
        assert!(validate_artifact_id("cmd-12345.txt").is_ok());
    }

    #[test]
    fn test_valid_artifact_id_simple() {
        assert!(validate_artifact_id("abc123").is_ok());
    }

    #[test]
    fn test_invalid_artifact_id_empty() {
        assert!(validate_artifact_id("").is_err());
    }

    #[test]
    fn test_invalid_artifact_id_traversal() {
        assert!(validate_artifact_id("../etc/passwd").is_err());
    }

    #[test]
    fn test_invalid_artifact_id_slash() {
        assert!(validate_artifact_id("foo/bar").is_err());
    }

    #[test]
    fn test_invalid_artifact_id_backslash() {
        assert!(validate_artifact_id("foo\\bar").is_err());
    }

    #[test]
    fn test_invalid_artifact_id_special_chars() {
        assert!(validate_artifact_id("foo@bar!baz").is_err());
    }

    #[test]
    fn test_invalid_artifact_id_double_dot() {
        assert!(validate_artifact_id("foo..bar").is_err());
    }

    #[test]
    fn test_valid_artifact_id_with_dots() {
        assert!(validate_artifact_id("cmd.123.txt").is_ok());
    }

    // ---- resolve_timeout tests ----

    #[test]
    fn test_resolve_timeout_none() {
        assert_eq!(resolve_timeout(None).unwrap(), crate::types::DEFAULT_TIMEOUT_SECS);
    }

    #[test]
    fn test_resolve_timeout_some() {
        assert_eq!(resolve_timeout(Some(30)).unwrap(), 30);
    }

    #[test]
    fn test_resolve_timeout_zero() {
        assert!(resolve_timeout(Some(0)).is_err());
    }

    // ---- format_command_output tests ----

    #[test]
    fn test_format_output_short() {
        let output = "hello world";
        let (result, truncated) = format_command_output(output, 100);
        assert_eq!(result, output);
        assert!(!truncated);
    }

    #[test]
    fn test_format_output_long() {
        let output = "a".repeat(200);
        let (result, truncated) = format_command_output(&output, 100);
        assert!(truncated);
        assert!(result.contains("[OUTPUT TRUNCATED"));
    }

    #[test]
    fn test_format_output_exact() {
        let output = "a".repeat(100);
        let (result, truncated) = format_command_output(&output, 100);
        assert_eq!(result, output);
        assert!(!truncated);
    }

    // ---- filter_output_by_search tests ----

    #[test]
    fn test_filter_regex() {
        let output = "line1: hello\nline2: world\nline3: hello again";
        let (result, count) = filter_output_by_search(output, "hello", true).unwrap();
        assert_eq!(count, 2);
        assert!(result.contains("line1: hello"));
        assert!(result.contains("line3: hello again"));
    }

    #[test]
    fn test_filter_literal() {
        let output = "error: foo\nwarning: bar\nerror: baz";
        let (_result, count) = filter_output_by_search(output, "error:", false).unwrap();
        assert_eq!(count, 2);
    }

    #[test]
    fn test_filter_empty_pattern() {
        let output = "line1\nline2";
        let (result, count) = filter_output_by_search(output, "", true).unwrap();
        assert_eq!(count, 2);
        assert_eq!(result, output);
    }

    #[test]
    fn test_filter_invalid_regex() {
        let result = filter_output_by_search("output", "[invalid", true);
        assert!(result.is_err());
    }

    #[test]
    fn test_filter_no_matches() {
        let output = "line1\nline2\nline3";
        let (result, count) = filter_output_by_search(output, "nonexistent", true).unwrap();
        assert_eq!(count, 0);
        assert!(result.is_empty());
    }

    // ---- paginate_output tests ----

    #[test]
    fn test_paginate_full() {
        let content = "hello world";
        let (result, has_more) = paginate_output(content, 0, 100);
        assert_eq!(result, content);
        assert!(!has_more);
    }

    #[test]
    fn test_paginate_with_offset() {
        let content = "hello world";
        let (result, has_more) = paginate_output(content, 6, 100);
        assert_eq!(result, "world");
        assert!(!has_more);
    }

    #[test]
    fn test_paginate_with_limit() {
        let content = "hello world";
        let (result, has_more) = paginate_output(content, 0, 5);
        assert_eq!(result, "hello");
        assert!(has_more);
    }

    #[test]
    fn test_paginate_offset_out_of_range() {
        let content = "hello";
        let (result, has_more) = paginate_output(content, 100, 10);
        assert!(result.is_empty());
        assert!(!has_more);
    }

    #[test]
    fn test_paginate_empty() {
        let (result, has_more) = paginate_output("", 0, 10);
        assert!(result.is_empty());
        assert!(!has_more);
    }

    // ---- CommandResult tests ----

    #[test]
    fn test_command_result_serde() {
        let result = crate::types::CommandResult {
            command: "echo hello".to_string(),
            stdout: "hello\n".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            truncated: false,
            artifact_id: Some("cmd-123.txt".to_string()),
        };
        let json = serde_json::to_string(&result).unwrap();
        let parsed: crate::types::CommandResult = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.command, "echo hello");
        assert_eq!(parsed.exit_code, Some(0));
    }

    // ---- PersistedOutput tests ----

    #[test]
    fn test_persisted_output_serde() {
        let output = crate::types::PersistedOutput {
            artifact_id: "cmd-123.txt".to_string(),
            stdout: "output".to_string(),
            stderr: String::new(),
            finished: true,
            exit_code: Some(0),
        };
        let json = serde_json::to_string(&output).unwrap();
        let parsed: crate::types::PersistedOutput = serde_json::from_str(&json).unwrap();
        assert!(parsed.finished);
    }

    // ---- ExecutionStatus tests ----

    #[test]
    fn test_execution_status_serde() {
        let status = crate::types::ExecutionStatus::Completed;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"completed\"");

        let status = crate::types::ExecutionStatus::Timeout;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"timeout\"");
    }

    // ---- CommandToolError tests ----

    #[test]
    fn test_command_tool_error_display() {
        let err = CommandToolError::InvalidCommand("rm -rf /".to_string());
        assert_eq!(format!("{err}"), "Invalid command: rm -rf /");

        let err = CommandToolError::InvalidArtifactId("../etc".to_string());
        assert!(format!("{err}").contains("../etc"));
    }

    // ---- format_bytes tests ----

    #[test]
    fn test_format_bytes_under_1k() {
        assert_eq!(format_bytes(512), "512B");
    }

    #[test]
    fn test_format_bytes_exactly_1k() {
        assert_eq!(format_bytes(1024), "1.0KB");
    }

    #[test]
    fn test_format_bytes_kilobytes() {
        assert_eq!(format_bytes(1536), "1.5KB");
    }

    #[test]
    fn test_format_bytes_megabytes() {
        let result = format_bytes(2 * 1024 * 1024);
        assert_eq!(result, "2.0MB");
    }

    #[test]
    fn test_format_bytes_zero() {
        assert_eq!(format_bytes(0), "0B");
    }

    // ---- format_exit_status tests ----

    #[test]
    fn test_format_exit_status_none() {
        let result = format_exit_status(None);
        assert!(result.contains("undefined"));
    }

    #[test]
    fn test_format_exit_status_zero() {
        let result = format_exit_status(Some(0));
        assert_eq!(result, "Exit code: 0");
    }

    #[test]
    fn test_format_exit_status_nonzero() {
        let result = format_exit_status(Some(1));
        assert!(result.contains("not successful"));
        assert!(result.contains("Exit code: 1"));
    }

    // ---- format_persisted_output tests ----

    #[test]
    fn test_format_persisted_output_basic() {
        let result = format_persisted_output(
            "/home/user/project",
            Some(0),
            2048,
            "cmd-abc123.txt",
            "first line\nsecond line",
        );
        assert!(result.contains("/home/user/project"));
        assert!(result.contains("Exit code: 0"));
        assert!(result.contains("2.0KB"));
        assert!(result.contains("cmd-abc123.txt"));
        assert!(result.contains("first line"));
        assert!(result.contains("read_command_output"));
    }
}

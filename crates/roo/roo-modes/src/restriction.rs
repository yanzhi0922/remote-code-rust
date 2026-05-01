//! File restriction checking for modes.
//!
//! Source: `src/shared/modes.ts` — `FileRestrictionError`

use regex::Regex;

// ---------------------------------------------------------------------------
// FileRestrictionError
// ---------------------------------------------------------------------------

/// Error produced when a file operation is blocked by mode restrictions.
///
/// The error message format exactly matches the TypeScript source:
/// - With tool: `Tool '{tool}' in mode '{mode}' can only edit files matching
///   pattern: {pattern} ({description}). Got: {file_path}`
/// - Without tool: `This mode ({mode}) can only edit files matching pattern:
///   {pattern} ({description}). Got: {file_path}`
///
/// Source: `src/shared/modes.ts` — `FileRestrictionError`
#[derive(Debug, thiserror::Error)]
pub struct FileRestrictionError {
    /// The mode slug.
    pub mode: String,
    /// The regex pattern that the file must match.
    pub pattern: String,
    /// Human-readable description of the restriction.
    pub description: Option<String>,
    /// The file path that was rejected.
    pub file_path: String,
    /// The tool that attempted the operation (optional).
    pub tool: Option<String>,
}

impl std::fmt::Display for FileRestrictionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tool_info = match &self.tool {
            Some(tool) => format!("Tool '{}' in mode '{}'", tool, self.mode),
            None => format!("This mode ({})", self.mode),
        };
        let desc_suffix = match &self.description {
            Some(d) => format!(" ({})", d),
            None => String::new(),
        };
        write!(
            f,
            "{} can only edit files matching pattern: {}{}. Got: {}",
            tool_info, self.pattern, desc_suffix, self.file_path
        )
    }
}

// ---------------------------------------------------------------------------
// check_file_restriction
// ---------------------------------------------------------------------------

/// Check whether a file path matches the given regex pattern.
///
/// If the file path does **not** match, returns `Err(FileRestrictionError)`
/// with a message matching the TypeScript source format.
/// If it matches, returns `Ok(())`.
///
/// # Arguments
/// * `mode` — The mode slug (used in the error message).
/// * `pattern` — A regex pattern that the file path must match.
/// * `description` — Optional human-readable description of the restriction.
/// * `file_path` — The file path to check.
/// * `tool` — Optional tool name that attempted the operation.
pub fn check_file_restriction(
    mode: &str,
    pattern: &str,
    description: Option<&str>,
    file_path: &str,
    tool: Option<&str>,
) -> Result<(), FileRestrictionError> {
    let re = Regex::new(pattern).unwrap_or_else(|_| {
        // If the pattern is invalid, create a regex that never matches
        Regex::new("$^").unwrap()
    });

    if re.is_match(file_path) {
        Ok(())
    } else {
        Err(FileRestrictionError {
            mode: mode.to_string(),
            pattern: pattern.to_string(),
            description: description.map(|s| s.to_string()),
            file_path: file_path.to_string(),
            tool: tool.map(|s| s.to_string()),
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_file_restriction_allows_matching() {
        let result = check_file_restriction(
            "architect",
            r"\.md$",
            Some("Markdown files only"),
            "plan.md",
            None,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_file_restriction_blocks_non_matching() {
        let result = check_file_restriction(
            "architect",
            r"\.md$",
            Some("Markdown files only"),
            "src/main.rs",
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_file_restriction_error_message_without_tool() {
        let err = check_file_restriction(
            "architect",
            r"\.md$",
            Some("Markdown files only"),
            "src/main.rs",
            None,
        )
        .unwrap_err();

        let msg = err.to_string();
        assert!(msg.starts_with("This mode (architect)"));
        assert!(msg.contains("can only edit files matching pattern: \\.md$"));
        assert!(msg.contains("(Markdown files only)"));
        assert!(msg.contains("Got: src/main.rs"));
    }

    #[test]
    fn test_file_restriction_error_message_with_tool() {
        let err = check_file_restriction(
            "architect",
            r"\.md$",
            Some("Markdown files only"),
            "src/main.rs",
            Some("write_to_file"),
        )
        .unwrap_err();

        let msg = err.to_string();
        assert!(msg.starts_with("Tool 'write_to_file' in mode 'architect'"));
        assert!(msg.contains("Got: src/main.rs"));
    }

    #[test]
    fn test_file_restriction_error_message_without_description() {
        let err = check_file_restriction(
            "architect",
            r"\.md$",
            None,
            "src/main.rs",
            None,
        )
        .unwrap_err();

        let msg = err.to_string();
        assert!(!msg.contains("()"));
        assert!(msg.contains("can only edit files matching pattern: \\.md$. Got:"));
    }

    #[test]
    fn test_file_restriction_error_fields() {
        let err = check_file_restriction(
            "code",
            r"\.rs$",
            Some("Rust files"),
            "test.py",
            Some("apply_diff"),
        )
        .unwrap_err();

        assert_eq!(err.mode, "code");
        assert_eq!(err.pattern, r"\.rs$");
        assert_eq!(err.description, Some("Rust files".to_string()));
        assert_eq!(err.file_path, "test.py");
        assert_eq!(err.tool, Some("apply_diff".to_string()));
    }
}

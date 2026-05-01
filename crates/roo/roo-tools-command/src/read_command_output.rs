//! read_command_output tool implementation.
//!
//! Provides validation, in-memory processing, and disk-based artifact retrieval.

use crate::helpers::*;
use crate::types::*;
use roo_types::tool::ReadCommandOutputParams;
use std::path::Path;

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Validate read_command_output parameters.
pub fn validate_read_command_output_params(
    params: &ReadCommandOutputParams,
) -> Result<(), CommandToolError> {
    validate_artifact_id(&params.artifact_id)?;

    if let Some(offset) = params.offset {
        if offset > i64::MAX as u64 {
            return Err(CommandToolError::Validation(
                "offset too large".to_string(),
            ));
        }
    }

    if let Some(limit) = params.limit {
        if limit == 0 {
            return Err(CommandToolError::Validation(
                "limit must be > 0".to_string(),
            ));
        }
    }

    // Validate search regex if provided
    if let Some(ref search) = params.search {
        if !search.is_empty() {
            regex::Regex::new(search)
                .map_err(|e| CommandToolError::InvalidRegex(format!("Invalid search pattern: {e}")))?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// In-memory processing
// ---------------------------------------------------------------------------

/// Process a read_command_output request.
///
/// Given a persisted output, applies pagination and search filtering.
pub fn process_read_output(
    params: &ReadCommandOutputParams,
    persisted: &PersistedOutput,
) -> Result<ReadOutputResult, CommandToolError> {
    validate_read_command_output_params(params)?;

    let full_output = format!("{}{}", persisted.stdout, persisted.stderr);
    let total_bytes = full_output.len();

    // Apply search filter if provided
    let (content, matched_lines) = if let Some(ref search) = params.search {
        if !search.is_empty() {
            let (filtered, count) = filter_output_by_search(&full_output, search, true)?;
            (filtered, Some(count))
        } else {
            (full_output.clone(), None)
        }
    } else {
        (full_output.clone(), None)
    };

    // Apply pagination
    let offset = params.offset.unwrap_or(0);
    let limit = params.limit.unwrap_or(DEFAULT_PAGE_SIZE as u64);

    let (paginated, has_more) = paginate_output(&content, offset, limit);

    Ok(ReadOutputResult {
        artifact_id: params.artifact_id.clone(),
        content: paginated,
        total_bytes,
        has_more,
        matched_lines,
    })
}

// ---------------------------------------------------------------------------
// Disk-based artifact retrieval
// ---------------------------------------------------------------------------

/// Read a command output artifact from disk.
///
/// Looks for the file at `<storage_dir>/command-output/<artifact_id>`.
/// Returns `Ok(Some(content))` if found, `Ok(None)` if not found.
pub async fn read_artifact_from_disk(
    storage_dir: &Path,
    artifact_id: &str,
) -> std::io::Result<Option<String>> {
    // Validate artifact_id to prevent path traversal
    if validate_artifact_id(artifact_id).is_err() {
        return Ok(None);
    }

    let file_path = storage_dir.join("command-output").join(artifact_id);
    if file_path.exists() {
        let content = tokio::fs::read_to_string(&file_path).await?;
        Ok(Some(content))
    } else {
        Ok(None)
    }
}

/// Read, filter, and paginate an artifact from disk.
///
/// Combines [`read_artifact_from_disk`] with search/pagination logic.
pub async fn read_command_output_from_disk(
    params: &ReadCommandOutputParams,
    storage_dir: &Path,
) -> Result<ReadOutputResult, CommandToolError> {
    validate_read_command_output_params(params)?;

    let content = read_artifact_from_disk(storage_dir, &params.artifact_id)
        .await
        .map_err(|e| CommandToolError::Execution(format!("Failed to read artifact: {}", e)))?
        .ok_or_else(|| CommandToolError::ArtifactNotFound(params.artifact_id.clone()))?;

    let total_bytes = content.len();

    // Apply search filter if provided
    let (filtered, matched_lines) = if let Some(ref search) = params.search {
        if !search.is_empty() {
            let (filtered, count) = filter_output_by_search(&content, search, true)?;
            (filtered, Some(count))
        } else {
            (content.clone(), None)
        }
    } else {
        (content.clone(), None)
    };

    // Apply pagination
    let offset = params.offset.unwrap_or(0);
    let limit = params.limit.unwrap_or(DEFAULT_PAGE_SIZE as u64);

    let (paginated, has_more) = paginate_output(&filtered, offset, limit);

    Ok(ReadOutputResult {
        artifact_id: params.artifact_id.clone(),
        content: paginated,
        total_bytes,
        has_more,
        matched_lines,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_persisted(stdout: &str, stderr: &str) -> PersistedOutput {
        PersistedOutput {
            artifact_id: "cmd-test.txt".to_string(),
            stdout: stdout.to_string(),
            stderr: stderr.to_string(),
            finished: true,
            exit_code: Some(0),
        }
    }

    #[test]
    fn test_validate_empty_artifact_id() {
        let params = ReadCommandOutputParams {
            artifact_id: "".to_string(),
            offset: None,
            limit: None,
            search: None,
        };
        assert!(validate_read_command_output_params(&params).is_err());
    }

    #[test]
    fn test_validate_invalid_artifact_id() {
        let params = ReadCommandOutputParams {
            artifact_id: "../etc/passwd".to_string(),
            offset: None,
            limit: None,
            search: None,
        };
        assert!(validate_read_command_output_params(&params).is_err());
    }

    #[test]
    fn test_validate_zero_limit() {
        let params = ReadCommandOutputParams {
            artifact_id: "cmd-test.txt".to_string(),
            offset: None,
            limit: Some(0),
            search: None,
        };
        assert!(validate_read_command_output_params(&params).is_err());
    }

    #[test]
    fn test_validate_invalid_search_regex() {
        let params = ReadCommandOutputParams {
            artifact_id: "cmd-test.txt".to_string(),
            offset: None,
            limit: None,
            search: Some("[invalid".to_string()),
        };
        assert!(validate_read_command_output_params(&params).is_err());
    }

    #[test]
    fn test_validate_valid_params() {
        let params = ReadCommandOutputParams {
            artifact_id: "cmd-test.txt".to_string(),
            offset: Some(0),
            limit: Some(100),
            search: Some("error".to_string()),
        };
        assert!(validate_read_command_output_params(&params).is_ok());
    }

    #[test]
    fn test_process_read_output_full() {
        let params = ReadCommandOutputParams {
            artifact_id: "cmd-test.txt".to_string(),
            offset: None,
            limit: None,
            search: None,
        };
        let persisted = make_persisted("hello\nworld\n", "");
        let result = process_read_output(&params, &persisted).unwrap();
        assert_eq!(result.content, "hello\nworld\n");
        assert!(!result.has_more);
        assert!(result.matched_lines.is_none());
    }

    #[test]
    fn test_process_read_output_with_search() {
        let params = ReadCommandOutputParams {
            artifact_id: "cmd-test.txt".to_string(),
            offset: None,
            limit: None,
            search: Some("error".to_string()),
        };
        let persisted = make_persisted("error: foo\nwarning: bar\nerror: baz\n", "");
        let result = process_read_output(&params, &persisted).unwrap();
        assert_eq!(result.matched_lines, Some(2));
        assert!(result.content.contains("error: foo"));
        assert!(!result.content.contains("warning"));
    }

    #[test]
    fn test_process_read_output_with_pagination() {
        let params = ReadCommandOutputParams {
            artifact_id: "cmd-test.txt".to_string(),
            offset: Some(0),
            limit: Some(5),
            search: None,
        };
        let persisted = make_persisted("hello world", "");
        let result = process_read_output(&params, &persisted).unwrap();
        assert_eq!(result.content, "hello");
        assert!(result.has_more);
    }

    #[test]
    fn test_process_read_output_combined_stdout_stderr() {
        let params = ReadCommandOutputParams {
            artifact_id: "cmd-test.txt".to_string(),
            offset: None,
            limit: None,
            search: None,
        };
        let persisted = make_persisted("stdout\n", "stderr\n");
        let result = process_read_output(&params, &persisted).unwrap();
        assert!(result.content.contains("stdout"));
        assert!(result.content.contains("stderr"));
    }

    #[tokio::test]
    async fn test_read_artifact_from_disk_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let result = read_artifact_from_disk(dir.path(), "nonexistent.txt")
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_read_artifact_from_disk_found() {
        let dir = tempfile::tempdir().unwrap();
        let cmd_dir = dir.path().join("command-output");
        tokio::fs::create_dir_all(&cmd_dir).await.unwrap();
        tokio::fs::write(cmd_dir.join("cmd-test.txt"), "hello world")
            .await
            .unwrap();

        let result = read_artifact_from_disk(dir.path(), "cmd-test.txt")
            .await
            .unwrap();
        assert_eq!(result.as_deref(), Some("hello world"));
    }

    #[tokio::test]
    async fn test_read_artifact_from_disk_traversal_blocked() {
        let dir = tempfile::tempdir().unwrap();
        let result = read_artifact_from_disk(dir.path(), "../etc/passwd")
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_read_command_output_from_disk_with_search() {
        let dir = tempfile::tempdir().unwrap();
        let cmd_dir = dir.path().join("command-output");
        tokio::fs::create_dir_all(&cmd_dir).await.unwrap();
        tokio::fs::write(
            cmd_dir.join("cmd-test.txt"),
            "error: foo\nwarning: bar\nerror: baz\n",
        )
        .await
        .unwrap();

        let params = ReadCommandOutputParams {
            artifact_id: "cmd-test.txt".to_string(),
            offset: None,
            limit: None,
            search: Some("error".to_string()),
        };

        let result = read_command_output_from_disk(&params, dir.path())
            .await
            .unwrap();
        assert_eq!(result.matched_lines, Some(2));
        assert!(result.content.contains("error: foo"));
        assert!(!result.content.contains("warning"));
    }

    #[tokio::test]
    async fn test_read_command_output_from_disk_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let params = ReadCommandOutputParams {
            artifact_id: "cmd-missing.txt".to_string(),
            offset: None,
            limit: None,
            search: None,
        };

        let result = read_command_output_from_disk(&params, dir.path()).await;
        assert!(result.is_err());
    }
}

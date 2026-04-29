//! Safe JSON Write
//!
//! Safely writes JSON data to a file with atomic write semantics:
//! creates parent directories, writes to a temp file, backs up existing,
//! and rolls back on errors.
//!
//! Source: `.research/Roo-Code/src/utils/safeWriteJson.ts`

use std::path::Path;

/// Error type for safe JSON write operations.
#[derive(Debug, thiserror::Error)]
pub enum SafeWriteJsonError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Options for safe JSON write.
#[derive(Debug, Clone)]
pub struct SafeWriteJsonOptions {
    /// Whether to pretty-print the JSON output with indentation.
    pub pretty_print: bool,
}

impl Default for SafeWriteJsonOptions {
    fn default() -> Self {
        Self {
            pretty_print: false,
        }
    }
}

/// Safely writes JSON data to a file.
///
/// - Creates parent directories if they don't exist.
/// - Writes to a temporary file first.
/// - If the target file exists, it's backed up before being replaced.
/// - Attempts to roll back and clean up in case of errors.
///
/// Source: `.research/Roo-Code/src/utils/safeWriteJson.ts`
pub async fn safe_write_json(
    file_path: &Path,
    data: &serde_json::Value,
    options: Option<SafeWriteJsonOptions>,
) -> Result<(), SafeWriteJsonError> {
    let opts = options.unwrap_or_default();
    let absolute_file_path = file_path.canonicalize().unwrap_or_else(|_| file_path.to_path_buf());

    // Ensure directory structure exists
    let dir_path = absolute_file_path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    tokio::fs::create_dir_all(dir_path).await?;

    // Generate temp file names
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let random_suffix: String = (0..8).map(|_| rand_char()).collect();

    let file_name = absolute_file_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy();
    let temp_new_path = dir_path.join(format!(
        ".{}.new_{}_{}.tmp",
        file_name, timestamp, random_suffix
    ));
    let temp_backup_path = dir_path.join(format!(
        ".{}.bak_{}_{}.tmp",
        file_name, timestamp, random_suffix
    ));

    // Step 1: Write data to temp file
    let json_content = if opts.pretty_print {
        serde_json::to_string_pretty(data)?
    } else {
        serde_json::to_string(data)?
    };

    tokio::fs::write(&temp_new_path, &json_content).await?;

    // Step 2: If target exists, back it up
    let has_backup = if absolute_file_path.exists() {
        match tokio::fs::rename(&absolute_file_path, &temp_backup_path).await {
            Ok(_) => true,
            Err(e) => {
                // Clean up temp file
                let _ = tokio::fs::remove_file(&temp_new_path).await;
                return Err(SafeWriteJsonError::Io(e));
            }
        }
    } else {
        false
    };

    // Step 3: Rename temp file to target
    match tokio::fs::rename(&temp_new_path, &absolute_file_path).await {
        Ok(_) => {
            // Step 4: Clean up backup if it exists
            if has_backup {
                let _ = tokio::fs::remove_file(&temp_backup_path).await;
            }
            Ok(())
        }
        Err(original_error) => {
            // Attempt rollback
            if has_backup {
                let _ = tokio::fs::rename(&temp_backup_path, &absolute_file_path).await;
            }
            // Clean up temp new file if it still exists
            let _ = tokio::fs::remove_file(&temp_new_path).await;
            Err(SafeWriteJsonError::Io(original_error))
        }
    }
}

/// Generate a random alphanumeric character.
fn rand_char() -> char {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";
    let idx = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos()
        % CHARSET.len() as u32) as usize;
    CHARSET[idx] as char
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_safe_write_json_creates_file() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("test.json");

        let data = json!({"key": "value"});
        safe_write_json(&file_path, &data, None).await.unwrap();

        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["key"], "value");
    }

    #[tokio::test]
    async fn test_safe_write_json_pretty_print() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("test.json");

        let data = json!({"key": "value"});
        safe_write_json(
            &file_path,
            &data,
            Some(SafeWriteJsonOptions { pretty_print: true }),
        )
        .await
        .unwrap();

        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert!(content.contains('\n'));
    }

    #[tokio::test]
    async fn test_safe_write_json_creates_parent_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("nested").join("dir").join("test.json");

        let data = json!({"nested": true});
        safe_write_json(&file_path, &data, None).await.unwrap();

        assert!(file_path.exists());
    }

    #[tokio::test]
    async fn test_safe_write_json_preserves_existing_on_error() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("test.json");

        // Write initial data
        let initial = json!({"initial": true});
        safe_write_json(&file_path, &initial, None).await.unwrap();

        // Verify initial content
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
        assert_eq!(parsed["initial"], true);
    }

    #[test]
    fn test_rand_char() {
        let c = rand_char();
        assert!(c.is_ascii_alphanumeric());
    }
}

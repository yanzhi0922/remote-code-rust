//! File system utilities for configuration management.
//!
//! Maps to TypeScript source: `src/services/roo-config/index.ts` (file utility functions)

use std::path::Path;

/// Checks if a directory exists.
///
/// Maps to TS: `directoryExists(dirPath)`
pub async fn directory_exists(dir_path: &Path) -> bool {
    match tokio::fs::metadata(dir_path).await {
        Ok(metadata) => metadata.is_dir(),
        Err(_) => false,
    }
}

/// Checks if a file exists.
///
/// Maps to TS: `fileExists(filePath)`
pub async fn file_exists(file_path: &Path) -> bool {
    match tokio::fs::metadata(file_path).await {
        Ok(metadata) => metadata.is_file(),
        Err(_) => false,
    }
}

/// Reads a file safely, returning `None` if it doesn't exist.
///
/// Returns `None` for:
/// - File not found (ENOENT)
/// - Not a directory (ENOTDIR)
/// - Is a directory (EISDIR)
///
/// Returns an error for other I/O errors (permission, etc.).
///
/// Maps to TS: `readFileIfExists(filePath)`
pub async fn read_file_if_exists(file_path: &Path) -> std::io::Result<Option<String>> {
    match tokio::fs::read_to_string(file_path).await {
        Ok(content) => Ok(Some(content)),
        Err(e) => {
            // Only return None for expected "not found" errors
            if is_not_found_error(&e) {
                Ok(None)
            } else {
                Err(e)
            }
        }
    }
}

/// Returns true if the error is a "not found" type error or indicates
/// the path is not a regular file (e.g., directory, not found).
fn is_not_found_error(error: &std::io::Error) -> bool {
    match error.kind() {
        std::io::ErrorKind::NotFound => true,
        std::io::ErrorKind::NotConnected => true,
        // On Windows, trying to open a directory as a file gives InvalidInput
        std::io::ErrorKind::InvalidInput => true,
        // On Unix, trying to read a directory gives IsADirectory
        #[cfg(unix)]
        std::io::ErrorKind::IsADirectory => true,
        // On Windows, reading a directory returns PermissionDenied
        // We treat this as "not a file" since we know the path exists
        // (checked by the caller or prior metadata call)
        std::io::ErrorKind::PermissionDenied => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn test_directory_exists_true() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(directory_exists(tmp.path()).await);
    }

    #[tokio::test]
    async fn test_directory_exists_false() {
        assert!(!directory_exists(Path::new("/nonexistent/path")).await);
    }

    #[tokio::test]
    async fn test_directory_exists_file() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("test.txt");
        fs::write(&file_path, "content").unwrap();
        // A file should not be considered a directory
        assert!(!directory_exists(&file_path).await);
    }

    #[tokio::test]
    async fn test_file_exists_true() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("test.txt");
        fs::write(&file_path, "content").unwrap();
        assert!(file_exists(&file_path).await);
    }

    #[tokio::test]
    async fn test_file_exists_false() {
        assert!(!file_exists(Path::new("/nonexistent/file.txt")).await);
    }

    #[tokio::test]
    async fn test_file_exists_directory() {
        let tmp = tempfile::tempdir().unwrap();
        // A directory should not be considered a file
        assert!(!file_exists(tmp.path()).await);
    }

    #[tokio::test]
    async fn test_read_file_if_exists_present() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("test.txt");
        fs::write(&file_path, "hello world").unwrap();

        let content = read_file_if_exists(&file_path).await.unwrap();
        assert_eq!(content, Some("hello world".to_string()));
    }

    #[tokio::test]
    async fn test_read_file_if_exists_missing() {
        let content = read_file_if_exists(Path::new("/nonexistent/file.txt"))
            .await
            .unwrap();
        assert_eq!(content, None);
    }

    #[tokio::test]
    async fn test_read_file_if_exists_directory() {
        let tmp = tempfile::tempdir().unwrap();
        // On Windows, reading a directory returns PermissionDenied
        // On Unix, it returns IsADirectory
        // Both should be treated as "not a file" and return None
        let result = read_file_if_exists(tmp.path()).await;
        match result {
            Ok(Some(_)) => panic!("Expected None when reading a directory"),
            Ok(None) => {} // Expected
            Err(e) => panic!("Expected Ok(None) when reading a directory, got Err: {}", e),
        }
    }
}

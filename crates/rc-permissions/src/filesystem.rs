//! Filesystem permission checks.
//!
//! Corresponds to `src/utils/permissions/filesystem.ts`.
//! Checks whether file operations are within the allowed working directory scope.

use std::path::{Path, PathBuf};

/// Result of a filesystem permission check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemCheckResult {
    /// Whether the operation is allowed.
    pub allowed: bool,
    /// Reason if denied.
    pub reason: Option<String>,
    /// Normalized path that was checked.
    pub normalized_path: PathBuf,
}

/// Check if a file path is within the allowed working directory scope.
///
/// # Arguments
/// * `path` - The file path to check.
/// * `cwd` - The current working directory.
/// * `additional_dirs` - Additional directories allowed by configuration.
///
/// # Returns
/// A [`FilesystemCheckResult`] indicating whether the operation is allowed.
pub fn check_filesystem_permission(
    path: &str,
    cwd: &str,
    additional_dirs: &[String],
) -> FilesystemCheckResult {
    let target = normalize_path(path, cwd);
    
    // Always allow paths within cwd
    if target.starts_with(cwd) {
        return FilesystemCheckResult {
            allowed: true,
            reason: None,
            normalized_path: target,
        };
    }

    // Check additional directories
    for dir in additional_dirs {
        let dir_normalized = normalize_path(dir, cwd);
        if target.starts_with(&dir_normalized) {
            return FilesystemCheckResult {
                allowed: true,
                reason: None,
                normalized_path: target,
            };
        }
    }

    FilesystemCheckResult {
        allowed: false,
        reason: Some(format!(
            "Path '{}' is outside the working directory and additional allowed directories",
            target.display()
        )),
        normalized_path: target,
    }
}

/// Check if a path is a system path that should be protected.
#[must_use]
pub fn is_system_path(path: &str) -> bool {
    let system_prefixes = [
        "/etc",
        "/usr",
        "/sys",
        "/proc",
        "/dev",
        "/boot",
        "/lib",
        "/lib64",
        "/sbin",
        "/bin",
        "C:\\Windows",
        "C:\\Program Files",
        "C:\\ProgramData",
    ];
    
    system_prefixes.iter().any(|prefix| path.starts_with(prefix))
}

/// Check if a path is a hidden file/directory (starts with dot).
#[must_use]
pub fn is_hidden_path(path: &str) -> bool {
    Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with('.'))
}

/// Normalize a path relative to cwd.
fn normalize_path(path: &str, cwd: &str) -> PathBuf {
    let p = Path::new(path);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        PathBuf::from(cwd).join(p)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allow_within_cwd() {
        let result = check_filesystem_permission("src/main.rs", "/home/user/project", &[]);
        assert!(result.allowed);
    }

    #[test]
    fn deny_outside_cwd() {
        let result = check_filesystem_permission("/etc/passwd", "/home/user/project", &[]);
        assert!(!result.allowed);
        assert!(result.reason.is_some());
    }

    #[test]
    fn allow_additional_dir() {
        let result = check_filesystem_permission(
            "/opt/shared/file.txt",
            "/home/user/project",
            &["/opt/shared".to_string()],
        );
        assert!(result.allowed);
    }

    #[test]
    fn system_path_detection() {
        assert!(is_system_path("/etc/passwd"));
        assert!(is_system_path("/usr/bin/python"));
        assert!(is_system_path("C:\\Windows\\System32"));
        assert!(!is_system_path("/home/user/file.txt"));
    }

    #[test]
    fn hidden_path_detection() {
        assert!(is_hidden_path(".gitignore"));
        assert!(is_hidden_path(".env"));
        assert!(!is_hidden_path("main.rs"));
        assert!(!is_hidden_path("/home/user/file.txt"));
    }

    #[test]
    fn normalize_relative_path() {
        let result = normalize_path("src/main.rs", "/home/user/project");
        assert_eq!(result, PathBuf::from("/home/user/project/src/main.rs"));
    }

    #[test]
    fn normalize_absolute_path() {
        let result = normalize_path("/absolute/path", "/home/user/project");
        assert_eq!(result, PathBuf::from("/absolute/path"));
    }
}

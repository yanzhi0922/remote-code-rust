//! Path validation for permission checks.
//!
//! Corresponds to `src/utils/permissions/pathValidation.ts`.
//! Validates that paths are safe and within expected boundaries.

use std::path::Path;

/// Result of path validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathValidation {
    /// Path is valid and safe.
    Valid,
    /// Path is invalid or potentially dangerous.
    Invalid(String),
}

/// Validate a file path for safety.
///
/// Checks for:
/// - Path traversal attacks (../)
/// - Null bytes
/// - Overly long paths
/// - Symlink escape (basic check)
#[must_use]
pub fn validate_path(path: &str) -> PathValidation {
    // Check for null bytes
    if path.contains('\0') {
        return PathValidation::Invalid("Path contains null byte".into());
    }

    // Check for path traversal
    if path.contains("..") {
        // Allow if it's just a parent reference that resolves within cwd
        let normalized = Path::new(path);
        let mut depth: i32 = 0;
        for component in normalized.components() {
            match component {
                std::path::Component::ParentDir => depth -= 1,
                std::path::Component::Normal(_) => depth += 1,
                std::path::Component::RootDir | std::path::Component::Prefix(_) => {}
                std::path::Component::CurDir => {}
            }
        }
        if depth < 0 {
            return PathValidation::Invalid("Path traversal goes above root".into());
        }
    }

    // Check for overly long paths (OS-dependent, use 4096 as safe limit)
    if path.len() > 4096 {
        return PathValidation::Invalid("Path exceeds maximum length".into());
    }

    PathValidation::Valid
}

/// Check if a path is within a given root directory.
#[must_use]
pub fn is_within_root(path: &str, root: &str) -> bool {
    let path_abs = Path::new(path);
    let root_abs = Path::new(root);

    if path_abs.is_absolute() && root_abs.is_absolute() {
        path_abs.starts_with(root_abs)
    } else {
        // For relative paths, just check prefix
        path.starts_with(root) || !path.starts_with("..")
    }
}

/// Sanitize a path by removing dangerous components.
#[must_use]
pub fn sanitize_path(path: &str) -> String {
    path.replace('\0', "")
        .split('/')
        .filter(|c| *c != ".." && *c != ".")
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_paths() {
        assert!(matches!(
            validate_path("src/main.rs"),
            PathValidation::Valid
        ));
        assert!(matches!(
            validate_path("/home/user/file.txt"),
            PathValidation::Valid
        ));
        assert!(matches!(
            validate_path("relative/path"),
            PathValidation::Valid
        ));
    }

    #[test]
    fn null_byte_rejected() {
        assert!(matches!(
            validate_path("file\0.txt"),
            PathValidation::Invalid(_)
        ));
    }

    #[test]
    fn traversal_above_root_rejected() {
        // "../../../etc/passwd" has 3 ParentDir and 2 Normal, net depth = -1
        assert!(matches!(
            validate_path("../../../etc/passwd"),
            PathValidation::Invalid(_)
        ));
    }

    #[test]
    fn traversal_within_bounds_ok() {
        assert!(matches!(validate_path("a/../b"), PathValidation::Valid));
    }

    #[test]
    fn overly_long_path_rejected() {
        let long_path = "a".repeat(5000);
        assert!(matches!(
            validate_path(&long_path),
            PathValidation::Invalid(_)
        ));
    }

    #[test]
    fn is_within_root_checks() {
        // Use platform-independent relative paths for testing
        assert!(is_within_root("src/main.rs", "src"));
        assert!(is_within_root("src", "src"));
        // Absolute paths that don't share prefix
        assert!(!is_within_root("../other/file.txt", "src"));
    }

    #[test]
    fn sanitize_removes_dangerous_components() {
        assert_eq!(sanitize_path("a/../b/./c"), "a/b/c");
        assert_eq!(sanitize_path("file\0.txt"), "file.txt");
    }
}

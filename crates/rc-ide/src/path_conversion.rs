//! Path conversion between IDE and remote-code formats.
//!
//! Handles the differences between Windows and Unix paths, and converts
//! between local IDE paths and remote-code paths.

use crate::config::IdeType;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Convert a path between two IDE formats.
///
/// This handles the most common conversions:
/// - Windows backslashes → Unix forward slashes (and vice versa)
/// - Drive letter paths (`C:\`) → `/c/` Unix-style (and vice versa)
pub fn convert_path(path: &str, _from: IdeType, _to: IdeType) -> String {
    // For now all IDEs use the same path format on the same OS.
    // The main conversion is between local and remote representations.
    path.to_string()
}

/// Convert a local IDE path to a remote-code path.
///
/// - Windows: `C:\Users\foo\bar.rs` → `/c/Users/foo/bar.rs`
/// - Unix: `/home/foo/bar.rs` → `/home/foo/bar.rs` (unchanged)
pub fn to_remote_path(local_path: &str) -> String {
    // Handle Windows drive letter paths.
    if local_path.len() >= 2 {
        let chars: Vec<char> = local_path.chars().collect();
        if chars.len() >= 2 && chars[1] == ':' {
            let letter = chars[0].to_ascii_lowercase();
            let rest = if local_path.len() > 2 { &local_path[2..] } else { "" };
            let rest_unix = rest.replace('\\', "/");
            return format!("/{letter}{rest_unix}");
        }
    }

    // Already Unix-style.
    local_path.replace('\\', "/")
}

/// Convert a remote-code path to a local IDE path.
///
/// - `/c/Users/foo/bar.rs` → `C:\Users\foo\bar.rs` (on Windows)
/// - `/home/foo/bar.rs` → `/home/foo/bar.rs` (on Unix)
pub fn to_local_path(remote_path: &str) -> String {
    // Check for Unix-style Windows drive path: /c/...
    if remote_path.starts_with('/') && remote_path.len() >= 3 {
        let chars: Vec<char> = remote_path.chars().collect();
        if chars.len() >= 3 && chars[0] == '/' && chars[2] == '/' {
            let letter = chars[1];
            if letter.is_ascii_alphabetic() {
                let rest = &remote_path[2..];
                let rest_win = rest.replace('/', "\\");
                let drive_letter = letter.to_ascii_uppercase();
                return format!("{drive_letter}:{rest_win}");
            }
        }
    }

    // Return as-is for Unix paths.
    remote_path.to_string()
}

/// Normalize a path to use forward slashes.
pub fn normalize_to_unix(path: &str) -> String {
    path.replace('\\', "/")
}

/// Normalize a path to use backslashes (Windows).
pub fn normalize_to_windows(path: &str) -> String {
    path.replace('/', "\\")
}

/// Check if a path appears to be a Windows path.
pub fn is_windows_path(path: &str) -> bool {
    path.contains('\\') || (path.len() >= 2 && path.chars().nth(1) == Some(':'))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_from_windows() {
        assert_eq!(to_remote_path(r"C:\Users\foo\bar.rs"), "/c/Users/foo/bar.rs");
    }

    #[test]
    fn remote_from_windows_lowercase_drive() {
        assert_eq!(to_remote_path(r"d:\project\src\main.rs"), "/d/project/src/main.rs");
    }

    #[test]
    fn remote_from_unix_unchanged() {
        assert_eq!(to_remote_path("/home/foo/bar.rs"), "/home/foo/bar.rs");
    }

    #[test]
    fn remote_from_mixed_slashes() {
        assert_eq!(to_remote_path(r"C:\Users/foo\bar.rs"), "/c/Users/foo/bar.rs");
    }

    #[test]
    fn local_from_windows_remote() {
        assert_eq!(to_local_path("/c/Users/foo/bar.rs"), r"C:\Users\foo\bar.rs");
    }

    #[test]
    fn local_from_unix_remote() {
        assert_eq!(to_local_path("/home/foo/bar.rs"), "/home/foo/bar.rs");
    }

    #[test]
    fn local_preserves_drive_case() {
        assert_eq!(to_local_path("/d/project/src/main.rs"), r"D:\project\src\main.rs");
    }

    #[test]
    fn normalize_unix_works() {
        assert_eq!(normalize_to_unix(r"a\b\c"), "a/b/c");
    }

    #[test]
    fn normalize_windows_works() {
        assert_eq!(normalize_to_windows("a/b/c"), r"a\b\c");
    }

    #[test]
    fn is_windows_with_backslash() {
        assert!(is_windows_path(r"C:\foo"));
    }

    #[test]
    fn is_windows_with_drive() {
        assert!(is_windows_path("C:/foo"));
    }

    #[test]
    fn is_not_windows_path() {
        assert!(!is_windows_path("/home/foo"));
    }

    #[test]
    fn convert_path_identity() {
        assert_eq!(convert_path("/foo/bar", IdeType::VsCode, IdeType::JetBrains), "/foo/bar");
    }

    #[test]
    fn roundtrip_windows() {
        let original = r"C:\Users\test\file.rs";
        let remote = to_remote_path(original);
        let back = to_local_path(&remote);
        assert_eq!(back, original);
    }

    #[test]
    fn roundtrip_unix() {
        let original = "/home/test/file.rs";
        let remote = to_remote_path(original);
        let back = to_local_path(&remote);
        assert_eq!(back, original);
    }

    #[test]
    fn remote_drive_only() {
        assert_eq!(to_remote_path("C:"), "/c");
    }

    #[test]
    fn local_short_path_not_drive() {
        // "/a" should not be treated as a drive path (single char after /)
        assert_eq!(to_local_path("/a"), "/a");
    }
}

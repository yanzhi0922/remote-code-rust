//! [`RooProtectedController`] — high-level API for write-protection checks.
//!
//! Ported from `RooProtectedController.ts`.

use std::collections::HashSet;
use std::path::PathBuf;

use crate::patterns;

// ---------------------------------------------------------------------------
// PathAnnotation
// ---------------------------------------------------------------------------

/// A path annotated with its protection status.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathAnnotation {
    pub path: String,
    pub is_protected: bool,
}

// ---------------------------------------------------------------------------
// RooProtectedController
// ---------------------------------------------------------------------------

/// Controls write access to Roo configuration files by enforcing protection
/// patterns. Prevents auto-approved modifications to sensitive Roo
/// configuration files.
#[derive(Debug, Clone)]
pub struct RooProtectedController {
    cwd: PathBuf,
}

impl RooProtectedController {
    /// Create a new controller rooted at `cwd`.
    pub fn new(cwd: &str) -> Self {
        Self {
            cwd: PathBuf::from(cwd),
        }
    }

    /// Check whether `file_path` is write-protected.
    ///
    /// If `file_path` is absolute it is resolved relative to `cwd`; if it is
    /// outside `cwd` the result is always `false`.
    pub fn is_write_protected(&self, file_path: &str) -> bool {
        let normalised = file_path.replace('\\', "/");

        // Detect absolute paths (Unix or Windows).
        let relative = if is_absolute_path(&normalised) {
            let cwd_norm = self.cwd.to_string_lossy().replace('\\', "/");
            match normalised.strip_prefix(&cwd_norm) {
                Some(rest) => rest.trim_start_matches('/').to_string(),
                None => return false, // outside cwd
            }
        } else {
            normalised
        };

        if relative.starts_with("..") {
            return false;
        }

        patterns::is_protected_path(&relative)
    }

    /// Return the set of write-protected files from `paths`.
    pub fn get_protected_files(&self, paths: &[&str]) -> HashSet<String> {
        paths
            .iter()
            .filter(|p| self.is_write_protected(p))
            .map(|s| s.to_string())
            .collect()
    }

    /// Annotate each path with its protection status.
    pub fn annotate_paths_with_protection(&self, paths: &[&str]) -> Vec<PathAnnotation> {
        paths
            .iter()
            .map(|&p| PathAnnotation {
                path: p.to_string(),
                is_protected: self.is_write_protected(p),
            })
            .collect()
    }

    /// Return the static protection message shown to the user.
    pub fn get_protection_message(&self) -> &'static str {
        "This is a Roo configuration file and requires approval for modifications"
    }

    /// Return formatted instructions about protected files for the LLM.
    pub fn get_instructions(&self) -> String {
        patterns::get_protection_description()
    }

    /// Return the list of protected patterns (for testing / debugging).
    pub fn get_protected_patterns() -> &'static [&'static str] {
        patterns::PROTECTED_PATTERNS
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns `true` for absolute Unix (`/…`) or Windows (`C:\…`) paths.
fn is_absolute_path(s: &str) -> bool {
    s.starts_with('/') || (s.len() > 2 && s.as_bytes()[1] == b':' && s.as_bytes()[2] == b'\\')
        || (s.len() > 2 && s.as_bytes()[1] == b':' && s.as_bytes()[2] == b'/')
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_CWD: &str = "/test/workspace";

    #[test]
    fn test_is_write_protected_relative() {
        let ctrl = RooProtectedController::new(TEST_CWD);
        assert!(ctrl.is_write_protected(".rooignore"));
        assert!(ctrl.is_write_protected(".roo/config.json"));
        assert!(!ctrl.is_write_protected("src/index.ts"));
    }

    #[test]
    fn test_is_write_protected_absolute_inside() {
        let ctrl = RooProtectedController::new(TEST_CWD);
        assert!(ctrl.is_write_protected(&format!("{TEST_CWD}/.rooignore")));
    }

    #[test]
    fn test_is_write_protected_absolute_outside() {
        let ctrl = RooProtectedController::new(TEST_CWD);
        assert!(!ctrl.is_write_protected("/tmp/comment-2-pr63.json"));
        assert!(!ctrl.is_write_protected("/etc/passwd"));
    }

    #[test]
    fn test_get_protected_files() {
        let ctrl = RooProtectedController::new(TEST_CWD);
        let files = [
            "src/index.ts",
            ".rooignore",
            "package.json",
            ".roo/config.json",
            "README.md",
        ];
        let protected = ctrl.get_protected_files(&files);
        assert_eq!(
            protected,
            HashSet::from([".rooignore".into(), ".roo/config.json".into()])
        );
    }

    #[test]
    fn test_get_protected_files_empty() {
        let ctrl = RooProtectedController::new(TEST_CWD);
        let files = ["src/index.ts", "package.json", "README.md"];
        let protected = ctrl.get_protected_files(&files);
        assert!(protected.is_empty());
    }

    #[test]
    fn test_annotate_paths() {
        let ctrl = RooProtectedController::new(TEST_CWD);
        let files = ["src/index.ts", ".rooignore", ".roo/config.json", "package.json"];
        let annotated = ctrl.annotate_paths_with_protection(&files);
        assert_eq!(
            annotated,
            vec![
                PathAnnotation { path: "src/index.ts".into(), is_protected: false },
                PathAnnotation { path: ".rooignore".into(), is_protected: true },
                PathAnnotation { path: ".roo/config.json".into(), is_protected: true },
                PathAnnotation { path: "package.json".into(), is_protected: false },
            ]
        );
    }

    #[test]
    fn test_protection_message() {
        let ctrl = RooProtectedController::new(TEST_CWD);
        assert_eq!(
            ctrl.get_protection_message(),
            "This is a Roo configuration file and requires approval for modifications"
        );
    }

    #[test]
    fn test_instructions() {
        let ctrl = RooProtectedController::new(TEST_CWD);
        let instructions = ctrl.get_instructions();
        assert!(instructions.contains("# Protected Files"));
        assert!(instructions.contains("write-protected"));
        assert!(instructions.contains(".rooignore"));
        assert!(instructions.contains(patterns::SHIELD_SYMBOL));
    }

    #[test]
    fn test_get_protected_patterns() {
        let patterns = RooProtectedController::get_protected_patterns();
        assert!(patterns.contains(&".rooignore"));
        assert!(patterns.contains(&".roo/**"));
        assert_eq!(patterns.len(), 10);
    }

    #[test]
    fn test_backslash_handling() {
        let ctrl = RooProtectedController::new(TEST_CWD);
        assert!(ctrl.is_write_protected(".roo\\config.json"));
        assert!(ctrl.is_write_protected(".roo/config.json"));
    }
}

//! Roo-ignore: File ignore controller for Roo Code.
//!
//! Provides .rooignore support with standard .gitignore syntax,
//! plus built-in directory ignore patterns for common large directories.
//!
//! Uses the `ignore` crate (from BurntSushi, the ripgrep author) for proper
//! gitignore semantics including negation patterns (`!`), directory-only
//! patterns (trailing `/`), and glob patterns.

use std::path::Path;

use ignore::gitignore::{Gitignore, GitignoreBuilder};

/// Lock symbol used to indicate blocked files in UI
pub const LOCK_TEXT_SYMBOL: &str = "\u{1F512}";

/// Error type for RooIgnoreController operations.
#[derive(Debug, thiserror::Error)]
pub enum RooIgnoreError {
    #[error("Invalid pattern: {0}")]
    InvalidPattern(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Gitignore builder error: {0}")]
    GitignoreBuild(String),
}

/// Controls LLM access to files by enforcing ignore patterns.
/// Uses standard .gitignore syntax in .rooignore files.
///
/// Powered by the `ignore` crate which provides full gitignore semantics
/// including negation patterns (`!`), directory-only patterns (trailing `/`),
/// character ranges, and glob wildcards.
#[derive(Debug, Clone)]
pub struct RooIgnoreController {
    /// Current working directory
    cwd: String,
    /// Built gitignore matcher (from the `ignore` crate)
    gitignore: Gitignore,
    /// Raw content of the .rooignore file
    content: Option<String>,
}

impl RooIgnoreController {
    /// Create a new RooIgnoreController with the given working directory.
    pub fn new(cwd: &str) -> Self {
        let gitignore = Gitignore::empty();
        Self {
            cwd: cwd.to_string(),
            gitignore,
            content: None,
        }
    }

    /// Load ignore patterns from .rooignore content string.
    ///
    /// Each line is treated as a separate gitignore pattern.
    /// Supports:
    /// - Standard glob patterns (`*.log`, `build/`)
    /// - Negation patterns (`!important.log` to un-ignore)
    /// - Directory-only patterns (trailing `/`)
    /// - Comments (lines starting with `#`)
    /// - Empty lines (ignored)
    pub fn load_patterns(&mut self, content: &str) {
        self.content = Some(content.to_string());

        let root = Path::new(&self.cwd);
        let mut builder = GitignoreBuilder::new(root);

        // Always add .rooignore itself
        let _ = builder.add_line(None, ".rooignore");

        for line in content.lines() {
            let trimmed = line.trim();
            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            // Add the line to the gitignore builder.
            // The ignore crate handles negation (!), directory patterns (/),
            // and all standard gitignore syntax.
            if let Err(_err) = builder.add_line(None, trimmed) {
                // Individual pattern errors are silently skipped;
                // invalid patterns in .rooignore are non-fatal.
            }
        }

        match builder.build() {
            Ok(gi) => self.gitignore = gi,
            Err(_err) => {
                // If the builder fails, keep the empty gitignore
            }
        }
    }

    /// Check if a file should be accessible to the LLM.
    /// Returns `true` if file is accessible, `false` if ignored.
    ///
    /// If no patterns are loaded, all files are accessible.
    ///
    /// Uses the `ignore` crate's gitignore matching which properly handles:
    /// - Negation patterns: `!foo` un-ignores `foo` even if `*` ignores it
    /// - Directory patterns: `dir/` only matches directories
    /// - Glob patterns: `*.log`, `build/**`, etc.
    pub fn validate_access(&self, path: &str) -> bool {
        // Always allow access if no patterns loaded
        if self.content.is_none() || self.gitignore.is_empty() {
            return true;
        }

        // Normalize the path
        let normalized = path.replace('\\', "/");

        // Remove leading ./ if present
        let relative_path = normalized.strip_prefix("./").unwrap_or(&normalized);

        // Use the ignore crate for matching.
        // `matched` returns:
        //   - Match::None       -> not matched (file is allowed)
        //   - Match::Ignore(_)  -> matched an ignore pattern (file is blocked)
        //   - Match::Whitelist(_) -> matched a negation pattern (file is allowed)
        let path = Path::new(relative_path);

        // Use matched_path_or_any_parents which properly handles directory patterns.
        // For example, if "target/" is in .rooignore, checking "target/debug/app"
        // will match because the parent directory "target" is ignored.
        //
        // We try with is_dir=true first to catch directory-only patterns,
        // then fall back to is_dir=false if that doesn't match.
        // This handles the case where a path like "target/debug/app" should be
        // blocked by "target/" even though we don't know if "target" is a directory.
        match self.gitignore.matched_path_or_any_parents(path, false) {
            ignore::Match::None => {
                // Also check with is_dir=true in case the path itself is a
                // directory that matches a directory-only pattern
                match self.gitignore.matched(path, true) {
                    ignore::Match::Ignore(_) => false,
                    _ => true,
                }
            }
            ignore::Match::Ignore(_) => false,
            ignore::Match::Whitelist(_) => true,
        }
    }

    /// Check if a terminal command should be allowed to execute based on file access patterns.
    /// Returns `Some(restricted_path)` if the command accesses a restricted file,
    /// or `None` if the command is allowed.
    pub fn validate_command(&self, command: &str) -> Option<String> {
        // Always allow if no patterns loaded
        if self.content.is_none() {
            return None;
        }

        let parts: Vec<&str> = command.trim().split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }

        let base_command = parts[0].to_lowercase();

        // Commands that read file contents
        let file_reading_commands = [
            "cat",
            "less",
            "more",
            "head",
            "tail",
            "grep",
            "awk",
            "sed",
            "get-content",
            "gc",
            "type",
            "select-string",
            "sls",
        ];

        if file_reading_commands.contains(&base_command.as_str()) {
            // Check each argument that could be a file path
            for arg in &parts[1..] {
                // Skip command flags/options (both Unix and PowerShell style)
                if arg.starts_with('-') || arg.starts_with('/') {
                    continue;
                }
                // Ignore PowerShell parameter names
                if arg.contains(':') {
                    continue;
                }
                // Validate file access
                if !self.validate_access(arg) {
                    return Some(arg.to_string());
                }
            }
        }

        None
    }

    /// Filter an array of paths, removing those that should be ignored.
    pub fn filter_paths(&self, paths: &[String]) -> Vec<String> {
        paths
            .iter()
            .filter(|p| self.validate_access(p))
            .cloned()
            .collect()
    }

    /// Get formatted instructions about the .rooignore file for the LLM.
    /// Returns `None` if .rooignore doesn't exist.
    pub fn get_instructions(&self) -> Option<String> {
        self.content.as_ref().map(|content| {
            format!(
                "# .rooignore\n\n(The following is provided by a root-level .rooignore file where the user has specified files and directories that should not be accessed. When using list_files, you'll notice a {LOCK_TEXT_SYMBOL} next to files that are blocked. Attempting to access the file's contents e.g. through read_file will result in an error.)\n\n{content}\n.rooignore"
            )
        })
    }

    /// Get the current working directory.
    pub fn cwd(&self) -> &str {
        &self.cwd
    }

    /// Get the raw .rooignore content.
    pub fn content(&self) -> Option<&str> {
        self.content.as_deref()
    }

    /// Get the number of loaded patterns.
    pub fn pattern_count(&self) -> usize {
        self.gitignore.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_controller() {
        let controller = RooIgnoreController::new("/project");
        assert_eq!(controller.cwd(), "/project");
        assert!(controller.content().is_none());
        assert_eq!(controller.pattern_count(), 0);
    }

    #[test]
    fn test_validate_access_no_patterns() {
        let controller = RooIgnoreController::new("/project");
        assert!(controller.validate_access("src/main.rs"));
        assert!(controller.validate_access("any/file.txt"));
    }

    #[test]
    fn test_load_patterns_simple() {
        let mut controller = RooIgnoreController::new("/project");
        controller.load_patterns("*.log\ntarget/");
        // .rooignore + *.log + target/ = 3 patterns
        assert!(controller.pattern_count() >= 2);
    }

    #[test]
    fn test_load_patterns_skips_comments() {
        let mut controller = RooIgnoreController::new("/project");
        controller.load_patterns("# This is a comment\n*.log\n# Another comment\ntarget/");
        assert!(controller.pattern_count() >= 2);
    }

    #[test]
    fn test_load_patterns_skips_empty_lines() {
        let mut controller = RooIgnoreController::new("/project");
        controller.load_patterns("*.log\n\n\ntarget/");
        assert!(controller.pattern_count() >= 2);
    }

    #[test]
    fn test_validate_access_with_pattern() {
        let mut controller = RooIgnoreController::new("/project");
        controller.load_patterns("*.log");
        assert!(!controller.validate_access("debug.log"));
        assert!(!controller.validate_access("logs/error.log"));
        assert!(controller.validate_access("src/main.rs"));
    }

    #[test]
    fn test_validate_access_directory_pattern() {
        let mut controller = RooIgnoreController::new("/project");
        controller.load_patterns("target/");
        assert!(!controller.validate_access("target/debug/app"));
    }

    #[test]
    fn test_validate_access_rooignore_always_blocked() {
        let mut controller = RooIgnoreController::new("/project");
        controller.load_patterns("*.log");
        assert!(!controller.validate_access(".rooignore"));
    }

    #[test]
    fn test_validate_command_no_patterns() {
        let controller = RooIgnoreController::new("/project");
        assert!(controller.validate_command("cat secret.txt").is_none());
    }

    #[test]
    fn test_validate_command_reading_blocked_file() {
        let mut controller = RooIgnoreController::new("/project");
        controller.load_patterns("secret.txt");
        let result = controller.validate_command("cat secret.txt");
        assert_eq!(result, Some("secret.txt".to_string()));
    }

    #[test]
    fn test_validate_command_reading_allowed_file() {
        let mut controller = RooIgnoreController::new("/project");
        controller.load_patterns("secret.txt");
        let result = controller.validate_command("cat readme.md");
        assert!(result.is_none());
    }

    #[test]
    fn test_validate_command_skips_flags() {
        let mut controller = RooIgnoreController::new("/project");
        controller.load_patterns("secret.txt");
        let result = controller.validate_command("cat -n secret.txt");
        assert_eq!(result, Some("secret.txt".to_string()));
    }

    #[test]
    fn test_validate_command_non_reading_command() {
        let mut controller = RooIgnoreController::new("/project");
        controller.load_patterns("secret.txt");
        let result = controller.validate_command("echo secret.txt");
        assert!(result.is_none());
    }

    #[test]
    fn test_filter_paths() {
        let mut controller = RooIgnoreController::new("/project");
        controller.load_patterns("*.log");
        let paths = vec![
            "src/main.rs".to_string(),
            "debug.log".to_string(),
            "lib/mod.rs".to_string(),
            "error.log".to_string(),
        ];
        let filtered = controller.filter_paths(&paths);
        assert_eq!(filtered, vec!["src/main.rs", "lib/mod.rs"]);
    }

    #[test]
    fn test_get_instructions_no_content() {
        let controller = RooIgnoreController::new("/project");
        assert!(controller.get_instructions().is_none());
    }

    #[test]
    fn test_get_instructions_with_content() {
        let mut controller = RooIgnoreController::new("/project");
        controller.load_patterns("*.log\ntarget/");
        let instructions = controller.get_instructions().unwrap();
        assert!(instructions.contains(".rooignore"));
        assert!(instructions.contains(LOCK_TEXT_SYMBOL));
        assert!(instructions.contains("*.log"));
    }

    #[test]
    fn test_validate_access_wildcard_pattern() {
        let mut controller = RooIgnoreController::new("/project");
        controller.load_patterns("*.secret");
        assert!(!controller.validate_access("keys.secret"));
        assert!(!controller.validate_access("config/keys.secret"));
        assert!(controller.validate_access("keys.txt"));
    }

    #[test]
    fn test_validate_access_backslash_path() {
        let mut controller = RooIgnoreController::new("/project");
        controller.load_patterns("*.log");
        assert!(!controller.validate_access("logs\\debug.log"));
    }

    #[test]
    fn test_validate_command_empty_command() {
        let mut controller = RooIgnoreController::new("/project");
        controller.load_patterns("secret.txt");
        assert!(controller.validate_command("").is_none());
    }

    #[test]
    fn test_validate_command_all_reading_commands() {
        let reading_commands = [
            "cat",
            "less",
            "more",
            "head",
            "tail",
            "grep",
            "awk",
            "sed",
            "get-content",
            "gc",
            "type",
            "select-string",
            "sls",
        ];

        for cmd in &reading_commands {
            let mut controller = RooIgnoreController::new("/project");
            controller.load_patterns("secret.txt");
            let result = controller.validate_command(&format!("{cmd} secret.txt"));
            assert_eq!(
                result,
                Some("secret.txt".to_string()),
                "Failed for command: {cmd}"
            );
        }
    }

    // ---------------------------------------------------------------
    // Negation pattern tests — the key fix
    // ---------------------------------------------------------------

    #[test]
    fn test_negation_pattern_unignores_file() {
        let mut controller = RooIgnoreController::new("/project");
        controller.load_patterns("*.log\n!important.log");
        // important.log should be accessible (un-ignored by !important.log)
        assert!(controller.validate_access("important.log"));
        // other .log files should still be blocked
        assert!(!controller.validate_access("debug.log"));
        assert!(!controller.validate_access("error.log"));
    }

    #[test]
    fn test_negation_pattern_with_directory() {
        let mut controller = RooIgnoreController::new("/project");
        controller.load_patterns("src/\n!src/important/");
        // Files in src/important/ should be accessible due to negation
        assert!(controller.validate_access("src/important/config.json"));
    }

    #[test]
    fn test_negation_pattern_order_matters() {
        // Negation patterns must appear after the patterns they negate
        let mut controller = RooIgnoreController::new("/project");
        controller.load_patterns("!keep.txt\n*.txt");
        // In gitignore, later patterns override earlier ones,
        // so *.txt should override !keep.txt
        assert!(!controller.validate_access("keep.txt"));
    }

    #[test]
    fn test_negation_with_wildcard() {
        let mut controller = RooIgnoreController::new("/project");
        // Negation patterns work to un-ignore specific files/directories
        controller.load_patterns("*\n!.gitignore\n!src/**");
        // Most things should be ignored by *, except .gitignore and src/** contents
        assert!(controller.validate_access(".gitignore"));
        assert!(controller.validate_access("src/main.rs"));
        assert!(!controller.validate_access("README.md"));
    }

    #[test]
    fn test_double_star_pattern() {
        let mut controller = RooIgnoreController::new("/project");
        controller.load_patterns("**/logs/*.txt");
        assert!(!controller.validate_access("logs/debug.txt"));
        assert!(!controller.validate_access("src/logs/debug.txt"));
        assert!(controller.validate_access("src/logs/debug.csv"));
    }
}

use std::path::Path;

/// Lock symbol used to indicate blocked files in UI
pub const LOCK_TEXT_SYMBOL: &str = "🔒";

/// Error type for RooIgnoreController operations
#[derive(Debug, thiserror::Error)]
pub enum RooIgnoreError {
    #[error("Invalid pattern: {0}")]
    InvalidPattern(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Controls LLM access to files by enforcing ignore patterns.
/// Uses standard .gitignore syntax in .rooignore files.
#[derive(Debug, Clone)]
pub struct RooIgnoreController {
    /// Current working directory
    cwd: String,
    /// Loaded ignore patterns (one per line from .rooignore)
    patterns: Vec<String>,
    /// Raw content of the .rooignore file
    content: Option<String>,
}

impl RooIgnoreController {
    /// Create a new RooIgnoreController with the given working directory.
    pub fn new(cwd: &str) -> Self {
        Self {
            cwd: cwd.to_string(),
            patterns: Vec::new(),
            content: None,
        }
    }

    /// Load ignore patterns from .rooignore content string.
    /// Each line is treated as a separate pattern (like .gitignore syntax).
    /// Lines starting with `#` are comments and are ignored.
    /// Empty lines are ignored.
    pub fn load_patterns(&mut self, content: &str) {
        self.content = Some(content.to_string());
        self.patterns = content
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .map(String::from)
            .collect();

        // Always add .rooignore itself
        self.patterns.push(".rooignore".to_string());
    }

    /// Check if a file should be accessible to the LLM.
    /// Returns `true` if file is accessible, `false` if ignored.
    ///
    /// If no patterns are loaded, all files are accessible.
    pub fn validate_access(&self, path: &str) -> bool {
        // Always allow access if no patterns loaded
        if self.content.is_none() || self.patterns.is_empty() {
            return true;
        }

        // Normalize the path
        let normalized = path.replace('\\', "/");

        // Remove leading ./ if present
        let relative_path = normalized.strip_prefix("./").unwrap_or(&normalized);

        // Try matching against each pattern
        for pattern in &self.patterns {
            if Self::matches_pattern(pattern, relative_path) {
                return false;
            }
        }

        true
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
        self.patterns.len()
    }

    /// Check if a path matches a gitignore-style pattern.
    fn matches_pattern(pattern: &str, path: &str) -> bool {
        // Handle negation patterns (patterns starting with !)
        if pattern.starts_with('!') {
            return false;
        }

        // Strip trailing slash for directory patterns
        let is_dir_pattern = pattern.ends_with('/');
        let clean_pattern = pattern.trim_end_matches('/');

        // Check if pattern contains path separators
        let has_separator = clean_pattern.contains('/');

        // 1. Directory pattern: pattern like "target/" means match any directory named "target"
        //    and everything inside it.
        if is_dir_pattern && !has_separator {
            // Match if path starts with "target/" or contains "/target/"
            if path.starts_with(&format!("{clean_pattern}/"))
                || path.contains(&format!("/{clean_pattern}/"))
            {
                return true;
            }
        }

        // 2. Build glob pattern for general matching
        let glob_pattern = if has_separator {
            // Pattern with path separator: match against full path
            clean_pattern.to_string()
        } else {
            // Pattern without path separator: match against any path component
            format!("**/{clean_pattern}")
        };

        // Try to match using the glob crate
        if let Ok(glob) = glob::Pattern::new(&glob_pattern) {
            if glob.matches(path) {
                return true;
            }
        }

        // 3. Also try matching just the filename
        if let Some(filename) = Path::new(path).file_name().and_then(|f| f.to_str()) {
            if let Ok(glob) = glob::Pattern::new(clean_pattern) {
                if glob.matches(filename) {
                    return true;
                }
            }
        }

        // 4. For non-directory patterns, also check directory containment
        if !is_dir_pattern && !has_separator {
            if path.starts_with(&format!("{clean_pattern}/"))
                || path.contains(&format!("/{clean_pattern}/"))
            {
                return true;
            }
        }

        false
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
        assert_eq!(controller.pattern_count(), 3); // *.log, target/, .rooignore
    }

    #[test]
    fn test_load_patterns_skips_comments() {
        let mut controller = RooIgnoreController::new("/project");
        controller.load_patterns("# This is a comment\n*.log\n# Another comment\ntarget/");
        assert_eq!(controller.pattern_count(), 3); // *.log, target/, .rooignore
    }

    #[test]
    fn test_load_patterns_skips_empty_lines() {
        let mut controller = RooIgnoreController::new("/project");
        controller.load_patterns("*.log\n\n\ntarget/");
        assert_eq!(controller.pattern_count(), 3);
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
            "cat", "less", "more", "head", "tail", "grep", "awk", "sed",
            "get-content", "gc", "type", "select-string", "sls",
        ];

        for cmd in &reading_commands {
            let mut controller = RooIgnoreController::new("/project");
            controller.load_patterns("secret.txt");
            let result = controller.validate_command(&format!("{cmd} secret.txt"));
            assert_eq!(result, Some("secret.txt".to_string()), "Failed for command: {cmd}");
        }
    }
}

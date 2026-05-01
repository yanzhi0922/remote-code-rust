//! Protected file patterns and matching logic.
//!
//! Ported from `RooProtectedController.ts` — controls write access to Roo
//! configuration files by enforcing protection patterns (gitignore-style).

use glob::Pattern;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Shield emoji used to visually mark protected files.
pub const SHIELD_SYMBOL: &str = "\u{1F6E1}";

/// Predefined list of protected Roo configuration patterns.
///
/// Semantics follow gitignore rules:
/// - Patterns **without** `/` are *unanchored* — they match the basename at
///   any directory depth.
/// - Patterns **with** `/` are *anchored* — they match from the workspace root.
/// - A trailing `/**` matches any file inside the named directory.
pub const PROTECTED_PATTERNS: &[&str] = &[
    ".rooignore",
    ".roomodes",
    ".roorules*",
    ".clinerules*",
    ".roo/**",
    ".vscode/**",
    "*.code-workspace",
    ".rooprotected",
    "AGENTS.md",
    "AGENT.md",
];

// ---------------------------------------------------------------------------
// Matching helpers
// ---------------------------------------------------------------------------

/// Check whether `file_path` matches any protected pattern.
///
/// The path is normalised to forward-slash separators before matching.
/// Paths that start with `..` (i.e. outside the workspace) always return
/// `false`.
pub fn is_protected_path(file_path: &str) -> bool {
    let normalised = file_path.replace('\\', "/");

    if normalised.is_empty() || normalised.starts_with("..") {
        return false;
    }

    for &pattern in PROTECTED_PATTERNS {
        if matches_pattern(&normalised, pattern) {
            return true;
        }
    }
    false
}

/// Return a human-readable description of the file protection rules,
/// including the shield symbol and the full list of protected patterns.
pub fn get_protection_description() -> String {
    let patterns = PROTECTED_PATTERNS.join(", ");
    format!(
        "# Protected Files\n\n\
         (The following Roo configuration file patterns are write-protected and \
         always require approval for modifications, regardless of autoapproval \
         settings. When using list_files, you'll notice a {SHIELD_SYMBOL} next \
         to files that are write-protected.)\n\n\
         Protected patterns: {patterns}"
    )
}

// ---------------------------------------------------------------------------
// Internal matching engine
// ---------------------------------------------------------------------------

/// Match a normalised (forward-slash) path against a single gitignore-style
/// pattern.
fn matches_pattern(path: &str, pattern: &str) -> bool {
    // Directory-glob pattern, e.g. ".roo/**"
    if let Some(dir) = pattern.strip_suffix("/**") {
        return path == dir || path.starts_with(&format!("{dir}/"));
    }

    let has_slash = pattern.contains('/');

    if !has_slash {
        // Unanchored pattern — try each path component as a basename.
        for component in path.split('/') {
            if let Ok(p) = Pattern::new(pattern) {
                if p.matches(component) {
                    return true;
                }
            }
        }
        return false;
    }

    // Anchored pattern — match against the full relative path.
    if let Ok(p) = Pattern::new(pattern) {
        p.matches(path)
    } else {
        false
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- is_protected_path: protected patterns should match ----

    #[test]
    fn test_protect_rooignore() {
        assert!(is_protected_path(".rooignore"));
    }

    #[test]
    fn test_protect_roomodes() {
        assert!(is_protected_path(".roomodes"));
    }

    #[test]
    fn test_protect_roorules_star() {
        assert!(is_protected_path(".roorules"));
        assert!(is_protected_path(".roorules.md"));
    }

    #[test]
    fn test_protect_clinerules_star() {
        assert!(is_protected_path(".clinerules"));
        assert!(is_protected_path(".clinerules.md"));
    }

    #[test]
    fn test_protect_roo_directory() {
        assert!(is_protected_path(".roo/config.json"));
        assert!(is_protected_path(".roo/settings/user.json"));
        assert!(is_protected_path(".roo/modes/custom.json"));
        assert!(is_protected_path(".roo/mcp.json"));
    }

    #[test]
    fn test_protect_vscode_directory() {
        assert!(is_protected_path(".vscode/settings.json"));
        assert!(is_protected_path(".vscode/launch.json"));
        assert!(is_protected_path(".vscode/tasks.json"));
    }

    #[test]
    fn test_protect_code_workspace() {
        assert!(is_protected_path("myproject.code-workspace"));
        assert!(is_protected_path("pentest.code-workspace"));
        assert!(is_protected_path(".code-workspace"));
        assert!(is_protected_path("folder/workspace.code-workspace"));
    }

    #[test]
    fn test_protect_rooprotected() {
        assert!(is_protected_path(".rooprotected"));
    }

    #[test]
    fn test_protect_agents_md() {
        assert!(is_protected_path("AGENTS.md"));
    }

    #[test]
    fn test_protect_agent_md() {
        assert!(is_protected_path("AGENT.md"));
    }

    // ---- is_protected_path: non-protected files should pass ----

    #[test]
    fn test_non_protected_regular_files() {
        assert!(!is_protected_path("src/index.ts"));
        assert!(!is_protected_path("package.json"));
        assert!(!is_protected_path("README.md"));
    }

    #[test]
    fn test_non_protected_roo_like_names() {
        assert!(!is_protected_path(".roosettings"));
        assert!(!is_protected_path(".rooconfig"));
    }

    #[test]
    fn test_non_protected_roo_in_path() {
        assert!(!is_protected_path("src/roo-utils.ts"));
        assert!(!is_protected_path("config/roo.config.js"));
    }

    // ---- Nested path matching ----

    #[test]
    fn test_nested_path_matching() {
        // Unanchored patterns match at any depth.
        assert!(is_protected_path("nested/.rooignore"));
        assert!(is_protected_path("nested/.roomodes"));
        assert!(is_protected_path("nested/.roorules.md"));
        assert!(is_protected_path("deep/nested/.clinerules"));
        assert!(is_protected_path("sub/AGENTS.md"));
        // Anchored patterns only match at root.
        assert!(is_protected_path(".roo/config.json")); // .roo/** at root
    }

    // ---- Paths outside workspace ----

    #[test]
    fn test_paths_outside_workspace() {
        assert!(!is_protected_path("../other/file.txt"));
        assert!(!is_protected_path(".."));
    }

    // ---- Case sensitivity ----

    #[test]
    fn test_case_sensitivity() {
        // Pattern matching is case-sensitive on all platforms.
        assert!(!is_protected_path("agents.md")); // lowercase
        assert!(!is_protected_path("Agents.md"));
        assert!(is_protected_path("AGENTS.md")); // exact match
    }

    // ---- Backslash normalisation ----

    #[test]
    fn test_backslash_normalisation() {
        assert!(is_protected_path(".roo\\config.json"));
        assert!(is_protected_path("nested\\.rooignore"));
    }

    // ---- Empty path ----

    #[test]
    fn test_empty_path() {
        assert!(!is_protected_path(""));
    }

    // ---- get_protection_description ----

    #[test]
    fn test_description_format() {
        let desc = get_protection_description();
        assert!(desc.contains("# Protected Files"));
        assert!(desc.contains("write-protected"));
        assert!(desc.contains(".rooignore"));
        assert!(desc.contains(".roo/**"));
        assert!(desc.contains(SHIELD_SYMBOL));
    }

    // ---- PROTECTED_PATTERNS constant ----

    #[test]
    fn test_protected_patterns_constant() {
        assert_eq!(
            PROTECTED_PATTERNS,
            &[
                ".rooignore",
                ".roomodes",
                ".roorules*",
                ".clinerules*",
                ".roo/**",
                ".vscode/**",
                "*.code-workspace",
                ".rooprotected",
                "AGENTS.md",
                "AGENT.md",
            ]
        );
    }
}

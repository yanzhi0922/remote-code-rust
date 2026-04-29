/// List of directories that are typically large and should be ignored
/// when showing recursive file listings or scanning for code indexing.
pub const DIRS_TO_IGNORE: &[&str] = &[
    "node_modules",
    "__pycache__",
    "env",
    "venv",
    "target/dependency",
    "build/dependencies",
    "dist",
    "out",
    "bundle",
    "vendor",
    "tmp",
    "temp",
    "deps",
    "pkg",
    "Pods",
    ".git",
    ".*",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dirs_to_ignore_contains_common_dirs() {
        assert!(DIRS_TO_IGNORE.contains(&"node_modules"));
        assert!(DIRS_TO_IGNORE.contains(&"__pycache__"));
        assert!(DIRS_TO_IGNORE.contains(&"env"));
        assert!(DIRS_TO_IGNORE.contains(&"venv"));
        assert!(DIRS_TO_IGNORE.contains(&"dist"));
        assert!(DIRS_TO_IGNORE.contains(&".git"));
        assert!(DIRS_TO_IGNORE.contains(&".*"));
    }

    #[test]
    fn test_dirs_to_ignore_has_correct_length() {
        assert_eq!(DIRS_TO_IGNORE.len(), 17);
    }

    #[test]
    fn test_dirs_to_ignore_contains_target_dependency() {
        assert!(DIRS_TO_IGNORE.contains(&"target/dependency"));
    }

    #[test]
    fn test_dirs_to_ignore_contains_build_dependencies() {
        assert!(DIRS_TO_IGNORE.contains(&"build/dependencies"));
    }
}

//! list_files tool implementation.
//!
//! Supports recursive listing with configurable depth limit and .gitignore
//! awareness. When .gitignore is respected, entries matching ignore patterns
//! are excluded from results.

use crate::types::*;
use roo_types::tool::ListFilesParams;
use roo_ignore::RooIgnoreController;
use std::path::{Path, PathBuf};

/// Validate list_files parameters.
pub fn validate_list_files_params(params: &ListFilesParams) -> Result<(), SearchToolError> {
    if params.path.trim().is_empty() {
        return Err(SearchToolError::Validation(
            "path must not be empty".to_string(),
        ));
    }

    if params.path.contains("..") {
        return Err(SearchToolError::Validation(
            "path must not contain '..' — path traversal is not allowed".to_string(),
        ));
    }

    Ok(())
}

/// Validate that the given directory path exists and is accessible.
///
/// Returns a detailed error message if the path does not exist or is not a directory.
pub fn validate_list_path_exists(path: &str, cwd: &Path) -> Result<(), SearchToolError> {
    let full_path = if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else {
        cwd.join(path)
    };

    if !full_path.exists() {
        return Err(SearchToolError::Validation(format!(
            "Path '{}' does not exist. \
             Please check that the path is correct. \
             If using a relative path, it is resolved against the current working directory. \
             Tried absolute path: '{}'",
            path,
            full_path.display()
        )));
    }

    if !full_path.is_dir() {
        return Err(SearchToolError::Validation(format!(
            "Path '{}' is not a directory. \
             list_files can only list directory contents. \
             Please provide a directory path.",
            path
        )));
    }

    Ok(())
}

/// Build a FileListResult from a list of paths.
///
/// Separates files from directories and applies truncation.
pub fn build_file_list_result(
    path: &str,
    recursive: bool,
    entries: Vec<String>,
    max_entries: usize,
) -> FileListResult {
    let mut files = Vec::new();
    let mut directories = Vec::new();

    for entry in &entries {
        // Simple heuristic: entries ending with '/' or '\' are directories
        if entry.ends_with('/') || entry.ends_with('\\') {
            directories.push(entry.clone());
        } else {
            files.push(entry.clone());
        }
    }

    let total_count = files.len() + directories.len();
    let truncated = total_count > max_entries;

    // Apply truncation
    if truncated {
        files.truncate(max_entries / 2);
        directories.truncate(max_entries / 2);
    }

    FileListResult {
        path: path.to_string(),
        recursive,
        files,
        directories,
        total_count,
        truncated,
    }
}

/// List files in a directory with optional recursive depth and .gitignore support.
///
/// # Arguments
/// * `dir` - The directory to list
/// * `recursive` - Whether to recurse into subdirectories
/// * `max_depth` - Maximum recursion depth (None = unlimited when recursive)
/// * `respect_gitignore` - Whether to skip entries matching .gitignore patterns
/// * `max_entries` - Maximum number of entries to return
pub fn list_files_advanced(
    dir: &Path,
    recursive: bool,
    max_depth: Option<usize>,
    respect_gitignore: bool,
    max_entries: usize,
) -> Vec<String> {
    let mut entries = Vec::new();
    let ignore_patterns = if respect_gitignore {
        load_gitignore_patterns(dir)
    } else {
        Vec::new()
    };
    collect_entries_advanced(
        dir,
        dir,
        recursive,
        max_depth.unwrap_or(usize::MAX),
        0,
        &ignore_patterns,
        &mut entries,
        max_entries,
    );
    entries
}

/// Recursively collect directory entries with depth limit and gitignore filtering.
fn collect_entries_advanced(
    base_dir: &Path,
    current_dir: &Path,
    recursive: bool,
    max_depth: usize,
    current_depth: usize,
    ignore_patterns: &[String],
    entries: &mut Vec<String>,
    max_entries: usize,
) {
    if entries.len() >= max_entries {
        return;
    }

    if current_depth > max_depth {
        return;
    }

    let dir_entries = match std::fs::read_dir(current_dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut sub_dirs = Vec::new();

    for entry in dir_entries.flatten() {
        if entries.len() >= max_entries {
            return;
        }

        let file_name = match entry.file_name().to_str() {
            Some(n) => n.to_string(),
            None => continue,
        };

        let full_path = entry.path();
        let relative = full_path
            .strip_prefix(base_dir)
            .unwrap_or(&full_path)
            .to_string_lossy()
            .to_string();

        // Skip hidden files/dirs (starting with .)
        if file_name.starts_with('.') {
            continue;
        }

        // Check gitignore patterns
        if should_ignore(&relative, ignore_patterns) {
            continue;
        }

        if full_path.is_dir() {
            entries.push(format!("{}/", relative));
            sub_dirs.push(full_path);
        } else {
            entries.push(relative);
        }
    }

    // Recurse into subdirectories
    if recursive {
        for sub_dir in sub_dirs {
            collect_entries_advanced(
                base_dir,
                &sub_dir,
                recursive,
                max_depth,
                current_depth + 1,
                ignore_patterns,
                entries,
                max_entries,
            );
        }
    }
}

/// Load .gitignore patterns from a directory.
///
/// Reads the `.gitignore` file if present and returns the patterns.
/// Lines starting with `#` are comments; empty lines are skipped.
fn load_gitignore_patterns(dir: &Path) -> Vec<String> {
    let gitignore_path = dir.join(".gitignore");
    if !gitignore_path.exists() {
        return Vec::new();
    }

    let content = match std::fs::read_to_string(&gitignore_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    content
        .lines()
        .map(|line| line.trim())
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| line.to_string())
        .collect()
}

/// Check if a relative path should be ignored based on gitignore patterns.
///
/// Supports basic gitignore patterns:
/// - `*.ext` — matches any file with the given extension
/// - `dir/` — matches a directory
/// - `name` — exact match
/// - `**/pattern` — matches at any depth
fn should_ignore(relative_path: &str, patterns: &[String]) -> bool {
    for pattern in patterns {
        let pat = pattern.trim_end_matches('/');
        if pat.is_empty() {
            continue;
        }

        // **/pattern — match at any depth (including root level)
        // Must check before *.ext to avoid ** being caught by *
        if pat.starts_with("**/") {
            let suffix = &pat[3..];
            if relative_path == suffix
                || relative_path.ends_with(&format!("/{}", suffix))
                || relative_path.ends_with(suffix)
            {
                return true;
            }
        }
        // *.ext pattern
        else if pat.starts_with('*') {
            let suffix = &pat[1..]; // e.g., ".rs"
            if relative_path.ends_with(suffix) {
                return true;
            }
        }
        // pattern/** — match directory prefix
        else if pat.ends_with("/**") {
            let prefix = &pat[..pat.len() - 3];
            if relative_path.starts_with(prefix) {
                return true;
            }
        }
        // Exact match or prefix match
        else if relative_path == pat
            || relative_path.starts_with(&format!("{}/", pat))
            || relative_path.ends_with(&format!("/{}", pat))
        {
            return true;
        }
    }
    false
}

/// Filter a list of file entries, removing those matched by .rooignore rules.
///
/// Entries whose paths (without trailing `/`) are not accessible according to
/// the controller are removed from the list.
pub fn filter_entries_by_rooignore(
    entries: Vec<String>,
    controller: Option<&RooIgnoreController>,
) -> Vec<String> {
    match controller {
        Some(ctrl) => entries
            .into_iter()
            .filter(|entry| {
                // Strip trailing slash for directory entries before checking
                let path = entry.trim_end_matches('/');
                ctrl.validate_access(path)
            })
            .collect(),
        None => entries,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_empty_path() {
        let params = ListFilesParams {
            path: "".to_string(),
            recursive: false,
        };
        assert!(validate_list_files_params(&params).is_err());
    }

    #[test]
    fn test_validate_valid() {
        let params = ListFilesParams {
            path: "src".to_string(),
            recursive: false,
        };
        assert!(validate_list_files_params(&params).is_ok());
    }

    #[test]
    fn test_build_file_list_basic() {
        let entries = vec![
            "src/main.rs".to_string(),
            "src/lib.rs".to_string(),
            "target/".to_string(),
        ];
        let result = build_file_list_result("src", false, entries, 100);
        assert_eq!(result.files.len(), 2);
        assert_eq!(result.directories.len(), 1);
        assert!(!result.truncated);
    }

    #[test]
    fn test_build_file_list_truncated() {
        let entries: Vec<String> = (0..200).map(|i| format!("file_{}.txt", i)).collect();
        let result = build_file_list_result("dir", false, entries, 100);
        assert!(result.truncated);
    }

    #[test]
    fn test_build_file_list_empty() {
        let result = build_file_list_result("empty", false, vec![], 100);
        assert!(result.files.is_empty());
        assert!(result.directories.is_empty());
        assert!(!result.truncated);
    }

    // --- L5.3: Advanced listing tests ---

    #[test]
    fn test_list_files_advanced_basic() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.txt"), "a").unwrap();
        std::fs::write(dir.path().join("b.rs"), "b").unwrap();
        std::fs::create_dir(dir.path().join("subdir")).unwrap();
        std::fs::write(dir.path().join("subdir/c.txt"), "c").unwrap();

        let entries = list_files_advanced(dir.path(), false, None, false, 100);
        // Non-recursive: should see a.txt, b.rs, subdir/
        assert!(entries.iter().any(|e| e.contains("a.txt")));
        assert!(entries.iter().any(|e| e.contains("b.rs")));
        assert!(entries.iter().any(|e| e.contains("subdir")));
    }

    #[test]
    fn test_list_files_advanced_recursive() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("a/b")).unwrap();
        std::fs::write(dir.path().join("a/b/c.txt"), "c").unwrap();

        let entries = list_files_advanced(dir.path(), true, None, false, 100);
        assert!(entries.iter().any(|e| e.contains("c.txt")));
    }

    #[test]
    fn test_list_files_advanced_depth_limit() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("l1/l2/l3")).unwrap();
        std::fs::write(dir.path().join("l1/l2/l3/deep.txt"), "deep").unwrap();
        std::fs::write(dir.path().join("l1/shallow.txt"), "shallow").unwrap();

        // Depth 1: should see l1/ and l1/shallow.txt but not l1/l2/l3/deep.txt
        let entries = list_files_advanced(dir.path(), true, Some(1), false, 100);
        assert!(entries.iter().any(|e| e.contains("shallow.txt")));
        // deep.txt is at depth 3, should not be present with max_depth=1
        assert!(!entries.iter().any(|e| e.contains("deep.txt")));
    }

    #[test]
    fn test_list_files_advanced_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "*.log\ntarget/\n").unwrap();
        std::fs::write(dir.path().join("good.txt"), "good").unwrap();
        std::fs::write(dir.path().join("bad.log"), "bad").unwrap();
        std::fs::create_dir(dir.path().join("target")).unwrap();

        let entries = list_files_advanced(dir.path(), false, None, true, 100);
        assert!(entries.iter().any(|e| e.contains("good.txt")));
        assert!(!entries.iter().any(|e| e.contains("bad.log")));
        assert!(!entries.iter().any(|e| e.contains("target")));
    }

    #[test]
    fn test_list_files_advanced_max_entries() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..50 {
            std::fs::write(dir.path().join(format!("file_{:03}.txt", i)), "x").unwrap();
        }

        let entries = list_files_advanced(dir.path(), false, None, false, 5);
        assert_eq!(entries.len(), 5);
    }

    #[test]
    fn test_should_ignore_extension() {
        let patterns = vec!["*.log".to_string()];
        assert!(should_ignore("debug.log", &patterns));
        assert!(!should_ignore("main.rs", &patterns));
    }

    #[test]
    fn test_should_ignore_directory() {
        let patterns = vec!["target".to_string()];
        assert!(should_ignore("target", &patterns));
        assert!(should_ignore("target/debug", &patterns));
        assert!(!should_ignore("src/main.rs", &patterns));
    }

    #[test]
    fn test_should_ignore_double_star() {
        let patterns = vec!["**/node_modules".to_string()];
        assert!(should_ignore("node_modules", &patterns));
        assert!(should_ignore("foo/node_modules", &patterns));
        assert!(should_ignore("foo/bar/node_modules", &patterns));
    }

    #[test]
    fn test_load_gitignore_patterns() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(".gitignore"),
            "# comment\n*.log\n\ntarget/\n*.tmp\n",
        )
        .unwrap();

        let patterns = load_gitignore_patterns(dir.path());
        assert_eq!(patterns.len(), 3);
        assert!(patterns.contains(&"*.log".to_string()));
        assert!(patterns.contains(&"target/".to_string()));
        assert!(patterns.contains(&"*.tmp".to_string()));
    }

    #[test]
    fn test_load_gitignore_no_file() {
        let dir = tempfile::tempdir().unwrap();
        let patterns = load_gitignore_patterns(dir.path());
        assert!(patterns.is_empty());
    }

    // --- RooIgnore filtering tests ---

    #[test]
    fn test_filter_entries_by_rooignore_no_controller() {
        let entries = vec![
            "src/main.rs".to_string(),
            "secret.txt".to_string(),
            "node_modules/".to_string(),
        ];
        let filtered = filter_entries_by_rooignore(entries, None);
        assert_eq!(filtered.len(), 3);
    }

    #[test]
    fn test_filter_entries_by_rooignore_blocks_ignored() {
        let mut ctrl = roo_ignore::RooIgnoreController::new("/tmp");
        ctrl.load_patterns("secret.txt\nnode_modules/");
        let entries = vec![
            "src/main.rs".to_string(),
            "secret.txt".to_string(),
            "node_modules/".to_string(),
        ];
        let filtered = filter_entries_by_rooignore(entries, Some(&ctrl));
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].contains("src/main.rs"));
    }

    // --- Detailed error message tests ---

    #[test]
    fn test_validate_list_path_not_found_detailed_error() {
        let result = validate_list_path_exists("/nonexistent/path", Path::new("."));
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("does not exist"), "Error should mention path not found");
        assert!(err.contains("check that the path"), "Error should suggest checking path");
    }

    #[test]
    fn test_validate_list_path_is_file_detailed_error() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "hello").unwrap();

        let result = validate_list_path_exists(
            file_path.to_str().unwrap(),
            Path::new("."),
        );
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("not a directory"), "Error should mention not a directory");
    }

    #[test]
    fn test_validate_list_path_traversal_rejected() {
        let params = ListFilesParams {
            path: "../etc".to_string(),
            recursive: false,
        };
        let result = validate_list_files_params(&params);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("path traversal"), "Error should mention path traversal");
    }
}

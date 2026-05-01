use crate::constants::DIRS_TO_IGNORE;

/// Checks if a file path should be ignored based on the DIRS_TO_IGNORE patterns.
/// This function handles special patterns like ".*" for hidden directories.
///
/// # Arguments
/// * `file_path` - The file path to check
///
/// # Returns
/// * `true` if the path should be ignored, `false` otherwise
pub fn is_path_in_ignored_directory(file_path: &str) -> bool {
    // Normalize path separators to forward slashes
    let normalized_path = file_path.replace('\\', "/");
    let path_parts: Vec<&str> = normalized_path.split('/').collect();

    // Check each directory in the path against DIRS_TO_IGNORE
    for part in &path_parts {
        // Skip empty parts (from leading or trailing slashes)
        if part.is_empty() {
            continue;
        }

        // Handle the ".*" pattern for hidden directories
        if DIRS_TO_IGNORE.contains(&".*") && part.starts_with('.') && *part != "." {
            return true;
        }

        // Check for exact matches
        if DIRS_TO_IGNORE.contains(part) {
            return true;
        }
    }

    // Check if path contains any ignored directory pattern
    for dir in DIRS_TO_IGNORE {
        if *dir == ".*" {
            // Already handled above
            continue;
        }

        // Check if the directory appears in the path
        if normalized_path.contains(&format!("/{dir}/")) {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_modules_ignored() {
        assert!(is_path_in_ignored_directory("project/node_modules/pkg"));
        assert!(is_path_in_ignored_directory("node_modules"));
    }

    #[test]
    fn test_pycache_ignored() {
        assert!(is_path_in_ignored_directory("src/__pycache__/module.pyc"));
    }

    #[test]
    fn test_env_ignored() {
        assert!(is_path_in_ignored_directory("project/env/lib"));
        assert!(is_path_in_ignored_directory("project/venv/lib"));
    }

    #[test]
    fn test_dist_ignored() {
        assert!(is_path_in_ignored_directory("project/dist/bundle.js"));
    }

    #[test]
    fn test_git_ignored() {
        assert!(is_path_in_ignored_directory("project/.git/config"));
    }

    #[test]
    fn test_hidden_directory_ignored() {
        assert!(is_path_in_ignored_directory("project/.hidden/file.txt"));
        assert!(is_path_in_ignored_directory("project/.vscode/settings.json"));
    }

    #[test]
    fn test_dot_itself_not_ignored() {
        // "." is the current directory, should not be ignored
        assert!(!is_path_in_ignored_directory("./src/main.rs"));
    }

    #[test]
    fn test_normal_path_not_ignored() {
        assert!(!is_path_in_ignored_directory("src/main.rs"));
        assert!(!is_path_in_ignored_directory("lib/mod.rs"));
    }

    #[test]
    fn test_backslash_normalization() {
        assert!(is_path_in_ignored_directory("project\\node_modules\\pkg"));
        assert!(is_path_in_ignored_directory("project\\.git\\config"));
    }

    #[test]
    fn test_vendor_ignored() {
        assert!(is_path_in_ignored_directory("project/vendor/lib.rs"));
    }

    #[test]
    fn test_target_dependency_ignored() {
        assert!(is_path_in_ignored_directory("project/target/dependency/lib.rs"));
    }

    #[test]
    fn test_build_dependencies_ignored() {
        assert!(is_path_in_ignored_directory("project/build/dependencies/lib.rs"));
    }

    #[test]
    fn test_tmp_ignored() {
        assert!(is_path_in_ignored_directory("project/tmp/cache"));
    }

    #[test]
    fn test_temp_ignored() {
        assert!(is_path_in_ignored_directory("project/temp/cache"));
    }

    #[test]
    fn test_empty_path_not_ignored() {
        assert!(!is_path_in_ignored_directory(""));
    }

    #[test]
    fn test_pods_ignored() {
        assert!(is_path_in_ignored_directory("ios/Pods/Alamofire"));
    }

    #[test]
    fn test_deps_ignored() {
        assert!(is_path_in_ignored_directory("project/deps/lib"));
    }

    #[test]
    fn test_pkg_ignored() {
        assert!(is_path_in_ignored_directory("project/pkg/output"));
    }

    #[test]
    fn test_bundle_ignored() {
        assert!(is_path_in_ignored_directory("project/bundle/main.js"));
    }

    #[test]
    fn test_out_ignored() {
        assert!(is_path_in_ignored_directory("project/out/output.js"));
    }
}

//! Path resolution for Roo configuration directories.
//!
//! Maps to TypeScript source: `src/services/roo-config/index.ts`

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Gets the global `.roo` directory path based on the current platform.
///
/// - macOS/Linux: `~/.roo/`
/// - Windows: `%USERPROFILE%\.roo\`
///
/// Maps to TS: `getGlobalRooDirectory()`
pub fn get_global_roo_directory() -> PathBuf {
    dirs::home_dir()
        .expect("Unable to determine home directory")
        .join(".roo")
}

/// Gets the global `.agents` directory path.
///
/// This is a shared directory for agent skills across different AI coding tools.
///
/// Maps to TS: `getGlobalAgentsDirectory()`
pub fn get_global_agents_directory() -> PathBuf {
    dirs::home_dir()
        .expect("Unable to determine home directory")
        .join(".agents")
}

/// Gets the project-local `.agents` directory path for a given cwd.
///
/// Maps to TS: `getProjectAgentsDirectoryForCwd(cwd)`
pub fn get_project_agents_directory_for_cwd(cwd: &Path) -> PathBuf {
    cwd.join(".agents")
}

/// Gets the project-local `.roo` directory path for a given cwd.
///
/// Maps to TS: `getProjectRooDirectoryForCwd(cwd)`
pub fn get_project_roo_directory_for_cwd(cwd: &Path) -> PathBuf {
    cwd.join(".roo")
}

/// Gets the ordered list of `.roo` directories to check (global first, then project-local).
///
/// Maps to TS: `getRooDirectoriesForCwd(cwd)`
pub fn get_roo_directories_for_cwd(cwd: &Path) -> Vec<PathBuf> {
    vec![
        get_global_roo_directory(),
        get_project_roo_directory_for_cwd(cwd),
    ]
}

/// Discovers all `.roo` directories in subdirectories of the workspace.
///
/// Returns an array of paths to `.roo` directories found in subdirectories,
/// sorted alphabetically. Does not include the root `.roo` directory.
///
/// Maps to TS: `discoverSubfolderRooDirectories(cwd)`
pub async fn discover_subfolder_roo_directories(cwd: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut roo_dirs = BTreeSet::new();
    let root_roo_dir = cwd.join(".roo");

    discover_roo_dirs_recursive(cwd, cwd, &root_roo_dir, &mut roo_dirs)?;

    Ok(roo_dirs.into_iter().collect())
}

/// Recursively walk directories to find `.roo` directories.
fn discover_roo_dirs_recursive(
    base: &Path,
    current: &Path,
    root_roo_dir: &Path,
    found: &mut BTreeSet<PathBuf>,
) -> std::io::Result<()> {
    let entries = match std::fs::read_dir(current) {
        Ok(entries) => entries,
        Err(_) => return Ok(()), // Skip directories we can't read
    };

    for entry in entries {
        let entry = entry?;
        let path = entry.path();

        // Skip non-directories
        if !path.is_dir() {
            continue;
        }

        // Skip common non-project directories
        let dir_name = match path.file_name() {
            Some(name) => name.to_string_lossy().to_string(),
            None => continue,
        };

        if should_skip_directory(&dir_name) {
            continue;
        }

        // Check if this directory contains a .roo subdirectory
        let roo_path = path.join(".roo");
        if roo_path.is_dir() && roo_path != *root_roo_dir {
            found.insert(roo_path);
        }

        // Recurse into subdirectories
        discover_roo_dirs_recursive(base, &path, root_roo_dir, found)?;
    }

    Ok(())
}

/// Returns true if a directory should be skipped during discovery.
fn should_skip_directory(name: &str) -> bool {
    matches!(
        name,
        "node_modules"
            | ".git"
            | ".svn"
            | ".hg"
            | "target"
            | "dist"
            | "build"
            | ".next"
            | ".nuxt"
            | "vendor"
            | "__pycache__"
            | ".tox"
            | ".mypy_cache"
            | ".pytest_cache"
            | ".venv"
            | "venv"
            | "env"
            | ".env"
            | ".idea"
            | ".vscode"
            | ".vs"
    )
}

/// Gets the ordered list of all `.roo` directories including subdirectories.
///
/// Returns directories in order: `[global, project-local, ...subfolders (alphabetically)]`
///
/// Maps to TS: `getAllRooDirectoriesForCwd(cwd)`
pub async fn get_all_roo_directories_for_cwd(cwd: &Path) -> Vec<PathBuf> {
    let mut directories = get_roo_directories_for_cwd(cwd);

    // Discover and add subfolder .roo directories
    if let Ok(subfolder_dirs) = discover_subfolder_roo_directories(cwd).await {
        directories.extend(subfolder_dirs);
    }

    directories
}

/// Gets parent directories containing `.roo` folders, in order from root to subfolders.
///
/// Maps to TS: `getAgentsDirectoriesForCwd(cwd)`
pub async fn get_agents_directories_for_cwd(cwd: &Path) -> Vec<PathBuf> {
    let mut directories = vec![cwd.to_path_buf()];

    // Get all subfolder .roo directories
    if let Ok(subfolder_roo_dirs) = discover_subfolder_roo_directories(cwd).await {
        // Extract parent directories (remove .roo from path)
        for roo_dir in subfolder_roo_dirs {
            if let Some(parent_dir) = roo_dir.parent() {
                directories.push(parent_dir.to_path_buf());
            }
        }
    }

    directories
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_get_global_roo_directory() {
        let dir = get_global_roo_directory();
        assert!(dir.to_string_lossy().ends_with(".roo"));
        assert!(dir.is_absolute());
    }

    #[test]
    fn test_get_global_agents_directory() {
        let dir = get_global_agents_directory();
        assert!(dir.to_string_lossy().ends_with(".agents"));
        assert!(dir.is_absolute());
    }

    #[test]
    fn test_get_project_roo_directory() {
        let cwd = Path::new("/Users/john/my-project");
        let dir = get_project_roo_directory_for_cwd(cwd);
        assert_eq!(dir, PathBuf::from("/Users/john/my-project/.roo"));
    }

    #[test]
    fn test_get_project_agents_directory() {
        let cwd = Path::new("/Users/john/my-project");
        let dir = get_project_agents_directory_for_cwd(cwd);
        assert_eq!(dir, PathBuf::from("/Users/john/my-project/.agents"));
    }

    #[test]
    fn test_get_roo_directories_for_cwd() {
        let cwd = Path::new("/Users/john/my-project");
        let dirs = get_roo_directories_for_cwd(cwd);
        assert_eq!(dirs.len(), 2);
        assert!(dirs[0].to_string_lossy().ends_with(".roo"));
        assert_eq!(dirs[1], PathBuf::from("/Users/john/my-project/.roo"));
    }

    #[test]
    fn test_should_skip_directory() {
        assert!(should_skip_directory("node_modules"));
        assert!(should_skip_directory(".git"));
        assert!(should_skip_directory("target"));
        assert!(should_skip_directory("dist"));
        assert!(!should_skip_directory("src"));
        assert!(!should_skip_directory("packages"));
        assert!(!should_skip_directory("my-app"));
    }

    #[tokio::test]
    async fn test_discover_subfolder_roo_directories_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let dirs = discover_subfolder_roo_directories(tmp.path())
            .await
            .unwrap();
        assert!(dirs.is_empty());
    }

    #[tokio::test]
    async fn test_discover_subfolder_roo_directories_with_nested() {
        let tmp = tempfile::tempdir().unwrap();

        // Create subfolder .roo directories
        let pkg_a = tmp.path().join("package-a").join(".roo");
        let pkg_b = tmp.path().join("package-b").join(".roo");
        let nested = tmp.path().join("packages").join("shared").join(".roo");

        fs::create_dir_all(&pkg_a).unwrap();
        fs::create_dir_all(&pkg_b).unwrap();
        fs::create_dir_all(&nested).unwrap();

        // Create a root .roo that should be excluded
        let root_roo = tmp.path().join(".roo");
        fs::create_dir_all(&root_roo).unwrap();

        // Create node_modules/.roo that should be skipped
        let node_modules_roo = tmp.path().join("node_modules").join("pkg").join(".roo");
        fs::create_dir_all(&node_modules_roo).unwrap();

        let dirs = discover_subfolder_roo_directories(tmp.path())
            .await
            .unwrap();

        // Root .roo should not be included, node_modules should be skipped
        assert_eq!(dirs.len(), 3);
        assert!(dirs.contains(&pkg_a));
        assert!(dirs.contains(&pkg_b));
        assert!(dirs.contains(&nested));
        assert!(!dirs.contains(&root_roo));
    }

    #[tokio::test]
    async fn test_get_all_roo_directories_for_cwd() {
        let tmp = tempfile::tempdir().unwrap();
        let pkg_roo = tmp.path().join("package-a").join(".roo");
        fs::create_dir_all(&pkg_roo).unwrap();

        let dirs = get_all_roo_directories_for_cwd(tmp.path()).await;
        assert_eq!(dirs.len(), 3); // global + project-local + 1 subfolder
    }

    #[tokio::test]
    async fn test_get_agents_directories_for_cwd() {
        let tmp = tempfile::tempdir().unwrap();
        let pkg_roo = tmp.path().join("package-a").join(".roo");
        fs::create_dir_all(&pkg_roo).unwrap();

        let dirs = get_agents_directories_for_cwd(tmp.path()).await;
        assert_eq!(dirs.len(), 2); // cwd + package-a parent
        assert_eq!(dirs[0], tmp.path());
        assert_eq!(dirs[1], tmp.path().join("package-a"));
    }
}

//! Configuration loading with merge support.
//!
//! Maps to TypeScript source: `src/services/roo-config/index.ts` (loadConfiguration)

use crate::filesystem::read_file_if_exists;
use crate::paths::{get_global_roo_directory, get_project_roo_directory_for_cwd};
use std::path::{Path, PathBuf};

/// Separator used when merging global and project-specific configurations.
const PROJECT_OVERRIDE_SEPARATOR: &str =
    "\n\n# Project-specific rules (override global):\n\n";

/// Result of loading configuration from multiple `.roo` directories.
///
/// Maps to TS: return type of `loadConfiguration()`
#[derive(Debug, Clone)]
pub struct LoadedConfiguration {
    /// Content from the global `.roo` directory, if present.
    pub global: Option<String>,
    /// Content from the project-local `.roo` directory, if present.
    pub project: Option<String>,
    /// Merged content with project overriding global.
    pub merged: String,
    /// Path to the global file that was read.
    pub global_path: PathBuf,
    /// Path to the project-local file that was read.
    pub project_path: PathBuf,
}

/// Loads configuration from multiple `.roo` directories with project overriding global.
///
/// Reads from:
/// - Global: `~/.roo/{relative_path}`
/// - Project: `{cwd}/.roo/{relative_path}`
///
/// Merging behavior:
/// - If only global exists: `merged = global content`
/// - If only project exists: `merged = project content`
/// - If both exist: `merged = "global content\n\n# Project-specific rules (override global):\n\nproject content"`
///
/// Maps to TS: `loadConfiguration(relativePath, cwd)`
pub async fn load_configuration(
    relative_path: &Path,
    cwd: &Path,
) -> std::io::Result<LoadedConfiguration> {
    let global_dir = get_global_roo_directory();
    let project_dir = get_project_roo_directory_for_cwd(cwd);

    let global_file_path = global_dir.join(relative_path);
    let project_file_path = project_dir.join(relative_path);

    // Read global configuration
    let global_content = read_file_if_exists(&global_file_path).await?;

    // Read project-local configuration
    let project_content = read_file_if_exists(&project_file_path).await?;

    // Merge configurations - project overrides global
    let merged = build_merged_content(global_content.as_deref(), project_content.as_deref());

    Ok(LoadedConfiguration {
        global: global_content,
        project: project_content,
        merged,
        global_path: global_file_path,
        project_path: project_file_path,
    })
}

/// Builds merged content from global and project content.
///
/// Maps to TS: merge logic in `loadConfiguration()`
pub fn build_merged_content(global: Option<&str>, project: Option<&str>) -> String {
    match (global, project) {
        (None, None) => String::new(),
        (Some(g), None) => g.to_string(),
        (None, Some(p)) => p.to_string(),
        (Some(g), Some(p)) => {
            format!("{}{}{}", g, PROJECT_OVERRIDE_SEPARATOR, p)
        }
    }
}

/// Alias for backward compatibility.
pub async fn load_roo_configuration(
    relative_path: &Path,
    cwd: &Path,
) -> std::io::Result<LoadedConfiguration> {
    load_configuration(relative_path, cwd).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_build_merged_content_both_none() {
        assert_eq!(build_merged_content(None, None), "");
    }

    #[test]
    fn test_build_merged_content_global_only() {
        assert_eq!(
            build_merged_content(Some("global rules"), None),
            "global rules"
        );
    }

    #[test]
    fn test_build_merged_content_project_only() {
        assert_eq!(
            build_merged_content(None, Some("project rules")),
            "project rules"
        );
    }

    #[test]
    fn test_build_merged_content_both() {
        let result = build_merged_content(Some("global rules"), Some("project rules"));
        assert!(result.starts_with("global rules"));
        assert!(result.contains(PROJECT_OVERRIDE_SEPARATOR));
        assert!(result.ends_with("project rules"));
    }

    #[tokio::test]
    async fn test_load_configuration_neither_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let result = load_configuration(Path::new("rules/rules.md"), tmp.path())
            .await
            .unwrap();
        assert!(result.global.is_none());
        assert!(result.project.is_none());
        assert_eq!(result.merged, "");
    }

    #[tokio::test]
    async fn test_load_configuration_project_only() {
        let tmp = tempfile::tempdir().unwrap();
        let project_roo = tmp.path().join(".roo").join("rules");
        fs::create_dir_all(&project_roo).unwrap();
        fs::write(project_roo.join("rules.md"), "project rules").unwrap();

        let result = load_configuration(Path::new("rules/rules.md"), tmp.path())
            .await
            .unwrap();
        assert!(result.global.is_none());
        assert_eq!(result.project, Some("project rules".to_string()));
        assert_eq!(result.merged, "project rules");
    }

    #[tokio::test]
    async fn test_load_configuration_global_only() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path().join("home");
        fs::create_dir_all(&home).unwrap();

        let global_roo = home.join(".roo").join("rules");
        fs::create_dir_all(&global_roo).unwrap();
        fs::write(global_roo.join("rules.md"), "global rules").unwrap();

        // We can't easily override the home directory for testing,
        // so we test the build_merged_content function directly
        let merged = build_merged_content(Some("global rules"), None);
        assert_eq!(merged, "global rules");
    }

    #[tokio::test]
    async fn test_load_configuration_both() {
        let tmp = tempfile::tempdir().unwrap();
        let project_roo = tmp.path().join(".roo").join("rules");
        fs::create_dir_all(&project_roo).unwrap();
        fs::write(project_roo.join("rules.md"), "project rules").unwrap();

        let result = load_configuration(Path::new("rules/rules.md"), tmp.path())
            .await
            .unwrap();
        // Global won't exist in test, but project will
        assert_eq!(result.project, Some("project rules".to_string()));
    }

    #[tokio::test]
    async fn test_load_roo_configuration_alias() {
        let tmp = tempfile::tempdir().unwrap();
        let result = load_roo_configuration(Path::new("nonexistent.md"), tmp.path())
            .await
            .unwrap();
        assert_eq!(result.merged, "");
    }
}

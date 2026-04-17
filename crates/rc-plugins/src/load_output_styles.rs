//! Output style loading from plugin manifests.
//!
//! Extracts output style configurations from plugin directories. Output styles
//! are defined as markdown files in the plugin's `output-styles/` directory.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::markdown_walker::walk_markdown_paths;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A plugin output style configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginOutputStyle {
    /// Fully-qualified style name (e.g., `"plugin-name:style-name"`).
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Source file path.
    pub file_path: PathBuf,
    /// Plugin name that provides this style.
    pub plugin_name: String,
    /// Style prompt / template.
    pub prompt: String,
    /// Whether this style should be forced for the plugin.
    #[serde(default)]
    pub force_for_plugin: Option<bool>,
}

/// Result of loading output styles from a plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadOutputStylesResult {
    /// Styles found.
    pub styles: Vec<PluginOutputStyle>,
    /// Errors encountered.
    pub errors: Vec<String>,
}

// ---------------------------------------------------------------------------
// Core functions
// ---------------------------------------------------------------------------

/// Load plugin output styles from a directory.
///
/// Walks the `output-styles/` directory and extracts style definitions
/// from markdown files.
pub fn load_plugin_output_styles(plugin_name: &str, styles_dir: &Path) -> LoadOutputStylesResult {
    let mut styles = Vec::new();
    let mut errors = Vec::new();

    if !styles_dir.exists() {
        return LoadOutputStylesResult { styles, errors };
    }

    if !styles_dir.is_dir() {
        errors.push(format!(
            "output-styles path {} is not a directory",
            styles_dir.display()
        ));
        return LoadOutputStylesResult { styles, errors };
    }

    let markdown_entries = walk_markdown_paths(styles_dir);

    for (file_path, _namespace) in markdown_entries {
        let file_stem: &str = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown");

        let style_name = format!("{plugin_name}:{file_stem}");

        let content = match std::fs::read_to_string(&file_path) {
            Ok(c) => c,
            Err(e) => {
                errors.push(format!(
                    "failed to read output style {}: {e}",
                    file_path.display()
                ));
                continue;
            }
        };

        let (description, prompt) = parse_style_content(&content, &style_name);

        styles.push(PluginOutputStyle {
            name: style_name,
            description,
            file_path: file_path.clone(),
            plugin_name: plugin_name.to_owned(),
            prompt,
            force_for_plugin: None,
        });
    }

    styles.sort_by(|a, b| a.name.cmp(&b.name));

    LoadOutputStylesResult { styles, errors }
}

/// Parse style content into description and prompt.
fn parse_style_content(content: &str, fallback_name: &str) -> (String, String) {
    let lines: Vec<&str> = content.lines().collect();

    // Extract description from first heading or first non-empty paragraph
    let description = lines
        .iter()
        .find(|line| line.starts_with('#'))
        .map(|line| line.trim_start_matches('#').trim().to_owned())
        .or_else(|| {
            lines
                .iter()
                .find(|line| !line.trim().is_empty())
                .map(|line| line.trim().to_owned())
        })
        .unwrap_or_else(|| fallback_name.to_owned());

    // Prompt is the content after any frontmatter/heading
    let prompt = content.trim().to_owned();

    (description, prompt)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn ok<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
        match result {
            Ok(value) => value,
            Err(error) => panic!("unexpected error: {error}"),
        }
    }

    #[test]
    fn load_plugin_output_styles_basic() {
        let temp = ok(tempdir());
        let styles_dir = temp.path().join("output-styles");
        fs::create_dir_all(&styles_dir).expect("create dir");
        fs::write(
            styles_dir.join("concise.md"),
            "# Concise\nOutput in a concise format.",
        )
        .expect("write");
        fs::write(
            styles_dir.join("detailed.md"),
            "# Detailed\nOutput with full details.",
        )
        .expect("write");

        let result = load_plugin_output_styles("my-plugin", &styles_dir);
        assert_eq!(result.styles.len(), 2);
        assert!(result.errors.is_empty());

        let names: Vec<&str> = result.styles.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"my-plugin:concise"));
        assert!(names.contains(&"my-plugin:detailed"));
    }

    #[test]
    fn load_plugin_output_styles_nonexistent() {
        let result = load_plugin_output_styles("my-plugin", Path::new("/nonexistent/styles"));
        assert!(result.styles.is_empty());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn load_plugin_output_styles_extracts_description() {
        let temp = ok(tempdir());
        let styles_dir = temp.path().join("output-styles");
        fs::create_dir_all(&styles_dir).expect("create dir");
        fs::write(
            styles_dir.join("concise.md"),
            "# My Concise Style\nBe brief.",
        )
        .expect("write");

        let result = load_plugin_output_styles("my-plugin", &styles_dir);
        assert_eq!(result.styles.len(), 1);
        assert_eq!(result.styles[0].description, "My Concise Style");
    }

    #[test]
    fn load_plugin_output_styles_not_a_directory() {
        let temp = ok(tempdir());
        let file = temp.path().join("notadir.md");
        fs::write(&file, "content").expect("write");

        let result = load_plugin_output_styles("my-plugin", &file);
        assert!(result.styles.is_empty());
        assert_eq!(result.errors.len(), 1);
    }
}

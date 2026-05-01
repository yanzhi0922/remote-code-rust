//! Automatic settings import from a predefined path.
//!
//! Derived from `src/utils/autoImportSettings.ts`.
//!
//! Automatically imports RooCode settings from a specified path if it exists.
//! This function is called during extension activation to allow users to
//! pre-configure their settings by placing a settings file at a predefined location.

use std::path::PathBuf;

use tracing::{info, warn};

use crate::filesystem::file_exists;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Options for auto-import operations.
///
/// These mirror the `ImportOptions` from the TS source.
pub struct AutoImportOptions {
    /// Optional settings path override.
    pub settings_path: Option<String>,
}

/// Result of an auto-import attempt.
#[derive(Debug)]
pub struct AutoImportResult {
    pub success: bool,
    pub message: String,
    pub path: Option<PathBuf>,
}

// ---------------------------------------------------------------------------
// Auto-import
// ---------------------------------------------------------------------------

/// Automatically imports settings from a predefined path.
///
/// Source: `src/utils/autoImportSettings.ts` — `autoImportSettings`
///
/// # Arguments
/// * `settings_path` - The path to the settings file to import from
///
/// # Returns
/// An `AutoImportResult` indicating success or failure.
pub async fn auto_import_settings(settings_path: Option<&str>) -> AutoImportResult {
    let path_str = match settings_path {
        Some(p) if !p.trim().is_empty() => p.trim(),
        _ => {
            info!("[AutoImport] No auto-import settings path specified, skipping auto-import");
            return AutoImportResult {
                success: false,
                message: "No auto-import settings path specified".to_string(),
                path: None,
            };
        }
    };

    let resolved_path = resolve_path(path_str);
    info!(
        "[AutoImport] Checking for settings file at: {}",
        resolved_path.display()
    );

    // Check if the file exists
    if !file_exists(&resolved_path).await {
        info!(
            "[AutoImport] Settings file not found at {}, skipping auto-import",
            resolved_path.display()
        );
        return AutoImportResult {
            success: false,
            message: format!("Settings file not found at {}", resolved_path.display()),
            path: Some(resolved_path),
        };
    }

    // Attempt to read and validate the file as JSON
    match tokio::fs::read_to_string(&resolved_path).await {
        Ok(content) => {
            match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(_) => {
                    info!(
                        "[AutoImport] Successfully validated settings from {}",
                        resolved_path.display()
                    );
                    AutoImportResult {
                        success: true,
                        message: format!(
                            "Successfully validated settings from {}",
                            resolved_path.display()
                        ),
                        path: Some(resolved_path),
                    }
                }
                Err(e) => {
                    warn!("[AutoImport] Failed to parse settings JSON: {}", e);
                    AutoImportResult {
                        success: false,
                        message: format!("Failed to parse settings JSON: {}", e),
                        path: Some(resolved_path),
                    }
                }
            }
        }
        Err(e) => {
            warn!("[AutoImport] Failed to read settings file: {}", e);
            AutoImportResult {
                success: false,
                message: format!("Failed to read settings file: {}", e),
                path: Some(resolved_path),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Path resolution
// ---------------------------------------------------------------------------

/// Resolves a file path, handling home directory expansion and relative paths.
///
/// Source: `src/utils/autoImportSettings.ts` — `resolvePath`
fn resolve_path(settings_path: &str) -> PathBuf {
    // Handle home directory expansion
    if settings_path.starts_with("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(&settings_path[2..]);
        }
    }

    let path = PathBuf::from(settings_path);

    // Handle absolute paths
    if path.is_absolute() {
        return path;
    }

    // Handle relative paths (relative to home directory for safety)
    if let Some(home) = dirs::home_dir() {
        return home.join(settings_path);
    }

    path
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_path_home_directory() {
        let path = resolve_path("~/config/settings.json");
        assert!(path.to_string_lossy().contains("config/settings.json"));
        assert!(!path.to_string_lossy().starts_with("~"));
    }

    #[test]
    fn test_resolve_path_absolute() {
        let path = resolve_path("/absolute/path/settings.json");
        // On Windows, "/absolute/..." resolves to "C:/absolute/..." (current drive prefix)
        // On Unix, it stays as "/absolute/..."
        assert!(path.is_absolute());
        assert!(path.to_string_lossy().ends_with("absolute/path/settings.json"));
    }

    #[test]
    fn test_resolve_path_relative() {
        let path = resolve_path("relative/path.json");
        // Should be relative to home directory
        assert!(path.to_string_lossy().contains("relative/path.json"));
    }

    #[tokio::test]
    async fn test_auto_import_no_path() {
        let result = auto_import_settings(None).await;
        assert!(!result.success);
        assert!(result.message.contains("No auto-import"));
    }

    #[tokio::test]
    async fn test_auto_import_empty_path() {
        let result = auto_import_settings(Some("")).await;
        assert!(!result.success);
    }

    #[tokio::test]
    async fn test_auto_import_whitespace_path() {
        let result = auto_import_settings(Some("   ")).await;
        assert!(!result.success);
    }

    #[tokio::test]
    async fn test_auto_import_nonexistent_file() {
        let result = auto_import_settings(Some("/nonexistent/path/settings.json")).await;
        assert!(!result.success);
        assert!(result.message.contains("not found"));
    }
}

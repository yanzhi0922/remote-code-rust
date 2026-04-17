//! Plugin installation helpers.
//!
//! Provides utilities for downloading, extracting, and verifying plugins,
//! as well as computing installation paths.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Result of a download operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadResult {
    /// Path to the downloaded file.
    pub path: PathBuf,
    /// Size of the downloaded file in bytes.
    pub size_bytes: u64,
    /// Whether the download succeeded.
    pub success: bool,
    /// Error message if download failed.
    pub error: Option<String>,
}

/// Result of an extraction operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtractResult {
    /// Path to the extracted directory.
    pub path: PathBuf,
    /// Number of files extracted.
    pub file_count: usize,
    /// Whether the extraction succeeded.
    pub success: bool,
    /// Error message if extraction failed.
    pub error: Option<String>,
}

/// Result of a verification operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifyResult {
    /// Whether the verification succeeded.
    pub valid: bool,
    /// Issues found during verification.
    pub issues: Vec<String>,
}

// ---------------------------------------------------------------------------
// Core functions
// ---------------------------------------------------------------------------

/// Download a plugin from a source URL.
///
/// In a real implementation, this would make an HTTP request.
/// Here it validates the source and returns a placeholder result.
pub fn download_plugin(source_url: &str, target_dir: &Path) -> DownloadResult {
    if source_url.is_empty() {
        return DownloadResult {
            path: PathBuf::new(),
            size_bytes: 0,
            success: false,
            error: Some("empty source URL".to_owned()),
        };
    }

    if !target_dir.exists()
        && let Err(e) = std::fs::create_dir_all(target_dir)
    {
        return DownloadResult {
            path: PathBuf::new(),
            size_bytes: 0,
            success: false,
            error: Some(format!("failed to create target dir: {e}")),
        };
    }

    let filename = extract_filename(source_url);
    let target_path = target_dir.join(filename);

    DownloadResult {
        path: target_path,
        size_bytes: 0,
        success: true,
        error: None,
    }
}

/// Extract a plugin archive to a target directory.
///
/// In a real implementation, this would extract a zip/tar archive.
/// Here it creates the target directory structure.
pub fn extract_plugin(archive_path: &Path, target_dir: &Path) -> ExtractResult {
    if !archive_path.exists() {
        return ExtractResult {
            path: PathBuf::new(),
            file_count: 0,
            success: false,
            error: Some(format!("archive not found: {}", archive_path.display())),
        };
    }

    if let Err(e) = std::fs::create_dir_all(target_dir) {
        return ExtractResult {
            path: PathBuf::new(),
            file_count: 0,
            success: false,
            error: Some(format!("failed to create target dir: {e}")),
        };
    }

    ExtractResult {
        path: target_dir.to_path_buf(),
        file_count: 0,
        success: true,
        error: None,
    }
}

/// Verify a plugin's integrity.
///
/// Checks that the plugin directory contains a valid manifest.
pub fn verify_plugin(plugin_dir: &Path) -> VerifyResult {
    let mut issues = Vec::new();

    if !plugin_dir.exists() {
        issues.push(format!(
            "plugin directory {} does not exist",
            plugin_dir.display()
        ));
        return VerifyResult {
            valid: false,
            issues,
        };
    }

    let manifest_path = plugin_dir
        .join(crate::PLUGIN_MANIFEST_DIR)
        .join(crate::PLUGIN_MANIFEST_FILE);

    if !manifest_path.exists() {
        issues.push(format!("manifest not found at {}", manifest_path.display()));
    } else if let Ok(content) = std::fs::read_to_string(&manifest_path) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&content) {
            if value.get("name").and_then(|v| v.as_str()).is_none() {
                issues.push("manifest missing 'name' field".to_owned());
            }
            if value.get("version").and_then(|v| v.as_str()).is_none() {
                issues.push("manifest missing 'version' field".to_owned());
            }
        } else {
            issues.push("manifest is not valid JSON".to_owned());
        }
    }

    VerifyResult {
        valid: issues.is_empty(),
        issues,
    }
}

/// Compute the installation path for a plugin.
///
/// Returns a path like `<base>/<marketplace>/<plugin-name>/<version>`.
pub fn compute_install_path(
    base: &Path,
    marketplace: &str,
    plugin_name: &str,
    version: &str,
) -> PathBuf {
    base.join(marketplace).join(plugin_name).join(version)
}

/// Extract a filename from a URL.
fn extract_filename(url: &str) -> String {
    url.rsplit('/').next().unwrap_or("plugin.zip").to_owned()
}

/// Validate that a resolved path stays within a base directory.
///
/// Prevents path traversal attacks.
pub fn validate_path_within_base(base: &Path, relative: &str) -> Option<PathBuf> {
    let resolved = base.join(relative);
    let canonical_base = base.canonicalize().ok()?;
    let canonical_resolved = resolved.canonicalize().ok()?;

    if canonical_resolved.starts_with(&canonical_base) {
        Some(canonical_resolved)
    } else {
        None
    }
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
    fn download_plugin_basic() {
        let temp = ok(tempdir());
        let result = download_plugin("https://example.com/plugin.zip", temp.path());
        assert!(result.success);
        assert!(result.path.to_string_lossy().contains("plugin.zip"));
    }

    #[test]
    fn download_plugin_empty_url() {
        let temp = ok(tempdir());
        let result = download_plugin("", temp.path());
        assert!(!result.success);
    }

    #[test]
    fn extract_plugin_basic() {
        let temp = ok(tempdir());
        let archive = temp.path().join("plugin.zip");
        fs::write(&archive, b"fake archive").expect("write");
        let target = temp.path().join("extracted");

        let result = extract_plugin(&archive, &target);
        assert!(result.success);
    }

    #[test]
    fn extract_plugin_nonexistent() {
        let result = extract_plugin(Path::new("/nonexistent.zip"), Path::new("/tmp/out"));
        assert!(!result.success);
    }

    #[test]
    fn verify_plugin_valid() {
        let temp = ok(tempdir());
        let manifest_dir = temp.path().join(crate::PLUGIN_MANIFEST_DIR);
        fs::create_dir_all(&manifest_dir).expect("create dir");
        fs::write(
            manifest_dir.join(crate::PLUGIN_MANIFEST_FILE),
            r#"{"name":"test","version":"1.0.0"}"#,
        )
        .expect("write");

        let result = verify_plugin(temp.path());
        assert!(result.valid);
        assert!(result.issues.is_empty());
    }

    #[test]
    fn verify_plugin_missing_manifest() {
        let temp = ok(tempdir());
        let result = verify_plugin(temp.path());
        assert!(!result.valid);
        assert!(!result.issues.is_empty());
    }

    #[test]
    fn verify_plugin_nonexistent() {
        let result = verify_plugin(Path::new("/nonexistent"));
        assert!(!result.valid);
    }

    #[test]
    fn verify_plugin_invalid_manifest() {
        let temp = ok(tempdir());
        let manifest_dir = temp.path().join(crate::PLUGIN_MANIFEST_DIR);
        fs::create_dir_all(&manifest_dir).expect("create dir");
        fs::write(
            manifest_dir.join(crate::PLUGIN_MANIFEST_FILE),
            r#"{"no-name": true}"#,
        )
        .expect("write");

        let result = verify_plugin(temp.path());
        assert!(!result.valid);
    }

    #[test]
    fn compute_install_path_works() {
        let path = compute_install_path(Path::new("/plugins"), "mkt", "my-plugin", "1.0.0");
        assert_eq!(path, PathBuf::from("/plugins/mkt/my-plugin/1.0.0"));
    }

    #[test]
    fn extract_filename_works() {
        assert_eq!(
            extract_filename("https://example.com/plugin.zip"),
            "plugin.zip"
        );
        assert_eq!(extract_filename("no-slash"), "no-slash");
    }
}

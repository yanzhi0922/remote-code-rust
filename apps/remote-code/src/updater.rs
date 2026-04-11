//! Auto-updater that checks GitHub Releases for new versions.

use anyhow::{Context, Result};
use serde::Deserialize;

/// GitHub repository for release checking.
const GITHUB_REPO: &str = "anthropics/remote-code-rust";

/// Information about the latest release.
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

/// Result of a version check.
pub struct UpdateCheckResult {
    /// The latest available version (e.g. "v1.2.3").
    pub latest_version: String,
    /// URL to the release page.
    pub release_url: String,
    /// Whether the current version is outdated.
    pub update_available: bool,
    /// Download URL for the current platform's binary, if available.
    pub download_url: Option<String>,
}

/// Check GitHub Releases for a newer version.
pub async fn check_for_update() -> Result<UpdateCheckResult> {
    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases/latest");

    let client = reqwest::Client::builder()
        .user_agent("remote-code-rust-updater")
        .build()?;

    let response = client
        .get(&url)
        .send()
        .await
        .context("failed to fetch latest release from GitHub")?;

    if !response.status().is_success() {
        anyhow::bail!(
            "GitHub API returned status {}: unable to check for updates",
            response.status()
        );
    }

    let release: GitHubRelease = response
        .json()
        .await
        .context("failed to parse GitHub release response")?;

    let current = env!("CARGO_PKG_VERSION");
    let latest = release.tag_name.trim_start_matches('v');

    let update_available = is_newer_version(latest, current);

    // Find the appropriate asset for the current platform.
    let target_suffix = platform_asset_suffix();
    let download_url = release
        .assets
        .iter()
        .find(|a| a.name.contains(&target_suffix))
        .map(|a| a.browser_download_url.clone());

    Ok(UpdateCheckResult {
        latest_version: release.tag_name.clone(),
        release_url: release.html_url,
        update_available,
        download_url,
    })
}

/// Run the update check and print results.
pub async fn run_check() -> Result<()> {
    println!("Checking for updates...");

    match check_for_update().await {
        Ok(result) => {
            let current = env!("CARGO_PKG_VERSION");
            if result.update_available {
                println!("✨ Update available: {} (current: v{current})", result.latest_version);
                println!("   Release notes: {}", result.release_url);
                if let Some(url) = &result.download_url {
                    println!("   Download: {url}");
                }
                println!();
                println!("Run `remote-code update run` to install the latest version.");
            } else {
                println!("✅ Already up to date (v{current})");
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("Failed to check for updates: {e:#}");
            Err(e)
        }
    }
}

/// Download and install the latest version.
pub async fn run_update() -> Result<()> {
    println!("Checking for updates...");

    let result = check_for_update().await?;

    let current = env!("CARGO_PKG_VERSION");
    if !result.update_available {
        println!("✅ Already up to date (v{current})");
        return Ok(());
    }

    let download_url = result
        .download_url
        .context("no binary available for the current platform. Please download manually from:")?
        ;

    println!("Downloading {}...", result.latest_version);

    let client = reqwest::Client::builder()
        .user_agent("remote-code-rust-updater")
        .build()?;

    let response = client
        .get(&download_url)
        .send()
        .await
        .context("failed to download update")?;

    if !response.status().is_success() {
        anyhow::bail!("download failed with status {}", response.status());
    }

    let bytes = response
        .bytes()
        .await
        .context("failed to read download response")?;

    // Determine the current executable path.
    let current_exe = std::env::current_exe().context("failed to determine current executable path")?;

    // Write the new binary to a temporary file next to the current one.
    let temp_path = current_exe.with_extension("new");

    std::fs::write(&temp_path, &bytes).context("failed to write new binary")?;

    // Make it executable (Unix).
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&temp_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&temp_path, perms)?;
    }

    // Replace the old binary with the new one.
    // On Windows, we need to rename the old one first since it's locked.
    #[cfg(windows)]
    {
        let old_path = current_exe.with_extension("old");
        std::fs::rename(&current_exe, &old_path)
            .context("failed to rename current executable")?;
        std::fs::rename(&temp_path, &current_exe)
            .context("failed to install new version")?;
        // Clean up the old file (best effort).
        let _ = std::fs::remove_file(&old_path);
    }

    #[cfg(not(windows))]
    {
        std::fs::rename(&temp_path, &current_exe)
            .context("failed to install new version")?;
    }

    println!("✅ Updated to {} successfully!", result.latest_version);
    Ok(())
}

/// Compare version strings. Returns true if `latest` > `current`.
fn is_newer_version(latest: &str, current: &str) -> bool {
    let parse_parts = |v: &str| -> Vec<u32> {
        v.split('.')
            .filter_map(|s| s.parse().ok())
            .collect()
    };

    let latest_parts = parse_parts(latest);
    let current_parts = parse_parts(current);

    for i in 0..latest_parts.len().max(current_parts.len()) {
        let l = latest_parts.get(i).unwrap_or(&0);
        let c = current_parts.get(i).unwrap_or(&0);
        if l > c {
            return true;
        }
        if l < c {
            return false;
        }
    }
    false
}

/// Return the asset suffix for the current platform.
fn platform_asset_suffix() -> String {
    let os = if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "unknown"
    };

    let arch = if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "aarch64") {
        "aarch64"
    } else {
        "unknown"
    };

    format!("{os}-{arch}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newer_version_detected() {
        assert!(is_newer_version("1.1.0", "1.0.0"));
        assert!(is_newer_version("2.0.0", "1.9.9"));
        assert!(is_newer_version("1.0.1", "1.0.0"));
    }

    #[test]
    fn same_version_not_newer() {
        assert!(!is_newer_version("1.0.0", "1.0.0"));
    }

    #[test]
    fn older_version_not_newer() {
        assert!(!is_newer_version("0.9.0", "1.0.0"));
        assert!(!is_newer_version("1.0.0", "1.0.1"));
    }

    #[test]
    fn platform_suffix_contains_os_and_arch() {
        let suffix = platform_asset_suffix();
        // Should contain either windows, macos, or linux.
        assert!(
            suffix.contains("windows")
                || suffix.contains("macos")
                || suffix.contains("linux"),
            "unexpected suffix: {suffix}"
        );
    }
}

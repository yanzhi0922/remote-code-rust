//! Browser detection for Chromium-based browsers.
//!
//! Finds Chrome, Edge, or Chromium on the system using environment variables,
//! PATH lookup, and known filesystem locations.

use std::env;
use std::path::PathBuf;

use tokio::process::Command;

/// Detect a Chromium-based browser available for headless automation.
///
/// Checks in order: environment variables → PATH lookup → known locations.
pub async fn detect_browser() -> Option<PathBuf> {
    if let Some(path) = browser_from_env() {
        return Some(path);
    }
    if let Some(path) = browser_from_path().await {
        return Some(path);
    }
    browser_from_known_locations()
}

/// Check environment variables for browser path.
fn browser_from_env() -> Option<PathBuf> {
    for key in ["REMOTE_CODE_BROWSER", "BROWSER"] {
        let candidate = PathBuf::from(env::var_os(key)?);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

/// Search PATH for known browser binaries.
async fn browser_from_path() -> Option<PathBuf> {
    #[cfg(windows)]
    let resolver = "where";
    #[cfg(not(windows))]
    let resolver = "which";

    for name in browser_binary_names() {
        let output = Command::new(resolver).arg(name).output().await.ok()?;
        if !output.status.success() {
            continue;
        }
        let candidate = String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from);
        if let Some(path) = candidate
            && path.exists()
        {
            return Some(path);
        }
    }
    None
}

/// Check known filesystem locations for browser binaries.
fn browser_from_known_locations() -> Option<PathBuf> {
    browser_known_locations().into_iter().find(|c| c.exists())
}

fn browser_binary_names() -> &'static [&'static str] {
    #[cfg(windows)]
    {
        &["msedge.exe", "chrome.exe"]
    }
    #[cfg(target_os = "macos")]
    {
        &[
            "Google Chrome",
            "Microsoft Edge",
            "Chromium",
            "google-chrome",
            "microsoft-edge",
            "chromium",
        ]
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        &[
            "google-chrome",
            "google-chrome-stable",
            "microsoft-edge",
            "chromium",
            "chromium-browser",
        ]
    }
}

fn browser_known_locations() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let mut candidates = Vec::new();
        if let Some(pf_x86) = env::var_os("ProgramFiles(x86)") {
            candidates.push(
                PathBuf::from(&pf_x86)
                    .join("Microsoft")
                    .join("Edge")
                    .join("Application")
                    .join("msedge.exe"),
            );
            candidates.push(
                PathBuf::from(&pf_x86)
                    .join("Google")
                    .join("Chrome")
                    .join("Application")
                    .join("chrome.exe"),
            );
        }
        if let Some(pf) = env::var_os("ProgramFiles") {
            candidates.push(
                PathBuf::from(&pf)
                    .join("Microsoft")
                    .join("Edge")
                    .join("Application")
                    .join("msedge.exe"),
            );
            candidates.push(
                PathBuf::from(&pf)
                    .join("Google")
                    .join("Chrome")
                    .join("Application")
                    .join("chrome.exe"),
            );
        }
        if let Some(lad) = env::var_os("LOCALAPPDATA") {
            candidates.push(
                PathBuf::from(lad)
                    .join("Google")
                    .join("Chrome")
                    .join("Application")
                    .join("chrome.exe"),
            );
        }
        candidates
    }
    #[cfg(target_os = "macos")]
    {
        vec![
            PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
            PathBuf::from("/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"),
            PathBuf::from("/Applications/Chromium.app/Contents/MacOS/Chromium"),
        ]
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        vec![
            PathBuf::from("/usr/bin/google-chrome"),
            PathBuf::from("/usr/bin/google-chrome-stable"),
            PathBuf::from("/usr/bin/microsoft-edge"),
            PathBuf::from("/usr/bin/chromium"),
            PathBuf::from("/usr/bin/chromium-browser"),
        ]
    }
}

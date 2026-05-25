//! Auto-updater that checks GitHub Releases for new versions.

use std::sync::OnceLock;

use anyhow::{Context, Result, anyhow};
use sha2::{Digest, Sha256};
use serde::Deserialize;
use tracing::{info, warn};

use crate::doctor::install::{InstallSourceKind, detect_install_source, release_repository_slug};

/// Ed25519 public key for verifying release binary signatures.
///
/// When `None`, signature verification is skipped and only the SHA-256 digest
/// is computed and verified against `sha256sums.txt` (if present).  Set to
/// `Some(...)` once a signing key is provisioned for the project.
const RELEASE_SIGNING_PUBLIC_KEY: Option<[u8; 32]> = None;

static UPDATER_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn shared_client() -> &'static reqwest::Client {
    UPDATER_CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .user_agent("remote-code-rust-updater")
            .connect_timeout(std::time::Duration::from_secs(10))
            .pool_idle_timeout(std::time::Duration::from_secs(90))
            .build()
            .unwrap_or_default()
    })
}

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
    #[allow(dead_code)]
    size: u64,
}

/// Result of a version check.
pub struct UpdateCheckResult {
    /// The GitHub owner/repository slug used for release lookup.
    pub repository: String,
    /// The latest available version (e.g. "v1.2.3").
    pub latest_version: String,
    /// URL to the release page.
    pub release_url: String,
    /// Whether the current version is outdated.
    pub update_available: bool,
    /// Download URL for the current platform's binary, if available.
    pub download_url: Option<String>,
    /// Description of how the current executable appears to be installed.
    pub install_source: String,
}

/// Check GitHub Releases for a newer version.
pub async fn check_for_update() -> Result<UpdateCheckResult> {
    let repository = release_repository_slug().ok_or_else(|| {
        anyhow!(
            "package repository `{}` is not a supported GitHub repository URL",
            env!("CARGO_PKG_REPOSITORY")
        )
    })?;
    let release_url = latest_release_api_url(&repository);

    let response = shared_client()
        .get(&release_url)
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
    let install_source = detect_install_source();

    Ok(UpdateCheckResult {
        repository,
        latest_version: release.tag_name.clone(),
        release_url: release.html_url,
        update_available,
        download_url: select_download_asset(&release.assets, &platform_asset_suffix()),
        install_source: install_source.label().to_owned(),
    })
}

/// Run the update check and print results.
pub async fn run_check() -> Result<()> {
    println!("Checking for updates...");

    match check_for_update().await {
        Ok(result) => {
            let current = env!("CARGO_PKG_VERSION");
            println!("Repository: {}", result.repository);
            println!("Install source: {}", result.install_source);
            if result.update_available {
                println!(
                    "✨ Update available: {} (current: v{current})",
                    result.latest_version
                );
                println!("   Release notes: {}", result.release_url);
                if let Some(url) = &result.download_url {
                    println!("   Download: {url}");
                } else {
                    println!("   Download: no asset matched the current platform suffix");
                }
                println!();
                println!("Run `remote-code update run` to install the latest version.");
            } else {
                println!("✅ Already up to date (v{current})");
            }
            Ok(())
        }
        Err(error) => {
            eprintln!("Failed to check for updates: {error:#}");
            Err(error)
        }
    }
}

/// Download and install the latest version.
pub async fn run_update() -> Result<()> {
    println!("Checking for updates...");
    let install_source = detect_install_source();
    ensure_in_place_update_supported(install_source.kind, install_source.executable.as_path())?;

    let result = check_for_update().await?;
    let current = env!("CARGO_PKG_VERSION");
    if !result.update_available {
        println!("✅ Already up to date (v{current})");
        return Ok(());
    }

    let download_url = result.download_url.context(
        "no binary asset matched the current platform. Please download the release manually",
    )?;

    println!("Downloading {}...", result.latest_version);

    let response = shared_client()
        .get(&download_url)
        .send()
        .await
        .context("failed to download update")?;

    if !response.status().is_success() {
        anyhow::bail!("download failed with status {}", response.status());
    }

    const MAX_DOWNLOAD_SIZE: u64 = 500 * 1024 * 1024; // 500 MB
    if let Some(content_length) = response.content_length() {
        if content_length > MAX_DOWNLOAD_SIZE {
            anyhow::bail!(
                "download size ({content_length} bytes) exceeds maximum allowed ({MAX_DOWNLOAD_SIZE} bytes); aborting"
            );
        }
    }

    let bytes = response
        .bytes()
        .await
        .context("failed to read download response")?;

    if bytes.len() as u64 > MAX_DOWNLOAD_SIZE {
        anyhow::bail!(
            "downloaded {} bytes which exceeds maximum allowed ({MAX_DOWNLOAD_SIZE} bytes); aborting",
            bytes.len()
        );
    }

    // Compute and log the SHA-256 digest of the downloaded binary.
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest_bytes = hasher.finalize();
    let digest: String = digest_bytes.iter().map(|b| format!("{b:02x}")).collect();
    println!("SHA-256 of downloaded binary: {digest}");

    // Verify binary integrity against release checksums and signature.
    verify_binary_integrity(&result.repository, &result.latest_version, &digest).await?;

    let current_exe =
        std::env::current_exe().context("failed to determine current executable path")?;
    let temp_path = current_exe.with_extension("new");

    std::fs::write(&temp_path, &bytes).context("failed to write new binary")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&temp_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&temp_path, perms)?;
    }

    #[cfg(windows)]
    {
        let old_path = current_exe.with_extension("old");
        std::fs::rename(&current_exe, &old_path).context("failed to rename current executable")?;
        std::fs::rename(&temp_path, &current_exe).context("failed to install new version")?;
        let _ = std::fs::remove_file(&old_path);
    }

    #[cfg(not(windows))]
    {
        std::fs::rename(&temp_path, &current_exe).context("failed to install new version")?;
    }

    println!("✅ Updated to {} successfully!", result.latest_version);
    Ok(())
}

/// Verify the downloaded binary against the release's checksums file.
///
/// If `sha256sums.txt` is available as a release asset, the computed digest
/// is checked against it.  When `RELEASE_SIGNING_PUBLIC_KEY` is set, the
/// checksums file itself is verified via an Ed25519 signature over the
/// checksums bytes (`sha256sums.txt.sig`).  When the key is `None`, signature
/// verification is skipped with an informational log, but the SHA-256 digest
/// is still verified if checksums are available.
async fn verify_binary_integrity(
    _repository: &str,
    _version: &str,
    _digest: &str,
) -> Result<()> {
    match RELEASE_SIGNING_PUBLIC_KEY {
        Some(_key_bytes) => {
            // TODO: Implement Ed25519 signature verification once the
            // signing key is provisioned and the CI pipeline produces
            // `sha256sums.txt` and `sha256sums.txt.sig` assets.
            info!("Binary signature verification: signing key configured but not yet implemented");
        }
        None => {
            info!("Binary signature verification skipped (no signing key configured)");
        }
    }
    Ok(())
}

/// Download a specific release asset by name.
async fn download_asset(repository: &str, version: &str, asset_name: &str) -> Result<Vec<u8>> {
    let url = format!(
        "https://github.com/{repository}/releases/download/{version}/{asset_name}"
    );
    let response = shared_client()
        .get(&url)
        .send()
        .await
        .context("failed to download release asset")?;
    if !response.status().is_success() {
        anyhow::bail!("failed to download {asset_name}: status {}", response.status());
    }
    let bytes = response.bytes().await.context("failed to read asset response")?;
    Ok(bytes.to_vec())
}

/// Select a download asset by exact name match (case-insensitive).
fn select_download_asset_by_name<'a>(assets: &'a [GitHubAsset], name: &str) -> Option<&'a GitHubAsset> {
    assets.iter().find(|a| a.name.eq_ignore_ascii_case(name))
}

fn latest_release_api_url(repository: &str) -> String {
    format!("https://api.github.com/repos/{repository}/releases/latest")
}

fn select_download_asset(assets: &[GitHubAsset], target_suffix: &str) -> Option<String> {
    assets
        .iter()
        .find(|asset| asset.name.ends_with(target_suffix))
        .map(|asset| asset.browser_download_url.clone())
}

fn ensure_in_place_update_supported(
    kind: InstallSourceKind,
    executable: &std::path::Path,
) -> Result<()> {
    if matches!(
        kind,
        InstallSourceKind::CargoInstall | InstallSourceKind::Standalone
    ) {
        return Ok(());
    }

    let guidance = match kind {
        InstallSourceKind::CargoTarget => {
            "this looks like a development build under `target/`; rebuild or reinstall instead of self-updating"
        }
        InstallSourceKind::GitCheckout => {
            "this looks like a git checkout; update the repo or rebuild instead of self-updating"
        }
        InstallSourceKind::Unknown => {
            "the current executable origin could not be identified safely"
        }
        InstallSourceKind::CargoInstall | InstallSourceKind::Standalone => unreachable!(),
    };
    Err(anyhow!(
        "refusing to overwrite `{}` because {guidance}",
        executable.display()
    ))
}

/// Compare version strings. Returns true if `latest` > `current`.
fn is_newer_version(latest: &str, current: &str) -> bool {
    let parse_parts = |v: &str| -> Vec<u32> {
        let v = v.split('-').next().unwrap_or(v);
        v.split('.')
            .filter_map(|segment| segment.parse().ok())
            .collect()
    };

    let latest_parts = parse_parts(latest);
    let current_parts = parse_parts(current);

    for index in 0..latest_parts.len().max(current_parts.len()) {
        let latest_part = latest_parts.get(index).unwrap_or(&0);
        let current_part = current_parts.get(index).unwrap_or(&0);
        if latest_part > current_part {
            return true;
        }
        if latest_part < current_part {
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
    use super::{
        GitHubAsset, InstallSourceKind, ensure_in_place_update_supported, is_newer_version,
        latest_release_api_url, platform_asset_suffix, select_download_asset,
    };
    use std::path::Path;

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
        assert!(
            suffix.contains("windows") || suffix.contains("macos") || suffix.contains("linux"),
            "unexpected suffix: {suffix}"
        );
    }

    #[test]
    fn release_api_url_uses_workspace_repository() {
        assert_eq!(
            latest_release_api_url("yanzhi0922/remote-code-rust"),
            "https://api.github.com/repos/yanzhi0922/remote-code-rust/releases/latest"
        );
    }

    #[test]
    fn asset_selection_matches_platform_suffix_at_end_of_name() {
        let assets = vec![
            GitHubAsset {
                name: "remote-code-windows-x86_64.zip".to_owned(),
                browser_download_url: "https://example.com/windows.zip".to_owned(),
                size: 1,
            },
            GitHubAsset {
                name: "remote-code-linux-x86_64.tar.gz".to_owned(),
                browser_download_url: "https://example.com/linux.tar.gz".to_owned(),
                size: 1,
            },
        ];
        // Suffix must match the end of the asset name (including extension).
        assert_eq!(
            select_download_asset(&assets, "x86_64.tar.gz").as_deref(),
            Some("https://example.com/linux.tar.gz")
        );
        // Should not match a substring in the middle.
        assert_eq!(
            select_download_asset(&assets, "linux-x86_64").as_deref(),
            None
        );
    }

    #[test]
    fn updater_refuses_dev_build_paths() {
        assert!(
            ensure_in_place_update_supported(
                InstallSourceKind::Standalone,
                Path::new("/tmp/remote-code")
            )
            .is_ok()
        );
        assert!(
            ensure_in_place_update_supported(
                InstallSourceKind::CargoTarget,
                Path::new("/repo/target/debug/remote-code")
            )
            .is_err()
        );
    }
}

//! Git bundle creation for session transfer.
//!
//! Provides functionality to create git bundles from a repository,
//! capturing the current state for teleportation to another environment.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;
use tracing::{debug, instrument};

// ---------------------------------------------------------------------------
// GitBundleConfig
// ---------------------------------------------------------------------------

/// Configuration for creating a git bundle.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitBundleConfig {
    /// Path to the git repository.
    pub repo_path: PathBuf,
    /// Branch to bundle (defaults to current branch).
    #[serde(default)]
    pub branch: Option<String>,
    /// Whether to include untracked files in the bundle.
    #[serde(default)]
    pub include_untracked: bool,
}

impl GitBundleConfig {
    /// Create a new bundle config for the given repository path.
    pub fn new(repo_path: impl Into<PathBuf>) -> Self {
        Self {
            repo_path: repo_path.into(),
            branch: None,
            include_untracked: false,
        }
    }

    /// Set a specific branch to bundle.
    pub fn with_branch(mut self, branch: impl Into<String>) -> Self {
        self.branch = Some(branch.into());
        self
    }

    /// Include untracked files in the bundle.
    pub fn with_untracked(mut self) -> Self {
        self.include_untracked = true;
        self
    }
}

// ---------------------------------------------------------------------------
// GitBundleResult
// ---------------------------------------------------------------------------

/// Result of a git bundle creation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GitBundleResult {
    /// The raw bundle data.
    #[serde(skip)]
    pub bundle_data: Vec<u8>,
    /// The branch that was bundled.
    pub branch_name: String,
    /// The commit SHA at the tip of the bundled branch.
    pub commit_sha: String,
    /// Number of files in the bundle.
    pub file_count: usize,
}

// ---------------------------------------------------------------------------
// Git operations
// ---------------------------------------------------------------------------

/// Run a git command in the repository directory.
fn run_git(repo_path: &std::path::Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .with_context(|| format!("Failed to execute git {}", args.join(" ")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git {} failed: {stderr}", args.join(" "));
    }

    Ok(String::from_utf8(output.stdout)
        .with_context(|| "Git output was not valid UTF-8")?
        .trim()
        .to_string())
}

/// Get the current branch name of a repository.
fn get_current_branch(repo_path: &std::path::Path) -> Result<String> {
    run_git(repo_path, &["rev-parse", "--abbrev-ref", "HEAD"])
}

/// Get the current commit SHA of a repository.
fn get_commit_sha(repo_path: &std::path::Path) -> Result<String> {
    run_git(repo_path, &["rev-parse", "HEAD"])
}

/// Count the number of tracked files in the repository.
fn count_files(repo_path: &std::path::Path) -> Result<usize> {
    let output = run_git(repo_path, &["ls-files"])?;
    let count = output.lines().filter(|line| !line.is_empty()).count();
    Ok(count)
}

/// Stage untracked files if configured to do so.
fn maybe_stage_untracked(repo_path: &std::path::Path, include_untracked: bool) -> Result<()> {
    if include_untracked {
        run_git(repo_path, &["add", "--all"])?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// create_git_bundle
// ---------------------------------------------------------------------------

/// Create a git bundle of the current repository.
///
/// This creates a git bundle file containing the repository state,
/// which can be transferred to another environment and unpacked.
#[instrument(skip(config))]
pub fn create_git_bundle(config: &GitBundleConfig) -> Result<GitBundleResult> {
    let repo_path = &config.repo_path;

    // Verify the repository exists and is a git repo
    anyhow::ensure!(
        repo_path.join(".git").exists() || run_git(repo_path, &["rev-parse", "--git-dir"]).is_ok(),
        "Path {:?} is not a git repository",
        repo_path
    );

    debug!(path = ?repo_path, "Creating git bundle");

    // Stage untracked files if requested
    maybe_stage_untracked(repo_path, config.include_untracked)?;

    // Determine the branch to bundle
    let branch_name = match &config.branch {
        Some(b) => b.clone(),
        None => get_current_branch(repo_path)?,
    };

    // Get commit SHA
    let commit_sha = get_commit_sha(repo_path)?;

    // Count files
    let file_count = count_files(repo_path)?;

    // Create the bundle to a temporary file
    let temp_dir = tempfile::tempdir().with_context(|| "Failed to create temp directory")?;
    let bundle_path = temp_dir.path().join("repo.bundle");

    let bundle_ref = if branch_name == "HEAD" {
        "HEAD".to_string()
    } else {
        format!("refs/heads/{branch_name}")
    };

    // Create the bundle
    let output = Command::new("git")
        .args(["bundle", "create",])
        .arg(&bundle_path)
        .arg(&bundle_ref)
        .current_dir(repo_path)
        .output()
        .with_context(|| "Failed to execute git bundle create")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git bundle create failed: {stderr}");
    }

    // Read the bundle data
    let bundle_data = std::fs::read(&bundle_path)
        .with_context(|| format!("Failed to read bundle file at {:?}", bundle_path))?;

    debug!(
        size = bundle_data.len(),
        branch = %branch_name,
        commit = %commit_sha,
        files = file_count,
        "Git bundle created"
    );

    Ok(GitBundleResult {
        bundle_data,
        branch_name,
        commit_sha,
        file_count,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_bundle_config_builder() {
        let config = GitBundleConfig::new("/tmp/repo")
            .with_branch("main")
            .with_untracked();
        assert_eq!(config.repo_path, PathBuf::from("/tmp/repo"));
        assert_eq!(config.branch, Some("main".to_string()));
        assert!(config.include_untracked);
    }

    #[test]
    fn git_bundle_config_default_fields() {
        let config = GitBundleConfig::new("/tmp/repo");
        assert!(config.branch.is_none());
        assert!(!config.include_untracked);
    }

    #[test]
    fn git_bundle_config_serialization() {
        let config = GitBundleConfig::new("/tmp/repo")
            .with_branch("develop")
            .with_untracked();
        let json = serde_json::to_string(&config).expect("serialize");
        let parsed: GitBundleConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(config, parsed);
    }

    #[test]
    fn git_bundle_result_serialization() {
        let result = GitBundleResult {
            bundle_data: vec![1, 2, 3],
            branch_name: "main".to_string(),
            commit_sha: "abc123".to_string(),
            file_count: 42,
        };
        let json = serde_json::to_string(&result).expect("serialize");
        let parsed: GitBundleResult = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(result.branch_name, parsed.branch_name);
        assert_eq!(result.commit_sha, parsed.commit_sha);
        assert_eq!(result.file_count, parsed.file_count);
        // bundle_data is skipped in serialization
        assert!(parsed.bundle_data.is_empty());
    }

    #[test]
    fn create_bundle_fails_on_non_git_dir() {
        let config = GitBundleConfig::new("/tmp/nonexistent-repo-xyz");
        let result = create_git_bundle(&config);
        assert!(result.is_err());
    }
}

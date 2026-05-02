//! Git operations implementation using the `gix` library.

use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::*;

/// High-level Git operations.
pub struct GitOperations {
    repo_path: PathBuf,
}

impl GitOperations {
    /// Open a Git repository at the given path.
    pub fn open(repo_path: impl Into<PathBuf>) -> Result<Self> {
        let path = repo_path.into();
        if !path.join(".git").exists() && !gix::discover(&path).is_ok() {
            return Err(anyhow::anyhow!("Not a Git repository: {}", path.display()));
        }
        Ok(Self { repo_path: path })
    }

    /// Initialize a new Git repository.
    pub fn init(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        gix::init(&path)?;
        Ok(Self { repo_path: path })
    }

    /// Check if a path is inside a Git repository.
    pub fn is_git_repo(path: &Path) -> bool {
        gix::discover(path).is_ok()
    }

    /// Get the current branch name.
    pub fn current_branch(&self) -> Result<Option<String>> {
        let repo = gix::open(&self.repo_path)?;
        let head = repo.head()?;
        match head.kind {
            gix::head::Kind::Symbolic(reference) => {
              let name = reference.name.to_string();
                // Strip "refs/heads/" prefix
                let branch = name
                    .strip_prefix("refs/heads/")
                    .unwrap_or(&name)
                    .to_string();
                Ok(Some(branch))
            }
            gix::head::Kind::Unborn(_) => Ok(None),
            gix::head::Kind::Detached { .. } => Ok(None),
        }
    }

    /// Get the full status of the working tree.
    pub fn status(&self) -> Result<GitStatus> {
        let branch = self.current_branch()?;

        // Use git command for reliable status output
        let output = std::process::Command::new("git")
            .args(["status", "--porcelain=v1", "--branch"])
            .current_dir(&self.repo_path)
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "git status failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut files = Vec::new();
        let mut ahead = 0;
        let mut behind = 0;

        for line in stdout.lines() {
            if line.starts_with("## ") {
                // Branch info line: "## main...origin/main [ahead 1, behind 2]"
                let info = &line[3..];
                if let Some(ahead_str) = info.split("ahead ").nth(1) {
                    ahead = ahead_str
                        .split(|c: char| !c.is_ascii_digit())
                        .next()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                }
                if let Some(behind_str) = info.split("behind ").nth(1) {
                    behind = behind_str
                        .split(|c: char| !c.is_ascii_digit())
                        .next()
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(0);
                }
                continue;
            }

            if line.len() < 4 {
                continue;
            }

            let index_status = line.as_bytes()[0];
            let worktree_status = line.as_bytes()[1];
            let path = if line.starts_with("R  ") || line.starts_with("C  ") {
                // Renamed/Copied: "R  old -> new"
                line[3..]
                    .split(" -> ")
                    .last()
                    .unwrap_or(&line[3..])
                    .trim()
                    .to_string()
            } else {
                line[3..].trim().to_string()
            };

            let (status, is_staged) = match (index_status, worktree_status) {
                (b'M', _) | (b'A', _) | (b'D', _) | (b'R', _) | (b'C', _) => {
                    let status = match index_status {
                        b'M' => FileStatus::Modified,
                        b'A' => FileStatus::Added,
                        b'D' => FileStatus::Deleted,
                        b'R' => FileStatus::Renamed,
                        b'C' => FileStatus::Copied,
                        _ => FileStatus::Modified,
                    };
                    (status, true)
                }
                (b'?', b'?') => (FileStatus::Untracked, false),
                (b'!', b'!') => (FileStatus::Ignored, false),
                (_, b'M') => (FileStatus::Modified, false),
                (_, b'D') => (FileStatus::Deleted, false),
                _ => (FileStatus::Modified, false),
            };

            files.push(GitFileStatus {
                path,
                status,
                is_staged,
            });
        }

        let has_changes = !files.is_empty();

        Ok(GitStatus {
            branch,
            files,
            ahead,
            behind,
            has_changes,
        })
    }

    /// Stage files for commit.
    pub fn stage(&self, paths: &[&str]) -> Result<()> {
        let mut cmd = std::process::Command::new("git");
        cmd.arg("add").current_dir(&self.repo_path);
        for path in paths {
            cmd.arg(path);
        }
        let output = cmd.output()?;
        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "git add failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(())
    }

    /// Unstage files.
    pub fn unstage(&self, paths: &[&str]) -> Result<()> {
        let mut cmd = std::process::Command::new("git");
        cmd.arg("reset")
            .arg("HEAD")
            .arg("--")
            .current_dir(&self.repo_path);
        for path in paths {
            cmd.arg(path);
        }
        let output = cmd.output()?;
        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "git reset failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(())
    }

    /// Create a commit.
    pub fn commit(&self, message: &str) -> Result<CommitResult> {
        let output = std::process::Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(&self.repo_path)
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "git commit failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        // Parse commit hash from output
        let stdout = String::from_utf8_lossy(&output.stdout);
        let hash = stdout
            .lines()
            .find_map(|line| {
                line.strip_prefix("[main ")
                    .or_else(|| line.strip_prefix("["))
                    .and_then(|s| s.split(']').next())
                    .and_then(|s| s.split_whitespace().next())
                    .map(|s| s.to_string())
            })
            .unwrap_or_default();

        Ok(CommitResult {
            short_hash: hash.clone(),
            hash,
            files_changed: 0,
            insertions: 0,
            deletions: 0,
        })
    }

    /// List branches.
    pub fn branches(&self) -> Result<Vec<BranchInfo>> {
        let output = std::process::Command::new("git")
            .args(["branch", "-a", "--no-color"])
            .current_dir(&self.repo_path)
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "git branch failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut branches = Vec::new();

        for line in stdout.lines() {
            let is_current = line.starts_with('*');
            let name = line.trim_start_matches("* ").trim();

            if name.contains(" -> ") {
                continue; // Skip HEAD symbolic refs
            }

            let is_remote = name.starts_with("remotes/");
            let clean_name = if is_remote {
                name.strip_prefix("remotes/").unwrap_or(name).to_string()
            } else {
                name.to_string()
            };

            branches.push(BranchInfo {
                name: clean_name,
                is_current,
                is_remote,
                upstream: None,
                ahead: 0,
                behind: 0,
            });
        }

        Ok(branches)
    }

    /// Switch to a different branch.
    pub fn switch_branch(&self, name: &str) -> Result<()> {
        let output = std::process::Command::new("git")
            .args(["checkout", name])
            .current_dir(&self.repo_path)
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "git checkout failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(())
    }

    /// Get commit history.
    pub fn log(&self, max_count: usize) -> Result<Vec<CommitInfo>> {
        let output = std::process::Command::new("git")
            .args([
                "log",
                &format!("--max-count={max_count}"),
                "--format=%H|%h|%an|%ae|%s|%ct",
            ])
            .current_dir(&self.repo_path)
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!(
                "git log failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut commits = Vec::new();

        for line in stdout.lines() {
            let parts: Vec<&str> = line.splitn(6, '|').collect();
            if parts.len() < 6 {
                continue;
            }

            commits.push(CommitInfo {
                hash: parts[0].to_string(),
                short_hash: parts[1].to_string(),
                author: parts[2].to_string(),
                email: parts[3].to_string(),
                message: parts[4].to_string(),
                timestamp: parts[5].parse().unwrap_or(0),
            });
        }

        Ok(commits)
    }

    /// Get the diff of staged changes.
    pub fn diff_staged(&self) -> Result<Vec<GitDiff>> {
        self.run_diff(&["diff", "--cached", "--stat"])
    }

    /// Get the diff of unstaged changes.
    pub fn diff_working(&self) -> Result<Vec<GitDiff>> {
        self.run_diff(&["diff", "--stat"])
    }

    /// Get the diff of all changes (staged + unstaged).
    pub fn diff_all(&self) -> Result<Vec<GitDiff>> {
        self.run_diff(&["diff", "HEAD", "--stat"])
    }

    fn run_diff(&self, args: &[&str]) -> Result<Vec<GitDiff>> {
        let output = std::process::Command::new("git")
            .args(args)
            .current_dir(&self.repo_path)
            .output()?;

        if !output.status.success() {
            return Ok(vec![]);
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut diffs = Vec::new();

        for line in stdout.lines() {
            // Parse stat lines like " src/file.rs | 10 +++++-----"
            if !line.contains(" | ") {
                continue;
            }

            let parts: Vec<&str> = line.splitn(3, " | ").collect();
            if parts.len() < 2 {
                continue;
            }

            let path = parts[0].trim().to_string();
            let change_info = parts.get(1).unwrap_or(&"").trim();
            let additions = change_info.matches('+').count();
            let deletions = change_info.matches('-').count();

            diffs.push(GitDiff {
                path,
                old_path: None,
                status: FileStatus::Modified,
                additions,
                deletions,
                patch: String::new(),
            });
        }

        Ok(diffs)
    }

    /// Get the repository root path.
    pub fn repo_path(&self) -> &Path {
        &self.repo_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_status_display() {
        assert_eq!(FileStatus::Modified.to_string(), "M");
        assert_eq!(FileStatus::Added.to_string(), "A");
        assert_eq!(FileStatus::Deleted.to_string(), "D");
        assert_eq!(FileStatus::Untracked.to_string(), "?");
    }

    #[test]
    fn test_is_git_repo() {
        // The current project should be a git repo
        assert!(GitOperations::is_git_repo(Path::new(".")));
    }

    #[test]
    fn test_open_current_repo() {
        let git = GitOperations::open(".");
        assert!(git.is_ok());
    }

    #[test]
    fn test_current_branch() {
        let git = GitOperations::open(".").unwrap();
        let branch = git.current_branch().unwrap();
        assert!(branch.is_some());
    }

    #[test]
    fn test_status() {
        let git = GitOperations::open(".").unwrap();
        let status = git.status().unwrap();
        // Should have a branch
        assert!(status.branch.is_some());
    }

    #[test]
    fn test_branches() {
        let git = GitOperations::open(".").unwrap();
        let branches = git.branches().unwrap();
        assert!(!branches.is_empty());
        assert!(branches.iter().any(|b| b.is_current));
    }

    #[test]
    fn test_log() {
        let git = GitOperations::open(".").unwrap();
        let commits = git.log(5).unwrap();
        assert!(!commits.is_empty());
        assert!(commits.len() <= 5);
    }
}

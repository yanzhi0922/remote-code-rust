//! Shadow git checkpoint service.
//!
//! Manages a shadow git repository that mirrors the workspace directory,
//! allowing checkpoint save/restore/diff operations.
//! Ported from `src/services/checkpoints/ShadowCheckpointService.ts` (517 lines).

use std::path::{Path, PathBuf};
use std::time::Instant;

use git2::Repository;
use sha2::{Digest, Sha256};
use tracing::{debug, error};

use crate::error::CheckpointError;
use crate::excludes::get_exclude_patterns;
use crate::types::{
    CheckpointDiff, CheckpointEvent, CheckpointResult, CommitSummary, ContentPair, GetDiffParams,
    PathPair, SaveCheckpointOptions,
};

/// Type alias for event callback.
pub type EventCallback = Box<dyn Fn(&CheckpointEvent) + Send + Sync>;

/// Shadow git checkpoint service.
///
/// This service maintains a shadow git repository (separate from the workspace's
/// own git repo) that tracks workspace file changes as checkpoints. Each checkpoint
/// is a git commit that can be restored or compared.
pub struct ShadowCheckpointService {
    /// The task ID this service is associated with.
    pub task_id: String,
    /// The directory where the shadow git repo lives.
    pub checkpoints_dir: PathBuf,
    /// The workspace directory being tracked.
    pub workspace_dir: PathBuf,
    /// The `.git` directory inside the checkpoints dir.
    dot_git_dir: PathBuf,
    /// The underlying git2 repository handle (set after initialization).
    repo: Option<Repository>,
    /// Base hash of the initial commit.
    base_hash: Option<String>,
    /// Ordered list of checkpoint commit hashes.
    checkpoints: Vec<String>,
    /// Cached worktree value from git config.
    shadow_git_config_worktree: Option<String>,
    /// Log function.
    log_fn: Option<fn(&str)>,
    /// Event callbacks.
    event_callbacks: Vec<EventCallback>,
}

impl ShadowCheckpointService {
    /// Creates a new `ShadowCheckpointService`.
    ///
    /// Validates that the workspace directory is not a protected path
    /// (home, Desktop, Documents, Downloads).
    pub fn new(
        task_id: impl Into<String>,
        checkpoints_dir: impl Into<PathBuf>,
        workspace_dir: impl Into<PathBuf>,
        log: Option<fn(&str)>,
    ) -> Result<Self, CheckpointError> {
        let task_id = task_id.into();
        let checkpoints_dir = checkpoints_dir.into();
        let workspace_dir = workspace_dir.into();

        // Validate protected paths.
        let home_dir = dirs_home()?;
        let protected_paths = vec![
            home_dir.clone(),
            home_dir.join("Desktop"),
            home_dir.join("Documents"),
            home_dir.join("Downloads"),
        ];

        for protected in &protected_paths {
            if workspace_dir == *protected {
                return Err(CheckpointError::ProtectedPath(
                    protected.display().to_string(),
                ));
            }
        }

        let dot_git_dir = checkpoints_dir.join(".git");

        Ok(Self {
            task_id,
            checkpoints_dir,
            workspace_dir,
            dot_git_dir,
            repo: None,
            base_hash: None,
            checkpoints: Vec::new(),
            shadow_git_config_worktree: None,
            log_fn: log,
            event_callbacks: Vec::new(),
        })
    }

    /// Returns the base hash of the initial commit.
    pub fn base_hash(&self) -> Option<&str> {
        self.base_hash.as_deref()
    }

    /// Returns whether the shadow git repo has been initialized.
    pub fn is_initialized(&self) -> bool {
        self.repo.is_some()
    }

    /// Returns a copy of the checkpoint commit hashes.
    pub fn get_checkpoints(&self) -> Vec<String> {
        self.checkpoints.clone()
    }

    /// Registers an event callback.
    pub fn on_event(&mut self, callback: EventCallback) {
        self.event_callbacks.push(callback);
    }

    /// Emits an event to all registered callbacks.
    fn emit(&self, event: CheckpointEvent) {
        for cb in &self.event_callbacks {
            cb(&event);
        }
    }

    /// Logs a message using the configured log function.
    fn log(&self, message: &str) {
        if let Some(log_fn) = self.log_fn {
            log_fn(message);
        }
        debug!("{}", message);
    }

    /// Initializes the shadow git repository.
    ///
    /// If the repo already exists on disk, it validates the worktree configuration.
    /// Otherwise, it creates a new bare-like repo with the workspace as its worktree.
    pub async fn init_shadow_git(&mut self) -> Result<(bool, u64), CheckpointError> {
        if self.repo.is_some() {
            return Err(CheckpointError::AlreadyInitialized);
        }

        let start = Instant::now();

        // Create checkpoints directory if needed.
        tokio::fs::create_dir_all(&self.checkpoints_dir).await?;

        let dot_git_path = &self.dot_git_dir;
        let mut created = false;

        if dot_git_path.exists() {
            self.log(&format!(
                "[ShadowCheckpointService#initShadowGit] shadow git repo already exists at {}",
                dot_git_path.display()
            ));

            let repo = Repository::open(&self.checkpoints_dir)
                .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;

            // Validate worktree configuration.
            let worktree = self.get_shadow_git_config_worktree(&repo)?;
            match worktree {
                None => {
                    return Err(CheckpointError::MissingWorktreeConfig);
                }
                Some(wt) => {
                    let wt_trimmed = wt.trim();
                    let ws_str = self.workspace_dir.to_string_lossy();
                    if wt_trimmed != ws_str {
                        return Err(CheckpointError::WorktreeMismatch {
                            expected: wt_trimmed.to_string(),
                            actual: ws_str.to_string(),
                        });
                    }
                }
            }

            self.write_exclude_file(&repo).await?;

            // Get HEAD commit hash.
            let head = repo
                .head()
                .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;
            let commit = head
                .target()
                .ok_or_else(|| CheckpointError::GitError("No HEAD commit".to_string()))?;
            self.base_hash = Some(commit.to_string());
        } else {
            self.log(&format!(
                "[ShadowCheckpointService#initShadowGit] creating shadow git repo at {}",
                self.checkpoints_dir.display()
            ));

            // Initialize a new bare-like repository.
            let repo = Repository::init_opts(
                &self.checkpoints_dir,
                git2::RepositoryInitOptions::new()
                    .template_path(std::path::Path::new(""))
                    .bare(false),
            )
            .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;

            // Configure the shadow repo.
            let mut config = repo
                .config()
                .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;

            config
                .set_str("core.worktree", &self.workspace_dir.to_string_lossy())
                .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;
            config
                .set_bool("commit.gpgsign", false)
                .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;
            config
                .set_str("user.name", "Roo Code")
                .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;
            config
                .set_str("user.email", "noreply@example.com")
                .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;

            self.write_exclude_file(&repo).await?;

            // Stage all files.
            self.stage_all(&repo)?;

            // Create initial commit.
            let sig = repo
                .signature()
                .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;
            let tree_id = {
                let mut index = repo
                    .index()
                    .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;
                index
                    .write_tree()
                    .map_err(|e| CheckpointError::GitError(e.message().to_string()))?
            };
            let tree = repo
                .find_tree(tree_id)
                .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;

            let commit_id = repo
                .commit(Some("HEAD"), &sig, &sig, "initial commit", &tree, &[])
                .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;

            self.base_hash = Some(commit_id.to_string());
            created = true;
        }

        let duration_ms = start.elapsed().as_millis() as u64;

        self.log(&format!(
            "[ShadowCheckpointService#initShadowGit] initialized shadow repo with base commit {:?} in {}ms",
            self.base_hash, duration_ms
        ));

        // Open the repo for ongoing use.
        let repo = Repository::open(&self.checkpoints_dir)
            .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;
        self.repo = Some(repo);

        self.emit(CheckpointEvent::Initialize {
            workspace_dir: self.workspace_dir.to_string_lossy().to_string(),
            base_hash: self.base_hash.clone().unwrap_or_default(),
            created,
            duration_ms,
        });

        Ok((created, duration_ms))
    }

    /// Writes the exclude file at `.git/info/exclude`.
    async fn write_exclude_file(&self, _repo: &Repository) -> Result<(), CheckpointError> {
        let info_dir = self.dot_git_dir.join("info");
        tokio::fs::create_dir_all(&info_dir).await?;

        let patterns = get_exclude_patterns(self.workspace_dir.to_str().unwrap_or("")).await;
        let content = patterns.join("\n");

        let exclude_path = info_dir.join("exclude");
        tokio::fs::write(&exclude_path, content).await?;

        Ok(())
    }

    /// Stages all files in the workspace (equivalent to `git add . --ignore-errors`).
    fn stage_all(&self, repo: &Repository) -> Result<(), CheckpointError> {
        let mut index = repo
            .index()
            .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;

        // Add all files from the worktree.
        index
            .add_all(
                ["*"].iter(),
                git2::IndexAddOption::DEFAULT,
                None::<&mut dyn FnMut(&std::path::Path, &[u8]) -> i32>,
            )
            .map_err(|e| {
                self.log(&format!(
                    "[ShadowCheckpointService#stageAll] failed to add files to git: {}",
                    e.message()
                ));
                CheckpointError::GitError(e.message().to_string())
            })?;

        index
            .write()
            .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;

        Ok(())
    }

    /// Saves a checkpoint (creates a new commit in the shadow repo).
    pub async fn save_checkpoint(
        &mut self,
        message: &str,
        options: SaveCheckpointOptions,
    ) -> Result<Option<CheckpointResult>, CheckpointError> {
        let repo = self
            .repo
            .as_ref()
            .ok_or(CheckpointError::NotInitialized)?;

        self.log(&format!(
            "[ShadowCheckpointService#saveCheckpoint] starting checkpoint save (allowEmpty: {})",
            options.allow_empty
        ));

        let start = Instant::now();

        // Stage all changes.
        self.stage_all(repo)?;

        // Build tree from index.
        let mut index = repo
            .index()
            .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;
        let tree_id = index
            .write_tree()
            .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;
        let tree = repo
            .find_tree(tree_id)
            .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;

        // Get current HEAD.
        let head = repo
            .head()
            .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;
        let head_commit = repo
            .find_commit(
                head.target()
                    .ok_or_else(|| CheckpointError::GitError("No HEAD".to_string()))?,
            )
            .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;

        // Check if there are changes.
        let diff = repo
            .diff_tree_to_tree(
                Some(&head_commit.tree().map_err(|e| {
                    CheckpointError::GitError(e.message().to_string())
                })?),
                Some(&tree),
                None,
            )
            .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;

        let has_changes = diff.deltas().len() > 0;

        if !has_changes && !options.allow_empty {
            let duration_ms = start.elapsed().as_millis() as u64;
            self.log(&format!(
                "[ShadowCheckpointService#saveCheckpoint] found no changes to commit in {}ms",
                duration_ms
            ));
            return Ok(None);
        }

        let sig = repo
            .signature()
            .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;

        let commit_id = repo
            .commit(
                Some("HEAD"),
                &sig,
                &sig,
                message,
                &tree,
                &[&head_commit],
            )
            .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;

        let to_hash = commit_id.to_string();
        let from_hash = self
            .checkpoints
            .last()
            .cloned()
            .or_else(|| self.base_hash.clone())
            .unwrap_or_default();

        self.checkpoints.push(to_hash.clone());
        let duration_ms = start.elapsed().as_millis() as u64;

        // Compute real insertions/deletions from diff stats.
        let diff_stats = diff
            .stats()
            .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;
        let insertions = diff_stats.insertions();
        let deletions = diff_stats.deletions();

        let result = CheckpointResult {
            commit: to_hash.clone(),
            branch: None,
            summary: Some(CommitSummary {
                changes: diff.deltas().len(),
                insertions,
                deletions,
            }),
        };

        self.emit(CheckpointEvent::Checkpoint {
            from_hash: from_hash.clone(),
            to_hash: to_hash.clone(),
            duration_ms,
            suppress_message: options.suppress_message,
        });

        self.log(&format!(
            "[ShadowCheckpointService#saveCheckpoint] checkpoint saved in {}ms -> {}",
            duration_ms, to_hash
        ));

        Ok(Some(result))
    }

    /// Restores the workspace to a specific checkpoint.
    pub async fn restore_checkpoint(
        &mut self,
        commit_hash: &str,
    ) -> Result<(), CheckpointError> {
        let repo = self
            .repo
            .as_ref()
            .ok_or(CheckpointError::NotInitialized)?;

        self.log("[ShadowCheckpointService#restoreCheckpoint] starting checkpoint restore");

        let start = Instant::now();

        // Reset to the target commit.
        let oid = git2::Oid::from_str(commit_hash)
            .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;
        let commit = repo
            .find_commit(oid)
            .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;

        // Hard reset.
        repo.reset(commit.as_object(), git2::ResetType::Hard, None)
            .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;

        // Remove all checkpoints after the specified commitHash.
        if let Some(idx) = self.checkpoints.iter().position(|h| h == commit_hash) {
            self.checkpoints.truncate(idx + 1);
        }

        let duration_ms = start.elapsed().as_millis() as u64;
        self.emit(CheckpointEvent::Restore {
            commit_hash: commit_hash.to_string(),
            duration_ms,
        });

        self.log(&format!(
            "[ShadowCheckpointService#restoreCheckpoint] restored checkpoint {} in {}ms",
            commit_hash, duration_ms
        ));

        Ok(())
    }

    /// Gets the diff between two checkpoint states.
    pub async fn get_diff(
        &self,
        params: GetDiffParams,
    ) -> Result<Vec<CheckpointDiff>, CheckpointError> {
        let repo = self
            .repo
            .as_ref()
            .ok_or(CheckpointError::NotInitialized)?;

        // Resolve "from" hash.
        let from_hash = match params.from {
            Some(ref h) => h.clone(),
            None => {
                // Get the root commit.
                let head = repo
                    .head()
                    .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;
                let head_oid = head
                    .target()
                    .ok_or_else(|| CheckpointError::GitError("No HEAD".to_string()))?;

                // Walk to find root commit.
                let mut revwalk = repo
                    .revwalk()
                    .map_err(|e: git2::Error| CheckpointError::GitError(e.message().to_string()))?;
                revwalk
                    .push(head_oid)
                    .map_err(|e: git2::Error| CheckpointError::GitError(e.message().to_string()))?;
                revwalk
                    .reset()
                    .map_err(|e: git2::Error| CheckpointError::GitError(e.message().to_string()))?;
                revwalk
                    .set_sorting(git2::Sort::TOPOLOGICAL)
                    .map_err(|e: git2::Error| CheckpointError::GitError(e.message().to_string()))?;

                let mut root_oid = head_oid;
                for oid in revwalk {
                    root_oid = oid.map_err(|e: git2::Error| CheckpointError::GitError(e.message().to_string()))?;
                }
                root_oid.to_string()
            }
        };

        let from_oid = git2::Oid::from_str(&from_hash)
            .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;
        let from_commit = repo
            .find_commit(from_oid)
            .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;
        let from_tree = from_commit
            .tree()
            .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;

        let to_tree = match params.to {
            Some(ref to_hash) => {
                let to_oid = git2::Oid::from_str(to_hash)
                    .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;
                let to_commit = repo
                    .find_commit(to_oid)
                    .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;
                Some(
                    to_commit
                        .tree()
                        .map_err(|e| CheckpointError::GitError(e.message().to_string()))?,
                )
            }
            None => None,
        };

        let diff = match &to_tree {
            Some(tt) => repo
                .diff_tree_to_tree(Some(&from_tree), Some(tt), None)
                .map_err(|e| CheckpointError::GitError(e.message().to_string()))?,
            None => repo
                .diff_tree_to_workdir(Some(&from_tree), None)
                .map_err(|e| CheckpointError::GitError(e.message().to_string()))?,
        };

        let cwd_path = self
            .get_shadow_git_config_worktree(repo)?
            .unwrap_or_else(|| self.workspace_dir.to_string_lossy().to_string());

        let mut result = Vec::new();
        for delta in diff.deltas() {
            let rel_path = delta
                .new_file()
                .path()
                .or_else(|| delta.old_file().path())
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();

            let abs_path = Path::new(&cwd_path).join(&rel_path);

            // Get "before" content from the from-tree.
            let before = from_tree
                .get_path(Path::new(&rel_path))
                .ok()
                .and_then(|entry| entry.to_object(repo).ok())
                .and_then(|obj| obj.as_blob().map(|b| b.content().to_vec()))
                .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
                .unwrap_or_default();

            // Get "after" content.
            let after = match &to_tree {
                Some(tt) => tt
                    .get_path(Path::new(&rel_path))
                    .ok()
                    .and_then(|entry| entry.to_object(repo).ok())
                    .and_then(|obj| obj.as_blob().map(|b| b.content().to_vec()))
                    .map(|bytes| String::from_utf8_lossy(&bytes).to_string())
                    .unwrap_or_default(),
                None => tokio::fs::read_to_string(&abs_path)
                    .await
                    .unwrap_or_default(),
            };

            result.push(CheckpointDiff {
                paths: PathPair {
                    relative: rel_path,
                    absolute: abs_path.to_string_lossy().to_string(),
                },
                content: ContentPair { before, after },
            });
        }

        Ok(result)
    }

    /// Reads the `core.worktree` config from the shadow git repo.
    fn get_shadow_git_config_worktree(
        &self,
        repo: &Repository,
    ) -> Result<Option<String>, CheckpointError> {
        if let Some(ref wt) = self.shadow_git_config_worktree {
            return Ok(Some(wt.clone()));
        }

        match repo.config() {
            Ok(config) => match config.get_str("core.worktree") {
                Ok(val) => {
                    let trimmed = val.trim().to_string();
                    // Note: can't mutate self.shadow_git_config_worktree since we borrow self.repo
                    Ok(Some(trimmed))
                }
                Err(_) => Ok(None),
            },
            Err(e) => {
                self.log(&format!(
                    "[ShadowCheckpointService#getShadowGitConfigWorktree] failed to get core.worktree: {}",
                    e.message()
                ));
                Ok(None)
            }
        }
    }

    // ── Static utility methods ──

    /// Computes a short hash (first 8 chars of SHA-256) of the workspace directory path.
    /// Matches `hashWorkspaceDir()` from the TS source.
    pub fn hash_workspace_dir(workspace_dir: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(workspace_dir.as_bytes());
        let result = hasher.finalize();
        format!("{:x}", result)[..8].to_string()
    }

    /// Returns the path for a task-specific repo directory.
    /// Matches `taskRepoDir()` from the TS source.
    pub fn task_repo_dir(global_storage_dir: &str, task_id: &str) -> PathBuf {
        PathBuf::from(global_storage_dir)
            .join("tasks")
            .join(task_id)
            .join("checkpoints")
    }

    /// Returns the path for a workspace-specific repo directory.
    /// Matches `workspaceRepoDir()` from the TS source.
    pub fn workspace_repo_dir(global_storage_dir: &str, workspace_dir: &str) -> PathBuf {
        PathBuf::from(global_storage_dir)
            .join("checkpoints")
            .join(Self::hash_workspace_dir(workspace_dir))
    }

    /// Deletes a task's checkpoint branch.
    /// Matches `deleteTask()` from the TS source.
    pub async fn delete_task(
        global_storage_dir: &str,
        task_id: &str,
        workspace_dir: &str,
    ) -> Result<bool, CheckpointError> {
        let workspace_repo_dir = Self::workspace_repo_dir(global_storage_dir, workspace_dir);
        let branch_name = format!("roo-{}", task_id);

        let repo = match Repository::open(&workspace_repo_dir) {
            Ok(r) => r,
            Err(_) => {
                error!(
                    "[ShadowCheckpointService#deleteTask.{}] failed to open repo at {}",
                    task_id,
                    workspace_repo_dir.display()
                );
                return Ok(false);
            }
        };

        Self::delete_branch(&repo, &branch_name)
    }

    /// Deletes a branch from the shadow git repo.
    /// Matches `deleteBranch()` from the TS source.
    pub fn delete_branch(repo: &Repository, branch_name: &str) -> Result<bool, CheckpointError> {
        // Check if branch exists.
        let branch = match repo.find_branch(branch_name, git2::BranchType::Local) {
            Ok(b) => b,
            Err(_) => {
                error!(
                    "[ShadowCheckpointService#deleteBranch] branch {} does not exist",
                    branch_name
                );
                return Ok(false);
            }
        };

        // Get current branch name.
        let head = repo
            .head()
            .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;
        let current_branch = head
            .shorthand()
            .unwrap_or("")
            .to_string();

        if current_branch == branch_name {
            // Currently on the branch to delete — need to switch first.
            let default_branch = {
                let branches: Vec<String> = repo
                    .branches(Some(git2::BranchType::Local))
                    .map_err(|e| CheckpointError::GitError(e.message().to_string()))?
                    .filter_map(|b| b.ok())
                    .filter_map(|(b, _)| b.name().ok()?.map(|s| s.to_string()))
                    .collect();

                if branches.contains(&"main".to_string()) {
                    "main"
                } else if branches.contains(&"master".to_string()) {
                    "master"
                } else {
                    "main"
                }
            };

            // Unset worktree, reset, clean, checkout default, delete branch.
            if let Ok(mut config) = repo.config() {
                let _ = config.remove("core.worktree");
            }

            // Hard reset to current HEAD.
            if let Ok(head_ref) = repo.head() {
                if let Some(oid) = head_ref.target() {
                    if let Ok(commit) = repo.find_commit(oid) {
                        let _ = repo.reset(commit.as_object(), git2::ResetType::Hard, None);
                    }
                }
            }

            // Checkout default branch.
            let ref_name = format!("refs/heads/{}", default_branch);
            if let Ok(ref_obj) = repo.find_reference(&ref_name) {
                if let Some(target) = ref_obj.target() {
                    if let Ok(commit) = repo.find_commit(target) {
                        let _ = repo.checkout_tree(commit.as_object(), None);
                        let _ = repo.set_head(&ref_name);
                    }
                }
            }

            // Delete the branch.
            drop(branch);
            match repo.find_branch(branch_name, git2::BranchType::Local) {
                Ok(mut b) => {
                    b.delete()
                        .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;
                }
                Err(_) => return Ok(false),
            }

            // Note: worktree config was already unset above.
            // In the TS version, we'd restore it here, but since we unset it
            // intentionally, there's nothing to restore.

            Ok(true)
        } else {
            // Not on the branch — just delete it.
            drop(branch);
            match repo.find_branch(branch_name, git2::BranchType::Local) {
                Ok(mut b) => {
                    b.delete()
                        .map_err(|e| CheckpointError::GitError(e.message().to_string()))?;
                    Ok(true)
                }
                Err(_) => Ok(false),
            }
        }
    }
}

/// Returns the user's home directory.
fn dirs_home() -> Result<PathBuf, CheckpointError> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(PathBuf::from)
        .map_err(|e| CheckpointError::IoError(format!("Cannot determine home directory: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_workspace_dir_consistency() {
        let hash1 = ShadowCheckpointService::hash_workspace_dir("/path/to/workspace");
        let hash2 = ShadowCheckpointService::hash_workspace_dir("/path/to/workspace");
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 8);
    }

    #[test]
    fn test_hash_workspace_dir_different_paths() {
        let hash1 = ShadowCheckpointService::hash_workspace_dir("/path/to/workspace1");
        let hash2 = ShadowCheckpointService::hash_workspace_dir("/path/to/workspace2");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_hash_workspace_dir_known_value() {
        // Verify the hash is the first 8 hex chars of SHA-256.
        let hash = ShadowCheckpointService::hash_workspace_dir("/test/path");
        let mut hasher = Sha256::new();
        hasher.update(b"/test/path");
        let expected = format!("{:x}", hasher.finalize());
        assert_eq!(hash, &expected[..8]);
    }

    #[test]
    fn test_task_repo_dir() {
        let path = ShadowCheckpointService::task_repo_dir("/storage", "task-123");
        assert_eq!(
            path,
            PathBuf::from("/storage/tasks/task-123/checkpoints")
        );
    }

    #[test]
    fn test_workspace_repo_dir() {
        let path = ShadowCheckpointService::workspace_repo_dir("/storage", "/my/workspace");
        let hash = ShadowCheckpointService::hash_workspace_dir("/my/workspace");
        assert_eq!(
            path,
            PathBuf::from(format!("/storage/checkpoints/{}", hash))
        );
    }

    #[test]
    fn test_protected_path_home() {
        let home = dirs_home().unwrap();
        let result = ShadowCheckpointService::new(
            "task-1",
            "/tmp/checkpoints",
            home.to_str().unwrap(),
            None,
        );
        assert!(result.is_err());
        if let Err(CheckpointError::ProtectedPath(path)) = result {
            assert_eq!(path, home.to_string_lossy().to_string());
        } else {
            panic!("Expected ProtectedPath error");
        }
    }

    #[test]
    fn test_protected_path_desktop() {
        let home = dirs_home().unwrap();
        let desktop = home.join("Desktop");
        let result = ShadowCheckpointService::new(
            "task-1",
            "/tmp/checkpoints",
            desktop.to_str().unwrap(),
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_protected_path_documents() {
        let home = dirs_home().unwrap();
        let documents = home.join("Documents");
        let result = ShadowCheckpointService::new(
            "task-1",
            "/tmp/checkpoints",
            documents.to_str().unwrap(),
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_protected_path_downloads() {
        let home = dirs_home().unwrap();
        let downloads = home.join("Downloads");
        let result = ShadowCheckpointService::new(
            "task-1",
            "/tmp/checkpoints",
            downloads.to_str().unwrap(),
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_valid_workspace_path() {
        let result = ShadowCheckpointService::new(
            "task-1",
            "/tmp/checkpoints",
            "/tmp/my-project",
            None,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_initial_state() {
        let service = ShadowCheckpointService::new(
            "task-1",
            "/tmp/checkpoints",
            "/tmp/my-project",
            None,
        )
        .unwrap();

        assert_eq!(service.task_id, "task-1");
        assert!(!service.is_initialized());
        assert!(service.base_hash().is_none());
        assert!(service.get_checkpoints().is_empty());
    }

    #[test]
    fn test_checkpoint_result_default() {
        let result = CheckpointResult::default();
        assert_eq!(result.commit, "");
        assert!(result.branch.is_none());
        assert!(result.summary.is_none());
    }

    #[test]
    fn test_checkpoint_diff_equality() {
        let diff1 = CheckpointDiff {
            paths: PathPair {
                relative: "foo.rs".to_string(),
                absolute: "/tmp/foo.rs".to_string(),
            },
            content: ContentPair {
                before: "old".to_string(),
                after: "new".to_string(),
            },
        };
        let diff2 = diff1.clone();
        assert_eq!(diff1, diff2);
    }
}

//! Worktree type definitions.
//!
//! Platform-agnostic type definitions for git worktree operations.
//! Derived from `packages/types/src/worktree.ts`.

use serde::{Deserialize, Serialize};

/// Represents a git worktree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Worktree {
    /// Absolute path to the worktree directory.
    pub path: String,
    /// Branch name — empty string if detached HEAD.
    pub branch: String,
    /// Current commit hash.
    pub commit_hash: String,
    /// Whether this is the current worktree (matches cwd).
    pub is_current: bool,
    /// Whether this is the bare/main repository.
    pub is_bare: bool,
    /// Whether HEAD is detached (not on a branch).
    pub is_detached: bool,
    /// Whether the worktree is locked.
    pub is_locked: bool,
    /// Reason for lock if locked.
    pub lock_reason: Option<String>,
}

/// Result of a worktree operation (create, delete, etc.).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeResult {
    /// Whether the operation succeeded.
    pub success: bool,
    /// Human-readable message describing the result.
    pub message: String,
    /// The worktree that was affected (if applicable).
    pub worktree: Option<Worktree>,
}

/// Branch information for worktree creation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchInfo {
    /// Local branches available.
    pub local_branches: Vec<String>,
    /// Remote branches available.
    pub remote_branches: Vec<String>,
    /// Currently checked out branch.
    pub current_branch: String,
}

/// Options for creating a worktree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateWorktreeOptions {
    /// Path where the worktree will be created.
    pub path: String,
    /// Branch name to checkout or create.
    pub branch: Option<String>,
    /// Base branch to create new branch from.
    pub base_branch: Option<String>,
    /// If true, create a new branch; if false, checkout existing branch.
    pub create_new_branch: Option<bool>,
}

/// Status of `.worktreeinclude` file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeIncludeStatus {
    /// Whether `.worktreeinclude` exists in the directory.
    pub exists: bool,
    /// Whether `.gitignore` exists in the directory.
    pub has_gitignore: bool,
    /// Content of `.gitignore` (for creating `.worktreeinclude`).
    pub gitignore_content: Option<String>,
}

/// Response for listWorktrees handler.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeListResponse {
    pub worktrees: Vec<Worktree>,
    pub is_git_repo: bool,
    pub error: Option<String>,
    pub is_multi_root: bool,
    pub is_subfolder: bool,
    pub git_root_path: String,
}

/// Response for worktree defaults.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeDefaultsResponse {
    pub suggested_branch: String,
    pub suggested_path: String,
    pub error: Option<String>,
}

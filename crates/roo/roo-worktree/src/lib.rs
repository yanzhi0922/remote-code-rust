//! `roo-worktree` — Git worktree management for Roo Code.
//!
//! Provides types and pure logic for creating, listing, deleting, and
//! switching git worktrees. Ported from `handlers.ts`.

pub mod git_ops;
pub mod ops;
pub mod types;

pub use git_ops::{
    branch_has_worktree_include, check_git_installed, check_git_repo, checkout_branch,
    create_worktree, create_worktree_include, delete_worktree, get_available_branches,
    get_current_branch, get_git_root_path, get_worktree_include_status, list_worktrees,
    CreateWorktreeOptions, CreateWorktreeResult, WorktreeInfo,
};
pub use ops::{generate_random_suffix, generate_worktree_name, is_workspace_subfolder};
pub use types::{
    BranchInfo, WorktreeCreateRequest, WorktreeCreateResponse, WorktreeDefaultsResponse,
    WorktreeDeleteRequest, WorktreeEntry, WorktreeIncludeStatus, WorktreeListResponse,
    WorktreeResult,
};

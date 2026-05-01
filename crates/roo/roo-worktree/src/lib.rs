//! `roo-worktree` — Git worktree management for Roo Code.
//!
//! Provides types and pure logic for creating, listing, deleting, and
//! switching git worktrees. Ported from `handlers.ts`.

pub mod ops;
pub mod types;

pub use ops::{generate_random_suffix, generate_worktree_name, is_workspace_subfolder};
pub use types::{
    BranchInfo, WorktreeCreateRequest, WorktreeCreateResponse, WorktreeDefaultsResponse,
    WorktreeDeleteRequest, WorktreeEntry, WorktreeIncludeStatus, WorktreeListResponse,
    WorktreeResult,
};

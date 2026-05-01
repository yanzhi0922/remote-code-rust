//! Git type definitions.
//!
//! Derived from `packages/types/src/git.ts`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// GitRepositoryInfo
// ---------------------------------------------------------------------------

/// Git repository information.
///
/// Source: `packages/types/src/git.ts` — `GitRepositoryInfo`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRepositoryInfo {
    pub repository_url: Option<String>,
    pub repository_name: Option<String>,
    pub default_branch: Option<String>,
}

// ---------------------------------------------------------------------------
// GitCommit
// ---------------------------------------------------------------------------

/// A git commit.
///
/// Source: `packages/types/src/git.ts` — `GitCommit`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommit {
    pub hash: String,
    pub short_hash: String,
    pub subject: String,
    pub author: String,
    pub date: String,
}

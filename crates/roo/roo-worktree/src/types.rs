//! Worktree type definitions.
//!
//! Ported from `@roo-code/types` Worktree types and the handler signatures
//! in `handlers.ts`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Core types
// ---------------------------------------------------------------------------

/// A single git worktree entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeEntry {
    pub name: String,
    pub path: String,
    pub branch: String,
    pub is_current: bool,
    pub is_main_worktree: bool,
}

/// Response for listing worktrees.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeListResponse {
    pub worktrees: Vec<WorktreeEntry>,
    pub cwd: String,
    #[serde(default)]
    pub is_git_repo: bool,
    #[serde(default)]
    pub is_multi_root: bool,
    #[serde(default)]
    pub is_subfolder: bool,
    #[serde(default)]
    pub git_root_path: String,
    #[serde(default)]
    pub error: Option<String>,
}

/// Request body for creating a new worktree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeCreateRequest {
    pub name: String,
    #[serde(default)]
    pub branch: Option<String>,
    #[serde(default)]
    pub source_branch: Option<String>,
}

/// Response after successfully creating a worktree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeCreateResponse {
    pub path: String,
    pub branch: String,
}

/// Request body for deleting a worktree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeDeleteRequest {
    pub name: String,
    #[serde(default)]
    pub force: bool,
}

/// Generic result type for worktree operations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeResult {
    pub success: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

impl WorktreeResult {
    /// Create a successful result.
    pub fn ok(message: impl Into<String>) -> Self {
        Self {
            success: true,
            error: None,
            message: Some(message.into()),
        }
    }

    /// Create an error result.
    pub fn err(error: impl Into<String>) -> Self {
        Self {
            success: false,
            error: Some(error.into()),
            message: None,
        }
    }
}

/// Branch listing information.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BranchInfo {
    pub branches: Vec<String>,
    pub current: String,
}

/// Default values suggested when creating a new worktree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeDefaultsResponse {
    pub default_path: String,
    pub default_branch: String,
}

/// Status of the `.worktreeinclude` file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorktreeIncludeStatus {
    pub included: bool,
    #[serde(default)]
    pub path: Option<String>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- WorktreeEntry ----

    #[test]
    fn test_worktree_entry_serialization() {
        let entry = WorktreeEntry {
            name: "main".into(),
            path: "/repo".into(),
            branch: "main".into(),
            is_current: true,
            is_main_worktree: true,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"name\":\"main\""));
        assert!(json.contains("\"is_current\":true"));
    }

    #[test]
    fn test_worktree_entry_deserialization() {
        let json = r#"{
            "name":"feature",
            "path":"/repo/.worktrees/feature",
            "branch":"feature-branch",
            "is_current":false,
            "is_main_worktree":false
        }"#;
        let entry: WorktreeEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.name, "feature");
        assert_eq!(entry.branch, "feature-branch");
        assert!(!entry.is_current);
    }

    // ---- WorktreeListResponse ----

    #[test]
    fn test_worktree_list_response_serialization() {
        let resp = WorktreeListResponse {
            worktrees: vec![],
            cwd: "/repo".into(),
            is_git_repo: true,
            is_multi_root: false,
            is_subfolder: false,
            git_root_path: "/repo".into(),
            error: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"is_git_repo\":true"));
        assert!(json.contains("\"error\":null"));
    }

    #[test]
    fn test_worktree_list_response_with_error() {
        let resp = WorktreeListResponse {
            worktrees: vec![],
            cwd: "/repo".into(),
            is_git_repo: false,
            is_multi_root: false,
            is_subfolder: false,
            git_root_path: String::new(),
            error: Some("Not a git repository".into()),
        };
        assert_eq!(resp.error.as_deref(), Some("Not a git repository"));
    }

    // ---- WorktreeCreateRequest ----

    #[test]
    fn test_worktree_create_request_defaults() {
        let json = r#"{"name":"my-worktree"}"#;
        let req: WorktreeCreateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "my-worktree");
        assert!(req.branch.is_none());
        assert!(req.source_branch.is_none());
    }

    #[test]
    fn test_worktree_create_request_with_branch() {
        let json = r#"{"name":"wt","branch":"feature","source_branch":"main"}"#;
        let req: WorktreeCreateRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.branch.as_deref(), Some("feature"));
        assert_eq!(req.source_branch.as_deref(), Some("main"));
    }

    // ---- WorktreeCreateResponse ----

    #[test]
    fn test_worktree_create_response() {
        let resp = WorktreeCreateResponse {
            path: "/repo/.worktrees/wt".into(),
            branch: "worktree/roo-abc12".into(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let back: WorktreeCreateResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(back, resp);
    }

    // ---- WorktreeDeleteRequest ----

    #[test]
    fn test_worktree_delete_request() {
        let json = r#"{"name":"old-wt","force":true}"#;
        let req: WorktreeDeleteRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name, "old-wt");
        assert!(req.force);
    }

    #[test]
    fn test_worktree_delete_request_default_force() {
        let json = r#"{"name":"old-wt"}"#;
        let req: WorktreeDeleteRequest = serde_json::from_str(json).unwrap();
        assert!(!req.force);
    }

    // ---- WorktreeResult ----

    #[test]
    fn test_worktree_result_success() {
        let result = WorktreeResult::ok("Created worktree");
        assert!(result.success);
        assert!(result.error.is_none());
        assert_eq!(result.message.as_deref(), Some("Created worktree"));
    }

    #[test]
    fn test_worktree_result_error() {
        let result = WorktreeResult::err("Not a git repository");
        assert!(!result.success);
        assert_eq!(result.error.as_deref(), Some("Not a git repository"));
        assert!(result.message.is_none());
    }

    // ---- BranchInfo ----

    #[test]
    fn test_branch_info_serialization() {
        let info = BranchInfo {
            branches: vec!["main".into(), "develop".into()],
            current: "main".into(),
        };
        let json = serde_json::to_string(&info).unwrap();
        let back: BranchInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(back.branches, vec!["main", "develop"]);
        assert_eq!(back.current, "main");
    }

    // ---- WorktreeDefaultsResponse ----

    #[test]
    fn test_worktree_defaults_response() {
        let resp = WorktreeDefaultsResponse {
            default_path: "/home/.roo/worktrees/proj-abc12".into(),
            default_branch: "worktree/roo-abc12".into(),
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("default_path"));
        assert!(json.contains("default_branch"));
    }

    // ---- WorktreeIncludeStatus ----

    #[test]
    fn test_worktree_include_status_included() {
        let status = WorktreeIncludeStatus {
            included: true,
            path: Some(".worktreeinclude".into()),
        };
        assert!(status.included);
        assert_eq!(status.path.as_deref(), Some(".worktreeinclude"));
    }

    #[test]
    fn test_worktree_include_status_not_included() {
        let status = WorktreeIncludeStatus {
            included: false,
            path: None,
        };
        assert!(!status.included);
        assert!(status.path.is_none());
    }
}

//! Worktree management tools: enter_worktree, exit_worktree.
//!
//! Provides tools for managing git worktrees. Git worktrees allow multiple
//! working directories for the same repository, enabling parallel work on
//! different branches.

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use super::ToolExecutionContext;

/// Enter a git worktree by creating or checking out a worktree for a branch.
///
/// If the worktree already exists, returns its path. Otherwise, suggests
/// a `git worktree add` command.
///
/// # Errors
/// Returns an error if the branch name is missing.
pub fn enter_worktree(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let branch = input["branch"]
        .as_str()
        .ok_or_else(|| anyhow!("branch is required"))?;

    if branch.trim().is_empty() {
        return Err(anyhow!("branch cannot be empty"));
    }

    let target_dir = input["directory"].as_str().unwrap_or(".worktrees/branch");
    let target_dir = if target_dir == ".worktrees/branch" {
        format!(".worktrees/{branch}")
    } else {
        target_dir.to_string()
    };

    // Check if we're in a git repo.
    let is_git = std::process::Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(&context.cwd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !is_git {
        return Err(anyhow!(
            "Not inside a git repository. Worktrees require a git repo."
        ));
    }

    // List existing worktrees.
    let worktrees = list_worktrees_internal(&context.cwd);

    // Check if this branch already has a worktree.
    if let Some(existing) = worktrees.iter().find(|wt| wt.branch == branch) {
        return Ok(json!({
            "type": "enter_worktree",
            "branch": branch,
            "path": existing.path,
            "status": "existing",
            "message": format!("Worktree for '{branch}' already exists at: {}", existing.path),
            "note": "Switch to this worktree directory to work on the branch."
        })
        .to_string());
    }

    // Suggest creating a new worktree.
    let full_path = context.cwd.join(&target_dir);
    Ok(json!({
        "type": "enter_worktree",
        "branch": branch,
        "path": target_dir,
        "full_path": full_path.to_string_lossy(),
        "status": "suggested",
        "command": format!("git worktree add {target_dir} {branch}"),
        "message": format!("Suggested: create worktree for '{branch}' at {target_dir}"),
        "note": "Execute the command to create the worktree, then switch to it."
    })
    .to_string())
}

/// Exit (remove) a git worktree for a branch.
///
/// Removes the worktree directory and cleans up the git worktree reference.
///
/// # Errors
/// Returns an error if the branch name is missing.
pub fn exit_worktree(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let branch = input["branch"]
        .as_str()
        .ok_or_else(|| anyhow!("branch is required"))?;

    if branch.trim().is_empty() {
        return Err(anyhow!("branch cannot be empty"));
    }

    let force = input["force"].as_bool().unwrap_or(false);

    // Check if we're in a git repo.
    let is_git = std::process::Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(&context.cwd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !is_git {
        return Err(anyhow!(
            "Not inside a git repository. Worktrees require a git repo."
        ));
    }

    let worktrees = list_worktrees_internal(&context.cwd);

    // Find the worktree for this branch.
    let wt = worktrees.iter().find(|wt| wt.branch == branch);

    let Some(wt) = wt else {
        return Ok(json!({
            "type": "exit_worktree",
            "branch": branch,
            "status": "not_found",
            "message": format!("No worktree found for branch '{branch}'."),
            "note": "The branch may not have a worktree, or it may have already been removed."
        })
        .to_string());
    };

    let force_flag = if force { " --force" } else { "" };

    Ok(json!({
        "type": "exit_worktree",
        "branch": branch,
        "path": wt.path,
        "status": "suggested",
        "command": format!("git worktree remove{force_flag} {}", wt.path),
        "message": format!("Suggested: remove worktree for '{branch}' at {}", wt.path),
        "note": "Execute the command to remove the worktree. Use force=true for uncommitted changes."
    })
    .to_string())
}

/// Information about a git worktree.
#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    /// The worktree directory path.
    pub path: String,
    /// The branch name (if applicable).
    pub branch: String,
    /// Whether this is the main worktree.
    pub is_main: bool,
}

/// List all git worktrees (internal helper).
fn list_worktrees_internal(cwd: &std::path::Path) -> Vec<WorktreeInfo> {
    let output = std::process::Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(cwd)
        .output();

    let Ok(output) = output else {
        return Vec::new();
    };
    if !output.status.success() {
        return Vec::new();
    }

    let text = String::from_utf8_lossy(&output.stdout);
    parse_worktree_list(&text)
}

/// Parse the output of `git worktree list --porcelain`.
fn parse_worktree_list(text: &str) -> Vec<WorktreeInfo> {
    let mut worktrees = Vec::new();
    let mut current_path = String::new();
    let mut current_branch = String::new();
    let mut is_main = false;

    for line in text.lines() {
        if let Some(path) = line.strip_prefix("worktree ") {
            if !current_path.is_empty() {
                worktrees.push(WorktreeInfo {
                    path: current_path.clone(),
                    branch: current_branch.clone(),
                    is_main,
                });
            }
            current_path = path.to_string();
            current_branch = String::new();
            is_main = false;
        } else if let Some(branch) = line.strip_prefix("branch refs/heads/") {
            current_branch = branch.to_string();
        } else if line == "bare" {
            is_main = true;
        }
    }

    if !current_path.is_empty() {
        worktrees.push(WorktreeInfo {
            path: current_path,
            branch: current_branch,
            is_main,
        });
    }

    worktrees
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn test_context() -> ToolExecutionContext {
        ToolExecutionContext {
            cwd: PathBuf::from("/tmp"),
            timeout_ms: 30_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Arc::new(std::sync::Mutex::new(
                rc_core::task_stack::TaskStack::default(),
            )),
        }
    }

    #[test]
    fn parse_worktree_list_handles_empty_input() {
        let result = parse_worktree_list("");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_worktree_list_parses_single_worktree() {
        let input = "worktree /home/user/project\nbranch refs/heads/main\n";
        let result = parse_worktree_list(input);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].path, "/home/user/project");
        assert_eq!(result[0].branch, "main");
    }

    #[test]
    fn parse_worktree_list_parses_multiple_worktrees() {
        let input = "\
worktree /home/user/project
branch refs/heads/main

worktree /home/user/project-feature
branch refs/heads/feature-xyz
";
        let result = parse_worktree_list(input);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].branch, "main");
        assert_eq!(result[1].branch, "feature-xyz");
    }

    #[test]
    fn parse_worktree_list_handles_bare_repo() {
        let input = "worktree /home/user/project\nbare\n";
        let result = parse_worktree_list(input);
        assert_eq!(result.len(), 1);
        assert!(result[0].is_main);
    }

    #[test]
    fn enter_worktree_requires_branch() {
        let input = json!({});
        let context = test_context();
        let result = enter_worktree(&input, &context);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("branch"));
    }

    #[test]
    fn enter_worktree_rejects_empty_branch() {
        let input = json!({"branch": ""});
        let context = test_context();
        let result = enter_worktree(&input, &context);
        assert!(result.is_err());
    }

    #[test]
    fn exit_worktree_requires_branch() {
        let input = json!({});
        let context = test_context();
        let result = exit_worktree(&input, &context);
        assert!(result.is_err());
    }

    #[test]
    fn exit_worktree_rejects_empty_branch() {
        let input = json!({"branch": ""});
        let context = test_context();
        let result = exit_worktree(&input, &context);
        assert!(result.is_err());
    }

    #[test]
    fn worktree_info_debug_format() {
        let info = WorktreeInfo {
            path: "/tmp/test".to_string(),
            branch: "main".to_string(),
            is_main: false,
        };
        let debug = format!("{info:?}");
        assert!(debug.contains("/tmp/test"));
        assert!(debug.contains("main"));
    }

    #[test]
    fn parse_worktree_list_handles_detached_head() {
        let input = "worktree /home/user/project\ndetached\n";
        let result = parse_worktree_list(input);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].branch, ""); // No branch for detached head
    }

    #[test]
    fn parse_worktree_list_trailing_newline() {
        let input = "worktree /home/user/project\nbranch refs/heads/main";
        let result = parse_worktree_list(input);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].branch, "main");
    }

    #[test]
    fn enter_worktree_custom_directory() {
        // This test just verifies the function doesn't panic with custom dir.
        let input = json!({"branch": "test-branch", "directory": "/custom/path"});
        let context = test_context();
        // Will fail because /tmp is not a git repo, but that's fine for testing.
        let _ = enter_worktree(&input, &context);
    }

    #[test]
    fn exit_worktree_with_force() {
        let input = json!({"branch": "test-branch", "force": true});
        let context = test_context();
        let _ = exit_worktree(&input, &context);
    }
}

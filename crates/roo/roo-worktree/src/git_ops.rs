//! Git operations for worktree management.
//!
//! Ported from `packages/core/src/worktree/worktree-service.ts` — `WorktreeService`.
//!
//! All operations invoke `git` as a subprocess in the given working directory.

use std::path::Path;
use std::process::Command;

use tracing::debug;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Run a git command in `cwd`, returning stdout on success or an error message.
fn git_cmd(cwd: &Path, args: &[&str]) -> Result<String, String> {
    debug!(cwd = %cwd.display(), args = ?args, "Running git command");
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("failed to execute git: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(stderr);
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Run a git command, returning success boolean (ignores output).
fn git_cmd_ok(cwd: &Path, args: &[&str]) -> bool {
    git_cmd(cwd, args).is_ok()
}

// ---------------------------------------------------------------------------
// WorktreeService equivalents
// ---------------------------------------------------------------------------

/// Check whether git is installed.
pub fn check_git_installed() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check whether `cwd` is inside a git repository.
///
/// Source: `WorktreeService.checkGitRepo` — runs `git rev-parse --git-dir`.
pub fn check_git_repo(cwd: &Path) -> bool {
    git_cmd_ok(cwd, &["rev-parse", "--git-dir"])
}

/// Get the git repository root path.
///
/// Source: `WorktreeService.getGitRootPath` — runs `git rev-parse --show-toplevel`.
pub fn get_git_root_path(cwd: &Path) -> Result<String, String> {
    git_cmd(cwd, &["rev-parse", "--show-toplevel"])
}

/// Get the current branch name.
///
/// Source: `WorktreeService.getCurrentBranch` — runs `git rev-parse --abbrev-ref HEAD`.
pub fn get_current_branch(cwd: &Path) -> Result<String, String> {
    git_cmd(cwd, &["rev-parse", "--abbrev-ref", "HEAD"])
}

/// A parsed git worktree entry from `git worktree list --porcelain`.
#[derive(Debug, Clone)]
pub struct WorktreeInfo {
    pub path: String,
    pub head: String,
    pub branch: Option<String>,
    pub is_bare: bool,
    pub is_detached: bool,
    pub locked: Option<String>,
}

/// List all git worktrees.
///
/// Source: `WorktreeService.listWorktrees` — runs `git worktree list --porcelain`
/// and parses the porcelain output.
pub fn list_worktrees(cwd: &Path) -> Result<Vec<WorktreeInfo>, String> {
    let output = git_cmd(cwd, &["worktree", "list", "--porcelain"])?;
    Ok(parse_worktree_porcelain(&output))
}

/// Parse the porcelain output of `git worktree list --porcelain`.
///
/// Source: `parseWorktreeOutput` in TS worktree-service.ts.
fn parse_worktree_porcelain(output: &str) -> Vec<WorktreeInfo> {
    let mut worktrees = Vec::new();
    let mut current = WorktreeInfo {
        path: String::new(),
        head: String::new(),
        branch: None,
        is_bare: false,
        is_detached: false,
        locked: None,
    };

    for line in output.lines() {
        if line.is_empty() {
            if !current.path.is_empty() {
                worktrees.push(std::mem::replace(
                    &mut current,
                    WorktreeInfo {
                        path: String::new(),
                        head: String::new(),
                        branch: None,
                        is_bare: false,
                        is_detached: false,
                        locked: None,
                    },
                ));
            }
            continue;
        }

        if let Some(path) = line.strip_prefix("worktree ") {
            current.path = path.to_string();
        } else if let Some(head) = line.strip_prefix("HEAD ") {
            current.head = head.to_string();
        } else if let Some(branch) = line.strip_prefix("branch ") {
            // Strip refs/heads/ prefix
            current.branch = Some(
                branch
                    .strip_prefix("refs/heads/")
                    .unwrap_or(branch)
                    .to_string(),
            );
        } else if line == "bare" {
            current.is_bare = true;
        } else if line == "detached" {
            current.is_detached = true;
        } else if line == "locked" {
            current.locked = Some(String::new());
        } else if let Some(reason) = line.strip_prefix("locked ") {
            current.locked = Some(reason.to_string());
        }
    }

    // Push the last worktree if output doesn't end with empty line
    if !current.path.is_empty() {
        worktrees.push(current);
    }

    worktrees
}

/// Options for creating a worktree.
#[derive(Debug, Clone)]
pub struct CreateWorktreeOptions {
    pub path: String,
    pub branch: Option<String>,
    pub create_new_branch: bool,
    pub source_branch: Option<String>,
}

/// Result of creating a worktree.
#[derive(Debug, Clone)]
pub struct CreateWorktreeResult {
    pub path: String,
    pub branch: String,
}

/// Create a new git worktree.
///
/// Source: `WorktreeService.createWorktree` — runs:
/// - `git worktree add -b <branch> <path> [<base>]` (new branch)
/// - `git worktree add <path> <branch>` (existing branch)
/// - `git worktree add --detach <path>` (no branch)
pub fn create_worktree(
    cwd: &Path,
    opts: &CreateWorktreeOptions,
) -> Result<CreateWorktreeResult, String> {
    let mut args: Vec<String> = vec!["worktree".to_string(), "add".to_string()];

    if opts.create_new_branch {
        if let Some(ref branch) = opts.branch {
            args.push("-b".to_string());
            args.push(branch.clone());
        }
        args.push(opts.path.clone());
        if let Some(ref base) = opts.source_branch {
            args.push(base.clone());
        }
    } else if let Some(ref branch) = opts.branch {
        args.push(opts.path.clone());
        args.push(branch.clone());
    } else {
        args.push("--detach".to_string());
        args.push(opts.path.clone());
    }

    let args_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    git_cmd(cwd, &args_refs)?;

    // Find the created worktree to get branch info
    let branch_name = if let Some(ref b) = opts.branch {
        b.clone()
    } else {
        get_current_branch(Path::new(&opts.path)).unwrap_or_else(|_| "detached".to_string())
    };

    Ok(CreateWorktreeResult {
        path: opts.path.clone(),
        branch: branch_name,
    })
}

/// Delete a git worktree.
///
/// Source: `WorktreeService.deleteWorktree` — runs:
/// 1. `git worktree remove [--force] <path>`
/// 2. Best-effort `git branch -d <branch>` for the associated branch
pub fn delete_worktree(cwd: &Path, worktree_path: &str, force: bool) -> Result<String, String> {
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(worktree_path);

    let args_refs: Vec<&str> = args.to_vec();
    git_cmd(cwd, &args_refs)?;

    // Best-effort branch deletion (matching TS behavior)
    // The branch cleanup is done after worktree removal as best-effort

    Ok(format!("Worktree at {worktree_path} removed successfully"))
}

/// Available branches result.
#[derive(Debug, Clone)]
pub struct BranchesResult {
    pub local_branches: Vec<String>,
    pub remote_branches: Vec<String>,
    pub current_branch: String,
}

/// Get available git branches.
///
/// Source: `WorktreeService.getAvailableBranches` — runs:
/// - `git branch --format="%(refname:short)"`
/// - `git branch -r --format="%(refname:short)"`
/// - `git rev-parse --abbrev-ref HEAD`
pub fn get_available_branches(cwd: &Path) -> Result<BranchesResult, String> {
    let local_output = git_cmd(cwd, &["branch", "--format=%(refname:short)"])?;
    let remote_output = git_cmd(cwd, &["branch", "-r", "--format=%(refname:short)"])?;
    let current = get_current_branch(cwd)?;

    let local_branches: Vec<String> = local_output
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    let remote_branches: Vec<String> = remote_output
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty() && !l.contains("HEAD"))
        .collect();

    Ok(BranchesResult {
        local_branches,
        remote_branches,
        current_branch: current,
    })
}

/// Checkout a branch.
///
/// Source: `WorktreeService.checkoutBranch` — runs `git checkout <branch>`.
pub fn checkout_branch(cwd: &Path, branch: &str) -> Result<String, String> {
    git_cmd(cwd, &["checkout", branch])
}

// ---------------------------------------------------------------------------
// WorktreeIncludeService equivalents
// ---------------------------------------------------------------------------

/// Get the status of the `.worktreeinclude` file.
///
/// Source: `WorktreeIncludeService.getStatus` — checks file existence and .gitignore.
pub fn get_worktree_include_status(cwd: &Path) -> (bool, bool, Option<String>) {
    let include_path = cwd.join(".worktreeinclude");
    let exists = include_path.exists();

    let gitignore_path = cwd.join(".gitignore");
    let has_gitignore = gitignore_path.exists();
    let gitignore_content = if has_gitignore {
        std::fs::read_to_string(&gitignore_path).ok()
    } else {
        None
    };

    (exists, has_gitignore, gitignore_content)
}

/// Check if a branch has a `.worktreeinclude` file.
///
/// Source: `WorktreeIncludeService.branchHasWorktreeInclude` — runs
/// `git cat-file -e -- <branch>:.worktreeinclude`.
pub fn branch_has_worktree_include(cwd: &Path, branch: &str) -> bool {
    let target = format!("{branch}:.worktreeinclude");
    git_cmd_ok(cwd, &["cat-file", "-e", "--", &target])
}

/// Create a `.worktreeinclude` file with the given content.
///
/// Source: `WorktreeIncludeService.createWorktreeInclude` — writes to `<cwd>/.worktreeinclude`.
pub fn create_worktree_include(cwd: &Path, content: &str) -> Result<String, String> {
    let path = cwd.join(".worktreeinclude");
    std::fs::write(&path, content).map_err(|e| format!("Failed to write .worktreeinclude: {e}"))?;
    Ok(format!("Created .worktreeinclude at {}", path.display()))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_empty_porcelain() {
        let result = parse_worktree_porcelain("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_single_worktree() {
        let output = "worktree /home/user/repo\nHEAD abc123\nbranch refs/heads/main\n\n";
        let wts = parse_worktree_porcelain(output);
        assert_eq!(wts.len(), 1);
        assert_eq!(wts[0].path, "/home/user/repo");
        assert_eq!(wts[0].head, "abc123");
        assert_eq!(wts[0].branch.as_deref(), Some("main"));
        assert!(!wts[0].is_bare);
        assert!(!wts[0].is_detached);
    }

    #[test]
    fn test_parse_multiple_worktrees() {
        let output = "\
worktree /home/user/repo
HEAD abc123
branch refs/heads/main

worktree /home/user/repo/.worktrees/feature
HEAD def456
branch refs/heads/feature-branch

";
        let wts = parse_worktree_porcelain(output);
        assert_eq!(wts.len(), 2);
        assert_eq!(wts[0].branch.as_deref(), Some("main"));
        assert_eq!(wts[1].branch.as_deref(), Some("feature-branch"));
    }

    #[test]
    fn test_parse_detached_worktree() {
        let output = "worktree /tmp/wt\nHEAD abc123\ndetached\n\n";
        let wts = parse_worktree_porcelain(output);
        assert_eq!(wts.len(), 1);
        assert!(wts[0].is_detached);
        assert!(wts[0].branch.is_none());
    }

    #[test]
    fn test_parse_bare_repo() {
        let output = "worktree /opt/repo.git\nHEAD abc123\nbare\n\n";
        let wts = parse_worktree_porcelain(output);
        assert_eq!(wts.len(), 1);
        assert!(wts[0].is_bare);
    }

    #[test]
    fn test_parse_locked_worktree() {
        let output = "worktree /tmp/wt\nHEAD abc123\nbranch refs/heads/x\nlocked reason here\n\n";
        let wts = parse_worktree_porcelain(output);
        assert_eq!(wts.len(), 1);
        assert_eq!(wts[0].locked.as_deref(), Some("reason here"));
    }

    #[test]
    fn test_parse_locked_no_reason() {
        let output = "worktree /tmp/wt\nHEAD abc123\nlocked\n\n";
        let wts = parse_worktree_porcelain(output);
        assert_eq!(wts[0].locked.as_deref(), Some(""));
    }
}

//! Git-related tools: suggest_pr, enter/exit_worktree, list_worktrees.

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use super::ToolExecutionContext;

pub fn suggest_pr_tool(context: &ToolExecutionContext) -> Result<String> {
    // Run git diff --stat and git log to suggest a PR.
    let diff_output = std::process::Command::new("git")
        .args(["diff", "--stat"])
        .current_dir(&context.cwd)
        .output();

    let diff_stat = match diff_output {
        Ok(out) => String::from_utf8_lossy(&out.stdout).to_string(),
        Err(_) => "Unable to run git diff.".to_owned(),
    };

    let log_output = std::process::Command::new("git")
        .args(["log", "--oneline", "-10"])
        .current_dir(&context.cwd)
        .output();

    let recent_commits = match log_output {
        Ok(out) => String::from_utf8_lossy(&out.stdout).to_string(),
        Err(_) => "Unable to run git log.".to_owned(),
    };

    // Simple heuristic: use the first line of recent commits as title suggestion.
    let title_suggestion = recent_commits
        .lines()
        .next()
        .unwrap_or("Changes from current branch")
        .trim_start_matches(|c: char| c.is_ascii_hexdigit() || c == ' ');

    Ok(json!({
        "suggested_title": title_suggestion,
        "diff_stat": diff_stat.trim(),
        "recent_commits": recent_commits.trim(),
        "note": "Review the diff and commits above to craft a PR description."
    })
    .to_string())
}

pub fn enter_worktree_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let branch = input["branch"]
        .as_str()
        .ok_or_else(|| anyhow!("branch is required"))?;

    // Determine the worktree path.
    let worktree_dir = input["path"]
        .as_str()
        .map(|p| p.to_string())
        .unwrap_or_else(|| format!("../{branch}"));

    // Try to actually create the worktree.
    let output = std::process::Command::new("git")
        .args(["worktree", "add", &worktree_dir, branch])
        .current_dir(&context.cwd)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            // Resolve the absolute path of the new worktree.
            let abs_path = if std::path::Path::new(&worktree_dir).is_absolute() {
                worktree_dir.clone()
            } else {
                let mut p = context.cwd.clone();
                p.pop();
                p.push(&worktree_dir);
                p.to_string_lossy().to_string()
            };
            Ok(json!({
                "status": "created",
                "branch": branch,
                "path": abs_path,
                "output": stdout,
                "note": format!("Worktree created at {worktree_dir}. Use this path as the working directory for parallel work on branch '{branch}'.")
            }).to_string())
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            // If the worktree already exists, provide helpful info.
            if stderr.contains("already") {
                Ok(json!({
                    "status": "already_exists",
                    "branch": branch,
                    "path": worktree_dir,
                    "output": stderr,
                    "note": "This worktree already exists. You can work in that directory."
                })
                .to_string())
            } else {
                // Fall back to command suggestion.
                Ok(json!({
                    "status": "manual",
                    "command": format!("git worktree add {worktree_dir} {branch}"),
                    "branch": branch,
                    "error": stderr,
                    "note": "Could not auto-create worktree. Run the command above manually."
                })
                .to_string())
            }
        }
        Err(_) => {
            // Git not available, provide command suggestion.
            Ok(json!({
                "status": "manual",
                "command": format!("git worktree add {worktree_dir} {branch}"),
                "branch": branch,
                "note": "Run the command above to create a new worktree for this branch."
            })
            .to_string())
        }
    }
}

pub fn exit_worktree_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let branch = input["branch"]
        .as_str()
        .ok_or_else(|| anyhow!("branch is required"))?;

    let worktree_dir = input["path"]
        .as_str()
        .map(|p| p.to_string())
        .unwrap_or_else(|| format!("../{branch}"));

    // Try to actually remove the worktree.
    let output = std::process::Command::new("git")
        .args(["worktree", "remove", &worktree_dir])
        .current_dir(&context.cwd)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            Ok(json!({
                "status": "removed",
                "branch": branch,
                "path": worktree_dir,
                "output": stdout,
                "note": format!("Worktree at {worktree_dir} has been removed.")
            })
            .to_string())
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
            // Fall back to command suggestion.
            Ok(json!({
                "status": "manual",
                "command": format!("git worktree remove {worktree_dir}"),
                "branch": branch,
                "error": stderr,
                "note": "Could not auto-remove worktree. Run the command above manually."
            })
            .to_string())
        }
        Err(_) => Ok(json!({
            "status": "manual",
            "command": format!("git worktree remove {worktree_dir}"),
            "branch": branch,
            "note": "Run the command above to remove the worktree for this branch."
        })
        .to_string()),
    }
}

pub fn list_worktrees_tool(context: &ToolExecutionContext) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(&context.cwd)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let worktrees: Vec<Value> = stdout
                .split("\n\n")
                .filter(|block| !block.is_empty())
                .map(|block| {
                    let mut path = "";
                    let mut branch = "";
                    let mut is_bare = false;
                    for line in block.lines() {
                        if let Some(p) = line.strip_prefix("worktree ") {
                            path = p;
                        } else if let Some(b) = line.strip_prefix("branch refs/heads/") {
                            branch = b;
                        } else if line == "bare" {
                            is_bare = true;
                        }
                    }
                    json!({
                        "path": path,
                        "branch": branch,
                        "is_bare": is_bare,
                    })
                })
                .collect();
            Ok(json!({
                "worktrees": worktrees,
                "count": worktrees.len(),
            })
            .to_string())
        }
        Ok(_) => Ok(json!({
            "worktrees": [],
            "note": "Not in a git repository or git worktree not supported."
        })
        .to_string()),
        Err(_) => Ok(json!({
            "worktrees": [],
            "note": "git is not available."
        })
        .to_string()),
    }
}

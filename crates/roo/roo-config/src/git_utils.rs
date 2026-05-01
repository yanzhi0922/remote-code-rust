//! Git utility functions.
//!
//! Derived from `src/utils/git.ts`.
//!
//! Provides functions for extracting git repository information,
//! converting git URLs, sanitizing URLs, searching commits,
//! and getting working state.

use std::path::Path;
use std::process::Command;

use regex::Regex;
use tracing::warn;

use roo_types::git::{GitCommit, GitRepositoryInfo};

/// Maximum number of lines to include in git output before truncation.
const GIT_OUTPUT_LINE_LIMIT: usize = 500;

// ---------------------------------------------------------------------------
// Repository info
// ---------------------------------------------------------------------------

/// Extracts git repository information from the workspace's `.git` directory.
///
/// Source: `src/utils/git.ts` — `getGitRepositoryInfo`
///
/// # Arguments
/// * `workspace_root` - The root path of the workspace
///
/// # Returns
/// `GitRepositoryInfo` with whatever fields could be extracted, or an empty
/// struct if the directory is not a git repository.
pub async fn get_git_repository_info(workspace_root: &Path) -> GitRepositoryInfo {
    let git_dir = workspace_root.join(".git");

    // Check if .git directory exists
    if !git_dir.exists() {
        return GitRepositoryInfo {
            repository_url: None,
            repository_name: None,
            default_branch: None,
        };
    }

    let mut info = GitRepositoryInfo {
        repository_url: None,
        repository_name: None,
        default_branch: None,
    };

    // Try to read git config file
    let config_path = git_dir.join("config");
    if let Ok(config_content) = tokio::fs::read_to_string(&config_path).await {
        // Find any URL line
        let url_re = Regex::new(r"url\s*=\s*(.+?)(?:\r?\n|$)").unwrap();
        if let Some(caps) = url_re.captures(&config_content) {
            if let Some(m) = caps.get(1) {
                let url = m.as_str().trim();
                info.repository_url = Some(convert_git_url_to_https(&sanitize_git_url(url)));
                let repo_name = extract_repository_name(url);
                if !repo_name.is_empty() {
                    info.repository_name = Some(repo_name);
                }
            }
        }

        // Extract default branch
        let branch_re = Regex::new(r#"\[branch "([^"]+)"\]"#).unwrap();
        if let Some(caps) = branch_re.captures(&config_content) {
            if let Some(m) = caps.get(1) {
                info.default_branch = Some(m.as_str().to_string());
            }
        }
    }

    // Try to read HEAD file to get current branch
    if info.default_branch.is_none() {
        let head_path = git_dir.join("HEAD");
        if let Ok(head_content) = tokio::fs::read_to_string(&head_path).await {
            let head_re = Regex::new(r"ref: refs/heads/(.+)").unwrap();
            if let Some(caps) = head_re.captures(&head_content) {
                if let Some(m) = caps.get(1) {
                    info.default_branch = Some(m.as_str().trim().to_string());
                }
            }
        }
    }

    info
}

// ---------------------------------------------------------------------------
// URL conversion
// ---------------------------------------------------------------------------

/// Converts a git URL to HTTPS format.
///
/// Source: `src/utils/git.ts` — `convertGitUrlToHttps`
pub fn convert_git_url_to_https(url: &str) -> String {
    // Already HTTPS
    if url.starts_with("https://") {
        return url.to_string();
    }

    // SSH format: git@github.com:user/repo.git -> https://github.com/user/repo.git
    if url.starts_with("git@") {
        let re = Regex::new(r"git@([^:]+):(.+)").unwrap();
        if let Some(caps) = re.captures(url) {
            if caps.len() == 3 {
                let host = caps.get(1).unwrap().as_str();
                let path = caps.get(2).unwrap().as_str();
                return format!("https://{host}/{path}");
            }
        }
    }

    // SSH with protocol: ssh://git@github.com/user/repo.git
    if url.starts_with("ssh://") {
        let re = Regex::new(r"ssh://(?:git@)?([^/]+)/(.+)").unwrap();
        if let Some(caps) = re.captures(url) {
            if caps.len() == 3 {
                let host = caps.get(1).unwrap().as_str();
                let path = caps.get(2).unwrap().as_str();
                return format!("https://{host}/{path}");
            }
        }
    }

    url.to_string()
}

/// Sanitizes a git URL to remove sensitive information like tokens.
///
/// Source: `src/utils/git.ts` — `sanitizeGitUrl`
pub fn sanitize_git_url(url: &str) -> String {
    // Remove credentials from HTTPS URLs
    if url.starts_with("https://") {
        if let Ok(mut url_obj) = url::Url::parse(url) {
            // Remove username and password
            url_obj.set_username("").unwrap_or(());
            url_obj.set_password(None).unwrap_or(());
            return url_obj.to_string();
        }
    }

    // For SSH URLs, return as-is
    if url.starts_with("git@") || url.starts_with("ssh://") {
        return url.to_string();
    }

    // For other formats, remove potential tokens (40+ hex chars after colon)
    let re = Regex::new(r"(?i):[a-f0-9]{40,}@").unwrap();
    re.replace(url, "@").to_string()
}

/// Extracts repository name from a git URL.
///
/// Source: `src/utils/git.ts` — `extractRepositoryName`
pub fn extract_repository_name(url: &str) -> String {
    let patterns = [
        // HTTPS
        Regex::new(r"https://[^/]+/([^/]+/[^/]+?)(?:\.git)?$").unwrap(),
        // SSH
        Regex::new(r"git@[^:]+:([^/]+/[^/]+?)(?:\.git)?$").unwrap(),
        // SSH with protocol
        Regex::new(r"ssh://[^/]+/([^/]+/[^/]+?)(?:\.git)?$").unwrap(),
    ];

    for pattern in &patterns {
        if let Some(caps) = pattern.captures(url) {
            if let Some(m) = caps.get(1) {
                return m.as_str().replace(".git", "");
            }
        }
    }

    String::new()
}

// ---------------------------------------------------------------------------
// Git command helpers
// ---------------------------------------------------------------------------

/// Checks if Git is installed on the system.
///
/// Source: `src/utils/git.ts` — `checkGitInstalled`
pub async fn check_git_installed() -> bool {
    run_git_command(&["--version"], Path::new(".")).is_some()
}

/// Checks if the given directory is inside a git repository.
fn check_git_repo(cwd: &Path) -> bool {
    run_git_command(&["rev-parse", "--git-dir"], cwd).is_some()
}

/// Runs a git command and returns the stdout if successful.
fn run_git_command(args: &[&str], cwd: &Path) -> Option<String> {
    Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
}

/// Searches git commits by query string.
///
/// Source: `src/utils/git.ts` — `searchCommits`
pub async fn search_commits(query: &str, cwd: &Path) -> Vec<GitCommit> {
    if !check_git_installed().await {
        warn!("Git is not installed");
        return vec![];
    }

    if !check_git_repo(cwd) {
        warn!("Not a git repository");
        return vec![];
    }

    // Search commits by message
    let output = Command::new("git")
        .args([
            "log",
            "-n",
            "10",
            "--format=%H%n%h%n%s%n%an%n%ad",
            "--date=short",
            &format!("--grep={query}"),
            "--regexp-ignore-case",
        ])
        .current_dir(cwd)
        .output();

    let mut output_text = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return vec![],
    };

    // If no results and query looks like a hash, try searching by hash
    let hash_re = Regex::new(r"^[a-f0-9]+$").unwrap();
    if output_text.trim().is_empty() && hash_re.is_match(query) {
        if let Ok(o) = Command::new("git")
            .args([
                "log",
                "-n",
                "10",
                "--format=%H%n%h%n%s%n%an%n%ad",
                "--date=short",
                "--author-date-order",
                query,
            ])
            .current_dir(cwd)
            .output()
        {
            if o.status.success() {
                output_text = String::from_utf8_lossy(&o.stdout).to_string();
            }
        }
    }

    if output_text.trim().is_empty() {
        return vec![];
    }

    let lines: Vec<&str> = output_text
        .trim()
        .split('\n')
        .filter(|l| *l != "--")
        .collect();

    let mut commits = Vec::new();
    let mut i = 0;
    while i + 4 < lines.len() {
        commits.push(GitCommit {
            hash: lines[i].to_string(),
            short_hash: lines[i + 1].to_string(),
            subject: lines[i + 2].to_string(),
            author: lines[i + 3].to_string(),
            date: lines[i + 4].to_string(),
        });
        i += 5;
    }

    commits
}

/// Gets detailed commit information.
///
/// Source: `src/utils/git.ts` — `getCommitInfo`
pub async fn get_commit_info(hash: &str, cwd: &Path) -> String {
    if !check_git_installed().await {
        return "Git is not installed".to_string();
    }

    if !check_git_repo(cwd) {
        return "Not a git repository".to_string();
    }

    // Get commit info
    let info_output = Command::new("git")
        .args([
            "show",
            &format!("--format=%H%n%h%n%s%n%an%n%ad%n%b"),
            "--no-patch",
            hash,
        ])
        .current_dir(cwd)
        .output();

    let info = match info_output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return "Failed to get commit info".to_string(),
    };

    let info_lines: Vec<&str> = info.trim().split('\n').collect();
    let (full_hash, short_hash, subject, author, date) = (
        info_lines.first().unwrap_or(&"").to_string(),
        info_lines.get(1).unwrap_or(&"").to_string(),
        info_lines.get(2).unwrap_or(&"").to_string(),
        info_lines.get(3).unwrap_or(&"").to_string(),
        info_lines.get(4).unwrap_or(&"").to_string(),
    );
    let body = info_lines.get(5..).map(|s| s.join("\n")).unwrap_or_default();

    // Get stats
    let stats_output = Command::new("git")
        .args(["show", "--stat", "--format=", hash])
        .current_dir(cwd)
        .output();

    let stats = match stats_output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => String::new(),
    };

    // Get diff
    let diff_output = Command::new("git")
        .args(["show", "--format=", hash])
        .current_dir(cwd)
        .output();

    let diff = match diff_output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => String::new(),
    };

    let body_section = if body.trim().is_empty() {
        String::new()
    } else {
        format!("\nDescription:\n{body}")
    };

    let summary = format!(
        "Commit: {short_hash} ({full_hash})\nAuthor: {author}\nDate: {date}\n\nMessage: {subject}{body_section}\nFiles Changed:\n{stats}\nFull Changes:"
    );

    let output = format!("{}\n\n{}", summary, diff.trim());
    truncate_output(&output, GIT_OUTPUT_LINE_LIMIT)
}

/// Gets the working directory state (changes and diff).
///
/// Source: `src/utils/git.ts` — `getWorkingState`
pub async fn get_working_state(cwd: &Path) -> String {
    if !check_git_installed().await {
        return "Git is not installed".to_string();
    }

    if !check_git_repo(cwd) {
        return "Not a git repository".to_string();
    }

    // Get status
    let status_output = Command::new("git")
        .args(["status", "--short"])
        .current_dir(cwd)
        .output();

    let status = match status_output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return "Failed to get working state".to_string(),
    };

    if status.trim().is_empty() {
        return "No changes in working directory".to_string();
    }

    // Get diff
    let diff_output = Command::new("git")
        .args(["diff", "HEAD"])
        .current_dir(cwd)
        .output();

    let diff = match diff_output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => String::new(),
    };

    let output = format!("Working directory changes:\n\n{}\n\n{}", status.trim(), diff);
    truncate_output(&output, GIT_OUTPUT_LINE_LIMIT)
}

/// Gets git status with configurable file limit.
///
/// Source: `src/utils/git.ts` — `getGitStatus`
pub async fn get_git_status(cwd: &Path, max_files: usize) -> Option<String> {
    if !check_git_installed().await {
        return None;
    }

    if !check_git_repo(cwd) {
        return None;
    }

    let output = Command::new("git")
        .args(["status", "--porcelain=v1", "--branch"])
        .current_dir(cwd)
        .output();

    let stdout = match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).to_string(),
        _ => return None,
    };

    if stdout.trim().is_empty() {
        return None;
    }

    let lines: Vec<&str> = stdout.trim().split('\n').collect();
    if lines.is_empty() {
        return None;
    }

    // First line is always branch info
    let branch_line = lines[0];
    let file_lines = &lines[1..];

    let mut result = vec![branch_line.to_string()];

    if max_files > 0 && !file_lines.is_empty() {
        let files_to_show = &file_lines[..file_lines.len().min(max_files)];
        for line in files_to_show {
            result.push(line.to_string());
        }

        if file_lines.len() > max_files {
            result.push(format!("... {} more files", file_lines.len() - max_files));
        }
    }

    Some(result.join("\n"))
}

/// Truncates output to a specified number of lines.
fn truncate_output(output: &str, line_limit: usize) -> String {
    let lines: Vec<&str> = output.lines().collect();
    if lines.len() <= line_limit {
        output.to_string()
    } else {
        let truncated: Vec<&str> = lines[..line_limit].to_vec();
        format!(
            "{}\n\n... (truncated {} lines)",
            truncated.join("\n"),
            lines.len() - line_limit
        )
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_git_url_to_https_already_https() {
        let url = "https://github.com/user/repo.git";
        assert_eq!(convert_git_url_to_https(url), url);
    }

    #[test]
    fn test_convert_git_url_to_https_ssh_format() {
        let url = "git@github.com:user/repo.git";
        assert_eq!(
            convert_git_url_to_https(url),
            "https://github.com/user/repo.git"
        );
    }

    #[test]
    fn test_convert_git_url_to_https_ssh_protocol() {
        let url = "ssh://git@github.com/user/repo.git";
        assert_eq!(
            convert_git_url_to_https(url),
            "https://github.com/user/repo.git"
        );
    }

    #[test]
    fn test_sanitize_git_url_removes_credentials() {
        let url = "https://user:pass@github.com/user/repo.git";
        let sanitized = sanitize_git_url(url);
        assert!(!sanitized.contains("user:pass"));
        assert!(sanitized.contains("github.com"));
    }

    #[test]
    fn test_sanitize_git_url_ssh_unchanged() {
        let url = "git@github.com:user/repo.git";
        assert_eq!(sanitize_git_url(url), url);
    }

    #[test]
    fn test_extract_repository_name_https() {
        let url = "https://github.com/user/repo.git";
        assert_eq!(extract_repository_name(url), "user/repo");
    }

    #[test]
    fn test_extract_repository_name_ssh() {
        let url = "git@github.com:user/repo.git";
        assert_eq!(extract_repository_name(url), "user/repo");
    }

    #[test]
    fn test_extract_repository_name_ssh_protocol() {
        let url = "ssh://git@github.com/user/repo.git";
        assert_eq!(extract_repository_name(url), "user/repo");
    }

    #[test]
    fn test_extract_repository_name_invalid() {
        assert_eq!(extract_repository_name("not-a-url"), "");
    }

    #[test]
    fn test_truncate_output_under_limit() {
        let output = "line1\nline2\nline3";
        assert_eq!(truncate_output(output, 5), output);
    }

    #[test]
    fn test_truncate_output_over_limit() {
        let output = "line1\nline2\nline3\nline4\nline5";
        let truncated = truncate_output(output, 3);
        assert!(truncated.contains("truncated 2 lines"));
        assert!(truncated.contains("line1"));
        assert!(!truncated.contains("line5"));
    }

    #[test]
    fn test_convert_git_url_to_https_unknown_format() {
        let url = "file:///local/path";
        assert_eq!(convert_git_url_to_https(url), url);
    }
}

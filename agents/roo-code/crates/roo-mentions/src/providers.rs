//! Mention content providers for resolving @ mention types.
//!
//! Each provider handles a specific mention type:
//! - `get_problems_content()` — workspace diagnostics
//! - `get_git_changes_content()` — git working tree changes
//! - `get_terminal_output()` — terminal output
//! - `get_url_content()` — URL content fetching

use std::path::Path;

// ---------------------------------------------------------------------------
// @problems — Diagnostics
// ---------------------------------------------------------------------------

/// Get workspace diagnostics content.
///
/// In CLI/server mode, there is no VS Code extension host to provide
/// diagnostics. This returns a message indicating diagnostics are not
/// available, but accepts an optional injection of diagnostic data
/// from the server layer.
pub fn get_problems_content(injected_diagnostics: Option<&str>) -> String {
    match injected_diagnostics {
        Some(diagnostics) => diagnostics.to_string(),
        None => "Diagnostics not available in CLI mode. \
                 In VS Code extension mode, workspace diagnostics would be shown here."
            .to_string(),
    }
}

// ---------------------------------------------------------------------------
// @git-changes — Git working tree changes
// ---------------------------------------------------------------------------

/// Get git working tree changes by running `git diff --stat` and `git status --short`.
///
/// Returns a summary of staged and unstaged changes.
pub async fn get_git_changes_content(cwd: &Path) -> String {
    let mut result = String::new();

    // Run git diff --stat for unstaged changes
    match run_git_command(cwd, &["diff", "--stat"]).await {
        Ok(output) => {
            if !output.trim().is_empty() {
                result.push_str("## Unstaged Changes\n\n");
                result.push_str(&output);
                result.push('\n');
            }
        }
        Err(e) => {
            result.push_str(&format!("Error getting unstaged changes: {}\n", e));
        }
    }

    // Run git diff --cached --stat for staged changes
    match run_git_command(cwd, &["diff", "--cached", "--stat"]).await {
        Ok(output) => {
            if !output.trim().is_empty() {
                result.push_str("## Staged Changes\n\n");
                result.push_str(&output);
                result.push('\n');
            }
        }
        Err(e) => {
            result.push_str(&format!("Error getting staged changes: {}\n", e));
        }
    }

    // Run git status --short for a summary
    match run_git_command(cwd, &["status", "--short"]).await {
        Ok(output) => {
            if !output.trim().is_empty() {
                result.push_str("## Status\n\n");
                result.push_str(&output);
                result.push('\n');
            }
        }
        Err(e) => {
            result.push_str(&format!("Error getting git status: {}\n", e));
        }
    }

    if result.trim().is_empty() {
        result = "No uncommitted changes found.".to_string();
    }

    result
}

/// Run a git command and return its stdout output.
async fn run_git_command(cwd: &Path, args: &[&str]) -> Result<String, String> {
    let output = tokio::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .await
        .map_err(|e| format!("Failed to execute git: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git {} failed: {}", args.join(" "), stderr.trim()));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

// ---------------------------------------------------------------------------
// @terminal — Terminal output
// ---------------------------------------------------------------------------

/// Get terminal output content.
///
/// Accepts an optional provider function that returns the most recent
/// terminal output. When no provider is available, returns a message
/// indicating terminal output is not available.
pub fn get_terminal_output(terminal_provider: Option<&dyn Fn() -> String>) -> String {
    match terminal_provider {
        Some(provider) => provider(),
        None => "Terminal output not available. No terminal registry configured.".to_string(),
    }
}

// ---------------------------------------------------------------------------
// @url — URL content fetching
// ---------------------------------------------------------------------------

/// Maximum URL content size (1 MB).
const MAX_URL_CONTENT_SIZE: usize = 1_000_000;

/// Fetch content from a URL.
///
/// Limits the response body to [`MAX_URL_CONTENT_SIZE`] bytes.
/// Returns the body text or an error message.
pub async fn get_url_content(url: &str) -> String {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build();

    let client = match client {
        Ok(c) => c,
        Err(e) => return format!("Failed to create HTTP client: {}", e),
    };

    match client.get(url).send().await {
        Ok(response) => {
            if !response.status().is_success() {
                return format!("HTTP error: {} {}", response.status().as_u16(), response.status().canonical_reason().unwrap_or("Unknown"));
            }

            match response.text().await {
                Ok(text) => {
                    if text.len() > MAX_URL_CONTENT_SIZE {
                        format!(
                            "{}\n\n[Content truncated: {} bytes total, showing first {} bytes]\n\n{}",
                            "[Content too large]",
                            text.len(),
                            MAX_URL_CONTENT_SIZE,
                            &text[..MAX_URL_CONTENT_SIZE]
                        )
                    } else {
                        text
                    }
                }
                Err(e) => format!("Failed to read response body: {}", e),
            }
        }
        Err(e) => format!("Failed to fetch URL: {}", e),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_problems_content_no_injection() {
        let content = get_problems_content(None);
        assert!(content.contains("not available"));
    }

    #[test]
    fn test_problems_content_with_injection() {
        let diagnostics = "error TS2304: Cannot find name 'foo'.";
        let content = get_problems_content(Some(diagnostics));
        assert_eq!(content, diagnostics);
    }

    #[test]
    fn test_terminal_output_no_provider() {
        let content = get_terminal_output(None);
        assert!(content.contains("not available"));
    }

    #[test]
    fn test_terminal_output_with_provider() {
        let output = "last command output";
        let content = get_terminal_output(Some(&|| output.to_string()));
        assert_eq!(content, output);
    }

    #[tokio::test]
    async fn test_git_changes_not_a_git_repo() {
        let dir = tempfile::tempdir().unwrap();
        let content = get_git_changes_content(dir.path()).await;
        // Should contain error or "no changes" since it's not a git repo
        assert!(!content.is_empty());
    }

    #[test]
    fn test_max_url_content_size() {
        assert_eq!(MAX_URL_CONTENT_SIZE, 1_000_000);
    }
}

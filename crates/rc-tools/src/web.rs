//! Web-related tools: web_fetch, web_search, web_browser.

use std::env;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use regex::Regex;
use serde_json::{Value, json};
use tokio::process::Command;
use tokio::time::{Duration, timeout};
use uuid::Uuid;

use super::ToolExecutionContext;

pub(crate) async fn web_fetch(input: &Value, _context: &ToolExecutionContext) -> Result<String> {
    let url = input
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("web_fetch requires a url"))?;
    let max_chars = input
        .get("max_chars")
        .and_then(Value::as_u64)
        .unwrap_or(50_000) as usize;
    let response = reqwest::get(url).await.context("failed to fetch URL")?;
    let status = response.status();
    if !status.is_success() {
        return Err(anyhow!("HTTP {} for {}", status, url));
    }
    let text = response
        .text()
        .await
        .context("failed to read response body")?;
    Ok(text.chars().take(max_chars).collect())
}

pub(crate) async fn web_search(input: &Value, _context: &ToolExecutionContext) -> Result<String> {
    let query = input
        .get("query")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("web_search requires a query"))?;
    let _max_results = input
        .get("max_results")
        .and_then(Value::as_u64)
        .unwrap_or(5)
        .min(10) as usize;

    // Use the DuckDuckGo Instant Answer API (no API key required).
    let url = format!(
        "https://api.duckduckgo.com/?q={}&format=json&no_html=1",
        urlencoding::encode(query)
    );
    let response = reqwest::get(&url)
        .await
        .context("failed to query DuckDuckGo search API")?;
    let body = response
        .text()
        .await
        .context("failed to read search response body")?;

    let parsed: Value = serde_json::from_str(&body).unwrap_or_default();

    // Extract the abstract text (instant answer summary).
    let abstract_text = parsed["AbstractText"].as_str().unwrap_or("");
    let abstract_source = parsed["AbstractSource"].as_str().unwrap_or("");

    if !abstract_text.is_empty() {
        let source_info = if abstract_source.is_empty() {
            String::new()
        } else {
            format!(" (source: {abstract_source})")
        };
        Ok(format!(
            "Search results for '{}':\n{}{}",
            query, abstract_text, source_info
        ))
    } else {
        // Try to extract related topics.
        let related: Vec<String> = parsed
            .get("RelatedTopics")
            .and_then(Value::as_array)
            .map(|topics| {
                topics
                    .iter()
                    .filter_map(|topic| topic.get("Text").and_then(Value::as_str).map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        if related.is_empty() {
            Ok(format!(
                "No instant answers found for '{}'. Try a more specific query.",
                query
            ))
        } else {
            Ok(format!(
                "Related topics for '{}':\n{}",
                query,
                related.join("\n")
            ))
        }
    }
}

pub(crate) async fn web_browser_tool(
    input: &Value,
    _context: &ToolExecutionContext,
) -> Result<String> {
    let url = input
        .get("url")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("url is required"))?;
    let action = input
        .get("action")
        .and_then(Value::as_str)
        .unwrap_or("fetch");
    match action {
        "fetch" => {
            let response = reqwest::get(url).await.context("failed to fetch URL")?;
            let status = response.status();
            if !status.is_success() {
                return Err(anyhow!("HTTP {} for {}", status, url));
            }
            let text = response
                .text()
                .await
                .context("failed to read response body")?;
            // Truncate to 50K chars
            let truncated: String = text.chars().take(50_000).collect();
            Ok(truncated)
        }
        "screenshot" => capture_visual_screenshot(url, _context).await,
        "extract_links" => {
            let response = reqwest::get(url)
                .await
                .context("failed to fetch URL for link extraction")?;
            let status = response.status();
            if !status.is_success() {
                return Err(anyhow!("HTTP {} for {}", status, url));
            }
            let text = response
                .text()
                .await
                .context("failed to read response body")?;
            let re = Regex::new(r#"href\s*=\s*"([^"]+)""#).expect("valid href regex");
            let links: Vec<String> = re
                .captures_iter(&text)
                .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_owned()))
                .take(200)
                .collect();
            Ok(json!({
                "url": url,
                "links": links,
                "count": links.len(),
            })
            .to_string())
        }
        "extract_text" => {
            let response = reqwest::get(url)
                .await
                .context("failed to fetch URL for text extraction")?;
            let status = response.status();
            if !status.is_success() {
                return Err(anyhow!("HTTP {} for {}", status, url));
            }
            let text = response
                .text()
                .await
                .context("failed to read response body")?;
            // Strip HTML tags for a plain-text approximation.
            let re = Regex::new(r"<[^>]+>").expect("valid html-stripping regex");
            let plain = re.replace_all(&text, " ");
            // Collapse whitespace.
            let collapsed: String = plain.split_whitespace().collect::<Vec<_>>().join(" ");
            let truncated: String = collapsed.chars().take(50_000).collect();
            Ok(truncated)
        }
        _ => Err(anyhow!(
            "action must be 'fetch', 'extract_links', 'extract_text', or 'screenshot'"
        )),
    }
}

async fn capture_visual_screenshot(url: &str, context: &ToolExecutionContext) -> Result<String> {
    let browser = detect_headless_browser().await.ok_or_else(|| {
        anyhow!("no compatible Chromium-based browser found for screenshot capture")
    })?;

    let screenshots_dir = env::temp_dir()
        .join("remote-code-rust")
        .join("web-screenshots");
    fs::create_dir_all(&screenshots_dir).context("failed to create screenshot directory")?;

    let browser_profile_dir = env::temp_dir()
        .join("remote-code-rust")
        .join("web-browser-profiles")
        .join(Uuid::new_v4().to_string());
    fs::create_dir_all(&browser_profile_dir)
        .context("failed to create browser profile directory")?;

    let screenshot_path = screenshots_dir.join(format!("{}.png", Uuid::new_v4()));
    let timeout_secs = (context.timeout_ms / 1000).clamp(5, 60);

    let mut last_error: Option<String> = None;
    for headless_flag in ["--headless=new", "--headless"] {
        let mut command = Command::new(&browser);
        command.args([
            headless_flag,
            "--disable-gpu",
            "--hide-scrollbars",
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-background-networking",
            "--window-size=1440,1024",
        ]);
        command.arg(format!(
            "--user-data-dir={}",
            browser_profile_dir.to_string_lossy()
        ));
        command.arg(format!(
            "--screenshot={}",
            screenshot_path.to_string_lossy()
        ));
        command.arg(url);

        let output = timeout(Duration::from_secs(timeout_secs), command.output())
            .await
            .with_context(|| {
                format!(
                    "timed out after {timeout_secs}s while launching {}",
                    browser.display()
                )
            })?
            .with_context(|| format!("failed to launch browser at {}", browser.display()))?;

        if output.status.success() && screenshot_path.exists() {
            let size_bytes = fs::metadata(&screenshot_path)
                .context("failed to read screenshot metadata")?
                .len();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            let _ = fs::remove_dir_all(&browser_profile_dir);
            return Ok(json!({
                "type": "web_browser_screenshot",
                "url": url,
                "path": screenshot_path.to_string_lossy(),
                "mime_type": "image/png",
                "size_bytes": size_bytes,
                "browser": browser.to_string_lossy(),
                "stderr": if stderr.is_empty() { Value::Null } else { Value::String(stderr) },
            })
            .to_string());
        }

        last_error = Some(build_browser_failure(&output, &browser, headless_flag));
        let _ = fs::remove_file(&screenshot_path);
    }

    let _ = fs::remove_dir_all(&browser_profile_dir);
    Err(anyhow!(last_error.unwrap_or_else(|| {
        "browser exited without creating a screenshot".to_owned()
    })))
}

fn build_browser_failure(
    output: &std::process::Output,
    browser: &std::path::Path,
    headless_flag: &str,
) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let detail = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        "no browser output captured".to_owned()
    };
    format!(
        "failed to capture screenshot with {} {}: {}",
        browser.display(),
        headless_flag,
        detail
    )
}

async fn detect_headless_browser() -> Option<PathBuf> {
    if let Some(path) = browser_from_env() {
        return Some(path);
    }
    if let Some(path) = browser_from_path().await {
        return Some(path);
    }
    browser_from_known_locations()
}

fn browser_from_env() -> Option<PathBuf> {
    for key in ["REMOTE_CODE_BROWSER", "BROWSER"] {
        let candidate = PathBuf::from(env::var_os(key)?);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

async fn browser_from_path() -> Option<PathBuf> {
    #[cfg(windows)]
    let resolver = "where";
    #[cfg(not(windows))]
    let resolver = "which";

    for name in browser_binary_names() {
        let output = Command::new(resolver).arg(name).output().await.ok()?;
        if !output.status.success() {
            continue;
        }
        let candidate = String::from_utf8_lossy(&output.stdout)
            .lines()
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from);
        if let Some(path) = candidate
            && path.exists()
        {
            return Some(path);
        }
    }
    None
}

fn browser_from_known_locations() -> Option<PathBuf> {
    browser_known_locations()
        .into_iter()
        .find(|candidate| candidate.exists())
}

fn browser_binary_names() -> &'static [&'static str] {
    #[cfg(windows)]
    {
        &["msedge.exe", "chrome.exe"]
    }
    #[cfg(target_os = "macos")]
    {
        &[
            "Google Chrome",
            "Microsoft Edge",
            "Chromium",
            "google-chrome",
            "microsoft-edge",
            "chromium",
        ]
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        &[
            "google-chrome",
            "google-chrome-stable",
            "microsoft-edge",
            "chromium",
            "chromium-browser",
        ]
    }
}

fn browser_known_locations() -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let mut candidates = Vec::new();
        if let Some(program_files_x86) = env::var_os("ProgramFiles(x86)") {
            candidates.push(
                PathBuf::from(&program_files_x86)
                    .join("Microsoft")
                    .join("Edge")
                    .join("Application")
                    .join("msedge.exe"),
            );
            candidates.push(
                PathBuf::from(&program_files_x86)
                    .join("Google")
                    .join("Chrome")
                    .join("Application")
                    .join("chrome.exe"),
            );
        }
        if let Some(program_files) = env::var_os("ProgramFiles") {
            candidates.push(
                PathBuf::from(&program_files)
                    .join("Microsoft")
                    .join("Edge")
                    .join("Application")
                    .join("msedge.exe"),
            );
            candidates.push(
                PathBuf::from(&program_files)
                    .join("Google")
                    .join("Chrome")
                    .join("Application")
                    .join("chrome.exe"),
            );
        }
        if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
            candidates.push(
                PathBuf::from(local_app_data)
                    .join("Google")
                    .join("Chrome")
                    .join("Application")
                    .join("chrome.exe"),
            );
        }
        candidates
    }
    #[cfg(target_os = "macos")]
    {
        vec![
            PathBuf::from("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome"),
            PathBuf::from("/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge"),
            PathBuf::from("/Applications/Chromium.app/Contents/MacOS/Chromium"),
        ]
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        vec![
            PathBuf::from("/usr/bin/google-chrome"),
            PathBuf::from("/usr/bin/google-chrome-stable"),
            PathBuf::from("/usr/bin/microsoft-edge"),
            PathBuf::from("/usr/bin/chromium"),
            PathBuf::from("/usr/bin/chromium-browser"),
        ]
    }
}

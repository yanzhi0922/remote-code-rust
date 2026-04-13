//! Web-related tools: web_fetch, web_search, web_browser.

use anyhow::{Context, Result, anyhow};
use regex::Regex;
use serde_json::{Value, json};

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
        "screenshot" => {
            // Structural page snapshot: fetch HTML and extract metadata.
            let response = reqwest::get(url)
                .await
                .context("failed to fetch URL for screenshot")?;
            let status = response.status();
            if !status.is_success() {
                return Err(anyhow!("HTTP {} for {}", status, url));
            }
            let text = response
                .text()
                .await
                .context("failed to read response body")?;

            // Extract <title>.
            let title_re = Regex::new(r"(?i)<title[^>]*>(.*?)</title>").expect("valid title regex");
            let title = title_re
                .captures(&text)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().trim().to_owned())
                .unwrap_or_default();

            // Extract headings (h1–h6).
            let heading_re =
                Regex::new(r"(?i)<h[1-6][^>]*>(.*?)</h[1-6]>").expect("valid heading regex");
            let headings: Vec<String> = heading_re
                .captures_iter(&text)
                .filter_map(|c| c.get(1).map(|m| m.as_str().trim().to_owned()))
                .take(20)
                .collect();

            // Count images and links.
            let img_count = Regex::new(r"(?i)<img")
                .expect("valid img regex")
                .find_iter(&text)
                .count();
            let link_count = Regex::new(r"(?i)<a ")
                .expect("valid link regex")
                .find_iter(&text)
                .count();

            // Extract meta description.
            let meta_re = Regex::new(r#"(?i)<meta\s+name="description"\s+content="([^"]*)""#)
                .expect("valid meta regex");
            let description = meta_re
                .captures(&text)
                .and_then(|c| c.get(1))
                .map(|m| m.as_str().to_owned())
                .unwrap_or_default();

            // Plain-text preview.
            let strip_re = Regex::new(r"<[^>]+>").expect("valid html-stripping regex");
            let plain = strip_re.replace_all(&text, " ");
            let collapsed: String = plain.split_whitespace().collect::<Vec<_>>().join(" ");
            let preview: String = collapsed.chars().take(3000).collect();

            Ok(json!({
                "type": "page_snapshot",
                "url": url,
                "title": title,
                "description": description,
                "headings": headings,
                "image_count": img_count,
                "link_count": link_count,
                "text_preview": preview,
                "note": "Structural snapshot extracted from HTML. For visual screenshots, use a headed browser."
            })
            .to_string())
        }
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

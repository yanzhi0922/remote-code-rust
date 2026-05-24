//! Navigate browser tab to a URL.

use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};

use crate::browser_pool::global_pool;

pub async fn navigate(input: &Value) -> Result<String> {
    let url = input["url"]
        .as_str()
        .ok_or_else(|| anyhow!("url is required"))?;
    let wait_until = input["waitUntil"].as_str().unwrap_or("load");

    let pool = global_pool().await?;
    let pool = pool.lock().await;

    let page = pool.get_or_create_page().await?;
    page.goto(url).await.context("navigation failed")?;

    if wait_until != "none" {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    let title = page.get_title().await.ok().flatten().unwrap_or_default();
    let final_url = page.url().await.unwrap_or_default();

    Ok(json!({
        "type": "navigate",
        "url": final_url,
        "title": title,
        "status": "success",
    })
    .to_string())
}

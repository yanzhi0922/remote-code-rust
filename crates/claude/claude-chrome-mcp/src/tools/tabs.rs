//! List, create, and close browser tabs.

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use crate::browser_pool::global_pool;

pub async fn tabs_list(_input: &Value) -> Result<String> {
    let pool = global_pool().await?;
    let pool = pool.lock().await;

    let pages = pool.pages().await?;
    let mut tabs = Vec::new();

    for page in &pages {
        let title = page.get_title().await.ok().flatten().unwrap_or_default();
        let url = page.url().await.unwrap_or_default().unwrap_or_default();
        tabs.push(json!({
            "title": title,
            "url": url,
        }));
    }

    Ok(json!({
        "type": "tabs_list",
        "count": tabs.len(),
        "tabs": tabs,
    })
    .to_string())
}

pub async fn tabs_create(input: &Value) -> Result<String> {
    let url = input["url"].as_str().unwrap_or("about:blank");

    let pool = global_pool().await?;
    let pool = pool.lock().await;

    let page = pool.new_page(url).await?;
    let title = page.get_title().await.ok().flatten().unwrap_or_default();

    Ok(json!({
        "type": "tabs_create",
        "url": url,
        "title": title,
        "status": "success",
    })
    .to_string())
}

pub async fn tabs_close(input: &Value) -> Result<String> {
    let pool = global_pool().await?;
    let pool = pool.lock().await;

    let pages = pool.pages().await?;
    let close_url = input["url"].as_str();

    let mut target = None;
    if let Some(close_url) = close_url {
        for page in pages {
            let url = page.url().await.unwrap_or_default().unwrap_or_default();
            if url.contains(close_url) {
                target = Some(page);
                break;
            }
        }
    } else {
        target = pages.into_iter().next();
    }

    if let Some(page) = target {
        let title = page.get_title().await.ok().flatten().unwrap_or_default();
        page.close().await.ok();
        Ok(json!({
            "type": "tabs_close",
            "closedTitle": title,
            "status": "success",
        })
        .to_string())
    } else {
        Err(anyhow!("no matching tab found to close"))
    }
}

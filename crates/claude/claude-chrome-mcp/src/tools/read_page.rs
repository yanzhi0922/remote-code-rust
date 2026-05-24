//! Read high-level page state (title, URL, metadata).

use anyhow::Result;
use serde_json::{Value, json};

use crate::browser_pool::global_pool;

pub async fn read_page(_input: &Value) -> Result<String> {
    let pool = global_pool().await?;
    let pool = pool.lock().await;

    let page = pool.get_or_create_page().await?;

    let title = page.get_title().await.ok().flatten().unwrap_or_default();
    let url = page.url().await.unwrap_or_default();

    let meta_desc: String = page
        .evaluate("document.querySelector('meta[name=\"description\"]')?.content || ''")
        .await
        .map(|v| v.value().cloned().unwrap_or_default().to_string())
        .unwrap_or_default();

    let viewport = page
        .evaluate("JSON.stringify({width: window.innerWidth, height: window.innerHeight})")
        .await
        .map(|v| v.value().cloned().unwrap_or_default().to_string())
        .unwrap_or_else(|_| r#"{"width":0,"height":0}"#.to_owned());

    Ok(json!({
        "type": "read_page",
        "title": title,
        "url": url,
        "metaDescription": meta_desc,
        "viewport": serde_json::from_str::<Value>(&viewport).unwrap_or_default(),
    })
    .to_string())
}

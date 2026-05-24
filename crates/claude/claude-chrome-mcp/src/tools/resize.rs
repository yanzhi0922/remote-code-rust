//! Resize the browser window.

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use crate::browser_pool::global_pool;

pub async fn resize_window(input: &Value) -> Result<String> {
    let width = input["width"]
        .as_u64()
        .ok_or_else(|| anyhow!("width is required"))? as u32;
    let height = input["height"]
        .as_u64()
        .ok_or_else(|| anyhow!("height is required"))? as u32;

    let pool = global_pool().await?;
    let pool = pool.lock().await;
    let page = pool.get_or_create_page().await?;

    let js = format!(
        "(function() {{ \
            window.resizeTo({w}, {h}); \
            return JSON.stringify({{ width: window.innerWidth, height: window.innerHeight }}); \
        }})()",
        w = width,
        h = height,
    );

    let result_str = page
        .evaluate(js.as_str())
        .await
        .map(|v| v.value().cloned().unwrap_or_default().to_string())
        .unwrap_or_default();

    Ok(json!({
        "type": "resize_window",
        "requestedWidth": width,
        "requestedHeight": height,
        "actual": serde_json::from_str::<Value>(&result_str).unwrap_or_default(),
    })
    .to_string())
}

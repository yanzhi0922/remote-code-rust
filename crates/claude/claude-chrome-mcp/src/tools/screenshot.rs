//! Capture a screenshot of the browser page via CDP.

use std::env;

use anyhow::{Context, Result};
use serde_json::{Value, json};
use uuid::Uuid;

use crate::browser_pool::global_pool;

pub async fn screenshot(input: &Value) -> Result<String> {
    let full_page = input["fullPage"].as_bool().unwrap_or(false);
    let selector = input["selector"].as_str();
    let format = input["format"].as_str().unwrap_or("png");

    let pool = global_pool().await?;
    let pool = pool.lock().await;
    let page = pool.get_or_create_page().await?;

    // If a selector is specified, scroll element into view first.
    if let Some(sel) = selector {
        let js = format!(
            "(function() {{ \
                var el = document.querySelector({sel:?}); \
                if (el) el.scrollIntoView({{ block: 'center' }}); \
            }})()",
            sel = sel,
        );
        page.evaluate(js.as_str()).await.ok();
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    let mut builder = chromiumoxide::page::ScreenshotParams::builder();
    if full_page {
        builder = builder.full_page(true);
    }

    let bytes = page
        .screenshot(builder.build())
        .await
        .context("screenshot capture failed")?;

    let screenshots_dir = env::temp_dir()
        .join("remote-code-rust")
        .join("chrome-mcp-screenshots");
    std::fs::create_dir_all(&screenshots_dir)?;

    let filename = format!("{}.{}", Uuid::new_v4(), format);
    let path = screenshots_dir.join(&filename);
    std::fs::write(&path, &bytes).context("failed to write screenshot file")?;

    let mime = match format {
        "jpeg" => "image/jpeg",
        _ => "image/png",
    };

    Ok(json!({
        "type": "chrome_mcp_screenshot",
        "path": path.to_string_lossy(),
        "mime_type": mime,
        "size_bytes": bytes.len(),
        "full_page": full_page,
        "selector": selector,
        "format": format,
    })
    .to_string())
}

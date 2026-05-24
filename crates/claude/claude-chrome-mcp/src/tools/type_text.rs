//! Type text into an element identified by CSS selector.

use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};

use crate::browser_pool::global_pool;

pub async fn type_text(input: &Value) -> Result<String> {
    let selector = input["selector"]
        .as_str()
        .ok_or_else(|| anyhow!("selector is required"))?;
    let text = input["text"]
        .as_str()
        .ok_or_else(|| anyhow!("text is required"))?;
    let clear = input["clear"].as_bool().unwrap_or(true);

    let pool = global_pool().await?;
    let pool = pool.lock().await;
    let page = pool.get_or_create_page().await?;

    let element = page
        .find_element(selector)
        .await
        .with_context(|| format!("element not found: {selector}"))?;

    element.click().await.context("failed to focus element")?;

    if clear {
        // Select all existing text and delete it.
        element
            .press_key("Control+a")
            .await
            .context("failed to select text")?;
        element.press_key("Backspace").await.ok();
    }

    element
        .type_str(text)
        .await
        .context("failed to type text")?;

    Ok(json!({
        "type": "type_text",
        "selector": selector,
        "textLength": text.len(),
        "cleared": clear,
        "status": "success",
    })
    .to_string())
}

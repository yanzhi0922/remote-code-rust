//! Extract visible text from the active page.

use anyhow::Result;
use serde_json::{Value, json};

use crate::browser_pool::global_pool;

const DEFAULT_MAX_CHARS: usize = 100_000;

pub async fn get_page_text(input: &Value) -> Result<String> {
    let selector = input["selector"].as_str().unwrap_or("body");
    let max_chars = input["maxChars"]
        .as_u64()
        .unwrap_or(DEFAULT_MAX_CHARS as u64) as usize;

    let pool = global_pool().await?;
    let pool = pool.lock().await;
    let page = pool.get_or_create_page().await?;

    let js = format!(
        "(function() {{ \
            var el = document.querySelector({sel:?}); \
            if (!el) return JSON.stringify({{ error: \"element not found\", text: \"\" }}); \
            return JSON.stringify({{ text: el.innerText }}); \
        }})()",
        sel = selector,
    );

    let result_str = page
        .evaluate(js.as_str())
        .await
        .map(|v| v.value().cloned().unwrap_or_default().to_string())
        .unwrap_or_default();

    let parsed: Value =
        serde_json::from_str(&result_str).unwrap_or_else(|_| json!({"text": result_str}));

    let raw_text = parsed["text"].as_str().unwrap_or("");
    let truncated = raw_text.len() > max_chars;
    let text: String = raw_text.chars().take(max_chars).collect();

    Ok(json!({
        "type": "get_page_text",
        "selector": selector,
        "textLength": text.len(),
        "truncated": truncated,
        "text": text,
    })
    .to_string())
}

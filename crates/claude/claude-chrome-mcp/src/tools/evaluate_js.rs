//! Evaluate JavaScript in the browser page context.

use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};

use crate::browser_pool::global_pool;

const DEFAULT_MAX_CHARS: usize = 50_000;

pub async fn evaluate_js(input: &Value) -> Result<String> {
    let expression = input["expression"]
        .as_str()
        .ok_or_else(|| anyhow!("expression is required"))?;
    let max_chars = input["maxChars"]
        .as_u64()
        .unwrap_or(DEFAULT_MAX_CHARS as u64) as usize;

    let pool = global_pool().await?;
    let pool = pool.lock().await;
    let page = pool.get_or_create_page().await?;

    let result = page
        .evaluate(expression)
        .await
        .context("JavaScript evaluation failed")?;

    let value = result.value().cloned().unwrap_or_default();
    let value_str = value.to_string();
    let truncated = value_str.len() > max_chars;
    let output: String = value_str.chars().take(max_chars).collect();

    let result_type = if value.is_string() {
        "string"
    } else if value.is_number() {
        "number"
    } else if value.is_boolean() {
        "boolean"
    } else if value.is_null() {
        "null"
    } else {
        "object"
    };

    Ok(json!({
        "type": "evaluate_js",
        "result": output,
        "resultType": result_type,
        "truncated": truncated,
    })
    .to_string())
}

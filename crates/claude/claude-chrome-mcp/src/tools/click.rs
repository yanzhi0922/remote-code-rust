//! Click an element by CSS selector or coordinates.

use anyhow::{Context, Result, anyhow};
use serde_json::{Value, json};

use crate::browser_pool::global_pool;

pub async fn click(input: &Value) -> Result<String> {
    let pool = global_pool().await?;
    let pool = pool.lock().await;
    let page = pool.get_or_create_page().await?;

    if let Some(selector) = input["selector"].as_str() {
        let element = page
            .find_element(selector)
            .await
            .with_context(|| format!("element not found: {selector}"))?;
        element.click().await.context("click failed")?;

        tokio::time::sleep(std::time::Duration::from_millis(300)).await;

        let title = page.get_title().await.ok().flatten().unwrap_or_default();
        let url = page.url().await.unwrap_or_default();

        Ok(json!({
            "type": "click",
            "selector": selector,
            "status": "success",
            "pageTitle": title,
            "pageUrl": url,
        })
        .to_string())
    } else if let (Some(x), Some(y)) = (input["x"].as_f64(), input["y"].as_f64()) {
        let xi = x as i32;
        let yi = y as i32;
        let js = format!(
            "(function() {{ \
                var el = document.elementFromPoint({xi}, {yi}); \
                if (el) {{ \
                    el.click(); \
                    return JSON.stringify({{ clicked: true, tag: el.tagName, \
                        text: (el.innerText || '').substring(0, 100) }}); \
                }} \
                return JSON.stringify({{ clicked: false }}); \
            }})()"
        );

        let result_str = page
            .evaluate(js.as_str())
            .await
            .map(|v| v.value().cloned().unwrap_or_default().to_string())
            .unwrap_or_default();

        Ok(json!({
            "type": "click",
            "x": x,
            "y": y,
            "result": serde_json::from_str::<Value>(&result_str).unwrap_or_default(),
        })
        .to_string())
    } else {
        Err(anyhow!(
            "either 'selector' or 'x'+'y' coordinates are required"
        ))
    }
}

//! Find a pattern within page content.

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use crate::browser_pool::global_pool;

pub async fn find(input: &Value) -> Result<String> {
    let pattern = input["pattern"]
        .as_str()
        .ok_or_else(|| anyhow!("pattern is required"))?;
    let selector = input["selector"].as_str();

    let pool = global_pool().await?;
    let pool = pool.lock().await;
    let page = pool.get_or_create_page().await?;

    let js = if let Some(sel) = selector {
        format!(
            "(function() {{ \
                var root = document.querySelector({sel:?}); \
                if (!root) return JSON.stringify({{ matches: [], count: 0 }}); \
                var text = root.innerText; \
                var re = new RegExp({pat:?}, 'gi'); \
                var m = [...text.matchAll(re)].slice(0, 50).map(function(x) {{ \
                    return {{ match: x[0], index: x.index, \
                        context: text.substring(Math.max(0, x.index - 40), x.index + x[0].length + 40) }}; \
                }}); \
                return JSON.stringify({{ matches: m, count: m.length }}); \
            }})()",
            sel = sel,
            pat = pattern,
        )
    } else {
        format!(
            "(function() {{ \
                var text = document.body.innerText; \
                var re = new RegExp({pat:?}, 'gi'); \
                var m = [...text.matchAll(re)].slice(0, 50).map(function(x) {{ \
                    return {{ match: x[0], index: x.index, \
                        context: text.substring(Math.max(0, x.index - 40), x.index + x[0].length + 40) }}; \
                }}); \
                return JSON.stringify({{ matches: m, count: m.length }}); \
            }})()",
            pat = pattern,
        )
    };

    let result_str = page
        .evaluate(js.as_str())
        .await
        .map(|v| v.value().cloned().unwrap_or_default().to_string())
        .unwrap_or_else(|_| "{}".to_owned());

    Ok(json!({
        "type": "find",
        "pattern": pattern,
        "result": serde_json::from_str::<Value>(&result_str).unwrap_or_default(),
    })
    .to_string())
}

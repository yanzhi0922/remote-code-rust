//! Read captured network requests.

use anyhow::Result;
use serde_json::{Value, json};

use crate::browser_pool::global_pool;

const SETUP_JS: &str = r#"
(function() {
    if (!window.__chromeMcpNetwork) {
        window.__chromeMcpNetwork = [];
        var observer = new PerformanceObserver(function(list) {
            var entries = list.getEntries();
            for (var i = 0; i < entries.length; i++) {
                var entry = entries[i];
                window.__chromeMcpNetwork.push({
                    url: entry.name,
                    type: entry.initiatorType,
                    duration: Math.round(entry.duration),
                    startTime: Math.round(entry.startTime),
                    transferSize: entry.transferSize || 0
                });
            }
            if (window.__chromeMcpNetwork.length > 500) {
                window.__chromeMcpNetwork = window.__chromeMcpNetwork.slice(-250);
            }
        });
        observer.observe({ type: 'resource', buffered: true });
    }
    return JSON.stringify({ count: window.__chromeMcpNetwork.length });
})()
"#;

pub async fn network_requests(input: &Value) -> Result<String> {
    let url_pattern = input["urlPattern"].as_str().unwrap_or("");
    let limit = input["limit"].as_u64().unwrap_or(100) as usize;

    let pool = global_pool().await?;
    let pool = pool.lock().await;
    let page = pool.get_or_create_page().await?;

    page.evaluate(SETUP_JS).await.ok();

    let escaped_pattern = url_pattern.replace('\\', "\\\\").replace('\'', "\\'");
    let filter_js = format!(
        "(function() {{ \
            var reqs = window.__chromeMcpNetwork || []; \
            if ('{pattern}') {{ \
                var re = new RegExp('{pattern}', 'i'); \
                reqs = reqs.filter(function(r) {{ return re.test(r.url); }}); \
            }} \
            return JSON.stringify(reqs.slice(-{limit})); \
        }})()",
        pattern = escaped_pattern,
        limit = limit,
    );

    let result_str = page
        .evaluate(filter_js.as_str())
        .await
        .map(|v| v.value().cloned().unwrap_or_default().to_string())
        .unwrap_or_else(|_| "[]".to_owned());

    let requests: Value = serde_json::from_str(&result_str).unwrap_or_default();

    Ok(json!({
        "type": "network_requests",
        "count": requests.as_array().map(|a| a.len()).unwrap_or(0),
        "requests": requests,
    })
    .to_string())
}

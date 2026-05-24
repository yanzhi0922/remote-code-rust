//! Read browser console messages.

use anyhow::Result;
use serde_json::{Value, json};

use crate::browser_pool::global_pool;

const SETUP_JS: &str = r#"
(function() {
    if (!window.__chromeMcpConsole) {
        window.__chromeMcpConsole = [];
        var origLog = console.log;
        var origWarn = console.warn;
        var origError = console.error;
        var origInfo = console.info;
        var capture = function(level) {
            return function() {
                var args = Array.prototype.slice.call(arguments);
                var text = args.map(function(a) { return typeof a === 'object' ? JSON.stringify(a) : String(a); }).join(' ');
                window.__chromeMcpConsole.push({ level: level, text: text, timestamp: Date.now() });
                if (window.__chromeMcpConsole.length > 1000) {
                    window.__chromeMcpConsole = window.__chromeMcpConsole.slice(-500);
                }
            };
        };
        console.log = capture('log');
        console.warn = capture('warn');
        console.error = capture('error');
        console.info = capture('info');
    }
    return JSON.stringify({ count: window.__chromeMcpConsole.length });
})()
"#;

pub async fn console_messages(input: &Value) -> Result<String> {
    let pattern = input["pattern"].as_str().unwrap_or("");
    let level = input["level"].as_str().unwrap_or("all");
    let limit = input["limit"].as_u64().unwrap_or(100) as usize;

    let pool = global_pool().await?;
    let pool = pool.lock().await;
    let page = pool.get_or_create_page().await?;

    page.evaluate(SETUP_JS).await.ok();

    let escaped_pattern = pattern.replace('\\', "\\\\").replace('\'', "\\'");
    let filter_js = format!(
        "(function() {{ \
            var msgs = window.__chromeMcpConsole || []; \
            var filtered = msgs; \
            if ('{level}' !== 'all') {{ \
                filtered = filtered.filter(function(m) {{ return m.level === '{level}'; }}); \
            }} \
            if ('{pattern}') {{ \
                var re = new RegExp('{pattern}', 'i'); \
                filtered = filtered.filter(function(m) {{ return re.test(m.text); }}); \
            }} \
            return JSON.stringify(filtered.slice(-{limit})); \
        }})()",
        level = level,
        pattern = escaped_pattern,
        limit = limit,
    );

    let result_str = page
        .evaluate(filter_js.as_str())
        .await
        .map(|v| v.value().cloned().unwrap_or_default().to_string())
        .unwrap_or_else(|_| "[]".to_owned());

    let messages: Value = serde_json::from_str(&result_str).unwrap_or_default();

    Ok(json!({
        "type": "console_messages",
        "count": messages.as_array().map(|a| a.len()).unwrap_or(0),
        "messages": messages,
    })
    .to_string())
}

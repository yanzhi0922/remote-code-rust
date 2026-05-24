//! Fill form inputs (text fields, selects, checkboxes).

use anyhow::{Result, anyhow};
use serde_json::{Value, json};

use crate::browser_pool::global_pool;

pub async fn form_input(input: &Value) -> Result<String> {
    let selector = input["selector"]
        .as_str()
        .ok_or_else(|| anyhow!("selector is required"))?;
    let value = input["value"]
        .as_str()
        .ok_or_else(|| anyhow!("value is required"))?;

    let pool = global_pool().await?;
    let pool = pool.lock().await;
    let page = pool.get_or_create_page().await?;

    let js = format!(
        "(function() {{ \
            var el = document.querySelector({selector:?}); \
            if (!el) return JSON.stringify({{ error: \"element not found\" }}); \
            var tag = el.tagName.toLowerCase(); \
            var type = (el.type || '').toLowerCase(); \
            if (tag === 'select') {{ \
                var opts = Array.from(el.options); \
                var match = opts.find(function(o) {{ return o.value === {val:?} || o.textContent.trim() === {val:?}; }}); \
                if (match) {{ \
                    el.value = match.value; \
                    el.dispatchEvent(new Event('change', {{ bubbles: true }})); \
                    return JSON.stringify({{ filled: true, tag: tag, value: match.value }}); \
                }} \
                return JSON.stringify({{ error: \"option not found\", availableValues: opts.map(function(o){{ return o.value; }}) }}); \
            }} \
            if (type === 'checkbox' || type === 'radio') {{ \
                el.checked = ({val:?} === 'true' || {val:?} === 'checked'); \
                el.dispatchEvent(new Event('change', {{ bubbles: true }})); \
                return JSON.stringify({{ filled: true, tag: tag, type: type, checked: el.checked }}); \
            }} \
            el.focus(); \
            el.value = ''; \
            el.value = {val:?}; \
            el.dispatchEvent(new Event('input', {{ bubbles: true }})); \
            el.dispatchEvent(new Event('change', {{ bubbles: true }})); \
            return JSON.stringify({{ filled: true, tag: tag, type: type, value: el.value.substring(0, 100) }}); \
        }})()",
        selector = selector,
        val = value,
    );

    let result_str = page
        .evaluate(js.as_str())
        .await
        .map(|v| v.value().cloned().unwrap_or_default().to_string())
        .unwrap_or_default();

    Ok(json!({
        "type": "form_input",
        "selector": selector,
        "result": serde_json::from_str::<Value>(&result_str).unwrap_or_default(),
    })
    .to_string())
}

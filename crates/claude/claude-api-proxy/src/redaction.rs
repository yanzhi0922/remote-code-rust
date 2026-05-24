use serde_json::Value;

const MAX_UPSTREAM_ERROR_CHARS: usize = 512;

pub(crate) fn upstream_error_summary(body: &str) -> String {
    let redacted = serde_json::from_str::<Value>(body)
        .map(|mut value| {
            redact_json_value(&mut value);
            value.to_string()
        })
        .unwrap_or_else(|_| redact_plain_text(body));

    truncate_chars(&redacted, MAX_UPSTREAM_ERROR_CHARS)
}

fn redact_json_value(value: &mut Value) {
    match value {
        Value::Object(map) => {
            for (key, value) in map.iter_mut() {
                if is_sensitive_key(key) {
                    *value = Value::String("<redacted>".to_owned());
                } else {
                    redact_json_value(value);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                redact_json_value(value);
            }
        }
        _ => {}
    }
}

fn is_sensitive_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase();
    key.contains("authorization")
        || key.contains("api_key")
        || key.contains("apikey")
        || key.contains("secret")
        || key.contains("token")
}

fn redact_plain_text(body: &str) -> String {
    body.split_whitespace()
        .map(|word| {
            let lower = word.to_ascii_lowercase();
            if lower.contains("sk-")
                || lower.contains("authorization:")
                || lower.contains("api_key")
                || lower.contains("apikey")
                || lower.contains("secret")
                || lower.contains("token")
            {
                "<redacted>"
            } else {
                word
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_owned();
    }

    let mut truncated = value.chars().take(max_chars).collect::<String>();
    truncated.push_str("...");
    truncated
}

#[cfg(test)]
mod tests {
    use super::upstream_error_summary;

    #[test]
    fn upstream_error_summary_redacts_json_secrets() {
        let body = r#"{"error":{"message":"bad key","api_key":"sk-live-secret","nested":{"token":"abc"}}}"#;

        let summary = upstream_error_summary(body);

        assert!(!summary.contains("sk-live-secret"));
        assert!(!summary.contains("abc"));
        assert!(summary.contains("<redacted>"));
    }

    #[test]
    fn upstream_error_summary_truncates_large_bodies() {
        let body = "x".repeat(600);

        let summary = upstream_error_summary(&body);

        assert!(summary.len() < body.len());
        assert!(summary.ends_with("..."));
    }
}

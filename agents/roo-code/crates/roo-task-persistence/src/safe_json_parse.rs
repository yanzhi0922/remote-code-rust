//! Safe JSON Parse
//!
//! Safely parses JSON without crashing on invalid input.
//! Mirrors `safeJsonParse.ts`.

use serde::de::DeserializeOwned;

// ---------------------------------------------------------------------------
// Safe JSON parse
// ---------------------------------------------------------------------------

/// Safely parse a JSON string, returning a default value on failure.
///
/// Source: `.research/Roo-Code/packages/core/src/message-utils/safeJsonParse.ts`
pub fn safe_json_parse<T: DeserializeOwned>(
    json_string: Option<&str>,
    default_value: Option<T>,
) -> Option<T> {
    let s = match json_string {
        Some(s) if !s.is_empty() => s,
        _ => return default_value,
    };

    match serde_json::from_str::<T>(s) {
        Ok(result) => Some(result),
        Err(e) => {
            eprintln!("Error parsing JSON: {}", e);
            default_value
        }
    }
}

/// Safely parse a JSON string with a required default.
pub fn safe_json_parse_or<T: DeserializeOwned>(
    json_string: Option<&str>,
    default: T,
) -> T {
    safe_json_parse(json_string, Some(default)).unwrap()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_parse_valid_json() {
        let result: Option<serde_json::Value> =
            safe_json_parse(Some(r#"{"key": "value"}"#), None);
        assert!(result.is_some());
        assert_eq!(result.unwrap()["key"], "value");
    }

    #[test]
    fn test_parse_invalid_json() {
        let default = Some(serde_json::json!({"default": true}));
        let result: Option<serde_json::Value> =
            safe_json_parse(Some("invalid json"), default.clone());
        assert_eq!(result, default);
    }

    #[test]
    fn test_parse_none_input() {
        let default = Some(42i32);
        let result: Option<i32> = safe_json_parse(None, default);
        assert_eq!(result, Some(42));
    }

    #[test]
    fn test_parse_empty_string() {
        let default = Some("fallback".to_string());
        let result: Option<String> = safe_json_parse(Some(""), default);
        assert_eq!(result, Some("fallback".to_string()));
    }

    #[test]
    fn test_parse_array() {
        let result: Option<Vec<i32>> = safe_json_parse(Some("[1,2,3]"), None);
        assert_eq!(result, Some(vec![1, 2, 3]));
    }

    #[test]
    fn test_parse_hashmap() {
        let result: Option<HashMap<String, String>> =
            safe_json_parse(Some(r#"{"a":"b","c":"d"}"#), None);
        assert!(result.is_some());
        let map = result.unwrap();
        assert_eq!(map.get("a"), Some(&"b".to_string()));
        assert_eq!(map.get("c"), Some(&"d".to_string()));
    }

    #[test]
    fn test_safe_json_parse_or_valid() {
        let result: Vec<i32> = safe_json_parse_or(Some("[1,2]"), vec![]);
        assert_eq!(result, vec![1, 2]);
    }

    #[test]
    fn test_safe_json_parse_or_invalid() {
        let result: Vec<i32> = safe_json_parse_or(Some("bad"), vec![99]);
        assert_eq!(result, vec![99]);
    }
}

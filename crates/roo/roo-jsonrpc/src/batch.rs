use crate::types::Message;

/// Error type for batch operations
#[derive(Debug, thiserror::Error)]
pub enum BatchError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Empty batch is not allowed")]
    EmptyBatch,
}

/// Encode a slice of messages as a JSON-RPC batch request/response array.
/// Returns an error if the slice is empty (empty batches are not per spec).
pub fn encode_batch(messages: &[Message]) -> Result<String, BatchError> {
    if messages.is_empty() {
        return Err(BatchError::EmptyBatch);
    }
    Ok(serde_json::to_string(messages)?)
}

/// Decode a JSON string that may be either a single message or a batch array.
/// Returns a Vec of messages. Handles both `[{...}]` and `{...}` formats.
pub fn decode_batch(data: &str) -> Result<Vec<Message>, BatchError> {
    let data = data.trim();

    if data.starts_with('[') {
        // Batch format
        let messages: Vec<Message> = serde_json::from_str(data)?;
        Ok(messages)
    } else if data.starts_with('{') {
        // Single message
        let msg: Message = serde_json::from_str(data)?;
        Ok(vec![msg])
    } else {
        // Try parsing as array first, then as single
        Err(BatchError::Serialization(serde_json::from_str::<
            serde_json::Value,
        >(data)
        .unwrap_err()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_encode_batch_multiple() {
        let msgs = vec![
            Message::request(1, "method1", json!({})),
            Message::request(2, "method2", json!({})),
        ];
        let encoded = encode_batch(&msgs).unwrap();
        assert!(encoded.starts_with('['));
        assert!(encoded.ends_with(']'));
        assert!(encoded.contains("method1"));
        assert!(encoded.contains("method2"));
    }

    #[test]
    fn test_encode_batch_single() {
        let msgs = vec![Message::request(1, "test", json!({}))];
        let encoded = encode_batch(&msgs).unwrap();
        assert!(encoded.starts_with('['));
    }

    #[test]
    fn test_encode_batch_empty_error() {
        let result = encode_batch(&[]);
        assert!(result.is_err());
        match result.unwrap_err() {
            BatchError::EmptyBatch => {}
            other => panic!("Expected EmptyBatch, got: {other}"),
        }
    }

    #[test]
    fn test_decode_batch_array() {
        let json = r#"[
            {"jsonrpc":"2.0","id":1,"method":"method1","params":{}},
            {"jsonrpc":"2.0","id":2,"method":"method2","params":{}}
        ]"#;
        let msgs = decode_batch(json).unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].method.as_deref(), Some("method1"));
        assert_eq!(msgs[1].method.as_deref(), Some("method2"));
    }

    #[test]
    fn test_decode_batch_single_object() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"test","params":{}}"#;
        let msgs = decode_batch(json).unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].method.as_deref(), Some("test"));
    }

    #[test]
    fn test_decode_batch_roundtrip() {
        let original = vec![
            Message::request(1, "method1", json!({"key": "value1"})),
            Message::request(2, "method2", json!({"key": "value2"})),
            Message::notification("notify", json!({"event": "done"})),
        ];
        let encoded = encode_batch(&original).unwrap();
        let decoded = decode_batch(&encoded).unwrap();
        assert_eq!(decoded.len(), 3);
        assert_eq!(decoded[0].method.as_deref(), Some("method1"));
        assert_eq!(decoded[1].method.as_deref(), Some("method2"));
        assert_eq!(decoded[2].method.as_deref(), Some("notify"));
        assert!(decoded[2].is_notification());
    }

    #[test]
    fn test_decode_batch_mixed_types() {
        let json = r#"[
            {"jsonrpc":"2.0","id":1,"method":"test","params":{}},
            {"jsonrpc":"2.0","id":2,"result":{"status":"ok"}},
            {"jsonrpc":"2.0","id":3,"error":{"code":-32600,"message":"Invalid Request"}}
        ]"#;
        let msgs = decode_batch(json).unwrap();
        assert_eq!(msgs.len(), 3);
        assert!(msgs[0].is_request());
        assert!(msgs[1].is_response());
        assert!(msgs[2].is_error());
    }

    #[test]
    fn test_decode_batch_invalid_json() {
        let result = decode_batch("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_decode_batch_empty_array() {
        let json = "[]";
        let msgs = decode_batch(json).unwrap();
        assert!(msgs.is_empty());
    }
}

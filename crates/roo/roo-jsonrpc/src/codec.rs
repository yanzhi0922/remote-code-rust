use crate::types::Message;

/// Error type for codec operations
#[derive(Debug, thiserror::Error)]
pub enum CodecError {
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Invalid message: {0}")]
    InvalidMessage(String),
}

/// Serialize a JSON-RPC message to a JSON string
pub fn encode_message(msg: &Message) -> Result<String, CodecError> {
    Ok(serde_json::to_string(msg)?)
}

/// Deserialize a JSON-RPC message from a JSON string
pub fn decode_message(data: &str) -> Result<Message, CodecError> {
    let msg: Message = serde_json::from_str(data)?;
    Ok(msg)
}

/// Encode a message with Content-Length header for stdio transport.
/// Format: `Content-Length: {len}\r\n\r\n{json}`
pub fn encode_with_content_length(msg: &Message) -> Vec<u8> {
    let json = serde_json::to_string(msg).unwrap_or_default();
    let header = format!("Content-Length: {}\r\n\r\n", json.len());
    let mut buf = Vec::with_capacity(header.len() + json.len());
    buf.extend_from_slice(header.as_bytes());
    buf.extend_from_slice(json.as_bytes());
    buf
}

/// Parse the Content-Length value from a header string.
/// Expects format like `Content-Length: 42`
pub fn parse_content_length_header(header: &str) -> Option<usize> {
    let header = header.trim();
    let parts: Vec<&str> = header.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let key = parts[0].trim();
    if !key.eq_ignore_ascii_case("Content-Length") {
        return None;
    }
    parts[1].trim().parse::<usize>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    #[test]
    fn test_encode_message() {
        let msg = Message::request(1, "test", json!({}));
        let encoded = encode_message(&msg).unwrap();
        assert!(encoded.contains("\"jsonrpc\":\"2.0\""));
        assert!(encoded.contains("\"method\":\"test\""));
    }

    #[test]
    fn test_decode_message() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"test","params":{}}"#;
        let msg = decode_message(json).unwrap();
        assert_eq!(msg.jsonrpc, "2.0");
        assert_eq!(msg.id_as_u64(), Some(1));
        assert_eq!(msg.method.as_deref(), Some("test"));
    }

    #[test]
    fn test_encode_decode_roundtrip() {
        let original = Message::request(42, "initialize", json!({"capabilities": {}}));
        let encoded = encode_message(&original).unwrap();
        let decoded = decode_message(&encoded).unwrap();
        assert_eq!(decoded.jsonrpc, original.jsonrpc);
        assert_eq!(decoded.id_as_u64(), original.id_as_u64());
        assert_eq!(decoded.method, original.method);
    }

    #[test]
    fn test_decode_invalid_json() {
        let result = decode_message("not valid json");
        assert!(result.is_err());
    }

    #[test]
    fn test_encode_with_content_length() {
        let msg = Message::request(1, "test", json!({}));
        let encoded = encode_with_content_length(&msg);
        let encoded_str = String::from_utf8(encoded).unwrap();

        // Should start with Content-Length header
        assert!(encoded_str.starts_with("Content-Length: "));
        assert!(encoded_str.contains("\r\n\r\n"));

        // Extract and verify the content length
        let parts: Vec<&str> = encoded_str.split("\r\n\r\n").collect();
        assert_eq!(parts.len(), 2);

        let header = parts[0];
        let content = parts[1];
        let content_length = parse_content_length_header(header).unwrap();
        assert_eq!(content.len(), content_length);
    }

    #[test]
    fn test_parse_content_length_header_valid() {
        assert_eq!(parse_content_length_header("Content-Length: 42"), Some(42));
        assert_eq!(
            parse_content_length_header("Content-Length: 0"),
            Some(0)
        );
        assert_eq!(
            parse_content_length_header("Content-Length: 999999"),
            Some(999999)
        );
    }

    #[test]
    fn test_parse_content_length_header_case_insensitive() {
        assert_eq!(
            parse_content_length_header("content-length: 100"),
            Some(100)
        );
        assert_eq!(
            parse_content_length_header("CONTENT-LENGTH: 100"),
            Some(100)
        );
    }

    #[test]
    fn test_parse_content_length_header_with_whitespace() {
        assert_eq!(
            parse_content_length_header("Content-Length:   50  "),
            Some(50)
        );
    }

    #[test]
    fn test_parse_content_length_header_invalid() {
        assert_eq!(parse_content_length_header("Invalid-Header: 42"), None);
        assert_eq!(parse_content_length_header("Content-Length: abc"), None);
        assert_eq!(parse_content_length_header(""), None);
        assert_eq!(parse_content_length_header("no colon here"), None);
    }

    #[test]
    fn test_encode_with_content_length_unicode() {
        let msg = Message::notification("test", json!({"text": "你好世界"}));
        let encoded = encode_with_content_length(&msg);
        let encoded_str = String::from_utf8(encoded).unwrap();

        let parts: Vec<&str> = encoded_str.split("\r\n\r\n").collect();
        let header = parts[0];
        let content = parts[1];
        let content_length = parse_content_length_header(header).unwrap();

        // Content-Length is in bytes, not characters
        assert_eq!(content.as_bytes().len(), content_length);
        assert!(content.contains("你好世界"));
    }

    #[test]
    fn test_encode_notification_no_optional_fields() {
        let msg = Message::notification("initialized", Value::Null);
        let encoded = encode_message(&msg).unwrap();
        // Notification should not have id, result, error, params
        assert!(!encoded.contains("\"id\""));
        assert!(!encoded.contains("\"result\""));
        assert!(!encoded.contains("\"error\""));
        assert!(!encoded.contains("\"params\""));
    }
}

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC 2.0 request/id type (can be number, string, or null)
pub type Id = Value;

/// JSON-RPC 2.0 protocol version constant
pub const JSONRPC_VERSION: &str = "2.0";

/// JSON-RPC 2.0 message (unified request/response/notification/error)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Id>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Error>,
}

impl Message {
    /// Create a JSON-RPC 2.0 request message (has id, method, params)
    pub fn request(id: u64, method: &str, params: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: Some(Value::Number(id.into())),
            method: Some(method.to_string()),
            params: if params.is_null() {
                None
            } else {
                Some(params)
            },
            result: None,
            error: None,
        }
    }

    /// Create a JSON-RPC 2.0 notification message (no id, has method, params)
    pub fn notification(method: &str, params: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: None,
            method: Some(method.to_string()),
            params: if params.is_null() {
                None
            } else {
                Some(params)
            },
            result: None,
            error: None,
        }
    }

    /// Create a JSON-RPC 2.0 success response message (has id, result)
    pub fn response(id: Id, result: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: Some(id),
            method: None,
            params: None,
            result: Some(result),
            error: None,
        }
    }

    /// Create a JSON-RPC 2.0 error response message (has id, error)
    pub fn error_response(id: Id, code: i64, message: &str) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: Some(id),
            method: None,
            params: None,
            result: None,
            error: Some(Error::new(code, message)),
        }
    }

    /// Returns true if this message is a request (has id, method, no result, no error)
    pub fn is_request(&self) -> bool {
        self.id.is_some()
            && self.method.is_some()
            && self.result.is_none()
            && self.error.is_none()
    }

    /// Returns true if this message is a notification (no id, has method)
    pub fn is_notification(&self) -> bool {
        self.id.is_none() && self.method.is_some()
    }

    /// Returns true if this message is a success response (has id, has result)
    pub fn is_response(&self) -> bool {
        self.id.is_some() && self.result.is_some() && self.error.is_none()
    }

    /// Returns true if this message is an error response (has id, has error)
    pub fn is_error(&self) -> bool {
        self.id.is_some() && self.error.is_some()
    }

    /// Extract the numeric id as u64, returns None if id is not a number
    pub fn id_as_u64(&self) -> Option<u64> {
        self.id.as_ref().and_then(|v| v.as_u64())
    }
}

/// JSON-RPC 2.0 error object
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Error {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl Error {
    /// Create a new JSON-RPC error with code and message
    pub fn new(code: i64, message: &str) -> Self {
        Self {
            code,
            message: message.to_string(),
            data: None,
        }
    }

    /// Create a new JSON-RPC error with code, message, and additional data
    pub fn with_data(code: i64, message: &str, data: Value) -> Self {
        Self {
            code,
            message: message.to_string(),
            data: Some(data),
        }
    }

    /// Create a parse error (-32700)
    pub fn parse_error() -> Self {
        Self::new(error_codes::PARSE_ERROR, "Parse error")
    }

    /// Create an invalid request error (-32600)
    pub fn invalid_request() -> Self {
        Self::new(error_codes::INVALID_REQUEST, "Invalid Request")
    }

    /// Create a method not found error (-32601)
    pub fn method_not_found() -> Self {
        Self::new(error_codes::METHOD_NOT_FOUND, "Method not found")
    }

    /// Create an invalid params error (-32602)
    pub fn invalid_params(message: &str) -> Self {
        Self::new(error_codes::INVALID_PARAMS, message)
    }

    /// Create an internal error (-32603)
    pub fn internal_error(message: &str) -> Self {
        Self::new(error_codes::INTERNAL_ERROR, message)
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "JSON-RPC error {}: {}", self.code, self.message)
    }
}

impl std::error::Error for Error {}

/// Standard JSON-RPC 2.0 error codes
pub mod error_codes {
    /// Invalid JSON was received by the server
    pub const PARSE_ERROR: i64 = -32700;
    /// The JSON sent is not a valid Request object
    pub const INVALID_REQUEST: i64 = -32600;
    /// The method does not exist / is not available
    pub const METHOD_NOT_FOUND: i64 = -32601;
    /// Invalid method parameter(s)
    pub const INVALID_PARAMS: i64 = -32602;
    /// Internal JSON-RPC error
    pub const INTERNAL_ERROR: i64 = -32603;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // ── Message construction tests ──

    #[test]
    fn test_request_creation() {
        let msg = Message::request(1, "initialize", json!({"capabilities": {}}));
        assert_eq!(msg.jsonrpc, "2.0");
        assert_eq!(msg.id_as_u64(), Some(1));
        assert_eq!(msg.method.as_deref(), Some("initialize"));
        assert!(msg.params.is_some());
        assert!(msg.result.is_none());
        assert!(msg.error.is_none());
    }

    #[test]
    fn test_request_with_null_params() {
        let msg = Message::request(42, "ping", Value::Null);
        assert_eq!(msg.id_as_u64(), Some(42));
        assert_eq!(msg.method.as_deref(), Some("ping"));
        assert!(msg.params.is_none()); // null params should be None
    }

    #[test]
    fn test_notification_creation() {
        let msg = Message::notification("initialized", json!({}));
        assert_eq!(msg.jsonrpc, "2.0");
        assert!(msg.id.is_none());
        assert_eq!(msg.method.as_deref(), Some("initialized"));
        assert!(msg.params.is_some());
    }

    #[test]
    fn test_notification_with_null_params() {
        let msg = Message::notification("cancel", Value::Null);
        assert!(msg.id.is_none());
        assert!(msg.params.is_none());
    }

    #[test]
    fn test_response_creation() {
        let msg = Message::response(Value::Number(1.into()), json!({"result": "ok"}));
        assert_eq!(msg.jsonrpc, "2.0");
        assert_eq!(msg.id_as_u64(), Some(1));
        assert!(msg.result.is_some());
        assert!(msg.method.is_none());
        assert!(msg.error.is_none());
    }

    #[test]
    fn test_error_response_creation() {
        let msg = Message::error_response(Value::Number(2.into()), -32600, "Invalid Request");
        assert_eq!(msg.jsonrpc, "2.0");
        assert_eq!(msg.id_as_u64(), Some(2));
        assert!(msg.error.is_some());
        assert_eq!(msg.error.as_ref().unwrap().code, -32600);
        assert_eq!(msg.error.as_ref().unwrap().message, "Invalid Request");
    }

    // ── Message classification tests ──

    #[test]
    fn test_is_request() {
        let msg = Message::request(1, "test", json!({}));
        assert!(msg.is_request());
        assert!(!msg.is_notification());
        assert!(!msg.is_response());
        assert!(!msg.is_error());
    }

    #[test]
    fn test_is_notification() {
        let msg = Message::notification("test", json!({}));
        assert!(!msg.is_request());
        assert!(msg.is_notification());
        assert!(!msg.is_response());
        assert!(!msg.is_error());
    }

    #[test]
    fn test_is_response() {
        let msg = Message::response(Value::Number(1.into()), json!("ok"));
        assert!(!msg.is_request());
        assert!(!msg.is_notification());
        assert!(msg.is_response());
        assert!(!msg.is_error());
    }

    #[test]
    fn test_is_error() {
        let msg = Message::error_response(Value::Number(1.into()), -32600, "bad");
        assert!(!msg.is_request());
        assert!(!msg.is_notification());
        assert!(!msg.is_response());
        assert!(msg.is_error());
    }

    // ── id_as_u64 tests ──

    #[test]
    fn test_id_as_u64_numeric() {
        let msg = Message::request(123, "test", json!({}));
        assert_eq!(msg.id_as_u64(), Some(123));
    }

    #[test]
    fn test_id_as_u64_string() {
        let msg = Message {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: Some(Value::String("abc".to_string())),
            method: Some("test".to_string()),
            params: None,
            result: None,
            error: None,
        };
        assert_eq!(msg.id_as_u64(), None);
    }

    #[test]
    fn test_id_as_u64_null() {
        let msg = Message {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: Some(Value::Null),
            method: Some("test".to_string()),
            params: None,
            result: None,
            error: None,
        };
        assert_eq!(msg.id_as_u64(), None);
    }

    #[test]
    fn test_id_as_u64_none() {
        let msg = Message::notification("test", json!({}));
        assert_eq!(msg.id_as_u64(), None);
    }

    // ── Serialization roundtrip tests ──

    #[test]
    fn test_request_roundtrip() {
        let msg = Message::request(1, "initialize", json!({"capabilities": {}}));
        let json_str = serde_json::to_string(&msg).unwrap();
        let deserialized: Message = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.jsonrpc, "2.0");
        assert_eq!(deserialized.id_as_u64(), Some(1));
        assert_eq!(deserialized.method.as_deref(), Some("initialize"));
    }

    #[test]
    fn test_notification_roundtrip() {
        let msg = Message::notification("initialized", json!({}));
        let json_str = serde_json::to_string(&msg).unwrap();
        let deserialized: Message = serde_json::from_str(&json_str).unwrap();
        assert!(deserialized.id.is_none());
        assert_eq!(deserialized.method.as_deref(), Some("initialized"));
    }

    #[test]
    fn test_response_roundtrip() {
        let msg = Message::response(Value::Number(5.into()), json!({"status": "ok"}));
        let json_str = serde_json::to_string(&msg).unwrap();
        let deserialized: Message = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.id_as_u64(), Some(5));
        assert!(deserialized.result.is_some());
    }

    #[test]
    fn test_error_response_roundtrip() {
        let msg = Message::error_response(Value::Number(3.into()), -32601, "Method not found");
        let json_str = serde_json::to_string(&msg).unwrap();
        let deserialized: Message = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.id_as_u64(), Some(3));
        let err = deserialized.error.unwrap();
        assert_eq!(err.code, -32601);
        assert_eq!(err.message, "Method not found");
    }

    // ── Error factory method tests ──

    #[test]
    fn test_parse_error() {
        let err = Error::parse_error();
        assert_eq!(err.code, -32700);
        assert_eq!(err.message, "Parse error");
        assert!(err.data.is_none());
    }

    #[test]
    fn test_invalid_request_error() {
        let err = Error::invalid_request();
        assert_eq!(err.code, -32600);
        assert_eq!(err.message, "Invalid Request");
    }

    #[test]
    fn test_method_not_found_error() {
        let err = Error::method_not_found();
        assert_eq!(err.code, -32601);
        assert_eq!(err.message, "Method not found");
    }

    #[test]
    fn test_invalid_params_error() {
        let err = Error::invalid_params("Missing required field");
        assert_eq!(err.code, -32602);
        assert_eq!(err.message, "Missing required field");
    }

    #[test]
    fn test_internal_error() {
        let err = Error::internal_error("Something went wrong");
        assert_eq!(err.code, -32603);
        assert_eq!(err.message, "Something went wrong");
    }

    #[test]
    fn test_error_with_data() {
        let err = Error::with_data(-32000, "Custom error", json!({"detail": "extra info"}));
        assert_eq!(err.code, -32000);
        assert_eq!(err.message, "Custom error");
        assert!(err.data.is_some());
        assert_eq!(err.data.unwrap()["detail"], "extra info");
    }

    #[test]
    fn test_error_display() {
        let err = Error::new(-32600, "Invalid Request");
        assert_eq!(format!("{err}"), "JSON-RPC error -32600: Invalid Request");
    }

    #[test]
    fn test_serialization_omits_none_fields() {
        let msg = Message::notification("test", Value::Null);
        let json_str = serde_json::to_string(&msg).unwrap();
        // Should not contain "id", "result", "error", "params" keys
        assert!(!json_str.contains("\"id\""));
        assert!(!json_str.contains("\"result\""));
        assert!(!json_str.contains("\"error\""));
        assert!(!json_str.contains("\"params\""));
        assert!(json_str.contains("\"jsonrpc\""));
        assert!(json_str.contains("\"method\""));
    }
}

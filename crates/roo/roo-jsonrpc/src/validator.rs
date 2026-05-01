use crate::types::{Message, JSONRPC_VERSION};

/// Validation error types for JSON-RPC 2.0 messages
#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("Invalid jsonrpc version: expected '2.0', got '{0}'")]
    InvalidVersion(String),
    #[error("Request must have a method field")]
    MissingMethod,
    #[error("Response must have either result or error field")]
    MissingResultOrError,
    #[error("Error object must have code and message fields")]
    InvalidErrorObject,
    #[error("Notification must have a method field")]
    NotificationMissingMethod,
}

/// Validate a JSON-RPC 2.0 message according to the specification.
///
/// Checks:
/// - `jsonrpc` field must be "2.0"
/// - Requests must have a `method` field
/// - Responses must have a `result` or `error` field
/// - Error objects must have `code` and `message` fields
/// - Notifications must have a `method` field
pub fn validate(msg: &Message) -> Result<(), ValidationError> {
    // Check jsonrpc version
    if msg.jsonrpc != JSONRPC_VERSION {
        return Err(ValidationError::InvalidVersion(msg.jsonrpc.clone()));
    }

    let has_id = msg.id.is_some();
    let has_method = msg.method.is_some();
    let has_result = msg.result.is_some();
    let has_error = msg.error.is_some();
    let has_params = msg.params.is_some();

    if has_id {
        // This is either a request or a response
        if has_method && !has_result && !has_error {
            // Request: has id, method, optionally params
            return Ok(());
        }
        if has_result && !has_method {
            // Success response: has id, result
            return Ok(());
        }
        if has_error && !has_method && !has_result {
            // Error response: has id, error
            // Validate error object
            if let Some(ref err) = msg.error {
                if err.message.is_empty() {
                    return Err(ValidationError::InvalidErrorObject);
                }
            }
            return Ok(());
        }

        // If we get here, the message has id but ambiguous fields
        if !has_method && !has_result && !has_error {
            return Err(ValidationError::MissingResultOrError);
        }
    } else {
        // No id: must be a notification
        if has_method {
            // Notification: no id, has method, optionally params
            return Ok(());
        }
        return Err(ValidationError::NotificationMissingMethod);
    }

    // If the message has id, method, and result/error, it's ambiguous but we allow it
    // (some implementations include method in responses)
    if has_id && (has_result || has_error) {
        if has_error {
            if let Some(ref err) = msg.error {
                if err.message.is_empty() {
                    return Err(ValidationError::InvalidErrorObject);
                }
            }
        }
        return Ok(());
    }

    // Suppress unused variable warning
    let _ = has_params;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Error, error_codes};
    use serde_json::json;

    #[test]
    fn test_valid_request() {
        let msg = Message::request(1, "initialize", json!({}));
        assert!(validate(&msg).is_ok());
    }

    #[test]
    fn test_valid_notification() {
        let msg = Message::notification("initialized", json!({}));
        assert!(validate(&msg).is_ok());
    }

    #[test]
    fn test_valid_response() {
        let msg = Message::response(json!(1), json!({"result": "ok"}));
        assert!(validate(&msg).is_ok());
    }

    #[test]
    fn test_valid_error_response() {
        let msg = Message::error_response(json!(1), -32600, "Invalid Request");
        assert!(validate(&msg).is_ok());
    }

    #[test]
    fn test_invalid_version() {
        let msg = Message {
            jsonrpc: "1.0".to_string(),
            id: Some(json!(1)),
            method: Some("test".to_string()),
            params: None,
            result: None,
            error: None,
        };
        let result = validate(&msg);
        assert!(result.is_err());
        match result.unwrap_err() {
            ValidationError::InvalidVersion(v) => assert_eq!(v, "1.0"),
            other => panic!("Expected InvalidVersion, got: {other}"),
        }
    }

    #[test]
    fn test_request_missing_method() {
        let msg = Message {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: Some(json!(1)),
            method: None,
            params: None,
            result: None,
            error: None,
        };
        let result = validate(&msg);
        assert!(result.is_err());
    }

    #[test]
    fn test_notification_missing_method() {
        let msg = Message {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: None,
            method: None,
            params: None,
            result: None,
            error: None,
        };
        let result = validate(&msg);
        assert!(result.is_err());
        match result.unwrap_err() {
            ValidationError::NotificationMissingMethod => {}
            other => panic!("Expected NotificationMissingMethod, got: {other}"),
        }
    }

    #[test]
    fn test_error_with_empty_message() {
        let msg = Message {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: Some(json!(1)),
            method: None,
            params: None,
            result: None,
            error: Some(Error::new(-32600, "")),
        };
        let result = validate(&msg);
        assert!(result.is_err());
        match result.unwrap_err() {
            ValidationError::InvalidErrorObject => {}
            other => panic!("Expected InvalidErrorObject, got: {other}"),
        }
    }

    #[test]
    fn test_valid_request_with_null_params() {
        let msg = Message::request(1, "ping", serde_json::Value::Null);
        assert!(validate(&msg).is_ok());
    }

    #[test]
    fn test_valid_notification_with_no_params() {
        let msg = Message::notification("cancel", serde_json::Value::Null);
        assert!(validate(&msg).is_ok());
    }

    #[test]
    fn test_valid_response_with_null_result() {
        let msg = Message::response(json!(1), serde_json::Value::Null);
        assert!(validate(&msg).is_ok());
    }

    #[test]
    fn test_valid_custom_error() {
        let msg = Message {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id: Some(json!(1)),
            method: None,
            params: None,
            result: None,
            error: Some(Error::with_data(
                error_codes::INTERNAL_ERROR,
                "Internal error",
                json!({"trace": "stack trace"}),
            )),
        };
        assert!(validate(&msg).is_ok());
    }
}

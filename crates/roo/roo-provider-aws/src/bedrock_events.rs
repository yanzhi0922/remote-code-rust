//! AWS Bedrock Converse Stream event parser.
//!
//! Parses the binary event stream format used by the Bedrock Runtime API.
//! The format follows the AWS event stream encoding:
//! https://docs.aws.amazon.com/transcribe/latest/dg/event-stream.html
//!
//! Each event frame:
//! - 4 bytes: total byte length (big-endian u32)
//! - 4 bytes: headers length (big-endian u32)
//! - 4 bytes: prelude CRC (CRC32)
//! - N bytes: headers section
//! - M bytes: payload section
//! - 4 bytes: message CRC (CRC32)

use serde::Deserialize;
use serde_json::Value;

/// A parsed Bedrock stream event.
#[derive(Debug, Clone)]
#[allow(dead_code)] // Fields used for complete event representation; some read only in specific contexts
pub enum BedrockEvent {
    /// Message start event.
    MessageStart {
        #[allow(dead_code)]
        role: String,
    },
    /// Content block start event.
    ContentBlockStart {
        #[allow(dead_code)]
        index: usize,
        content_block: ContentBlockStartData,
    },
    /// Content block delta event.
    ContentBlockDelta {
        #[allow(dead_code)]
        index: usize,
        delta: ContentBlockDeltaData,
    },
    /// Content block stop event.
    ContentBlockStop {
        #[allow(dead_code)]
        index: usize,
    },
    /// Message stop event.
    MessageStop {
        #[allow(dead_code)]
        stop_reason: Option<String>,
        #[allow(dead_code)]
        additional_model_response_fields: Option<Value>,
    },
    /// Metadata event with usage information.
    Metadata {
        usage: BedrockUsage,
        metrics: Option<BedrockMetrics>,
    },
    /// An internal error from the stream.
    InternalServerException {
        message: String,
    },
    /// A service unavailable error.
    ServiceUnavailableException {
        message: String,
    },
    /// Throttling error.
    ThrottlingException {
        message: String,
    },
    /// Validation error.
    ValidationException {
        message: String,
    },
    /// Unknown event type (graceful degradation).
    Unknown {
        event_type: String,
        payload: Value,
    },
}

/// Content block start data.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlockStartData {
    /// Text content block.
    #[serde(rename = "text")]
    Text {},
    /// Tool use content block.
    #[serde(rename = "toolUse")]
    ToolUse {
        #[serde(rename = "toolUseId")]
        tool_use_id: String,
        name: String,
    },
    /// Reasoning content block.
    #[serde(rename = "reasoningContent")]
    ReasoningContent {
        #[serde(rename = "reasoningContentId")]
        #[allow(dead_code)]
        reasoning_content_id: Option<String>,
    },
}

/// Content block delta data.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
pub enum ContentBlockDeltaData {
    /// Text delta.
    #[serde(rename = "textDelta")]
    TextDelta {
        text: String,
    },
    /// Tool use delta (input JSON).
    #[serde(rename = "toolUseDelta")]
    ToolUseDelta {
        #[serde(rename = "toolUseId")]
        tool_use_id: String,
        input: String,
    },
    /// Reasoning text delta.
    #[serde(rename = "reasoningContentDelta")]
    ReasoningTextDelta {
        text: String,
    },
    /// Reasoning signature delta.
    #[serde(rename = "reasoningContentSignatureDelta")]
    ReasoningSignatureDelta {
        signature: String,
    },
}

/// Usage metadata from Bedrock.
#[derive(Debug, Clone, Deserialize)]
pub struct BedrockUsage {
    #[serde(default)]
    #[serde(rename = "inputTokens")]
    pub input_tokens: u64,
    #[serde(default)]
    #[serde(rename = "outputTokens")]
    pub output_tokens: u64,
    #[serde(default)]
    #[serde(rename = "cacheReadInputTokens")]
    pub cache_read_input_tokens: Option<u64>,
    #[serde(default)]
    #[serde(rename = "cacheWriteInputTokens")]
    pub cache_write_input_tokens: Option<u64>,
}

/// Metrics from Bedrock.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // Parsed for completeness; latency may be used for observability later
pub struct BedrockMetrics {
    #[serde(default)]
    #[serde(rename = "latencyMs")]
    pub latency_ms: Option<u64>,
}

/// Parse the AWS event stream binary format into Bedrock events.
pub fn parse_bedrock_event_stream(data: &[u8]) -> Vec<BedrockEvent> {
    let mut events = Vec::new();
    let mut offset = 0;

    while offset + 12 < data.len() {
        // Read prelude
        let total_len = u32::from_be_bytes([
            data[offset],
            data[offset + 1],
            data[offset + 2],
            data[offset + 3],
        ]) as usize;

        if total_len < 12 || offset + total_len > data.len() {
            // Invalid frame, skip
            break;
        }

        let headers_len = u32::from_be_bytes([
            data[offset + 4],
            data[offset + 5],
            data[offset + 6],
            data[offset + 7],
        ]) as usize;

        // Skip prelude CRC (4 bytes at offset+8)
        let headers_start = offset + 12;
        let payload_start = headers_start + headers_len;
        let payload_end = offset + total_len - 4; // minus message CRC

        if payload_start > payload_end {
            offset += total_len;
            continue;
        }

        // Parse headers
        let headers_data = &data[headers_start..headers_start + headers_len];
        let (event_type, _content_type) = parse_headers(headers_data);

        // Parse payload
        let payload_data = &data[payload_start..payload_end];

        if let Some(event) = parse_event(&event_type, payload_data) {
            events.push(event);
        }

        offset += total_len;
    }

    events
}

/// Parse headers from the event stream header section.
fn parse_headers(data: &[u8]) -> (String, String) {
    let mut event_type = String::new();
    let mut content_type = String::new();
    let mut offset = 0;

    while offset + 2 < data.len() {
        // Header name length (1 byte)
        let name_len = data[offset] as usize;
        offset += 1;

        if offset + name_len >= data.len() {
            break;
        }

        // Header name
        let name = String::from_utf8_lossy(&data[offset..offset + name_len]).to_string();
        offset += name_len;

        // Header value type (1 byte)
        let value_type = data[offset];
        offset += 1;

        match value_type {
            7 => {
                // String type: 2 bytes length + string data
                if offset + 2 > data.len() {
                    break;
                }
                let str_len =
                    u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
                offset += 2;
                if offset + str_len > data.len() {
                    break;
                }
                let value =
                    String::from_utf8_lossy(&data[offset..offset + str_len]).to_string();
                offset += str_len;

                if name == ":event-type" {
                    event_type = value;
                } else if name == ":content-type" {
                    content_type = value;
                }
            }
            0 => {
                // Bool true (no value bytes)
            }
            1 => {
                // Bool false (no value bytes)
            }
            2 => {
                // Byte (1 byte)
                offset += 1;
            }
            3 => {
                // Short (2 bytes)
                offset += 2;
            }
            4 => {
                // Int (4 bytes)
                offset += 4;
            }
            5 => {
                // Long (8 bytes)
                offset += 8;
            }
            6 => {
                // Bytes: 2 bytes length + data
                if offset + 2 > data.len() {
                    break;
                }
                let bytes_len =
                    u16::from_be_bytes([data[offset], data[offset + 1]]) as usize;
                offset += 2 + bytes_len;
            }
            8 => {
                // Timestamp (8 bytes)
                offset += 8;
            }
            9 => {
                // UUID (16 bytes)
                offset += 16;
            }
            _ => {
                // Unknown type, try to skip
                break;
            }
        }
    }

    (event_type, content_type)
}

/// Parse a single event based on its type and payload.
fn parse_event(event_type: &str, payload: &[u8]) -> Option<BedrockEvent> {
    let payload_str = String::from_utf8_lossy(payload);
    let payload_json: Value = serde_json::from_str(&payload_str).ok()?;

    match event_type {
        "messageStart" => {
            let role = payload_json
                .get("role")
                .and_then(|v| v.as_str())
                .unwrap_or("assistant")
                .to_string();
            Some(BedrockEvent::MessageStart { role })
        }
        "contentBlockStart" => {
            let index = payload_json
                .get("contentBlockIndex")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let start = payload_json.get("start")?;
            let content_block: ContentBlockStartData =
                serde_json::from_value(start.clone()).ok()?;
            Some(BedrockEvent::ContentBlockStart { index, content_block })
        }
        "contentBlockDelta" => {
            let index = payload_json
                .get("contentBlockIndex")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let delta_value = payload_json.get("delta")?;
            let delta: ContentBlockDeltaData =
                serde_json::from_value(delta_value.clone()).ok()?;
            Some(BedrockEvent::ContentBlockDelta { index, delta })
        }
        "contentBlockStop" => {
            let index = payload_json
                .get("contentBlockIndex")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            Some(BedrockEvent::ContentBlockStop { index })
        }
        "messageStop" => {
            let stop_reason = payload_json
                .get("stopReason")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let additional = payload_json
                .get("additionalModelResponseFields")
                .cloned();
            Some(BedrockEvent::MessageStop {
                stop_reason,
                additional_model_response_fields: additional,
            })
        }
        "metadata" => {
            let usage_value = payload_json.get("usage")?;
            let usage: BedrockUsage = serde_json::from_value(usage_value.clone()).ok()?;
            let metrics = payload_json
                .get("metrics")
                .and_then(|v| serde_json::from_value(v.clone()).ok());
            Some(BedrockEvent::Metadata { usage, metrics })
        }
        "internalServerException" => {
            let message = payload_json
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Internal server error")
                .to_string();
            Some(BedrockEvent::InternalServerException { message })
        }
        "serviceUnavailableException" => {
            let message = payload_json
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Service unavailable")
                .to_string();
            Some(BedrockEvent::ServiceUnavailableException { message })
        }
        "throttlingException" => {
            let message = payload_json
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Throttled")
                .to_string();
            Some(BedrockEvent::ThrottlingException { message })
        }
        "validationException" => {
            let message = payload_json
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("Validation error")
                .to_string();
            Some(BedrockEvent::ValidationException { message })
        }
        _ => Some(BedrockEvent::Unknown {
            event_type: event_type.to_string(),
            payload: payload_json,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to build a minimal AWS event stream frame.
    fn build_frame(event_type: &str, payload: &[u8]) -> Vec<u8> {
        let content_type = "application/json";

        // Build headers
        let mut headers = Vec::new();

        // Header: :event-type (type 7 = string)
        let event_type_name = b":event-type";
        headers.push(event_type_name.len() as u8);
        headers.extend_from_slice(event_type_name);
        headers.push(7); // string type
        let et_bytes = event_type.as_bytes();
        headers.extend_from_slice(&(et_bytes.len() as u16).to_be_bytes());
        headers.extend_from_slice(et_bytes);

        // Header: :content-type (type 7 = string)
        let ct_name = b":content-type";
        headers.push(ct_name.len() as u8);
        headers.extend_from_slice(ct_name);
        headers.push(7); // string type
        let ct_bytes = content_type.as_bytes();
        headers.extend_from_slice(&(ct_bytes.len() as u16).to_be_bytes());
        headers.extend_from_slice(ct_bytes);

        // Header: :message-type (type 7 = string)
        let mt_name = b":message-type";
        headers.push(mt_name.len() as u8);
        headers.extend_from_slice(mt_name);
        headers.push(7); // string type
        let mt_bytes = b"event";
        headers.extend_from_slice(&(mt_bytes.len() as u16).to_be_bytes());
        headers.extend_from_slice(mt_bytes);

        let headers_len = headers.len() as u32;
        let total_len: u32 = 4 + 4 + 4 + headers_len + payload.len() as u32 + 4;

        let mut frame = Vec::with_capacity(total_len as usize);
        frame.extend_from_slice(&total_len.to_be_bytes());
        frame.extend_from_slice(&headers_len.to_be_bytes());
        // Prelude CRC (placeholder zeros for test)
        frame.extend_from_slice(&[0u8; 4]);
        frame.extend_from_slice(&headers);
        frame.extend_from_slice(payload);
        // Message CRC (placeholder zeros for test)
        frame.extend_from_slice(&[0u8; 4]);

        frame
    }

    #[test]
    fn test_parse_message_start_event() {
        let payload = br#"{"role":"assistant"}"#;
        let frame = build_frame("messageStart", payload);

        let events = parse_bedrock_event_stream(&frame);
        assert_eq!(events.len(), 1);
        match &events[0] {
            BedrockEvent::MessageStart { role } => {
                assert_eq!(role, "assistant");
            }
            _ => panic!("Expected MessageStart event"),
        }
    }

    #[test]
    fn test_parse_text_delta_event() {
        let payload = br#"{"contentBlockIndex":0,"delta":{"type":"textDelta","text":"Hello"}}"#;
        let frame = build_frame("contentBlockDelta", payload);

        let events = parse_bedrock_event_stream(&frame);
        assert_eq!(events.len(), 1);
        match &events[0] {
            BedrockEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(*index, 0);
                match delta {
                    ContentBlockDeltaData::TextDelta { text } => {
                        assert_eq!(text, "Hello");
                    }
                    _ => panic!("Expected TextDelta"),
                }
            }
            _ => panic!("Expected ContentBlockDelta event"),
        }
    }

    #[test]
    fn test_parse_tool_use_start_event() {
        let payload = br#"{"contentBlockIndex":1,"start":{"type":"toolUse","toolUseId":"call_123","name":"read_file"}}"#;
        let frame = build_frame("contentBlockStart", payload);

        let events = parse_bedrock_event_stream(&frame);
        assert_eq!(events.len(), 1);
        match &events[0] {
            BedrockEvent::ContentBlockStart {
                index,
                content_block,
            } => {
                assert_eq!(*index, 1);
                match content_block {
                    ContentBlockStartData::ToolUse {
                        tool_use_id,
                        name,
                    } => {
                        assert_eq!(tool_use_id, "call_123");
                        assert_eq!(name, "read_file");
                    }
                    _ => panic!("Expected ToolUse"),
                }
            }
            _ => panic!("Expected ContentBlockStart event"),
        }
    }

    #[test]
    fn test_parse_metadata_event() {
        let payload = br#"{"usage":{"inputTokens":100,"outputTokens":50},"metrics":{"latencyMs":1234}}"#;
        let frame = build_frame("metadata", payload);

        let events = parse_bedrock_event_stream(&frame);
        assert_eq!(events.len(), 1);
        match &events[0] {
            BedrockEvent::Metadata { usage, metrics } => {
                assert_eq!(usage.input_tokens, 100);
                assert_eq!(usage.output_tokens, 50);
                assert!(metrics.is_some());
                assert_eq!(metrics.as_ref().unwrap().latency_ms, Some(1234));
            }
            _ => panic!("Expected Metadata event"),
        }
    }

    #[test]
    fn test_parse_multiple_events() {
        let frame1 = build_frame("messageStart", br#"{"role":"assistant"}"#);
        let frame2 =
            build_frame("contentBlockDelta", br#"{"contentBlockIndex":0,"delta":{"type":"textDelta","text":"Hi"}}"#);
        let frame3 = build_frame(
            "messageStop",
            br#"{"stopReason":"end_turn"}"#,
        );

        let mut data = Vec::new();
        data.extend_from_slice(&frame1);
        data.extend_from_slice(&frame2);
        data.extend_from_slice(&frame3);

        let events = parse_bedrock_event_stream(&data);
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn test_parse_empty_stream() {
        let events = parse_bedrock_event_stream(&[]);
        assert!(events.is_empty());
    }

    #[test]
    fn test_parse_invalid_frame() {
        let events = parse_bedrock_event_stream(&[0u8; 5]);
        assert!(events.is_empty());
    }

    #[test]
    fn test_parse_throttling_exception() {
        let payload = br#"{"message":"Rate exceeded"}"#;
        let frame = build_frame("throttlingException", payload);

        let events = parse_bedrock_event_stream(&frame);
        assert_eq!(events.len(), 1);
        match &events[0] {
            BedrockEvent::ThrottlingException { message } => {
                assert_eq!(message, "Rate exceeded");
            }
            _ => panic!("Expected ThrottlingException"),
        }
    }

    #[test]
    fn test_parse_reasoning_delta() {
        let payload = br#"{"contentBlockIndex":0,"delta":{"type":"reasoningContentDelta","text":"Let me think..."}}"#;
        let frame = build_frame("contentBlockDelta", payload);

        let events = parse_bedrock_event_stream(&frame);
        assert_eq!(events.len(), 1);
        match &events[0] {
            BedrockEvent::ContentBlockDelta { index, delta } => {
                assert_eq!(*index, 0);
                match delta {
                    ContentBlockDeltaData::ReasoningTextDelta { text } => {
                        assert_eq!(text, "Let me think...");
                    }
                    _ => panic!("Expected ReasoningTextDelta"),
                }
            }
            _ => panic!("Expected ContentBlockDelta event"),
        }
    }
}

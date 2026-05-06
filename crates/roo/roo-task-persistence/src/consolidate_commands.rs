//! Consolidate Commands
//!
//! Merges command + command_output sequences, and use_mcp_server +
//! mcp_server_response sequences into single messages.
//! Also merges api_req_started + api_req_finished pairs.
//!
//! Mirrors `consolidateCommands.ts` and `consolidateApiRequests.ts`.

use roo_types::message::{ClineAsk, ClineMessage, ClineSay, MessageType};

use crate::safe_json_parse;

// ---------------------------------------------------------------------------
// consolidate_commands
// ---------------------------------------------------------------------------

/// Merges command + command_output sequences, and use_mcp_server +
/// mcp_server_response sequences into single messages.
///
/// Source: `.research/Roo-Code/packages/core/src/message-utils/consolidateCommands.ts`
pub fn consolidate_commands(messages: &[ClineMessage]) -> Vec<ClineMessage> {
    let mut result: Vec<ClineMessage> = Vec::new();
    let mut i = 0;

    while i < messages.len() {
        let msg = &messages[i];

        // Check for command + command_output pair:
        //   ask=Command followed by say=CommandOutput (with no ask in between)
        if msg.r#type == MessageType::Ask && msg.ask == Some(ClineAsk::Command) {
            // Look ahead for a matching command_output
            if i + 1 < messages.len() {
                let next = &messages[i + 1];
                if next.r#type == MessageType::Ask
                    && next.ask == Some(ClineAsk::CommandOutput)
                    && next.say == Some(ClineSay::CommandOutput)
                {
                    // Merge: combine the text from command_output into the command message
                    let mut merged = msg.clone();
                    if let Some(output_text) = &next.text {
                        let combined = match &msg.text {
                            Some(t) => format!("{}\n{}", t, output_text),
                            None => output_text.clone(),
                        };
                        merged.text = Some(combined);
                    }
                    result.push(merged);
                    i += 2;
                    continue;
                }
            }
        }

        // Check for use_mcp_server + mcp_server_response pair:
        //   ask=UseMcpServer followed by say=McpServerResponse
        if msg.r#type == MessageType::Ask && msg.ask == Some(ClineAsk::UseMcpServer) {
            if i + 1 < messages.len() {
                let next = &messages[i + 1];
                if next.r#type == MessageType::Say
                    && next.say == Some(ClineSay::McpServerResponse)
                {
                    // Merge: combine the response text into the use_mcp_server message
                    let mut merged = msg.clone();
                    if let Some(response_text) = &next.text {
                        let combined = match &msg.text {
                            Some(t) => format!("{}\n{}", t, response_text),
                            None => response_text.clone(),
                        };
                        merged.text = Some(combined);
                    }
                    result.push(merged);
                    i += 2;
                    continue;
                }
            }
        }

        result.push(msg.clone());
        i += 1;
    }

    result
}

// ---------------------------------------------------------------------------
// consolidate_api_requests
// ---------------------------------------------------------------------------

/// Merges api_req_started + api_req_finished pairs by merging their JSON text fields.
///
/// When an api_req_started message is immediately followed by an api_req_finished
/// message, the two JSON objects in their `text` fields are merged (started fields
/// take precedence on conflict) into a single message.
///
/// Source: `.research/Roo-Code/packages/core/src/message-utils/consolidateApiRequests.ts`
pub fn consolidate_api_requests(messages: &[ClineMessage]) -> Vec<ClineMessage> {
    let mut result: Vec<ClineMessage> = Vec::new();
    let mut i = 0;

    while i < messages.len() {
        let msg = &messages[i];

        // Check for api_req_started + api_req_finished pair
        if msg.r#type == MessageType::Say && msg.say == Some(ClineSay::ApiReqStarted) {
            if i + 1 < messages.len() {
                let next = &messages[i + 1];
                if next.r#type == MessageType::Say && next.say == Some(ClineSay::ApiReqFinished) {
                    // Merge the two JSON text fields
                    let mut merged = msg.clone();
                    let started_obj: Option<serde_json::Value> =
                        safe_json_parse(msg.text.as_deref(), None);
                    let finished_obj: Option<serde_json::Value> =
                        safe_json_parse(next.text.as_deref(), None);

                    match (started_obj, finished_obj) {
                        (Some(mut s), Some(f)) => {
                            // Merge finished into started (started values take precedence)
                            if let (serde_json::Value::Object(s_map), serde_json::Value::Object(f_map)) =
                                (&mut s, f)
                            {
                                for (k, v) in f_map {
                                    s_map.entry(k).or_insert(v);
                                }
                            }
                            merged.text = Some(serde_json::to_string(&s).unwrap_or_default());
                        }
                        (Some(s), None) => {
                            merged.text = Some(serde_json::to_string(&s).unwrap_or_default());
                        }
                        (None, Some(f)) => {
                            merged.text = Some(serde_json::to_string(&f).unwrap_or_default());
                        }
                        (None, None) => {
                            // Keep original text
                        }
                    }

                    result.push(merged);
                    i += 2;
                    continue;
                }
            }
        }

        result.push(msg.clone());
        i += 1;
    }

    result
}

// ---------------------------------------------------------------------------
// combine_messages
// ---------------------------------------------------------------------------

/// Convenience function that chains consolidation passes:
/// first merges command/MCP sequences, then merges API request pairs.
pub fn combine_messages(messages: &[ClineMessage]) -> Vec<ClineMessage> {
    consolidate_api_requests(&consolidate_commands(messages))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_ask_message(ts: f64, ask: ClineAsk, say: Option<ClineSay>, text: Option<&str>) -> ClineMessage {
        ClineMessage {
            ts,
            r#type: MessageType::Ask,
            ask: Some(ask),
            say,
            text: text.map(|s| s.to_string()),
            images: None,
            partial: None,
            reasoning: None,
            conversation_history_index: None,
            checkpoint: None,
            progress_status: None,
            context_condense: None,
            context_truncation: None,
            is_protected: None,
            api_protocol: None,
            is_answered: None,
        }
    }

    fn make_say_message(ts: f64, say: ClineSay, text: Option<&str>) -> ClineMessage {
        ClineMessage {
            ts,
            r#type: MessageType::Say,
            ask: None,
            say: Some(say),
            text: text.map(|s| s.to_string()),
            images: None,
            partial: None,
            reasoning: None,
            conversation_history_index: None,
            checkpoint: None,
            progress_status: None,
            context_condense: None,
            context_truncation: None,
            is_protected: None,
            api_protocol: None,
            is_answered: None,
        }
    }

    #[test]
    fn test_consolidate_commands_merges_command_and_output() {
        let messages = vec![
            make_ask_message(1.0, ClineAsk::Command, None, Some("ls -la")),
            make_ask_message(2.0, ClineAsk::CommandOutput, Some(ClineSay::CommandOutput), Some("file1.txt\nfile2.txt")),
        ];
        let result = consolidate_commands(&messages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].ask, Some(ClineAsk::Command));
        assert_eq!(result[0].text.as_deref(), Some("ls -la\nfile1.txt\nfile2.txt"));
    }

    #[test]
    fn test_consolidate_commands_merges_mcp_server_and_response() {
        let messages = vec![
            make_ask_message(1.0, ClineAsk::UseMcpServer, None, Some("request data")),
            make_say_message(2.0, ClineSay::McpServerResponse, Some("response data")),
        ];
        let result = consolidate_commands(&messages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].ask, Some(ClineAsk::UseMcpServer));
        assert_eq!(result[0].text.as_deref(), Some("request data\nresponse data"));
    }

    #[test]
    fn test_consolidate_commands_no_merge_unrelated() {
        let messages = vec![
            make_ask_message(1.0, ClineAsk::Command, None, Some("ls")),
            make_ask_message(2.0, ClineAsk::Followup, None, Some("question")),
        ];
        let result = consolidate_commands(&messages);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_consolidate_api_requests_merges_pair() {
        let messages = vec![
            make_say_message(1.0, ClineSay::ApiReqStarted, Some(r#"{"tokensIn":10,"cost":0.005}"#)),
            make_say_message(2.0, ClineSay::ApiReqFinished, Some(r#"{"tokensOut":20}"#)),
        ];
        let result = consolidate_api_requests(&messages);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].say, Some(ClineSay::ApiReqStarted));
        // The merged JSON should have both tokensIn and tokensOut
        let text = result[0].text.as_ref().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(parsed["tokensIn"], 10);
        assert_eq!(parsed["tokensOut"], 20);
        assert_eq!(parsed["cost"], 0.005);
    }

    #[test]
    fn test_consolidate_api_requests_no_merge_unrelated() {
        let messages = vec![
            make_say_message(1.0, ClineSay::ApiReqStarted, Some(r#"{"tokensIn":10}"#)),
            make_say_message(2.0, ClineSay::Text, Some("hello")),
        ];
        let result = consolidate_api_requests(&messages);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_combine_messages_chains_both() {
        let messages = vec![
            make_ask_message(1.0, ClineAsk::Command, None, Some("ls")),
            make_ask_message(2.0, ClineAsk::CommandOutput, Some(ClineSay::CommandOutput), Some("output")),
            make_say_message(3.0, ClineSay::ApiReqStarted, Some(r#"{"tokensIn":10}"#)),
            make_say_message(4.0, ClineSay::ApiReqFinished, Some(r#"{"tokensOut":20}"#)),
        ];
        let result = combine_messages(&messages);
        // Command+output merged to 1, api_req_started+finished merged to 1
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_empty_messages() {
        let result = consolidate_commands(&[]);
        assert!(result.is_empty());
        let result = consolidate_api_requests(&[]);
        assert!(result.is_empty());
        let result = combine_messages(&[]);
        assert!(result.is_empty());
    }
}
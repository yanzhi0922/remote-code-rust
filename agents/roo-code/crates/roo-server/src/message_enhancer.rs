//! Message enhancement using AI.
//!
//! Derived from `src/core/webview/messageEnhancer.ts`.
//!
//! Enhances user message prompts using AI, optionally including task history
//! for context. Captures telemetry for prompt enhancement events.

use serde::{Deserialize, Serialize};
use tracing::debug;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Options for message enhancement.
///
/// Source: `src/core/webview/messageEnhancer.ts` — `MessageEnhancerOptions`
#[derive(Debug, Clone)]
pub struct MessageEnhancerOptions {
    /// The text to enhance.
    pub text: String,
    /// Custom support prompts configuration.
    pub custom_support_prompts: Option<serde_json::Value>,
    /// List of API configuration metadata.
    pub list_api_config_meta: Vec<ApiConfigMeta>,
    /// Optional enhancement API config ID.
    pub enhancement_api_config_id: Option<String>,
    /// Whether to include task history in the enhancement.
    pub include_task_history_in_enhance: bool,
    /// Current task messages for context.
    pub current_cline_messages: Vec<ClineMessage>,
}

/// API configuration metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiConfigMeta {
    pub id: String,
    pub name: Option<String>,
}

/// A simplified Cline message for enhancement context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClineMessage {
    pub msg_type: String, // "ask" or "say"
    pub text: Option<String>,
    pub say: Option<String>,
    pub ts: Option<u64>,
}

/// Result of message enhancement.
///
/// Source: `src/core/webview/messageEnhancer.ts` — `MessageEnhancerResult`
#[derive(Debug, Clone)]
pub struct MessageEnhancerResult {
    pub success: bool,
    pub enhanced_text: Option<String>,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Message enhancer
// ---------------------------------------------------------------------------

/// Enhances a message prompt using AI.
///
/// Source: `src/core/webview/messageEnhancer.ts` — `MessageEnhancer.enhanceMessage`
///
/// This function:
/// 1. Determines which API configuration to use
/// 2. Optionally includes task history for context
/// 3. Creates the enhancement prompt
/// 4. Calls the AI to enhance the prompt
///
/// # Arguments
/// * `options` - Configuration options for message enhancement
/// * `enhance_fn` - Function that performs the actual AI completion
///
/// # Returns
/// A `MessageEnhancerResult` with the enhanced text or error.
pub fn enhance_message<F>(
    options: MessageEnhancerOptions,
    enhance_fn: F,
) -> MessageEnhancerResult
where
    F: Fn(&str) -> Result<String, String>,
{
    let text = &options.text;

    // Prepare the prompt to enhance
    let prompt_to_enhance = if options.include_task_history_in_enhance
        && !options.current_cline_messages.is_empty()
    {
        let task_history = extract_task_history(&options.current_cline_messages);
        if task_history.is_empty() {
            text.clone()
        } else {
            format!(
                "{text}\n\nUse the following previous conversation context as needed:\n{task_history}"
            )
        }
    } else {
        text.clone()
    };

    // Create the enhancement prompt
    let enhancement_prompt = create_enhancement_prompt(&prompt_to_enhance, options.custom_support_prompts.as_ref());

    // Call the enhancement function
    match enhance_fn(&enhancement_prompt) {
        Ok(enhanced_text) => MessageEnhancerResult {
            success: true,
            enhanced_text: Some(enhanced_text),
            error: None,
        },
        Err(e) => MessageEnhancerResult {
            success: false,
            enhanced_text: None,
            error: Some(e),
        },
    }
}

/// Extracts relevant task history from Cline messages for context.
///
/// Source: `src/core/webview/messageEnhancer.ts` — `MessageEnhancer.extractTaskHistory`
///
/// Filters to user messages (type: "ask" with text) and assistant messages
/// (type: "say" with say: "text"), limited to the last 10 messages.
/// Messages are truncated to 500 characters.
pub fn extract_task_history(messages: &[ClineMessage]) -> String {
    let relevant: Vec<&ClineMessage> = messages
        .iter()
        .filter(|msg| {
            // Include user messages with text
            if msg.msg_type == "ask" && msg.text.as_ref().map_or(false, |t| !t.is_empty()) {
                return true;
            }
            // Include assistant text messages
            if msg.msg_type == "say"
                && msg.say.as_ref().map_or(false, |s| s == "text")
                && msg.text.as_ref().map_or(false, |t| !t.is_empty())
            {
                return true;
            }
            false
        })
        .collect();

    // Take last 10 messages
    let relevant: Vec<&ClineMessage> = relevant.into_iter().rev().take(10).collect::<Vec<_>>().into_iter().rev().collect();

    relevant
        .iter()
        .map(|msg| {
            let role = if msg.msg_type == "ask" {
                "User"
            } else {
                "Assistant"
            };
            let content = msg.text.as_deref().unwrap_or("");
            let truncated = if content.len() > 500 {
                format!("{}...", &content[..500])
            } else {
                content.to_string()
            };
            format!("{role}: {truncated}")
        })
        .collect::<Vec<String>>()
        .join("\n")
}

/// Creates an enhancement prompt using the support prompt system.
///
/// Source: `src/core/webview/messageEnhancer.ts` — uses `supportPrompt.create("ENHANCE", ...)`
fn create_enhancement_prompt(
    user_input: &str,
    _custom_support_prompts: Option<&serde_json::Value>,
) -> String {
    // Default enhancement prompt template
    format!(
        "You are a prompt enhancement assistant. Your task is to improve the following user prompt \
         to make it clearer, more specific, and more likely to get a good response from an AI coding assistant. \
         Do not change the intent of the prompt. Just make it clearer and more detailed.\n\n\
         User prompt:\n{user_input}\n\n\
         Enhanced prompt:"
    )
}

/// Captures telemetry for prompt enhancement.
///
/// Source: `src/core/webview/messageEnhancer.ts` — `MessageEnhancer.captureTelemetry`
pub fn capture_enhancement_telemetry(task_id: Option<&str>, include_task_history: bool) {
    debug!(
        "Prompt enhanced - task_id: {:?}, include_task_history: {}",
        task_id, include_task_history
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enhance_message_basic() {
        let options = MessageEnhancerOptions {
            text: "fix the bug".to_string(),
            custom_support_prompts: None,
            list_api_config_meta: vec![],
            enhancement_api_config_id: None,
            include_task_history_in_enhance: false,
            current_cline_messages: vec![],
        };
        let result = enhance_message(options, |prompt| {
            Ok(format!("Enhanced: {prompt}"))
        });
        assert!(result.success);
        assert!(result.enhanced_text.is_some());
        assert!(result.enhanced_text.unwrap().contains("Enhanced"));
    }

    #[test]
    fn test_enhance_message_with_error() {
        let options = MessageEnhancerOptions {
            text: "fix the bug".to_string(),
            custom_support_prompts: None,
            list_api_config_meta: vec![],
            enhancement_api_config_id: None,
            include_task_history_in_enhance: false,
            current_cline_messages: vec![],
        };
        let result = enhance_message(options, |_prompt| {
            Err("API error".to_string())
        });
        assert!(!result.success);
        assert_eq!(result.error, Some("API error".to_string()));
    }

    #[test]
    fn test_extract_task_history() {
        let messages = vec![
            ClineMessage {
                msg_type: "ask".to_string(),
                text: Some("Hello".to_string()),
                say: None,
                ts: Some(1),
            },
            ClineMessage {
                msg_type: "say".to_string(),
                text: Some("Hi there".to_string()),
                say: Some("text".to_string()),
                ts: Some(2),
            },
            ClineMessage {
                msg_type: "say".to_string(),
                text: None,
                say: Some("tool".to_string()),
                ts: Some(3),
            },
        ];
        let history = extract_task_history(&messages);
        assert!(history.contains("User: Hello"));
        assert!(history.contains("Assistant: Hi there"));
        assert!(!history.contains("tool"));
    }

    #[test]
    fn test_extract_task_history_truncation() {
        let long_text = "a".repeat(600);
        let messages = vec![ClineMessage {
            msg_type: "ask".to_string(),
            text: Some(long_text),
            say: None,
            ts: Some(1),
        }];
        let history = extract_task_history(&messages);
        assert!(history.contains("..."));
        assert!(history.len() < 700);
    }

    #[test]
    fn test_extract_task_history_empty() {
        let messages: Vec<ClineMessage> = vec![];
        let history = extract_task_history(&messages);
        assert!(history.is_empty());
    }

    #[test]
    fn test_create_enhancement_prompt() {
        let prompt = create_enhancement_prompt("fix the bug", None);
        assert!(prompt.contains("fix the bug"));
        assert!(prompt.contains("Enhanced prompt:"));
    }
}

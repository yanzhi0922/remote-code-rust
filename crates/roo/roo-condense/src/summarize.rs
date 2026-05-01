//! Conversation summarization.
//!
//! Summarizes conversation messages using an LLM call, implementing the "fresh start"
//! model where the summary becomes a user message and all previous messages are tagged
//! with `condense_parent`.
//!
//! Source: `src/core/condense/index.ts` — `summarizeConversation`

use std::sync::Arc;

use futures::StreamExt;
use roo_provider::handler::{CreateMessageMetadata, Provider};
use roo_types::api::{ApiMessage, ContentBlock, MessageRole};

use crate::convert::extract_command_blocks;
use crate::history::get_messages_since_last_summary;
use crate::transform::{inject_synthetic_tool_results, transform_messages_for_condensing};

/// The system prompt used for summarization requests.
///
/// Source: `src/core/condense/index.ts` — `SUMMARY_PROMPT`
const SUMMARY_PROMPT: &str = r#"You are a helpful AI assistant tasked with summarizing conversations.

CRITICAL: This is a summarization-only request. DO NOT call any tools or functions.
Your ONLY task is to analyze the conversation and produce a text summary.
Respond with text only - no tool calls will be processed.

CRITICAL: This summarization request is a SYSTEM OPERATION, not a user message.
When analyzing "user requests" and "user intent", completely EXCLUDE this summarization message.
The "most recent user request" and "next step" must be based on what the user was doing BEFORE this system message appeared.
The goal is for work to continue seamlessly after condensation - as if it never happened."#;

/// Default condense prompt used when no custom prompt is provided.
///
/// Source: `src/shared/support-prompt.ts` — `CONDENSE`
const DEFAULT_CONDENSE_PROMPT: &str = "Condense the conversation above into a concise summary that captures the key context, decisions, and current state. Preserve all important details needed to continue the task seamlessly.";

/// Response from a summarization operation.
///
/// Source: `src/core/condense/index.ts` — `SummarizeResponse`
#[derive(Debug, Clone)]
pub struct SummarizeResponse {
    /// The messages after summarization.
    pub messages: Vec<ApiMessage>,
    /// The summary text; empty string for no summary.
    pub summary: String,
    /// The cost of the summarization operation.
    pub cost: f64,
    /// The number of tokens in the context for the next API request.
    pub new_context_tokens: Option<u64>,
    /// Error message shown to the user on failure.
    pub error: Option<String>,
    /// Detailed error information including stack trace and API error info.
    pub error_details: Option<String>,
    /// The unique ID of the created summary message.
    pub condense_id: Option<String>,
}

/// Options for summarizing a conversation.
///
/// Source: `src/core/condense/index.ts` — `SummarizeConversationOptions`
pub struct SummarizeConversationOptions {
    /// The conversation messages to summarize.
    pub messages: Vec<ApiMessage>,
    /// The API handler to use for summarization.
    pub api_handler: Arc<dyn Provider>,
    /// The system prompt for the conversation.
    pub system_prompt: String,
    /// The task ID for telemetry.
    pub task_id: String,
    /// Whether this is an automatic trigger (vs manual).
    pub is_automatic_trigger: bool,
    /// Optional custom condensing prompt.
    pub custom_condensing_prompt: Option<String>,
    /// Optional metadata to pass through to the condensing API call.
    pub metadata: Option<CreateMessageMetadata>,
    /// Optional environment details string to include in the condensed summary.
    pub environment_details: Option<String>,
    /// Optional array of file paths read by Roo during the task.
    pub files_read_by_roo: Option<Vec<String>>,
    /// Optional current working directory for resolving file paths.
    pub cwd: Option<String>,
}

/// Summarizes the conversation messages using an LLM call.
///
/// This implements the "fresh start" model where:
/// - The summary becomes a user message (not assistant)
/// - Post-condense, the model sees only the summary (true fresh start)
/// - All messages are still stored but tagged with `condense_parent`
/// - `<command>` blocks from the original task are preserved across condensings
///
/// Environment details handling:
/// - For AUTOMATIC condensing: Environment details are included in the summary
/// - For MANUAL condensing: Environment details are NOT included (fresh details on next turn)
///
/// Source: `src/core/condense/index.ts` — `summarizeConversation`
pub async fn summarize_conversation(
    options: SummarizeConversationOptions,
) -> anyhow::Result<SummarizeResponse> {
    let SummarizeConversationOptions {
        messages,
        api_handler,
        system_prompt,
        task_id: _task_id,
        is_automatic_trigger,
        custom_condensing_prompt,
        metadata,
        environment_details,
        files_read_by_roo,
        cwd,
    } = options;

    let default_response = SummarizeResponse {
        messages: messages.clone(),
        cost: 0.0,
        summary: String::new(),
        new_context_tokens: None,
        error: None,
        error_details: None,
        condense_id: None,
    };

    // Get messages to summarize (all messages since the last summary, if any)
    let messages_to_summarize = get_messages_since_last_summary(&messages);

    if messages_to_summarize.len() <= 1 {
        let error = if messages.len() <= 1 {
            "Not enough messages to condense".to_string()
        } else {
            "Conversation was condensed recently".to_string()
        };
        return Ok(SummarizeResponse {
            error: Some(error),
            ..default_response
        });
    }

    // Check if there's a recent summary in the messages (edge case)
    let recent_summary_exists = messages_to_summarize
        .iter()
        .any(|message| message.is_summary.unwrap_or(false));

    if recent_summary_exists && messages_to_summarize.len() <= 2 {
        let error = "Conversation was condensed recently".to_string();
        return Ok(SummarizeResponse {
            error: Some(error),
            ..default_response
        });
    }

    // Use custom prompt if provided and non-empty, otherwise use the default CONDENSE prompt
    let condense_instructions = custom_condensing_prompt
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_CONDENSE_PROMPT);

    let final_request_message = ApiMessage {
        role: MessageRole::User,
        content: vec![ContentBlock::Text {
            text: condense_instructions.to_string(),
        }],
        reasoning: None,
        ts: None,
        truncation_parent: None,
        is_truncation_marker: None,
        truncation_id: None,
        condense_parent: None,
        is_summary: None,
        condense_id: None,
            reasoning_details: None,
    };

    // Inject synthetic tool_results for orphan tool_calls to prevent API rejections
    let mut messages_with_tool_results = inject_synthetic_tool_results(&messages_to_summarize);
    messages_with_tool_results.push(final_request_message);

    // Remove image blocks if the provider's model does not support images.
    let (_, model_info) = api_handler.get_model();
    let messages_with_tool_results =
        roo_provider::transform::maybe_remove_image_blocks(messages_with_tool_results, &model_info);

    // Transform tool_use and tool_result blocks to text representations.
    let messages_with_text_tool_blocks =
        transform_messages_for_condensing(&messages_with_tool_results);

    // Build request messages (role + content only)
    let request_messages: Vec<ApiMessage> = messages_with_text_tool_blocks;

    // Validate that the API handler supports message creation
    // (In Rust, the Provider trait always has create_message, so we skip this check)

    let mut summary = String::new();
    let mut cost = 0.0f64;

    // Call the API to generate summary
    let stream_result = api_handler
        .create_message(
            SUMMARY_PROMPT,
            request_messages,
            None, // No tools needed for condensation
            metadata.clone().unwrap_or_default(),
        )
        .await;

    match stream_result {
        Ok(stream) => {
            use std::pin::pin;
            let mut stream = pin!(stream);
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => match chunk {
                        roo_types::api::ApiStreamChunk::Text { text } => {
                            summary.push_str(&text);
                        }
                        roo_types::api::ApiStreamChunk::Usage { total_cost, .. } => {
                            cost = total_cost.unwrap_or(0.0);
                        }
                        _ => {}
                    },
                    Err(e) => {
                        let error_details = format!("Error: {e}");
                        return Ok(SummarizeResponse {
                            cost,
                            error: Some(format!("Condensation API call failed: {e}")),
                            error_details: Some(error_details),
                            ..default_response
                        });
                    }
                }
            }
        }
        Err(e) => {
            let error_details = format!("Error: {e}");
            return Ok(SummarizeResponse {
                cost,
                error: Some(format!("Condensation API call failed: {e}")),
                error_details: Some(error_details),
                ..default_response
            });
        }
    }

    let summary = summary.trim().to_string();

    if summary.is_empty() {
        return Ok(SummarizeResponse {
            cost,
            error: Some("Condensation failed: empty summary".to_string()),
            ..default_response
        });
    }

    // Extract command blocks from the first message (original task)
    let first_message = messages.first();
    let command_blocks = first_message
        .map(|m| extract_command_blocks(&m.content))
        .unwrap_or_default();

    // Build the summary content as separate text blocks
    let mut summary_content: Vec<ContentBlock> = vec![ContentBlock::Text {
        text: format!("## Conversation Summary\n{summary}"),
    }];

    // Add command blocks (active workflows) in their own system-reminder block if present
    if !command_blocks.is_empty() {
        summary_content.push(ContentBlock::Text {
            text: format!(
                "<system-reminder>\n\
                 ## Active Workflows\n\
                 The following directives must be maintained across all future condensings:\n\
                 {command_blocks}\n\
                 </system-reminder>"
            ),
        });
    }

    // Generate simplified folded file context.
    // A full implementation would use tree-sitter for smart code folding;
    // this simplified version reads the first N lines of referenced files.
    if let Some(ref file_paths) = files_read_by_roo {
        if !file_paths.is_empty() {
            const MAX_LINES_PER_FILE: usize = 20;
            let mut file_contexts = Vec::new();

            for file_path in file_paths.iter().take(10) {
                let resolved = if let Some(ref workdir) = cwd {
                    std::path::Path::new(workdir).join(file_path)
                } else {
                    std::path::PathBuf::from(file_path)
                };

                match std::fs::read_to_string(&resolved) {
                    Ok(content) => {
                        let lines: Vec<&str> = content.lines().take(MAX_LINES_PER_FILE).collect();
                        let total_lines = content.lines().count();
                        let preview = lines.join("\n");
                        let suffix = if total_lines > MAX_LINES_PER_FILE {
                            format!(
                                "\n... ({} more lines)",
                                total_lines - MAX_LINES_PER_FILE
                            )
                        } else {
                            String::new()
                        };
                        file_contexts.push(format!("### {file_path}\n```\n{preview}{suffix}\n```"));
                    }
                    Err(_) => {
                        // File may no longer exist; skip silently.
                    }
                }
            }

            if !file_contexts.is_empty() {
                summary_content.push(ContentBlock::Text {
                    text: format!(
                        "<system-reminder>\n\
                         ## File Context\n\
                         Key files referenced during the conversation:\n\
                         {}\n\
                         </system-reminder>",
                        file_contexts.join("\n\n")
                    ),
                });
            }
        }
    }
    let _ = (files_read_by_roo, cwd);

    // Add environment details as a separate text block if provided AND this is an automatic trigger
    if is_automatic_trigger {
        if let Some(env_details) = environment_details.as_ref() {
            let trimmed = env_details.trim();
            if !trimmed.is_empty() {
                summary_content.push(ContentBlock::Text {
                    text: trimmed.to_string(),
                });
            }
        }
    }

    // Generate a unique condenseId for this summary
    let condense_id = uuid::Uuid::now_v7().to_string();

    // Use the last message's timestamp + 1 to ensure unique timestamp for summary
    let last_msg_ts = messages
        .last()
        .and_then(|m| m.ts)
        .unwrap_or_else(|| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as f64
        });

    let summary_message = ApiMessage {
        role: MessageRole::User, // Fresh start model: summary is a user message
        content: summary_content,
        ts: Some(last_msg_ts + 1.0), // Unique timestamp after last message
        reasoning: None,
        reasoning_details: None,
        truncation_parent: None,
        is_truncation_marker: None,
        truncation_id: None,
        is_summary: Some(true),
        condense_id: Some(condense_id.clone()),
        condense_parent: None,
    };

    // NON-DESTRUCTIVE CONDENSE:
    // Tag ALL existing messages with condenseParent so they are filtered out when
    // the effective history is computed.
    let mut new_messages: Vec<ApiMessage> = messages
        .into_iter()
        .map(|mut msg| {
            // If message already has a condenseParent, leave it
            if msg.condense_parent.is_none() {
                msg.condense_parent = Some(condense_id.clone());
            }
            msg
        })
        .collect();

    // Append the summary message at the end
    new_messages.push(summary_message);

    // Count the tokens in the context for the next API request
    let system_prompt_blocks = vec![ContentBlock::Text {
        text: system_prompt,
    }];
    let summary_msg = new_messages.last().unwrap();
    let context_blocks: Vec<ContentBlock> = system_prompt_blocks
        .into_iter()
        .chain(summary_msg.content.iter().cloned())
        .collect();

    let message_tokens = api_handler.count_tokens(&context_blocks).await?;

    // Count tool definition tokens if tools are provided
    let mut tool_tokens: u64 = 0;
    if let Some(ref meta) = metadata {
        if let Some(ref tools) = meta.tools {
            if !tools.is_empty() {
                let tools_text = serde_json::to_string(tools)?;
                let tool_blocks = vec![ContentBlock::Text { text: tools_text }];
                tool_tokens = api_handler.count_tokens(&tool_blocks).await?;
            }
        }
    }

    let new_context_tokens = message_tokens + tool_tokens;

    Ok(SummarizeResponse {
        messages: new_messages,
        summary,
        cost,
        new_context_tokens: Some(new_context_tokens),
        error: None,
        error_details: None,
        condense_id: Some(condense_id),
    })
}

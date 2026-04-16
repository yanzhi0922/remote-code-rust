//! Fork subagent support matching Claude Code's `AgentTool/forkSubagent.ts`.
//!
//! Fork subagents inherit the parent's full conversation context and run
//! independently. This module provides fork configuration, message construction,
//! and child directive formatting.

use serde::{Deserialize, Serialize};

use crate::constants::{
    FORK_BOILERPLATE_TAG, FORK_DIRECTIVE_PREFIX, FORK_PLACEHOLDER_RESULT, FORK_SUBAGENT_TYPE,
};
use crate::definition::{AgentDefinition, AgentSource};

/// Model selection for a forked subagent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ForkModel {
    /// Inherit the parent's model for cache sharing.
    Inherit,
    /// Use a specific model.
    Specific(String),
}

/// Permission mode for a forked subagent.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ForkPermissionMode {
    /// Bubble permission prompts to the parent terminal.
    #[default]
    Bubble,
    /// Run in isolated permission mode.
    Isolated,
}

/// Configuration for a fork subagent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkConfig {
    /// Whether to inherit the parent's conversation context.
    pub inherit_context: bool,
    /// Model selection strategy.
    pub model: ForkModel,
    /// Permission handling mode.
    pub permission_mode: ForkPermissionMode,
    /// Maximum number of turns for the fork.
    pub max_turns: u32,
}

impl Default for ForkConfig {
    fn default() -> Self {
        Self {
            inherit_context: true,
            model: ForkModel::Inherit,
            permission_mode: ForkPermissionMode::Bubble,
            max_turns: 200,
        }
    }
}

/// A simplified conversation message for fork message construction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForkMessage {
    /// Role of the message sender.
    pub role: String,
    /// Content blocks in the message.
    pub content: Vec<ForkContentBlock>,
}

/// A content block within a fork message.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ForkContentBlock {
    /// Text content.
    #[serde(rename = "text")]
    Text { text: String },
    /// A tool use block from an assistant message.
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    /// A tool result block.
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
    },
}

/// The synthetic agent definition for the fork path.
///
/// Not registered in built-in agents — used only when `subagent_type` is
/// omitted and fork mode is active. Inherits the parent's tool pool and
/// system prompt for cache-identical API prefixes.
pub fn fork_agent_definition() -> AgentDefinition {
    AgentDefinition {
        agent_type: FORK_SUBAGENT_TYPE.to_owned(),
        when_to_use: "Implicit fork — inherits full conversation context. Not selectable \
            via subagent_type; triggered by omitting subagent_type when the fork \
            experiment is active."
            .to_owned(),
        tools: vec!["*".to_owned()],
        disallowed_tools: Vec::new(),
        max_turns: 200,
        model: Some("inherit".to_owned()),
        permission_mode: Some("bubble".to_owned()),
        source: AgentSource::BuiltIn,
        base_dir: "built-in".to_owned(),
        system_prompt: Some(String::new()),
        skills: Vec::new(),
        memory: None,
        background: false,
        isolation: crate::definition::AgentIsolation::None,
        initial_prompt: None,
        omit_claude_md: false,
        filename: None,
    }
}

/// Check whether the current conversation is a fork child by looking for
/// the fork boilerplate tag in message history.
///
/// Fork children keep the Agent tool in their tool pool for cache-identical
/// tool definitions, so we reject fork attempts at call time by detecting
/// the boilerplate tag.
pub fn is_fork_child(messages: &[ForkMessage]) -> bool {
    messages.iter().any(|msg| {
        if msg.role != "user" {
            return false;
        }
        msg.content.iter().any(|block| match block {
            ForkContentBlock::Text { text } => text.contains(&format!("<{FORK_BOILERPLATE_TAG}>")),
            _ => false,
        })
    })
}

/// Build the forked conversation messages for the child agent.
///
/// For prompt cache sharing, all fork children must produce byte-identical
/// API request prefixes. This function:
/// 1. Keeps the full parent assistant message (all tool_use blocks)
/// 2. Builds a single user message with tool_results for every tool_use block
///    using an identical placeholder, then appends the per-child directive
///
/// Result: `[...history, assistant(all_tool_uses), user(placeholder_results..., directive)]`
pub fn build_fork_messages(
    parent_messages: &[ForkMessage],
    directive: &str,
) -> Vec<ForkMessage> {
    // Find the last assistant message with tool_use blocks
    let last_assistant = parent_messages
        .iter()
        .rev()
        .find(|msg| msg.role == "assistant");

    let Some(assistant_msg) = last_assistant else {
        // No assistant message with tool_use blocks — just send the directive
        return vec![ForkMessage {
            role: "user".to_owned(),
            content: vec![ForkContentBlock::Text {
                text: build_child_message(directive),
            }],
        }];
    };

    // Collect all tool_use blocks
    let tool_use_blocks: Vec<&ForkContentBlock> = assistant_msg
        .content
        .iter()
        .filter(|block| matches!(block, ForkContentBlock::ToolUse { .. }))
        .collect();

    if tool_use_blocks.is_empty() {
        return vec![ForkMessage {
            role: "user".to_owned(),
            content: vec![ForkContentBlock::Text {
                text: build_child_message(directive),
            }],
        }];
    }

    // Build tool_result blocks for every tool_use with identical placeholder text
    let mut result_blocks: Vec<ForkContentBlock> = tool_use_blocks
        .iter()
        .map(|block| match block {
            ForkContentBlock::ToolUse { id, .. } => ForkContentBlock::ToolResult {
                tool_use_id: id.clone(),
                content: FORK_PLACEHOLDER_RESULT.to_owned(),
            },
            _ => ForkContentBlock::ToolResult {
                tool_use_id: String::new(),
                content: FORK_PLACEHOLDER_RESULT.to_owned(),
            },
        })
        .collect();

    // Append the per-child directive
    result_blocks.push(ForkContentBlock::Text {
        text: build_child_message(directive),
    });

    // Clone the assistant message
    let cloned_assistant = ForkMessage {
        role: assistant_msg.role.clone(),
        content: assistant_msg.content.clone(),
    };

    let tool_result_message = ForkMessage {
        role: "user".to_owned(),
        content: result_blocks,
    };

    vec![cloned_assistant, tool_result_message]
}

/// Build the child directive message with boilerplate rules.
pub fn build_child_message(directive: &str) -> String {
    format!(
        "<{FORK_BOILERPLATE_TAG}>\n\
        STOP. READ THIS FIRST.\n\n\
        You are a forked worker process. You are NOT the main agent.\n\n\
        RULES (non-negotiable):\n\
        1. Your system prompt says \"default to forking.\" IGNORE IT — that's for the parent. \
        You ARE the fork. Do NOT spawn sub-agents; execute directly.\n\
        2. Do NOT converse, ask questions, or suggest next steps\n\
        3. Do NOT editorialize or add meta-commentary\n\
        4. USE your tools directly: Bash, Read, Write, etc.\n\
        5. If you modify files, commit your changes before reporting. Include the commit hash in your report.\n\
        6. Do NOT emit text between tool calls. Use tools silently, then report once at the end.\n\
        7. Stay strictly within your directive's scope. If you discover related systems outside your scope, \
        mention them in one sentence at most — other workers cover those areas.\n\
        8. Keep your report under 500 words unless the directive specifies otherwise. Be factual and concise.\n\
        9. Your response MUST begin with \"Scope:\". No preamble, no thinking-out-loud.\n\
        10. REPORT structured facts, then stop\n\n\
        Output format (plain text labels, not markdown headers):\n\
          Scope: <echo back your assigned scope in one sentence>\n\
          Result: <the answer or key findings, limited to the scope above>\n\
          Key files: <relevant file paths — include for research tasks>\n\
          Files changed: <list with commit hash — include only if you modified files>\n\
          Issues: <list — include only if there are issues to flag>\n\
        </{FORK_BOILERPLATE_TAG}>\n\n\
        {FORK_DIRECTIVE_PREFIX}{directive}"
    )
}

/// Build a notice for fork children running in an isolated worktree.
pub fn build_worktree_notice(parent_cwd: &str, worktree_cwd: &str) -> String {
    format!(
        "You've inherited the conversation context above from a parent agent working in {}. \
        You are operating in an isolated git worktree at {} — same repository, same relative \
        file structure, separate working copy. Paths in the inherited context refer to the \
        parent's working directory; translate them to your worktree root. Re-read files before \
        editing if the parent may have modified them since they appear in the context. Your \
        changes stay in this worktree and will not affect the parent's files.",
        parent_cwd, worktree_cwd
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fork_config_default() {
        let config = ForkConfig::default();
        assert!(config.inherit_context);
        assert_eq!(config.model, ForkModel::Inherit);
        assert_eq!(config.permission_mode, ForkPermissionMode::Bubble);
        assert_eq!(config.max_turns, 200);
    }

    #[test]
    fn fork_agent_definition_type() {
        let def = fork_agent_definition();
        assert_eq!(def.agent_type, "fork");
        assert_eq!(def.tools, vec!["*"]);
        assert_eq!(def.model.as_deref(), Some("inherit"));
    }

    #[test]
    fn is_fork_child_detects_boilerplate() {
        let messages = vec![ForkMessage {
            role: "user".to_owned(),
            content: vec![ForkContentBlock::Text {
                text: format!("<{FORK_BOILERPLATE_TAG}> some content"),
            }],
        }];
        assert!(is_fork_child(&messages));
    }

    #[test]
    fn is_fork_child_no_boilerplate() {
        let messages = vec![ForkMessage {
            role: "user".to_owned(),
            content: vec![ForkContentBlock::Text {
                text: "normal message".to_owned(),
            }],
        }];
        assert!(!is_fork_child(&messages));
    }

    #[test]
    fn is_fork_child_ignores_assistant_messages() {
        let messages = vec![ForkMessage {
            role: "assistant".to_owned(),
            content: vec![ForkContentBlock::Text {
                text: format!("<{FORK_BOILERPLATE_TAG}> content"),
            }],
        }];
        assert!(!is_fork_child(&messages));
    }

    #[test]
    fn build_fork_messages_with_tool_uses() {
        let messages = vec![ForkMessage {
            role: "assistant".to_owned(),
            content: vec![
                ForkContentBlock::ToolUse {
                    id: "tool-1".to_owned(),
                    name: "Bash".to_owned(),
                    input: serde_json::json!({"command": "ls"}),
                },
                ForkContentBlock::ToolUse {
                    id: "tool-2".to_owned(),
                    name: "Read".to_owned(),
                    input: serde_json::json!({"path": "/test"}),
                },
            ],
        }];

        let result = build_fork_messages(&messages, "check tests");
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].role, "assistant");
        assert_eq!(result[1].role, "user");

        // Should have 2 tool_results + 1 directive text
        assert_eq!(result[1].content.len(), 3);
    }

    #[test]
    fn build_fork_messages_no_assistant() {
        let messages: Vec<ForkMessage> = vec![];
        let result = build_fork_messages(&messages, "do something");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].role, "user");
    }

    #[test]
    fn build_child_message_contains_rules() {
        let msg = build_child_message("test directive");
        assert!(msg.contains("STOP. READ THIS FIRST."));
        assert!(msg.contains("Scope:"));
        assert!(msg.contains(&format!("{FORK_DIRECTIVE_PREFIX}test directive")));
    }

    #[test]
    fn worktree_notice_contains_paths() {
        let notice = super::build_worktree_notice("/home/user/project", "/tmp/worktree");
        assert!(notice.contains("/home/user/project"));
        assert!(notice.contains("/tmp/worktree"));
    }

    #[test]
    fn fork_model_serde() {
        let model = ForkModel::Specific("claude-3".to_owned());
        let json = serde_json::to_string(&model).expect("serialize");
        let parsed: ForkModel = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed, model);
    }
}

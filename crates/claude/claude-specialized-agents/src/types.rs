//! Core types for the specialized agent system.

use serde::{Deserialize, Serialize};

/// The scope where an agent is defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentScope {
    /// Built into the application.
    BuiltIn,
    /// User-level agent (~/.remote-code/agents/).
    User,
    /// Project-level agent (.remote-code/agents/).
    Project,
}

/// A specialized agent definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecializedAgent {
    /// Unique name for the agent (used in @mentions).
    pub name: String,
    /// Human-readable description shown in the agent picker.
    pub description: String,
    /// The model to use (or "inherit" to use the current session model).
    pub model: AgentModel,
    /// List of tool names this agent is allowed to use.
    pub allowed_tools: Vec<String>,
    /// Maximum number of agent turns before stopping.
    pub max_turns: Option<u32>,
    /// Whether this agent can modify files.
    pub read_only: bool,
    /// The system prompt for this agent.
    pub system_prompt: String,
    /// Where this agent is defined.
    pub scope: AgentScope,
}

/// Model specification for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentModel {
    /// Inherit the model from the current session.
    Inherit,
    /// Use a specific model by name.
    Specific(String),
}

impl std::fmt::Display for AgentModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentModel::Inherit => write!(f, "inherit"),
            AgentModel::Specific(name) => write!(f, "{name}"),
        }
    }
}

/// Parsed frontmatter from an agent Markdown file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentFrontmatter {
    pub name: String,
    pub description: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default)]
    pub tools: Vec<String>,
    pub max_turns: Option<u32>,
    #[serde(default)]
    pub read_only: bool,
}

fn default_model() -> String {
    "inherit".to_string()
}

/// Result of invoking a specialized agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInvocation {
    /// The agent that was invoked.
    pub agent_name: String,
    /// The user's original message.
    pub user_message: String,
    /// The constructed system prompt.
    pub system_prompt: String,
    /// The model to use.
    pub model: AgentModel,
    /// Tools available to this agent.
    pub allowed_tools: Vec<String>,
}

/// An @mention parsed from user input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMention {
    /// The agent name extracted from the @mention.
    pub agent_name: String,
    /// The remaining text after the @mention.
    pub remaining_text: String,
    /// The position of the @mention in the original text.
    pub position: usize,
}

/// Error type for agent operations.
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("Agent not found: {0}")]
    NotFound(String),

    #[error("Invalid agent definition in {path}: {reason}")]
    InvalidDefinition { path: String, reason: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parse error: {0}")]
    YamlParse(#[from] serde_yaml::Error),
}

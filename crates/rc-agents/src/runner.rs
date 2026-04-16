//! Agent execution runner matching Claude Code's `AgentTool/runAgent.ts`.
//!
//! The [`AgentRunner`] orchestrates agent execution: resolving tools, building
//! the system prompt, and tracking turns and usage.

use std::path::PathBuf;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::definition::AgentDefinition;

/// Configuration for a single agent run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRunConfig {
    /// Maximum number of agentic turns before stopping.
    pub max_turns: u32,
    /// Model to use for this run.
    pub model: String,
    /// Tools available to the agent.
    pub tools: Vec<String>,
    /// Optional system prompt override.
    pub system_prompt: Option<String>,
    /// Working directory for the agent.
    pub working_dir: PathBuf,
}

/// Summary of token usage from an agent run.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct UsageSummary {
    /// Total input tokens consumed.
    pub input_tokens: u64,
    /// Total output tokens generated.
    pub output_tokens: u64,
    /// Tokens written to cache.
    pub cache_creation_tokens: u64,
    /// Tokens read from cache.
    pub cache_read_tokens: u64,
}

/// Result of an agent run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRunResult {
    /// The agent's final output text.
    pub output: String,
    /// Whether the run completed successfully.
    pub success: bool,
    /// Number of turns completed.
    pub turns: u32,
    /// Token usage summary.
    pub usage: UsageSummary,
}

/// A simplified conversation entry for providing context to an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationEntry {
    /// Role of the message sender.
    pub role: String,
    /// Text content of the message.
    pub content: String,
}

/// The agent execution runner.
///
/// Orchestrates the execution of an agent according to its definition and
/// configuration. Resolves the effective tool set, builds the system prompt,
/// and tracks execution state.
#[derive(Debug, Clone)]
pub struct AgentRunner {
    /// The agent definition being run.
    definition: AgentDefinition,
    /// Run-specific configuration.
    config: AgentRunConfig,
}

impl AgentRunner {
    /// Create a new runner for the given agent definition and configuration.
    pub fn new(definition: AgentDefinition, config: AgentRunConfig) -> Self {
        Self { definition, config }
    }

    /// Get a reference to the agent definition.
    pub fn definition(&self) -> &AgentDefinition {
        &self.definition
    }

    /// Get a reference to the run configuration.
    pub fn config(&self) -> &AgentRunConfig {
        &self.config
    }

    /// Resolve the effective tool set for this agent run.
    ///
    /// If the agent has an allowlist (`tools`), use that (filtered by denylist).
    /// If the agent has only a denylist, start with all tools and exclude.
    /// Otherwise, all tools are available.
    pub fn resolve_tools(&self, available_tools: &[String]) -> Vec<String> {
        if self.definition.has_tool_allowlist() {
            // Check for wildcard
            if self.definition.tools.contains(&"*".to_owned()) {
                return available_tools.to_owned();
            }
            // Filter allowlist by denylist
            let deny_set: std::collections::HashSet<&str> = self
                .definition
                .disallowed_tools
                .iter()
                .map(|s| s.as_str())
                .collect();
            self.definition
                .tools
                .iter()
                .filter(|t| !deny_set.contains(t.as_str()))
                .cloned()
                .collect()
        } else if self.definition.has_tool_denylist() {
            let deny_set: std::collections::HashSet<&str> = self
                .definition
                .disallowed_tools
                .iter()
                .map(|s| s.as_str())
                .collect();
            available_tools
                .iter()
                .filter(|t| !deny_set.contains(t.as_str()))
                .cloned()
                .collect()
        } else {
            available_tools.to_owned()
        }
    }

    /// Build the system prompt for this agent run.
    ///
    /// Uses the agent's system prompt if defined, otherwise generates a
    /// default prompt based on the agent type.
    pub fn build_system_prompt(&self) -> String {
        match &self.definition.system_prompt {
            Some(prompt) if !prompt.is_empty() => prompt.clone(),
            _ => format!(
                "You are an agent of type '{}'. Complete the task as instructed.",
                self.definition.agent_type
            ),
        }
    }

    /// Resolve the model to use for this run.
    ///
    /// Priority: config override > agent definition > default.
    pub fn resolve_model(&self, default_model: &str) -> String {
        if !self.config.model.is_empty() && self.config.model != "inherit" {
            return self.config.model.clone();
        }
        match &self.definition.model {
            Some(m) if m != "inherit" && !m.is_empty() => m.clone(),
            _ => default_model.to_owned(),
        }
    }

    /// Resolve the maximum number of turns.
    ///
    /// Priority: config override > agent definition > default (200).
    pub fn resolve_max_turns(&self) -> u32 {
        if self.config.max_turns > 0 {
            return self.config.max_turns;
        }
        self.definition.max_turns
    }

    /// Run the agent with the given task and conversation context.
    ///
    /// This is the main entry point for agent execution. In a real
    /// implementation, this would invoke the LLM API with the resolved
    /// tools, system prompt, and conversation history.
    ///
    /// For now, this returns a placeholder result indicating the agent
    /// was configured correctly.
    pub async fn run(
        &self,
        task: &str,
        _context: &[ConversationEntry],
    ) -> Result<AgentRunResult> {
        let system_prompt = self.build_system_prompt();
        let max_turns = self.resolve_max_turns();
        let model = self.resolve_model("default");

        // In a real implementation, this would:
        // 1. Create a query loop with the resolved tools
        // 2. Inject the system prompt
        // 3. Run the agent loop up to max_turns
        // 4. Collect the final output and usage

        tracing::info!(
            agent_type = %self.definition.agent_type,
            model = %model,
            max_turns = max_turns,
            prompt_len = system_prompt.len(),
            "Agent run configured for task: {}",
            task.chars().take(80).collect::<String>()
        );

        Ok(AgentRunResult {
            output: format!(
                "Agent '{}' configured but not yet executed (model: {}, max_turns: {})",
                self.definition.agent_type, model, max_turns
            ),
            success: true,
            turns: 0,
            usage: UsageSummary::default(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definition::AgentSource;

    fn test_definition() -> AgentDefinition {
        AgentDefinition {
            agent_type: "test-agent".to_owned(),
            when_to_use: "Test agent".to_owned(),
            tools: vec!["Bash".to_owned(), "Read".to_owned()],
            disallowed_tools: vec!["Agent".to_owned()],
            max_turns: 100,
            model: Some("haiku".to_owned()),
            permission_mode: None,
            source: AgentSource::BuiltIn,
            base_dir: "built-in".to_owned(),
            system_prompt: Some("You are a test agent.".to_owned()),
            skills: Vec::new(),
            memory: None,
            background: false,
            isolation: crate::definition::AgentIsolation::None,
            initial_prompt: None,
            omit_claude_md: false,
            filename: None,
        }
    }

    fn test_config() -> AgentRunConfig {
        AgentRunConfig {
            max_turns: 0,
            model: String::new(),
            tools: vec!["Bash".to_owned(), "Read".to_owned(), "Write".to_owned()],
            system_prompt: None,
            working_dir: PathBuf::from("/tmp"),
        }
    }

    #[test]
    fn resolve_tools_with_allowlist_and_denylist() {
        let runner = AgentRunner::new(test_definition(), test_config());
        let available = vec![
            "Bash".to_owned(),
            "Read".to_owned(),
            "Write".to_owned(),
            "Agent".to_owned(),
        ];
        let tools = runner.resolve_tools(&available);
        assert_eq!(tools, vec!["Bash", "Read"]); // Agent filtered by denylist
    }

    #[test]
    fn resolve_tools_wildcard() {
        let mut def = test_definition();
        def.tools = vec!["*".to_owned()];
        let runner = AgentRunner::new(def, test_config());
        let available = vec!["Bash".to_owned(), "Read".to_owned()];
        let tools = runner.resolve_tools(&available);
        assert_eq!(tools, vec!["Bash", "Read"]);
    }

    #[test]
    fn resolve_tools_denylist_only() {
        let mut def = test_definition();
        def.tools = Vec::new();
        let runner = AgentRunner::new(def, test_config());
        let available = vec![
            "Bash".to_owned(),
            "Read".to_owned(),
            "Agent".to_owned(),
        ];
        let tools = runner.resolve_tools(&available);
        assert_eq!(tools, vec!["Bash", "Read"]);
    }

    #[test]
    fn resolve_tools_no_restrictions() {
        let mut def = test_definition();
        def.tools = Vec::new();
        def.disallowed_tools = Vec::new();
        let runner = AgentRunner::new(def, test_config());
        let available = vec!["Bash".to_owned(), "Read".to_owned()];
        let tools = runner.resolve_tools(&available);
        assert_eq!(tools, vec!["Bash", "Read"]);
    }

    #[test]
    fn build_system_prompt_uses_definition() {
        let runner = AgentRunner::new(test_definition(), test_config());
        let prompt = runner.build_system_prompt();
        assert_eq!(prompt, "You are a test agent.");
    }

    #[test]
    fn build_system_prompt_default_when_empty() {
        let mut def = test_definition();
        def.system_prompt = None;
        let runner = AgentRunner::new(def, test_config());
        let prompt = runner.build_system_prompt();
        assert!(prompt.contains("test-agent"));
    }

    #[test]
    fn resolve_model_from_definition() {
        let runner = AgentRunner::new(test_definition(), test_config());
        assert_eq!(runner.resolve_model("sonnet"), "haiku");
    }

    #[test]
    fn resolve_model_config_overrides() {
        let mut config = test_config();
        config.model = "opus".to_owned();
        let runner = AgentRunner::new(test_definition(), config);
        assert_eq!(runner.resolve_model("sonnet"), "opus");
    }

    #[test]
    fn resolve_model_inherit_falls_through() {
        let mut def = test_definition();
        def.model = Some("inherit".to_owned());
        let runner = AgentRunner::new(def, test_config());
        assert_eq!(runner.resolve_model("sonnet"), "sonnet");
    }

    #[test]
    fn resolve_max_turns_from_definition() {
        let runner = AgentRunner::new(test_definition(), test_config());
        assert_eq!(runner.resolve_max_turns(), 100);
    }

    #[test]
    fn resolve_max_turns_config_overrides() {
        let mut config = test_config();
        config.max_turns = 50;
        let runner = AgentRunner::new(test_definition(), config);
        assert_eq!(runner.resolve_max_turns(), 50);
    }

    #[tokio::test]
    async fn run_returns_placeholder_result() {
        let runner = AgentRunner::new(test_definition(), test_config());
        let result = runner.run("test task", &[]).await.expect("run");
        assert!(result.success);
        assert!(result.output.contains("test-agent"));
    }
}

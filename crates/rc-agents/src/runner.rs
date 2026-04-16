//! Agent execution runner matching Claude Code's `AgentTool/runAgent.ts`.
//!
//! The [`AgentRunner`] orchestrates agent execution: resolving tools, building
//! the system prompt, and tracking turns and usage.
//!
//! # Enhanced Functions
//!
//! - [`enhance_system_prompt_with_env_details`] — Inject environment info into prompts
//! - [`resolve_effective_tools`] — Resolve tool set with wildcard/denylist support
//! - [`aggregate_run_results`] — Aggregate multiple run results

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

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

// ── Enhanced functions ────────────────────────────────────────────────────

/// Result of resolving effective tools for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedTools {
    /// Whether the agent has a wildcard tool specification.
    pub has_wildcard: bool,
    /// Valid tool names from the agent's specification.
    pub valid_tools: Vec<String>,
    /// Invalid tool names that couldn't be resolved.
    pub invalid_tools: Vec<String>,
    /// The resolved set of tool names.
    pub resolved: Vec<String>,
}

/// Enhance a system prompt with environment details.
///
/// Appends information about the working directory, absolute paths,
/// and formatting guidelines (no emoji, no colons before tool calls).
pub fn enhance_system_prompt_with_env_details(
    base_prompt: &str,
    working_dir: &Path,
    absolute_paths: &[&Path],
) -> String {
    let mut prompt = base_prompt.to_owned();

    prompt.push_str("\n\n## Environment\n");
    prompt.push_str(&format!(
        "Working directory: {}\n",
        working_dir.display()
    ));

    if !absolute_paths.is_empty() {
        prompt.push_str("Additional paths:\n");
        for path in absolute_paths {
            prompt.push_str(&format!("- {}\n", path.display()));
        }
    }

    prompt.push_str("\n## Formatting Guidelines\n");
    prompt.push_str("- Use absolute paths when referring to files\n");
    prompt.push_str("- Do not use emoji in output\n");
    prompt.push_str("- Do not use colons before tool calls\n");

    prompt
}

/// Resolve the effective tool set for an agent.
///
/// Handles wildcard expansion (`*`), denylist filtering, and validation
/// against the available tool set. Returns a [`ResolvedTools`] with
/// detailed information about the resolution.
pub fn resolve_effective_tools(
    agent_tools: &[String],
    disallowed_tools: &[String],
    available_tools: &[String],
) -> ResolvedTools {
    let deny_set: BTreeSet<&str> = disallowed_tools.iter().map(|s| s.as_str()).collect();
    let available_set: BTreeSet<&str> = available_tools.iter().map(|s| s.as_str()).collect();

    // Check for wildcard (explicit "*" or empty means all tools)
    let has_wildcard = agent_tools.is_empty()
        || (agent_tools.len() == 1 && agent_tools[0] == "*");

    if has_wildcard || agent_tools.is_empty() {
        let resolved: Vec<String> = available_tools
            .iter()
            .filter(|t| !deny_set.contains(t.as_str()))
            .cloned()
            .collect();
        return ResolvedTools {
            has_wildcard,
            valid_tools: Vec::new(),
            invalid_tools: Vec::new(),
            resolved,
        };
    }

    let mut valid = Vec::new();
    let mut invalid = Vec::new();
    let mut resolved = Vec::new();
    let mut seen = BTreeSet::new();

    for tool in agent_tools {
        if deny_set.contains(tool.as_str()) {
            // Tool is in denylist — skip
            continue;
        }
        if available_set.contains(tool.as_str()) {
            valid.push(tool.clone());
            if seen.insert(tool.as_str()) {
                resolved.push(tool.clone());
            }
        } else {
            invalid.push(tool.clone());
        }
    }

    ResolvedTools {
        has_wildcard: false,
        valid_tools: valid,
        invalid_tools: invalid,
        resolved,
    }
}

/// Aggregate multiple agent run results into a single summary.
///
/// Combines output, usage, and success status from multiple runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedRunResults {
    /// Combined output from all runs.
    pub combined_output: String,
    /// Whether all runs succeeded.
    pub all_succeeded: bool,
    /// Total turns across all runs.
    pub total_turns: u32,
    /// Aggregated usage.
    pub total_usage: UsageSummary,
    /// Number of runs.
    pub run_count: usize,
}

/// Aggregate multiple run results.
pub fn aggregate_run_results(results: &[AgentRunResult]) -> AggregatedRunResults {
    let mut combined_output = String::new();
    let mut all_succeeded = true;
    let mut total_turns = 0u32;
    let mut total_usage = UsageSummary::default();

    for (i, result) in results.iter().enumerate() {
        if i > 0 {
            combined_output.push_str("\n---\n");
        }
        combined_output.push_str(&result.output);

        if !result.success {
            all_succeeded = false;
        }
        total_turns += result.turns;
        total_usage.input_tokens += result.usage.input_tokens;
        total_usage.output_tokens += result.usage.output_tokens;
        total_usage.cache_creation_tokens += result.usage.cache_creation_tokens;
        total_usage.cache_read_tokens += result.usage.cache_read_tokens;
    }

    AggregatedRunResults {
        combined_output,
        all_succeeded,
        total_turns,
        total_usage,
        run_count: results.len(),
    }
}

/// Format an agent result for return to the caller.
///
/// Produces a structured output string with the agent's final text,
/// usage information, and status.
pub fn format_agent_run_result(agent_id: &str, result: &AgentRunResult) -> String {
    let status = if result.success { "completed" } else { "failed" };
    format!(
        "Agent {agent_id} {status}\n\
         Turns: {turns}\n\
         Tokens: {input_in}+{output_out} (cache: +{cache_create}, -{cache_read})\n\
         Output:\n{output}",
        turns = result.turns,
        input_in = result.usage.input_tokens,
        output_out = result.usage.output_tokens,
        cache_create = result.usage.cache_creation_tokens,
        cache_read = result.usage.cache_read_tokens,
        output = result.output,
    )
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

    // ── Enhanced tests ──────────────────────────────────────────────────

    #[test]
    fn enhance_system_prompt_adds_env_details() {
        let base = "You are a test agent.";
        let working_dir = PathBuf::from("/home/user/project");
        let tmp = PathBuf::from("/tmp");
        let extra = vec![tmp.as_path()];
        let enhanced = enhance_system_prompt_with_env_details(base, &working_dir, &extra);
        assert!(enhanced.starts_with("You are a test agent."));
        assert!(enhanced.contains("## Environment"));
        assert!(enhanced.contains("/home/user/project"));
        assert!(enhanced.contains("/tmp"));
    }

    #[test]
    fn enhance_system_prompt_no_extra_paths() {
        let base = "Base prompt";
        let working_dir = PathBuf::from("/tmp");
        let enhanced = enhance_system_prompt_with_env_details(base, &working_dir, &[]);
        assert!(enhanced.contains("## Environment"));
        assert!(!enhanced.contains("Additional paths"));
    }

    #[test]
    fn enhance_system_prompt_formatting_guidelines() {
        let base = "Base";
        let working_dir = PathBuf::from("/tmp");
        let enhanced = enhance_system_prompt_with_env_details(base, &working_dir, &[]);
        assert!(enhanced.contains("## Formatting Guidelines"));
        assert!(enhanced.contains("absolute paths"));
        assert!(enhanced.contains("emoji"));
        assert!(enhanced.contains("colons before tool calls"));
    }

    #[test]
    fn resolve_effective_tools_wildcard() {
        let agent_tools = vec!["*".to_owned()];
        let disallowed = vec!["Agent".to_owned()];
        let available = vec!["Bash".to_owned(), "Read".to_owned(), "Agent".to_owned()];
        let result = resolve_effective_tools(&agent_tools, &disallowed, &available);
        assert!(result.has_wildcard);
        assert_eq!(result.resolved, vec!["Bash", "Read"]);
    }

    #[test]
    fn resolve_effective_tools_specific_list() {
        let agent_tools = vec!["Bash".to_owned(), "Read".to_owned(), "NonExistent".to_owned()];
        let disallowed: Vec<String> = Vec::new();
        let available = vec!["Bash".to_owned(), "Read".to_owned(), "Write".to_owned()];
        let result = resolve_effective_tools(&agent_tools, &disallowed, &available);
        assert!(!result.has_wildcard);
        assert_eq!(result.valid_tools, vec!["Bash", "Read"]);
        assert_eq!(result.invalid_tools, vec!["NonExistent"]);
        assert_eq!(result.resolved, vec!["Bash", "Read"]);
    }

    #[test]
    fn resolve_effective_tools_denylist_filters() {
        let agent_tools = vec!["Bash".to_owned(), "Agent".to_owned()];
        let disallowed = vec!["Agent".to_owned()];
        let available = vec!["Bash".to_owned(), "Agent".to_owned()];
        let result = resolve_effective_tools(&agent_tools, &disallowed, &available);
        assert_eq!(result.resolved, vec!["Bash"]);
    }

    #[test]
    fn resolve_effective_tools_empty_agent_tools() {
        let agent_tools: Vec<String> = Vec::new();
        let disallowed: Vec<String> = Vec::new();
        let available = vec!["Bash".to_owned()];
        let result = resolve_effective_tools(&agent_tools, &disallowed, &available);
        assert!(result.has_wildcard); // Empty means all tools
        assert_eq!(result.resolved, vec!["Bash"]);
    }

    #[test]
    fn resolve_effective_tools_deduplicates() {
        let agent_tools = vec!["Bash".to_owned(), "Bash".to_owned()];
        let disallowed: Vec<String> = Vec::new();
        let available = vec!["Bash".to_owned()];
        let result = resolve_effective_tools(&agent_tools, &disallowed, &available);
        assert_eq!(result.resolved, vec!["Bash"]);
    }

    #[test]
    fn aggregate_run_results_single() {
        let results = vec![AgentRunResult {
            output: "Done".to_owned(),
            success: true,
            turns: 3,
            usage: UsageSummary {
                input_tokens: 100,
                output_tokens: 50,
                cache_creation_tokens: 0,
                cache_read_tokens: 0,
            },
        }];
        let agg = aggregate_run_results(&results);
        assert!(agg.all_succeeded);
        assert_eq!(agg.run_count, 1);
        assert_eq!(agg.total_turns, 3);
        assert_eq!(agg.total_usage.input_tokens, 100);
        assert!(agg.combined_output.contains("Done"));
    }

    #[test]
    fn aggregate_run_results_multiple() {
        let results = vec![
            AgentRunResult {
                output: "First".to_owned(),
                success: true,
                turns: 2,
                usage: UsageSummary {
                    input_tokens: 50,
                    output_tokens: 25,
                    cache_creation_tokens: 0,
                    cache_read_tokens: 0,
                },
            },
            AgentRunResult {
                output: "Second".to_owned(),
                success: false,
                turns: 1,
                usage: UsageSummary {
                    input_tokens: 30,
                    output_tokens: 15,
                    cache_creation_tokens: 0,
                    cache_read_tokens: 0,
                },
            },
        ];
        let agg = aggregate_run_results(&results);
        assert!(!agg.all_succeeded);
        assert_eq!(agg.run_count, 2);
        assert_eq!(agg.total_turns, 3);
        assert_eq!(agg.total_usage.input_tokens, 80);
        assert!(agg.combined_output.contains("First"));
        assert!(agg.combined_output.contains("Second"));
        assert!(agg.combined_output.contains("---"));
    }

    #[test]
    fn aggregate_run_results_empty() {
        let agg = aggregate_run_results(&[]);
        assert!(agg.all_succeeded);
        assert_eq!(agg.run_count, 0);
        assert!(agg.combined_output.is_empty());
    }

    #[test]
    fn format_agent_run_result_success() {
        let result = AgentRunResult {
            output: "Fixed the bug".to_owned(),
            success: true,
            turns: 5,
            usage: UsageSummary {
                input_tokens: 100,
                output_tokens: 50,
                cache_creation_tokens: 10,
                cache_read_tokens: 20,
            },
        };
        let formatted = format_agent_run_result("agent-123", &result);
        assert!(formatted.contains("agent-123 completed"));
        assert!(formatted.contains("Turns: 5"));
        assert!(formatted.contains("100+50"));
        assert!(formatted.contains("Fixed the bug"));
    }

    #[test]
    fn format_agent_run_result_failure() {
        let result = AgentRunResult {
            output: "Error occurred".to_owned(),
            success: false,
            turns: 0,
            usage: UsageSummary::default(),
        };
        let formatted = format_agent_run_result("agent-456", &result);
        assert!(formatted.contains("agent-456 failed"));
    }
}

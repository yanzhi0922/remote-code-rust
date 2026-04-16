//! Agent tool prompt builder matching Claude Code's `AgentTool/prompt.ts`.
//!
//! Generates the tool description prompt for the Agent tool, including agent
//! listings, usage notes, fork instructions, and examples.

use crate::constants::AGENT_TOOL_NAME;
use crate::definition::AgentDefinition;

/// Build the complete agent tool prompt.
///
/// This function constructs the prompt text that describes how to use the
/// Agent tool, including available agent types, usage guidelines, and examples.
///
/// # Arguments
/// * `agents` - The list of available agent definitions
/// * `is_fork_enabled` - Whether fork subagent mode is enabled
/// * `is_coordinator` - Whether this is a coordinator-mode prompt (slim version)
/// * `allowed_agent_types` - Optional filter restricting which agents can be spawned
pub fn build_agent_prompt(
    agents: &[AgentDefinition],
    is_fork_enabled: bool,
    is_coordinator: bool,
    allowed_agent_types: Option<&[String]>,
) -> String {
    let effective_agents = filter_agents(agents, allowed_agent_types);

    let shared = build_shared_prompt(&effective_agents, is_fork_enabled);

    if is_coordinator {
        return shared;
    }

    let when_not_to_use = if is_fork_enabled {
        String::new()
    } else {
        build_when_not_to_use_section()
    };

    let when_to_fork = if is_fork_enabled {
        build_when_to_fork_section()
    } else {
        String::new()
    };

    let writing_the_prompt = build_writing_the_prompt_section(is_fork_enabled);

    let examples = if is_fork_enabled {
        build_fork_examples()
    } else {
        build_current_examples()
    };

    format!(
        "{shared}\n{when_not_to_use}\n\n\
        Usage notes:\n\
        - Always include a short description (3-5 words) summarizing what the agent will do\n\
        - Launch multiple agents concurrently whenever possible, to maximize performance; \
        to do that, use a single message with multiple tool uses\n\
        - When the agent is done, it will return a single message back to you. The result \
        returned by the agent is not visible to the user. To show the user the result, you \
        should send a text message back to the user with a concise summary of the result.\n\
        - You can optionally run agents in the background using the run_in_background parameter. \
        When an agent runs in the background, you will be automatically notified when it completes \
        — do NOT sleep, poll, or proactively check on its progress.\n\
        - **Foreground vs background**: Use foreground (default) when you need the agent's results \
        before you can proceed. Use background when you have genuinely independent work to do in parallel.\n\
        - To continue a previously spawned agent, use SendMessage with the agent's ID or name as the `to` field. \
        {continuation_note}\n\
        - The agent's outputs should generally be trusted\n\
        - Clearly tell the agent whether you expect it to write code or just to do research\n\
        - If the agent description mentions that it should be used proactively, then you should \
        try your best to use it without the user having to ask for it first.\n\
        - If the user specifies that they want you to run agents \"in parallel\", you MUST send a \
        single message with multiple {AGENT_TOOL_NAME} tool use content blocks.\n\
        {when_to_fork}\n\
        {writing_the_prompt}\n\n\
        {examples}",
        continuation_note = if is_fork_enabled {
            "Each fresh Agent invocation with a subagent_type starts without context — provide a complete task description."
        } else {
            "Each Agent invocation starts fresh — provide a complete task description."
        }
    )
}

/// Filter agents by allowed types.
fn filter_agents<'a>(
    agents: &'a [AgentDefinition],
    allowed: Option<&[String]>,
) -> Vec<&'a AgentDefinition> {
    match allowed {
        Some(types) => agents
            .iter()
            .filter(|a| types.contains(&a.agent_type))
            .collect(),
        None => agents.iter().collect(),
    }
}

/// Build the shared core prompt used by both coordinator and non-coordinator modes.
fn build_shared_prompt(agents: &[&AgentDefinition], is_fork_enabled: bool) -> String {
    let agent_list = if agents.is_empty() {
        "No agents available.".to_owned()
    } else {
        agents
            .iter()
            .map(|a| format_agent_line(a))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let fork_or_type = if is_fork_enabled {
        format!(
            "When using the {AGENT_TOOL_NAME} tool, specify a subagent_type to use a specialized \
            agent, or omit it to fork yourself — a fork inherits your full conversation context."
        )
    } else {
        format!(
            "When using the {AGENT_TOOL_NAME} tool, specify a subagent_type parameter to select \
            which agent type to use. If omitted, the general-purpose agent is used."
        )
    };

    format!(
        "Launch a new agent to handle complex, multi-step tasks autonomously.\n\n\
        The {AGENT_TOOL_NAME} tool launches specialized agents (subprocesses) that autonomously \
        handle complex tasks. Each agent type has specific capabilities and tools available to it.\n\n\
        Available agent types and the tools they have access to:\n\
        {agent_list}\n\n\
        {fork_or_type}"
    )
}

/// Build the "When NOT to use" section.
fn build_when_not_to_use_section() -> String {
    format!(
        "\nWhen NOT to use the {AGENT_TOOL_NAME} tool:\n\
        - If you want to read a specific file path, use the Read tool or Glob tool instead, \
        to find the match more quickly\n\
        - If you are searching for a specific class definition like \"class Foo\", use the Grep \
        tool instead, to find the match more quickly\n\
        - If you are searching for code within a specific file or set of 2-3 files, use the Read \
        tool instead of the {AGENT_TOOL_NAME} tool, to find the match more quickly\n\
        - Other tasks that are not related to the agent descriptions above"
    )
}

/// Build the "When to fork" section (fork mode only).
fn build_when_to_fork_section() -> String {
    "\n## When to fork\n\n\
        Fork yourself (omit `subagent_type`) when the intermediate tool output isn't worth \
        keeping in your context. The criterion is qualitative — \"will I need this output again\" \
        — not task size.\n\
        - **Research**: fork open-ended questions. If research can be broken into independent \
        questions, launch parallel forks in one message. A fork beats a fresh subagent for this — \
        it inherits context and shares your cache.\n\
        - **Implementation**: prefer to fork implementation work that requires more than a couple \
        of edits. Do research before jumping to implementation.\n\n\
        Forks are cheap because they share your prompt cache. Don't set `model` on a fork — a \
        different model can't reuse the parent's cache. Pass a short `name` (one or two words, \
        lowercase) so the user can see the fork in the teams panel and steer it mid-run.\n\n\
        **Don't peek.** The tool result includes an `output_file` path — do not Read or tail it \
        unless the user explicitly asks for a progress check. You get a completion notification; \
        trust it.\n\n\
        **Don't race.** After launching, you know nothing about what the fork found. Never fabricate \
        or predict fork results in any format. The notification arrives as a user-role message in \
        a later turn; it is never something you write yourself.\n\n\
        **Writing a fork prompt.** Since the fork inherits your context, the prompt is a *directive* \
        — what to do, not what the situation is. Be specific about scope: what's in, what's out, \
        what another agent is handling. Don't re-explain background.".to_string()
}

/// Build the "Writing the prompt" section.
fn build_writing_the_prompt_section(is_fork_enabled: bool) -> String {
    let prefix = if is_fork_enabled {
        "When spawning a fresh agent (with a `subagent_type`), it starts with zero context. "
    } else {
        ""
    };

    format!(
        "\n## Writing the prompt\n\n\
        {prefix}Brief the agent like a smart colleague who just walked into the room — it hasn't \
        seen this conversation, doesn't know what you've tried, doesn't understand why this task matters.\n\
        - Explain what you're trying to accomplish and why.\n\
        - Describe what you've already learned or ruled out.\n\
        - Give enough context about the surrounding problem that the agent can make judgment calls \
        rather than just following a narrow instruction.\n\
        - If you need a short response, say so (\"report in under 200 words\").\n\
        - Lookups: hand over the exact command. Investigations: hand over the question — prescribed \
        steps become dead weight when the premise is wrong.\n\n\
        {terse_note}\n\n\
        **Never delegate understanding.** Don't write \"based on your findings, fix the bug\" or \
        \"based on the research, implement it.\" Those phrases push synthesis onto the agent instead \
        of doing it yourself. Write prompts that prove you understood: include file paths, line \
        numbers, what specifically to change.",
        prefix = prefix,
        terse_note = if is_fork_enabled {
            "For fresh agents, terse command-style prompts produce shallow, generic work."
        } else {
            "Terse command-style prompts produce shallow, generic work."
        }
    )
}

/// Build fork-mode examples.
fn build_fork_examples() -> String {
    format!(
        "Example usage:\n\n\
        <example>\n\
        user: \"What's left on this branch before we can ship?\"\n\
        assistant: Forking this — it's a survey question.\n\
        {AGENT_TOOL_NAME}({{\n\
          name: \"ship-audit\",\n\
          description: \"Branch ship-readiness audit\",\n\
          prompt: \"Audit what's left before this branch can ship. Check: uncommitted changes, \
        commits ahead of main, whether tests exist. Report a punch list — done vs. missing. \
        Under 200 words.\"\n\
        }})\n\
        </example>\n\n\
        <example>\n\
        user: \"Can you get a second opinion on whether this migration is safe?\"\n\
        assistant: I'll ask the verification agent — it won't see my analysis, so it can give \
        an independent read.\n\
        {AGENT_TOOL_NAME}({{\n\
          name: \"migration-review\",\n\
          description: \"Independent migration review\",\n\
          subagent_type: \"verification\",\n\
          prompt: \"Review migration 0042_user_schema.sql for safety. Context: we're adding a \
        NOT NULL column to a 50M-row table. I want a second opinion on whether the backfill \
        approach is safe under concurrent writes. Report: is this safe, and if not, what \
        specifically breaks?\"\n\
        }})\n\
        </example>"
    )
}

/// Build standard (non-fork) examples.
fn build_current_examples() -> String {
    format!(
        "Example usage:\n\n\
        <example_agent_descriptions>\n\
        \"test-runner\": use this agent after you are done writing code to run tests\n\
        \"greeting-responder\": use this agent to respond to user greetings with a friendly joke\n\
        </example_agent_descriptions>\n\n\
        <example>\n\
        user: \"Please write a function that checks if a number is prime\"\n\
        assistant: I'm going to use the Write tool to write the following code.\n\
        Since a significant piece of code was written and the task was completed, now use the \
        test-runner agent to run the tests.\n\
        assistant: Uses the {AGENT_TOOL_NAME} tool to launch the test-runner agent\n\
        </example>"
    )
}

/// Format one agent line for the agent listing: `- type: whenToUse (Tools: ...)`.
pub fn format_agent_line(agent: &AgentDefinition) -> String {
    let tools_description = get_tools_description(agent);
    format!(
        "- {}: {} (Tools: {})",
        agent.agent_type, agent.when_to_use, tools_description
    )
}

/// Get a human-readable description of an agent's tool access.
pub fn get_tools_description(agent: &AgentDefinition) -> String {
    let has_allowlist = agent.has_tool_allowlist();
    let has_denylist = agent.has_tool_denylist();

    if has_allowlist && has_denylist {
        // Both defined: filter allowlist by denylist
        let deny_set: std::collections::HashSet<&str> =
            agent.disallowed_tools.iter().map(|s| s.as_str()).collect();
        let effective: Vec<&str> = agent
            .tools
            .iter()
            .map(|s| s.as_str())
            .filter(|t| !deny_set.contains(t))
            .collect();
        if effective.is_empty() {
            return "None".to_owned();
        }
        effective.join(", ")
    } else if has_allowlist {
        agent.tools.join(", ")
    } else if has_denylist {
        format!("All tools except {}", agent.disallowed_tools.join(", "))
    } else {
        "All tools".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definition::AgentDefinition;

    #[test]
    fn format_agent_line_basic() {
        let agent = AgentDefinition::new("test", "A test agent");
        let line = format_agent_line(&agent);
        assert!(line.starts_with("- test: A test agent (Tools: All tools)"));
    }

    #[test]
    fn format_agent_line_with_tools() {
        let mut agent = AgentDefinition::new("test", "desc");
        agent.tools = vec!["Bash".to_owned(), "Read".to_owned()];
        let line = format_agent_line(&agent);
        assert!(line.contains("Tools: Bash, Read"));
    }

    #[test]
    fn format_agent_line_with_denylist() {
        let mut agent = AgentDefinition::new("test", "desc");
        agent.disallowed_tools = vec!["Agent".to_owned(), "Write".to_owned()];
        let line = format_agent_line(&agent);
        assert!(line.contains("All tools except Agent, Write"));
    }

    #[test]
    fn tools_description_both_lists() {
        let mut agent = AgentDefinition::new("test", "desc");
        agent.tools = vec!["Bash".to_owned(), "Read".to_owned(), "Write".to_owned()];
        agent.disallowed_tools = vec!["Write".to_owned()];
        let desc = get_tools_description(&agent);
        assert_eq!(desc, "Bash, Read");
    }

    #[test]
    fn tools_description_empty_after_filter() {
        let mut agent = AgentDefinition::new("test", "desc");
        agent.tools = vec!["Write".to_owned()];
        agent.disallowed_tools = vec!["Write".to_owned()];
        let desc = get_tools_description(&agent);
        assert_eq!(desc, "None");
    }

    #[test]
    fn tools_description_wildcard() {
        let mut agent = AgentDefinition::new("test", "desc");
        agent.tools = vec!["*".to_owned()];
        let desc = get_tools_description(&agent);
        assert_eq!(desc, "*");
    }

    #[test]
    fn build_prompt_coordinator_mode_is_slim() {
        let agents = vec![AgentDefinition::new("test", "desc")];
        let prompt = build_agent_prompt(&agents, false, true, None);
        assert!(prompt.contains("Launch a new agent"));
        assert!(!prompt.contains("Usage notes:"));
    }

    #[test]
    fn build_prompt_non_coordinator_has_usage_notes() {
        let agents = vec![AgentDefinition::new("test", "desc")];
        let prompt = build_agent_prompt(&agents, false, false, None);
        assert!(prompt.contains("Usage notes:"));
        assert!(prompt.contains("When NOT to use"));
    }

    #[test]
    fn build_prompt_fork_mode_adds_fork_section() {
        let agents = vec![AgentDefinition::new("test", "desc")];
        let prompt = build_agent_prompt(&agents, true, false, None);
        assert!(prompt.contains("When to fork"));
        assert!(prompt.contains("Writing a fork prompt"));
        assert!(!prompt.contains("When NOT to use"));
    }

    #[test]
    fn build_prompt_filters_by_allowed_types() {
        let agents = vec![
            AgentDefinition::new("a", "agent a"),
            AgentDefinition::new("b", "agent b"),
        ];
        let allowed = vec!["a".to_owned()];
        let prompt = build_agent_prompt(&agents, false, false, Some(&allowed));
        assert!(prompt.contains("- a:"));
        assert!(!prompt.contains("- b:"));
    }

    #[test]
    fn build_prompt_no_agents() {
        let agents: Vec<AgentDefinition> = vec![];
        let prompt = build_agent_prompt(&agents, false, false, None);
        assert!(prompt.contains("No agents available"));
    }

    #[test]
    fn full_prompt_with_builtins() {
        let agents = crate::builtins::get_built_in_agents();
        let prompt = build_agent_prompt(&agents, false, false, None);
        assert!(prompt.contains("general-purpose"));
        assert!(prompt.contains("Explore"));
        assert!(prompt.contains("Plan"));
        assert!(prompt.contains("verification"));
    }
}

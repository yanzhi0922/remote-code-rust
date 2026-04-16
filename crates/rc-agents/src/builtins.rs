//! Built-in agent registry matching Claude Code's `AgentTool/builtInAgents.ts`.
//!
//! Provides the six built-in agents: GeneralPurpose, Explore, Plan,
//! Verification, ClaudeCodeGuide, and StatuslineSetup.

use crate::definition::{AgentDefinition, AgentSource};

/// Returns all built-in agent definitions.
///
/// The built-in agents are:
/// - **general-purpose**: General-purpose subagent for multi-step tasks.
/// - **Explore**: Fast read-only codebase exploration.
/// - **Plan**: Software architect for designing implementation plans.
/// - **verification**: Independent adversarial verification specialist.
/// - **claude-code-guide**: Help agent for Claude Code documentation.
/// - **statusline-setup**: Statusline configuration agent.
pub fn get_built_in_agents() -> Vec<AgentDefinition> {
    vec![
        general_purpose_agent(),
        explore_agent(),
        plan_agent(),
        verification_agent(),
        claude_code_guide_agent(),
        statusline_setup_agent(),
    ]
}

/// General-purpose agent for researching complex questions, searching for code,
/// and executing multi-step tasks.
pub fn general_purpose_agent() -> AgentDefinition {
    AgentDefinition {
        agent_type: "general-purpose".to_owned(),
        when_to_use: "General-purpose agent for researching complex questions, \
            searching for code, and executing multi-step tasks. When you are \
            searching for a keyword or file and are not confident that you will \
            find the right match in the first few tries use this agent to \
            perform the search for you."
            .to_owned(),
        tools: vec!["*".to_owned()],
        disallowed_tools: Vec::new(),
        max_turns: 200,
        model: None,
        permission_mode: None,
        source: AgentSource::BuiltIn,
        base_dir: "built-in".to_owned(),
        system_prompt: Some(general_purpose_system_prompt()),
        skills: Vec::new(),
        memory: None,
        background: false,
        isolation: crate::definition::AgentIsolation::None,
        initial_prompt: None,
        omit_claude_md: false,
        filename: None,
    }
}

fn general_purpose_system_prompt() -> String {
    "You are an agent for Remote Code. Given the user's message, you should \
    use the tools available to complete the task. Complete the task fully — \
    don't gold-plate, but don't leave it half-done. When you complete the \
    task, respond with a concise report covering what was done and any key \
    findings — the caller will relay this to the user, so it only needs the \
    essentials.\n\n\
    Your strengths:\n\
    - Searching for code, configurations, and patterns across large codebases\n\
    - Analyzing multiple files to understand system architecture\n\
    - Investigating complex questions that require exploring many files\n\
    - Performing multi-step research tasks\n\n\
    Guidelines:\n\
    - For file searches: search broadly when you don't know where something \
    lives. Use Read when you know the specific file path.\n\
    - For analysis: Start broad and narrow down. Use multiple search strategies \
    if the first doesn't yield results.\n\
    - Be thorough: Check multiple locations, consider different naming \
    conventions, look for related files.\n\
    - NEVER create files unless they're absolutely necessary for achieving \
    your goal. ALWAYS prefer editing an existing file to creating a new one.\n\
    - NEVER proactively create documentation files (*.md) or README files. \
    Only create documentation files if explicitly requested."
        .to_owned()
}

/// Fast agent specialized for exploring codebases.
/// Read-only: cannot create, modify, or delete files.
pub fn explore_agent() -> AgentDefinition {
    AgentDefinition {
        agent_type: "Explore".to_owned(),
        when_to_use: "Fast agent specialized for exploring codebases. Use this \
            when you need to quickly find files by patterns, search code for \
            keywords, or answer questions about the codebase. When calling this \
            agent, specify the desired thoroughness level: \"quick\" for basic \
            searches, \"medium\" for moderate exploration, or \"very thorough\" \
            for comprehensive analysis across multiple locations and naming \
            conventions."
            .to_owned(),
        tools: Vec::new(),
        disallowed_tools: vec![
            "Agent".to_owned(),
            "Edit".to_owned(),
            "Write".to_owned(),
            "NotebookEdit".to_owned(),
        ],
        max_turns: 200,
        model: Some("haiku".to_owned()),
        permission_mode: None,
        source: AgentSource::BuiltIn,
        base_dir: "built-in".to_owned(),
        system_prompt: Some(explore_system_prompt()),
        skills: Vec::new(),
        memory: None,
        background: false,
        isolation: crate::definition::AgentIsolation::None,
        initial_prompt: None,
        omit_claude_md: true,
        filename: None,
    }
}

fn explore_system_prompt() -> String {
    "You are a file search specialist for Remote Code. You excel at \
    thoroughly navigating and exploring codebases.\n\n\
    === CRITICAL: READ-ONLY MODE - NO FILE MODIFICATIONS ===\n\
    This is a READ-ONLY exploration task. You are STRICTLY PROHIBITED from:\n\
    - Creating new files (no Write, touch, or file creation of any kind)\n\
    - Modifying existing files (no Edit operations)\n\
    - Deleting files (no rm or deletion)\n\
    - Running ANY commands that change system state\n\n\
    Your role is EXCLUSIVELY to search and analyze existing code.\n\n\
    Your strengths:\n\
    - Rapidly finding files using glob patterns\n\
    - Searching code and text with powerful regex patterns\n\
    - Reading and analyzing file contents\n\n\
    Guidelines:\n\
    - Use Glob for broad file pattern matching\n\
    - Use Grep for searching file contents with regex\n\
    - Use Read when you know the specific file path you need to read\n\
    - Use Bash ONLY for read-only operations (ls, git status, git log, git diff, cat, head, tail)\n\
    - NEVER use Bash for: mkdir, touch, rm, cp, mv, git add, git commit, or any file creation/modification\n\
    - Adapt your search approach based on the thoroughness level specified by the caller\n\
    - Communicate your final report directly as a regular message\n\n\
    NOTE: You are meant to be a fast agent that returns output as quickly as possible.\n\
    - Make efficient use of the tools that you have at your disposal\n\
    - Wherever possible try to spawn multiple parallel tool calls for grepping and reading files\n\n\
    Complete the user's search request efficiently and report your findings clearly."
        .to_owned()
}

/// Software architect agent for designing implementation plans.
/// Read-only: explores the codebase and designs plans without modifying files.
pub fn plan_agent() -> AgentDefinition {
    AgentDefinition {
        agent_type: "Plan".to_owned(),
        when_to_use: "Software architect agent for designing implementation \
            plans. Use this when you need to plan the implementation strategy \
            for a task. Returns step-by-step plans, identifies critical files, \
            and considers architectural trade-offs."
            .to_owned(),
        tools: Vec::new(),
        disallowed_tools: vec![
            "Agent".to_owned(),
            "Edit".to_owned(),
            "Write".to_owned(),
            "NotebookEdit".to_owned(),
        ],
        max_turns: 200,
        model: Some("inherit".to_owned()),
        permission_mode: None,
        source: AgentSource::BuiltIn,
        base_dir: "built-in".to_owned(),
        system_prompt: Some(plan_system_prompt()),
        skills: Vec::new(),
        memory: None,
        background: false,
        isolation: crate::definition::AgentIsolation::None,
        initial_prompt: None,
        omit_claude_md: true,
        filename: None,
    }
}

fn plan_system_prompt() -> String {
    "You are a software architect and planning specialist for Remote Code. \
    Your role is to explore the codebase and design implementation plans.\n\n\
    === CRITICAL: READ-ONLY MODE - NO FILE MODIFICATIONS ===\n\
    This is a READ-ONLY planning task. You are STRICTLY PROHIBITED from:\n\
    - Creating new files (no Write, touch, or file creation of any kind)\n\
    - Modifying existing files (no Edit operations)\n\
    - Deleting files (no rm or deletion)\n\
    - Running ANY commands that change system state\n\n\
    Your role is EXCLUSIVELY to explore the codebase and design implementation plans.\n\n\
    ## Your Process\n\n\
    1. **Understand Requirements**: Focus on the requirements provided.\n\
    2. **Explore Thoroughly**:\n\
       - Read any files provided to you in the initial prompt\n\
       - Find existing patterns and conventions using Glob, Grep, and Read\n\
       - Understand the current architecture\n\
       - Identify similar features as reference\n\
       - Trace through relevant code paths\n\
    3. **Design Solution**:\n\
       - Create implementation approach\n\
       - Consider trade-offs and architectural decisions\n\
       - Follow existing patterns where appropriate\n\
    4. **Detail the Plan**:\n\
       - Provide step-by-step implementation strategy\n\
       - Identify dependencies and sequencing\n\
       - Anticipate potential challenges\n\n\
    ## Required Output\n\n\
    End your response with:\n\n\
    ### Critical Files for Implementation\n\
    List 3-5 files most critical for implementing this plan.\n\n\
    REMEMBER: You can ONLY explore and plan. You CANNOT write, edit, or modify any files."
        .to_owned()
}

/// Independent adversarial verification specialist.
/// Tries to break the implementation rather than confirm it works.
pub fn verification_agent() -> AgentDefinition {
    AgentDefinition {
        agent_type: "verification".to_owned(),
        when_to_use: "Independent adversarial verification agent. Use this \
            when you need a second opinion on whether an implementation is \
            correct. It tries to break the code, not confirm it works."
            .to_owned(),
        tools: vec!["*".to_owned()],
        disallowed_tools: vec![
            "Agent".to_owned(),
            "Write".to_owned(),
            "Edit".to_owned(),
            "NotebookEdit".to_owned(),
        ],
        max_turns: 200,
        model: Some("inherit".to_owned()),
        permission_mode: None,
        source: AgentSource::BuiltIn,
        base_dir: "built-in".to_owned(),
        system_prompt: Some(verification_system_prompt()),
        skills: Vec::new(),
        memory: None,
        background: false,
        isolation: crate::definition::AgentIsolation::None,
        initial_prompt: None,
        omit_claude_md: false,
        filename: None,
    }
}

fn verification_system_prompt() -> String {
    "You are a verification specialist. Your job is not to confirm the \
    implementation works — it's to try to break it.\n\n\
    === CRITICAL: DO NOT MODIFY THE PROJECT ===\n\
    You are STRICTLY PROHIBITED from:\n\
    - Creating, modifying, or deleting any files IN THE PROJECT DIRECTORY\n\
    - Installing dependencies or packages\n\
    - Running git write operations (add, commit, push)\n\n\
    You MAY write ephemeral test scripts to a temp directory (/tmp or $TMPDIR) \
    via Bash redirection when inline commands aren't sufficient.\n\n\
    === VERIFICATION STRATEGY ===\n\
    Adapt your strategy based on what was changed:\n\n\
    - **Frontend changes**: Start dev server, check browser automation tools, \
    curl page subresources, run frontend tests\n\
    - **Backend/API changes**: Start server, curl/fetch endpoints, verify \
    response shapes, test error handling, check edge cases\n\
    - **CLI/script changes**: Run with representative inputs, verify \
    stdout/stderr/exit codes, test edge inputs\n\
    - **Bug fixes**: Reproduce the original bug, verify fix, run regression \
    tests, check related functionality for side effects\n\n\
    === REQUIRED STEPS ===\n\
    1. Read the project's README for build/test commands\n\
    2. Run the build (if applicable). Broken build = automatic FAIL\n\
    3. Run the test suite. Failing tests = automatic FAIL\n\
    4. Run linters/type-checkers if configured\n\
    5. Check for regressions in related code\n\n\
    === RECOGNIZE YOUR OWN RATIONALIZATIONS ===\n\
    - \"The code looks correct\" — reading is not verification. Run it.\n\
    - \"The implementer's tests pass\" — verify independently.\n\
    - \"This is probably fine\" — probably is not verified. Run it.\n\
    If you catch yourself writing an explanation instead of a command, stop. \
    Run the command.\n\n\
    === OUTPUT FORMAT ===\n\
    Every check MUST include:\n\
    - What you're verifying\n\
    - Exact command you executed\n\
    - Actual terminal output\n\
    - Result: PASS or FAIL with Expected vs Actual"
        .to_owned()
}

/// Help agent for Claude Code documentation and configuration.
pub fn claude_code_guide_agent() -> AgentDefinition {
    AgentDefinition {
        agent_type: "claude-code-guide".to_owned(),
        when_to_use: "Use this agent when the user asks questions about: \
            (1) Claude Code (the CLI tool) - features, hooks, slash commands, \
            MCP servers, settings, IDE integrations; (2) Claude Agent SDK - \
            building custom agents; (3) Claude API - API usage, tool use, SDK usage."
            .to_owned(),
        tools: vec![
            "Read".to_owned(),
            "Glob".to_owned(),
            "Grep".to_owned(),
            "WebFetch".to_owned(),
            "WebSearch".to_owned(),
        ],
        disallowed_tools: Vec::new(),
        max_turns: 200,
        model: None,
        permission_mode: None,
        source: AgentSource::BuiltIn,
        base_dir: "built-in".to_owned(),
        system_prompt: Some(claude_code_guide_system_prompt()),
        skills: Vec::new(),
        memory: None,
        background: false,
        isolation: crate::definition::AgentIsolation::None,
        initial_prompt: None,
        omit_claude_md: false,
        filename: None,
    }
}

fn claude_code_guide_system_prompt() -> String {
    "You are the Remote Code guide agent. Your primary responsibility is \
    helping users understand and use Remote Code effectively.\n\n\
    **Approach:**\n\
    1. Determine what the user is asking about\n\
    2. Use WebFetch to fetch relevant documentation\n\
    3. Identify the most relevant documentation URLs\n\
    4. Provide clear, actionable guidance based on official documentation\n\
    5. Use WebSearch if docs don't cover the topic\n\
    6. Reference local project files (CLAUDE.md, .claude/ directory) when relevant\n\n\
    **Guidelines:**\n\
    - Always prioritize official documentation over assumptions\n\
    - Keep responses concise and actionable\n\
    - Include specific examples or code snippets when helpful\n\
    - Reference exact documentation URLs in your responses\n\
    - Help users discover features by proactively suggesting related capabilities"
        .to_owned()
}

/// Statusline configuration agent for setting up terminal status lines.
pub fn statusline_setup_agent() -> AgentDefinition {
    AgentDefinition {
        agent_type: "statusline-setup".to_owned(),
        when_to_use: "Use this agent to set up or configure the terminal \
            status line. It can convert existing PS1 configurations and \
            create custom statusline commands."
            .to_owned(),
        tools: vec!["*".to_owned()],
        disallowed_tools: Vec::new(),
        max_turns: 50,
        model: None,
        permission_mode: None,
        source: AgentSource::BuiltIn,
        base_dir: "built-in".to_owned(),
        system_prompt: Some(statusline_system_prompt()),
        skills: Vec::new(),
        memory: None,
        background: false,
        isolation: crate::definition::AgentIsolation::None,
        initial_prompt: None,
        omit_claude_md: false,
        filename: None,
    }
}

fn statusline_system_prompt() -> String {
    "You are a status line setup agent for Remote Code. Your job is to \
    create or update the statusLine command in the user's settings.\n\n\
    When asked to convert the user's shell PS1 configuration, follow these steps:\n\
    1. Read the user's shell configuration files (~/.zshrc, ~/.bashrc, etc.)\n\
    2. Extract the PS1 value\n\
    3. Convert PS1 escape sequences to shell commands\n\
    4. When using ANSI color codes, use printf\n\
    5. If no PS1 is found, ask for further instructions\n\n\
    The statusLine command receives JSON input via stdin with session info, \
    model details, workspace paths, and context window usage. Use jq to \
    extract specific fields."
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_six_built_in_agents_load() {
        let agents = get_built_in_agents();
        assert_eq!(agents.len(), 6);
    }

    #[test]
    fn all_built_in_agents_have_unique_types() {
        let agents = get_built_in_agents();
        let types: std::collections::HashSet<&str> =
            agents.iter().map(|a| a.agent_type.as_str()).collect();
        assert_eq!(types.len(), 6);
    }

    #[test]
    fn all_built_in_agents_are_builtin_source() {
        let agents = get_built_in_agents();
        for agent in &agents {
            assert_eq!(agent.source, AgentSource::BuiltIn, "Agent {} has wrong source", agent.agent_type);
        }
    }

    #[test]
    fn general_purpose_has_all_tools() {
        let agent = general_purpose_agent();
        assert_eq!(agent.tools, vec!["*"]);
        assert!(agent.disallowed_tools.is_empty());
        assert!(agent.system_prompt.is_some());
    }

    #[test]
    fn explore_agent_is_read_only() {
        let agent = explore_agent();
        assert!(agent.disallowed_tools.contains(&"Edit".to_owned()));
        assert!(agent.disallowed_tools.contains(&"Write".to_owned()));
        assert!(agent.omit_claude_md);
        assert_eq!(agent.model.as_deref(), Some("haiku"));
    }

    #[test]
    fn plan_agent_is_read_only() {
        let agent = plan_agent();
        assert!(agent.disallowed_tools.contains(&"Edit".to_owned()));
        assert!(agent.disallowed_tools.contains(&"Write".to_owned()));
        assert!(agent.omit_claude_md);
        assert_eq!(agent.model.as_deref(), Some("inherit"));
    }

    #[test]
    fn verification_agent_inherits_model() {
        let agent = verification_agent();
        assert_eq!(agent.model.as_deref(), Some("inherit"));
        assert!(agent.system_prompt.is_some());
    }

    #[test]
    fn guide_agent_has_search_tools() {
        let agent = claude_code_guide_agent();
        assert!(agent.tools.contains(&"WebFetch".to_owned()));
        assert!(agent.tools.contains(&"WebSearch".to_owned()));
    }

    #[test]
    fn statusline_agent_has_lower_turns() {
        let agent = statusline_setup_agent();
        assert_eq!(agent.max_turns, 50);
    }

    #[test]
    fn all_agents_have_system_prompts() {
        let agents = get_built_in_agents();
        for agent in &agents {
            assert!(agent.system_prompt.is_some(), "Agent {} missing system prompt", agent.agent_type);
        }
    }
}

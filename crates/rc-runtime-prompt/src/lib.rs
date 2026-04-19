use std::collections::HashSet;
use std::fs;
use std::path::Path;

use anyhow::Result;
use chrono::Local;
use rc_agents::coordinator::{
    COORDINATOR_MODE_ALLOWED_TOOLS, get_coordinator_system_prompt, is_coordinator_mode,
};
use rc_config::RuntimeConfig;
use rc_core::{ConversationEntry, ConversationRole, ProviderProtocol};
use rc_model::is_first_party_base_url;
use rc_provider::{DiscoveredToolScope, provider_runtime_tool_specs_for_request};
use rc_system_prompt::{
    EffectiveSystemPromptOptions, McpClientInfo as PromptMcpClientInfo,
    OutputStyleConfig as PromptOutputStyleConfig, PromptContext, PromptFeatures,
    SystemPromptBuilder, SystemPromptSplitOptions, build_effective_system_prompt,
    render_system_prompt_for_api,
};
use rc_tools::{ToolSpec, is_runtime_dynamic_mcp_tool_name, runtime_provider_tool_specs};

const MEMORY_INSTRUCTION_PROMPT: &str = "Codebase and user instructions are shown below. Be sure to adhere to these instructions. IMPORTANT: These instructions OVERRIDE any default behavior and you MUST follow them exactly as written.";
const MAX_GIT_STATUS_CHARS: usize = 2000;

#[derive(Debug, Clone, Default)]
pub struct PromptRuntimeOverrides {
    pub system_prompt: Option<String>,
    pub agent_system_prompt: Option<String>,
    pub allowed_tools: Option<Vec<String>>,
    pub critical_system_reminder: Option<String>,
    pub omit_claude_md: bool,
    pub omit_git_status: bool,
}

#[must_use]
pub fn effective_allowed_tool_names(
    overrides: &PromptRuntimeOverrides,
    tool_specs: &[ToolSpec],
) -> Option<HashSet<String>> {
    let mut allowed = overrides
        .allowed_tools
        .as_ref()
        .map(|requested| expand_requested_tool_names(requested, tool_specs));

    if is_coordinator_mode() && overrides.agent_system_prompt.is_none() {
        let coordinator_requested = COORDINATOR_MODE_ALLOWED_TOOLS
            .iter()
            .map(|tool| (*tool).to_owned())
            .collect::<Vec<_>>();
        let mut coordinator_allowed =
            expand_requested_tool_names(&coordinator_requested, tool_specs);
        for spec in tool_specs {
            if is_coordinator_pr_subscription_tool(spec) {
                insert_prompt_tool_aliases(spec, &mut coordinator_allowed);
            }
        }
        allowed = Some(match allowed {
            Some(existing) => existing
                .intersection(&coordinator_allowed)
                .cloned()
                .collect(),
            None => coordinator_allowed,
        });
    }

    allowed
}

fn is_coordinator_pr_subscription_tool(spec: &ToolSpec) -> bool {
    [
        spec.name.as_str(),
        spec.protocol_name.as_str(),
        spec.permission_tool_name.as_str(),
    ]
    .iter()
    .any(|name| {
        name.ends_with("subscribe_pr_activity") || name.ends_with("unsubscribe_pr_activity")
    })
}

#[derive(Debug, Clone)]
pub struct RuntimePromptSettings {
    pub language: Option<String>,
    pub output_style: Option<String>,
    pub proactive_active: bool,
    pub brief_enabled: bool,
    pub mcp_instructions_delta_enabled: bool,
    pub is_non_interactive: bool,
    pub user_invocable_skills_available: bool,
    pub include_token_budget_prompt: bool,
}

impl RuntimePromptSettings {
    #[must_use]
    pub fn from_config(config: &RuntimeConfig) -> Self {
        Self {
            language: config.language.clone(),
            output_style: config.output_style.clone(),
            proactive_active: config.proactive_active,
            brief_enabled: config.brief_enabled,
            mcp_instructions_delta_enabled: true,
            is_non_interactive: false,
            user_invocable_skills_available: discover_user_invocable_skills(config),
            include_token_budget_prompt: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RuntimeSystemPrompt {
    pub text: String,
    pub content_blocks: Vec<serde_json::Value>,
}

pub fn runtime_env_truthy(name: &str) -> bool {
    std::env::var(name).is_ok_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

pub fn runtime_env_defined_falsy(name: &str) -> bool {
    std::env::var(name).is_ok_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "" | "0" | "false" | "no" | "off"
        )
    })
}

#[must_use]
pub fn runtime_deferred_tools_delta_enabled() -> bool {
    if runtime_env_truthy("CLAUDE_CODE_DEFERRED_TOOLS_DELTA") {
        return true;
    }
    if runtime_env_defined_falsy("CLAUDE_CODE_DEFERRED_TOOLS_DELTA") {
        return false;
    }
    true
}

#[must_use]
pub fn runtime_mcp_instructions_delta_enabled() -> bool {
    if runtime_env_truthy("CLAUDE_CODE_MCP_INSTR_DELTA") {
        return true;
    }
    if runtime_env_defined_falsy("CLAUDE_CODE_MCP_INSTR_DELTA") {
        return false;
    }
    true
}

pub fn insert_prompt_tool_aliases(spec: &ToolSpec, enabled_tools: &mut HashSet<String>) {
    enabled_tools.insert(spec.name.clone());
    enabled_tools.insert(spec.protocol_name.clone());
    match spec.name.as_str() {
        "read_file" => {
            enabled_tools.insert("Read".to_owned());
        }
        "write_file" => {
            enabled_tools.insert("Write".to_owned());
        }
        "edit_file" | "replace_in_file" => {
            enabled_tools.insert("Edit".to_owned());
        }
        "bash_command" => {
            enabled_tools.insert("Bash".to_owned());
        }
        "glob" => {
            enabled_tools.insert("Glob".to_owned());
        }
        "grep" => {
            enabled_tools.insert("Grep".to_owned());
        }
        "ask_user" => {
            enabled_tools.insert("AskUserQuestion".to_owned());
        }
        "agent" => {
            enabled_tools.insert("Agent".to_owned());
        }
        "task_create" => {
            enabled_tools.insert("Task".to_owned());
            enabled_tools.insert("TaskCreate".to_owned());
        }
        "todo_write" => {
            enabled_tools.insert("TodoWrite".to_owned());
        }
        "send_message" => {
            enabled_tools.insert("SendMessage".to_owned());
        }
        "skill_execute" | "discover_skills" => {
            enabled_tools.insert("Skill".to_owned());
        }
        "sleep" => {
            enabled_tools.insert("Sleep".to_owned());
        }
        _ => {}
    }
}

#[must_use]
pub fn expand_requested_tool_names(
    requested_tools: &[String],
    tool_specs: &[ToolSpec],
) -> HashSet<String> {
    if requested_tools.is_empty() {
        return HashSet::new();
    }

    let requested = requested_tools
        .iter()
        .map(|tool| tool.as_str())
        .collect::<HashSet<_>>();
    let mut expanded = requested_tools.iter().cloned().collect::<HashSet<_>>();

    for spec in tool_specs {
        let mut aliases = HashSet::new();
        insert_prompt_tool_aliases(spec, &mut aliases);
        if aliases
            .iter()
            .any(|alias| requested.contains(alias.as_str()))
        {
            expanded.extend(aliases);
            expanded.insert(spec.name.clone());
            expanded.insert(spec.protocol_name.clone());
        }
    }

    expanded
}

#[must_use]
pub fn conversation_with_runtime_user_context(
    config: &RuntimeConfig,
    conversation: &[ConversationEntry],
    overrides: &PromptRuntimeOverrides,
) -> Vec<ConversationEntry> {
    let context_entries = runtime_user_context_entries(config, overrides);
    if context_entries.is_empty()
        || conversation.iter().any(|entry| {
            entry.role == ConversationRole::User
                && entry.text.contains(
                    "As you answer the user's questions, you can use the following context:",
                )
        })
    {
        return conversation.to_vec();
    }

    let body = context_entries
        .into_iter()
        .map(|(key, value)| format!("# {key}\n{value}"))
        .collect::<Vec<_>>()
        .join("\n");
    let reminder = ConversationEntry::user(format!(
        "<system-reminder>\nAs you answer the user's questions, you can use the following context:\n{body}\n\n      IMPORTANT: this context may or may not be relevant to your tasks. You should not respond to this context unless it is highly relevant to your task.\n</system-reminder>\n"
    ));

    let mut augmented = Vec::with_capacity(conversation.len() + 1);
    if let Some((first, rest)) = conversation.split_first()
        && first.role == ConversationRole::System
    {
        augmented.push(first.clone());
        augmented.push(reminder);
        augmented.extend(rest.iter().cloned());
        return augmented;
    }
    augmented.push(reminder);
    augmented.extend(conversation.iter().cloned());
    augmented
}

pub async fn build_runtime_system_prompt(
    config: &RuntimeConfig,
    conversation: &[ConversationEntry],
    overrides: &PromptRuntimeOverrides,
    settings: &RuntimePromptSettings,
    discovered_tool_scope: &DiscoveredToolScope,
) -> Result<RuntimeSystemPrompt> {
    let custom_system_prompt_provided = overrides.system_prompt.is_some();
    let agent_system_prompt_provided = overrides.agent_system_prompt.is_some();
    let session_start_date = Local::now().format("%Y-%m-%d").to_string();
    if runtime_env_truthy("CLAUDE_CODE_SIMPLE") {
        let default_prompt_blocks = if custom_system_prompt_provided
            || (agent_system_prompt_provided && !settings.proactive_active)
        {
            Vec::new()
        } else {
            vec![format!(
                "You are Claude Code, Anthropic's official CLI for Claude.\n\nCWD: {}\nDate: {}",
                config.cwd.display(),
                session_start_date
            )]
        };
        let mut prompt_blocks = build_effective_system_prompt(
            default_prompt_blocks,
            &EffectiveSystemPromptOptions {
                agent_system_prompt: overrides.agent_system_prompt.clone(),
                coordinator_system_prompt: is_coordinator_mode()
                    .then(|| get_coordinator_system_prompt(true)),
                custom_system_prompt: overrides.system_prompt.clone(),
                proactive_active: settings.proactive_active,
                ..Default::default()
            },
        );
        if !custom_system_prompt_provided
            && let Some(system_context) = runtime_system_context_block(config, overrides)
        {
            prompt_blocks.push(system_context);
        }
        let rendered = render_system_prompt_for_api(
            &prompt_blocks,
            &SystemPromptSplitOptions {
                skip_global_cache_for_system_prompt: true,
            },
        );
        return Ok(RuntimeSystemPrompt {
            text: rendered.text,
            content_blocks: rendered.content_blocks,
        });
    }

    let tool_specs = runtime_provider_tool_specs().await;
    let carried_discovered_tools = discovered_tool_scope.snapshot();
    let visible_tool_specs = provider_runtime_tool_specs_for_request(
        &config.provider,
        conversation,
        &carried_discovered_tools,
    )
    .await;
    let mcp_catalog = rc_tools::mcp_catalog::runtime_mcp_catalog().await;
    let mut enabled_tools = HashSet::new();
    for spec in &visible_tool_specs {
        insert_prompt_tool_aliases(spec, &mut enabled_tools);
    }
    if let Some(allowed) = effective_allowed_tool_names(overrides, &tool_specs) {
        enabled_tools.retain(|tool| allowed.contains(tool.as_str()));
    }
    let enabled_tool_names = enabled_tools.clone();
    let use_global_prompt_cache = should_use_global_prompt_cache_scope(config);

    let prompt_ctx = PromptContext {
        model: config
            .provider
            .model
            .clone()
            .unwrap_or_else(|| "unknown".to_owned()),
        cwd: config.cwd.clone(),
        is_git: detect_git_repository(&config.cwd),
        platform: std::env::consts::OS.to_owned(),
        shell: detect_shell_name(),
        os_version: detect_os_version(),
        enabled_tools,
        language: settings.language.clone(),
        output_style: runtime_output_style_config(settings.output_style.as_deref()),
        mcp_clients: mcp_catalog
            .clients
            .into_iter()
            .map(|client| PromptMcpClientInfo {
                name: client.server_name,
                instructions: client.instructions,
            })
            .collect(),
        mcp_instructions_delta_enabled: settings.mcp_instructions_delta_enabled,
        is_worktree: detect_git_worktree(&config.cwd),
        additional_dirs: Vec::new(),
        is_non_interactive: settings.is_non_interactive,
        is_fork_subagent_enabled: false,
        session_start_date,
        features: PromptFeatures {
            ant_user: runtime_is_ant_user(),
            proactive_active: settings.proactive_active,
            brief_enabled: settings.brief_enabled,
            repl_mode_active: false,
            embedded_search_tools: false,
            user_invocable_skills_available: settings.user_invocable_skills_available,
            explore_plan_agents_enabled: true,
            verification_agent_enabled: false,
            memory_prompt: None,
            scratchpad_dir: None,
            function_result_keep_recent: None,
            include_token_budget_prompt: settings.include_token_budget_prompt,
        },
    };

    let mut builder = SystemPromptBuilder::with_default_sections();
    builder.set_global_cache_scope(use_global_prompt_cache);
    let default_prompt_blocks = if custom_system_prompt_provided
        || (agent_system_prompt_provided && !prompt_ctx.features.proactive_active)
    {
        Vec::new()
    } else {
        builder.build(&prompt_ctx)?
    };
    let mut prompt_blocks = build_effective_system_prompt(
        default_prompt_blocks,
        &EffectiveSystemPromptOptions {
            agent_system_prompt: overrides.agent_system_prompt.clone(),
            coordinator_system_prompt: is_coordinator_mode()
                .then(|| get_coordinator_system_prompt(false)),
            custom_system_prompt: overrides.system_prompt.clone(),
            proactive_active: prompt_ctx.features.proactive_active,
            ..Default::default()
        },
    );
    if !custom_system_prompt_provided
        && let Some(system_context) = runtime_system_context_block(config, overrides)
    {
        prompt_blocks.push(system_context);
    }

    let skip_global_cache_for_system_prompt = use_global_prompt_cache
        && visible_tool_specs
            .iter()
            .filter(|spec| enabled_tool_names.contains(spec.name.as_str()))
            .any(|spec| is_runtime_dynamic_mcp_tool_name(&spec.name));
    let rendered = render_system_prompt_for_api(
        &prompt_blocks,
        &SystemPromptSplitOptions {
            skip_global_cache_for_system_prompt,
        },
    );

    Ok(RuntimeSystemPrompt {
        text: rendered.text,
        content_blocks: rendered.content_blocks,
    })
}

pub async fn refresh_runtime_system_prompt(
    config: &RuntimeConfig,
    conversation: &mut Vec<ConversationEntry>,
    overrides: &PromptRuntimeOverrides,
    settings: &RuntimePromptSettings,
    discovered_tool_scope: &DiscoveredToolScope,
) -> Result<()> {
    let prompt = build_runtime_system_prompt(
        config,
        conversation,
        overrides,
        settings,
        discovered_tool_scope,
    )
    .await?;
    apply_runtime_system_prompt(conversation, prompt);
    Ok(())
}

pub fn apply_runtime_system_prompt(
    conversation: &mut Vec<ConversationEntry>,
    prompt: RuntimeSystemPrompt,
) {
    if let Some(system_entry) = conversation
        .iter_mut()
        .find(|entry| matches!(entry.role, ConversationRole::System))
    {
        system_entry.text = prompt.text;
        system_entry.history_text = None;
        system_entry.content_blocks = prompt.content_blocks;
        return;
    }
    conversation.insert(
        0,
        ConversationEntry {
            role: ConversationRole::System,
            text: prompt.text,
            history_text: None,
            content_blocks: prompt.content_blocks,
            tool_calls: Vec::new(),
            attachments: Vec::new(),
            tool_call_id: None,
            name: None,
            is_error: false,
        },
    );
}

fn detect_shell_name() -> String {
    if cfg!(windows) {
        return "powershell".to_owned();
    }
    std::env::var("SHELL").unwrap_or_else(|_| "bash".to_owned())
}

fn detect_os_version() -> String {
    std::env::var("OS").unwrap_or_else(|_| std::env::consts::OS.to_owned())
}

fn detect_git_repository(cwd: &Path) -> bool {
    if cwd.join(".git").exists() {
        return true;
    }
    std::process::Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(cwd)
        .output()
        .ok()
        .is_some_and(|output| output.status.success())
}

fn git_output(cwd: &Path, args: &[&str]) -> Option<String> {
    std::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|output| !output.is_empty())
}

fn initial_git_status_context(cwd: &Path) -> Option<String> {
    if !detect_git_repository(cwd) {
        return None;
    }

    let branch = git_output(cwd, &["rev-parse", "--abbrev-ref", "HEAD"])
        .unwrap_or_else(|| "HEAD".to_owned());
    let main_branch = git_output(
        cwd,
        &["symbolic-ref", "refs/remotes/origin/HEAD", "--short"],
    )
    .map(|value| {
        value
            .strip_prefix("origin/")
            .map(str::to_owned)
            .unwrap_or(value)
    })
    .unwrap_or_else(|| "main".to_owned());
    let status = git_output(cwd, &["--no-optional-locks", "status", "--short"]).unwrap_or_default();
    let log = git_output(cwd, &["--no-optional-locks", "log", "--oneline", "-n", "5"])
        .unwrap_or_default();
    let user_name = git_output(cwd, &["config", "user.name"]);
    let truncated_status = if status.chars().count() > MAX_GIT_STATUS_CHARS {
        let prefix = status
            .chars()
            .take(MAX_GIT_STATUS_CHARS)
            .collect::<String>();
        format!(
            "{prefix}\n... (truncated because it exceeds 2k characters. If you need more information, run \"git status\" using BashTool)"
        )
    } else {
        status
    };

    let mut parts = vec![
        "This is the git status at the start of the conversation. Note that this status is a snapshot in time, and will not update during the conversation.".to_owned(),
        format!("Current branch: {branch}"),
        format!("Main branch (you will usually use this for PRs): {main_branch}"),
    ];
    if let Some(user_name) = user_name {
        parts.push(format!("Git user: {user_name}"));
    }
    parts.push(format!(
        "Status:\n{}",
        if truncated_status.is_empty() {
            "(clean)"
        } else {
            truncated_status.as_str()
        }
    ));
    parts.push(format!("Recent commits:\n{log}"));

    Some(parts.join("\n\n"))
}

fn runtime_system_context_block(
    config: &RuntimeConfig,
    overrides: &PromptRuntimeOverrides,
) -> Option<String> {
    if overrides.omit_git_status {
        return None;
    }
    initial_git_status_context(&config.cwd).map(|git_status| format!("gitStatus: {git_status}"))
}

fn collect_claude_md_context(cwd: &Path) -> Option<String> {
    let mut dirs = cwd.ancestors().collect::<Vec<_>>();
    dirs.reverse();
    let mut memories = Vec::new();

    for dir in dirs {
        for path in [dir.join("CLAUDE.md"), dir.join(".claude").join("CLAUDE.md")] {
            let Ok(content) = fs::read_to_string(&path) else {
                continue;
            };
            let content = content.trim();
            if content.is_empty() {
                continue;
            }
            memories.push(format!(
                "Contents of {} (project instructions, checked into the codebase):\n\n{}",
                path.display(),
                content
            ));
        }
    }

    if memories.is_empty() {
        None
    } else {
        Some(format!(
            "{MEMORY_INSTRUCTION_PROMPT}\n\n{}",
            memories.join("\n\n")
        ))
    }
}

fn runtime_user_context_entries(
    config: &RuntimeConfig,
    overrides: &PromptRuntimeOverrides,
) -> Vec<(&'static str, String)> {
    let mut entries = Vec::new();
    if !overrides.omit_claude_md
        && let Some(claude_md) = collect_claude_md_context(&config.cwd)
    {
        entries.push(("claudeMd", claude_md));
    }
    entries.push((
        "currentDate",
        format!("Today's date is {}.", Local::now().format("%Y-%m-%d")),
    ));
    entries
}

fn detect_git_worktree(cwd: &Path) -> bool {
    cwd.join(".git").is_file()
}

fn runtime_is_ant_user() -> bool {
    std::env::var("USER_TYPE")
        .ok()
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("ant"))
}

fn discover_user_invocable_skills(config: &RuntimeConfig) -> bool {
    rc_skills::discover_skills(&config.paths.skills_dir)
        .map(|skills| !skills.is_empty())
        .unwrap_or(false)
}

fn explanatory_feature_prompt() -> String {
    "\n## Insights\nIn order to encourage learning, before and after writing code, always provide brief educational explanations about implementation choices using (with backticks):\n\"`★ Insight ─────────────────────────────────────`\n[2-3 key educational points]\n`─────────────────────────────────────────────────`\"\n\nThese insights should be included in the conversation, not in the codebase. You should generally focus on interesting insights that are specific to the codebase or the code you just wrote, rather than general programming concepts.".to_owned()
}

fn runtime_output_style_config(style_name: Option<&str>) -> Option<PromptOutputStyleConfig> {
    match style_name?.trim() {
        "" | "default" => None,
        "Explanatory" => {
            let feature_prompt = explanatory_feature_prompt();
            Some(PromptOutputStyleConfig {
                name: "Explanatory".to_owned(),
                prompt: format!(
                    "You are an interactive CLI tool that helps users with software engineering tasks. In addition to software engineering tasks, you should provide educational insights about the codebase along the way.\n\nYou should be clear and educational, providing helpful explanations while remaining focused on the task. Balance educational content with task completion. When providing insights, you may exceed typical length constraints, but remain focused and relevant.\n\n# Explanatory Style Active\n{feature_prompt}"
                ),
                keep_coding_instructions: true,
            })
        }
        "Learning" => {
            let feature_prompt = explanatory_feature_prompt();
            Some(PromptOutputStyleConfig {
                name: "Learning".to_owned(),
                prompt: format!(
                    "You are an interactive CLI tool that helps users with software engineering tasks. In addition to software engineering tasks, you should help users learn more about the codebase through hands-on practice and educational insights.\n\nYou should be collaborative and encouraging. Balance task completion with learning by requesting user input for meaningful design decisions while handling routine implementation yourself.\n\n# Learning Style Active\n{feature_prompt}"
                ),
                keep_coding_instructions: true,
            })
        }
        _ => None,
    }
}

fn should_use_global_prompt_cache_scope(config: &RuntimeConfig) -> bool {
    matches!(config.provider.protocol, ProviderProtocol::Anthropic)
        && config
            .provider
            .base_url
            .as_deref()
            .is_some_and(is_first_party_base_url)
}

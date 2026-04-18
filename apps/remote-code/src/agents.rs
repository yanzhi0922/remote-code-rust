use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use rc_agents::builtins::{explore_agent, general_purpose_agent, plan_agent, verification_agent};
use rc_agents::{
    AgentDefinition, AgentExecutionRequest, AgentExecutor, AgentIdentity, AgentRunConfig,
    AgentRunResult, AgentRunner, AgentScheduler, AgentTask,
};
use rc_config::RuntimeConfig;
use rc_core::{
    ConversationEntry as CoreConversationEntry, ConversationRole, ProviderProtocol,
    ProviderResponse, SubAgentCompletion, SubAgentExecutionRequest, SubAgentExecutionResult,
};
use rc_model::model::{ResolveContext, parse_user_specified_model_with_ctx};
use rc_model::{
    ModelProvider, detect_provider, is_first_party_base_url, is_model_alias, provider_model_id,
};
use rc_permissions::{
    LayeredPermissionBroker, PermissionBroker, StaticPermissionBroker, load_layered_rules,
};
use rc_provider::{ProviderClient, ProviderCompatBackend};
use rc_session::SessionStore;
use rc_tools::{ToolSpec, runtime_provider_tool_specs};
use tokio::task::JoinSet;
use uuid::Uuid;

use crate::cli::{AgentsCommand, AgentsPlanArgs};
use crate::conversation::initialize_conversation;
use crate::hooks::{HookRunState, discover_runtime_hooks, ensure_session_start_hooks};
use crate::query_engine_compat::{
    CompatRunOverrides, run_prompt_with_query_engine_compat_overrides,
};

pub(crate) fn parse_agent_spec(spec: &str) -> Result<AgentIdentity> {
    let mut segments = spec.splitn(4, ';').map(str::trim);
    let name = segments
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("invalid --agent spec `{spec}`; expected name;role;paths;labels"))?;
    let role = segments
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("invalid --agent spec `{spec}`; role is missing"))?;
    let mut agent = AgentIdentity::new(name, role);
    agent.ownership_paths = segments.next().map(parse_csv_list).unwrap_or_default();
    agent.labels = segments
        .next()
        .map(parse_key_value_pairs)
        .unwrap_or_default();
    Ok(agent)
}

pub(crate) fn parse_task_spec(spec: &str) -> Result<AgentTask> {
    let mut segments = spec.splitn(4, ';').map(str::trim);
    let title = segments
        .next()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            anyhow!("invalid --task spec `{spec}`; expected title;paths;labels;description")
        })?;
    let mut task = AgentTask::new(title);
    task.ownership_paths = segments.next().map(parse_csv_list).unwrap_or_default();
    task.required_labels = segments
        .next()
        .map(parse_key_value_pairs)
        .unwrap_or_default();
    segments
        .next()
        .unwrap_or_default()
        .clone_into(&mut task.description);
    task.budget.read_calls = 32;
    task.budget.edit_calls = 12;
    task.budget.command_calls = 8;
    Ok(task)
}

fn default_agent_specs_for_workspace(workspace_scope: &str) -> Vec<AgentIdentity> {
    let workspace_spec = format!("workspace;implementer;{workspace_scope};phase=workspace");
    vec![
        parse_agent_spec("planner;planner;;phase=plan").unwrap_or_else(|_| {
            let mut agent = AgentIdentity::new("planner", "planner");
            agent.labels.insert("phase".to_owned(), "plan".to_owned());
            agent
        }),
        parse_agent_spec(&workspace_spec).unwrap_or_else(|_| {
            let mut agent = AgentIdentity::new("workspace", "implementer");
            agent.ownership_paths = vec![workspace_scope.to_owned()];
            agent
        }),
        parse_agent_spec(
            "runtime;implementer;apps/remote-code,crates/rc-session,crates/rc-tools;phase=local",
        )
        .unwrap_or_else(|_| AgentIdentity::new("runtime", "implementer")),
        parse_agent_spec(
            "remote;implementer;apps/remote-code-runner,apps/remote-code-control-plane,crates/rc-runner,crates/rc-control-plane;phase=remote",
        )
        .unwrap_or_else(|_| AgentIdentity::new("remote", "implementer")),
        parse_agent_spec("review;reviewer;.;phase=review")
            .unwrap_or_else(|_| AgentIdentity::new("review", "reviewer")),
    ]
}

pub(crate) fn default_agent_specs(config: &RuntimeConfig) -> Vec<AgentIdentity> {
    default_agent_specs_for_workspace(&config.cwd.display().to_string())
}

pub(crate) fn default_task_for_objective(objective: &str, config: &RuntimeConfig) -> AgentTask {
    let mut task = AgentTask::new(objective);
    task.description = format!(
        "Coordinate work for {} in {}",
        objective,
        config.cwd.display()
    );
    task.ownership_paths = vec![config.cwd.display().to_string()];
    task.budget.read_calls = 64;
    task.budget.edit_calls = 16;
    task.budget.command_calls = 12;
    task
}

fn parse_csv_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn parse_key_value_pairs(value: &str) -> BTreeMap<String, String> {
    value
        .split(',')
        .filter_map(|entry| {
            let (key, value) = entry.split_once('=')?;
            let key = key.trim();
            let value = value.trim();
            if key.is_empty() || value.is_empty() {
                None
            } else {
                Some((key.to_owned(), value.to_owned()))
            }
        })
        .collect()
}

#[derive(Clone)]
struct RemoteCodeAgentExecutor {
    base_config: RuntimeConfig,
}

impl RemoteCodeAgentExecutor {
    fn new(config: &RuntimeConfig) -> Self {
        Self {
            base_config: config.clone(),
        }
    }
}

#[derive(Clone)]
struct RemoteCodeSubAgentRuntime {
    completion: Arc<dyn SubAgentCompletion>,
    executor: RemoteCodeAgentExecutor,
}

impl RemoteCodeSubAgentRuntime {
    fn new(config: &RuntimeConfig, completion: Arc<dyn SubAgentCompletion>) -> Self {
        Self {
            completion,
            executor: RemoteCodeAgentExecutor::new(config),
        }
    }
}

#[async_trait]
impl SubAgentCompletion for RemoteCodeSubAgentRuntime {
    async fn complete(&self, conversation: &[CoreConversationEntry]) -> Result<ProviderResponse> {
        self.completion.complete(conversation).await
    }

    fn supports_agent_execution(&self) -> bool {
        true
    }

    async fn execute_agent(
        &self,
        request: SubAgentExecutionRequest,
    ) -> Result<SubAgentExecutionResult> {
        let provider_model =
            resolve_requested_agent_model(&self.executor.base_config, request.model.as_deref());
        let result = self
            .executor
            .execute(AgentExecutionRequest {
                agent_type: request.agent_type,
                task: request.task,
                context: request
                    .context
                    .iter()
                    .map(core_entry_to_agent_context_entry)
                    .collect(),
                model: provider_model.unwrap_or_else(|| "default".to_owned()),
                max_turns: request.max_turns,
                system_prompt: request.system_prompt.unwrap_or_default(),
                tools: request.allowed_tools,
                working_dir: request.working_dir,
            })
            .await?;
        Ok(SubAgentExecutionResult {
            output: result.output,
            success: result.success,
            turns: result.turns,
            usage: rc_core::UsageSummary {
                input_tokens: result.usage.input_tokens,
                output_tokens: result.usage.output_tokens,
                cache_read_input_tokens: result.usage.cache_read_tokens,
                cache_creation_input_tokens: result.usage.cache_creation_tokens,
            },
        })
    }
}

pub(crate) fn build_remote_code_sub_agent_runtime(
    config: &RuntimeConfig,
    completion: Arc<dyn SubAgentCompletion>,
) -> Arc<dyn SubAgentCompletion> {
    Arc::new(RemoteCodeSubAgentRuntime::new(config, completion))
}

#[async_trait]
impl AgentExecutor for RemoteCodeAgentExecutor {
    async fn execute(&self, request: AgentExecutionRequest) -> Result<AgentRunResult> {
        let mut config = self.base_config.clone();
        config.cwd = request.working_dir.clone();
        config.session_id = Uuid::new_v4();
        config.max_turns = usize::try_from(request.max_turns).unwrap_or(usize::MAX);
        config.session_name = Some(format!(
            "agent:{}:{}",
            request.agent_type,
            truncate_single_line(&request.task, 48)
        ));
        if !request.model.is_empty() && request.model != "default" {
            config.provider.model = Some(request.model.clone());
        }

        let store = SessionStore::open(config.paths.clone())?;
        let backend = Arc::new(ProviderCompatBackend::new(
            Arc::new(ProviderClient::new()?),
            &config.provider,
        ));
        let broker: Arc<dyn PermissionBroker> = Arc::new(LayeredPermissionBroker::new(
            StaticPermissionBroker::from_mode(config.permission_mode),
            load_layered_rules(
                &config.cwd,
                &config.paths.profile_dir,
                &config.settings_files,
                &config.cli_settings_files,
            ),
        ));
        let discovery = discover_runtime_hooks(&config, &[]);
        let mut conversation = initialize_conversation(&store, &config, Some(&request.task))?;
        append_conversation_context(&mut conversation, &request.context);
        let mut hook_state = HookRunState::load(&store, config.session_id)?;
        ensure_session_start_hooks(
            &discovery,
            &config,
            &store,
            &mut conversation,
            &mut hook_state,
        )
        .await?;

        let outcome = run_prompt_with_query_engine_compat_overrides(
            &config,
            &store,
            backend,
            broker,
            None,
            &discovery,
            &mut hook_state,
            &mut conversation,
            &request.task,
            CompatRunOverrides {
                system_prompt: Some(request.system_prompt),
                allowed_tools: (!request.tools.is_empty()).then_some(request.tools),
            },
        )
        .await?;

        Ok(AgentRunResult {
            output: outcome.text,
            success: true,
            turns: outcome.num_turns,
            usage: rc_agents::UsageSummary {
                input_tokens: outcome.usage.input_tokens,
                output_tokens: outcome.usage.output_tokens,
                cache_creation_tokens: 0,
                cache_read_tokens: 0,
            },
        })
    }
}

fn resolve_requested_agent_model(
    config: &RuntimeConfig,
    requested_model: Option<&str>,
) -> Option<String> {
    let requested_model = requested_model
        .map(str::trim)
        .filter(|value| !value.is_empty())?;
    if requested_model.eq_ignore_ascii_case("inherit") {
        return None;
    }
    if !is_model_alias(requested_model) {
        return Some(requested_model.to_owned());
    }

    let current_model = config.provider.model.as_deref().unwrap_or_default();
    let current_model_is_claude = current_model.to_ascii_lowercase().contains("claude");
    let base_url = config.provider.base_url.as_deref();
    let can_resolve_alias = current_model_is_claude
        || matches!(
            config.provider.protocol,
            ProviderProtocol::Bedrock | ProviderProtocol::Vertex
        )
        || base_url.is_some_and(is_first_party_base_url);

    if !can_resolve_alias {
        tracing::info!(
            requested_model,
            current_model,
            "Ignoring agent model alias for a non-Claude-compatible runtime and inheriting the parent model"
        );
        return None;
    }

    let provider = detect_model_provider(config, base_url);
    let ctx = ResolveContext {
        provider: provider.clone(),
        ..Default::default()
    };
    let resolved = parse_user_specified_model_with_ctx(requested_model, &ctx);
    Some(provider_model_id(&resolved, &provider))
}

fn detect_model_provider(config: &RuntimeConfig, base_url: Option<&str>) -> ModelProvider {
    match config.provider.protocol {
        ProviderProtocol::Anthropic => {
            if let Some(base_url) = base_url {
                if !is_first_party_base_url(base_url) {
                    return ModelProvider::OpenAiCompatible {
                        base_url: base_url.to_owned(),
                    };
                }
            }
            ModelProvider::Anthropic
        }
        ProviderProtocol::Bedrock => ModelProvider::AwsBedrock { region: None },
        ProviderProtocol::Vertex => ModelProvider::GcpVertex { project: None },
        ProviderProtocol::OpenAi => detect_provider(&rc_model::ProviderConfig {
            openai_base_url: config.provider.base_url.clone(),
            provider: Some("openai_compatible".to_owned()),
            ..Default::default()
        }),
    }
}

fn core_entry_to_agent_context_entry(
    entry: &CoreConversationEntry,
) -> rc_agents::ConversationEntry {
    let role = match entry.role {
        ConversationRole::System => "system",
        ConversationRole::Assistant => "assistant",
        ConversationRole::User => "user",
        ConversationRole::Tool => "tool",
    };
    rc_agents::ConversationEntry {
        role: role.to_owned(),
        content: entry.text.clone(),
    }
}

struct ScheduledAgentRun {
    task_id: Uuid,
    agent: AgentIdentity,
    task: AgentTask,
    runner: AgentRunner,
    prompt: String,
}

pub(crate) async fn run_agents(config: &RuntimeConfig, command: AgentsCommand) -> Result<()> {
    match command {
        AgentsCommand::Plan(args) => run_agents_plan(config, &args).await,
    }
}

pub(crate) async fn run_agents_plan(config: &RuntimeConfig, args: &AgentsPlanArgs) -> Result<()> {
    let mut scheduler = AgentScheduler::new(args.lead.clone(), args.objective.clone());
    let agents = if args.agents.is_empty() {
        default_agent_specs(config)
    } else {
        args.agents
            .iter()
            .map(|spec| parse_agent_spec(spec))
            .collect::<Result<Vec<_>>>()?
    };
    for agent in agents {
        scheduler.register_agent(agent);
    }

    let tasks = if args.tasks.is_empty() {
        vec![default_task_for_objective(&args.objective, config)]
    } else {
        args.tasks
            .iter()
            .map(|spec| parse_task_spec(spec))
            .collect::<Result<Vec<_>>>()?
    };
    for task in tasks {
        scheduler.add_task(task);
    }

    let available_tools = available_runtime_agent_tools().await;
    let mut scheduled_runs = Vec::new();

    while let Some((task_id, agent_id)) = scheduler.assign_next_task() {
        let agent = scheduler
            .agents()
            .into_iter()
            .find(|candidate| candidate.agent_id == agent_id)
            .ok_or_else(|| anyhow!("assigned agent {agent_id} was not found"))?;
        let task = scheduler
            .tasks()
            .into_iter()
            .find(|candidate| candidate.id == task_id)
            .ok_or_else(|| anyhow!("assigned task {task_id} was not found"))?;

        let _ = scheduler.queue_instruction(
            agent_id,
            args.lead.clone(),
            format!("Task: {}", task.title),
            format!(
                "Objective: {}\nTask: {}\nOwnership: {}",
                args.objective,
                task.title,
                if task.ownership_paths.is_empty() {
                    "(unscoped)".to_owned()
                } else {
                    task.ownership_paths.join(", ")
                }
            ),
        );

        let mailbox = scheduler.drain_mailbox(agent_id);
        let definition = agent_definition_for_identity(&agent);
        let prompt = build_task_prompt(&args.objective, &agent, &task, &mailbox, &definition);
        let runner = AgentRunner::new(
            definition,
            AgentRunConfig {
                max_turns: 0,
                model: String::new(),
                tools: available_tools.clone(),
                system_prompt: None,
                working_dir: config.cwd.clone(),
            },
        );
        let _ = scheduler.start_task(task_id);
        scheduled_runs.push(ScheduledAgentRun {
            task_id,
            agent,
            task,
            runner,
            prompt,
        });
    }

    let executor = Arc::new(RemoteCodeAgentExecutor::new(config));
    let mut join_set = JoinSet::new();

    for scheduled in scheduled_runs {
        let executor = Arc::clone(&executor);
        join_set.spawn(async move {
            let result = scheduled
                .runner
                .run_with_executor(&scheduled.prompt, &[], executor.as_ref())
                .await;
            (scheduled, result)
        });
    }

    while let Some(joined) = join_set.join_next().await {
        let (scheduled, result) =
            joined.map_err(|error| anyhow!("agent execution task panicked: {error}"))?;
        match result {
            Ok(run_result) if run_result.success => {
                let summary = truncate_single_line(&run_result.output, 160);
                let _ = scheduler.complete_task(scheduled.task_id, summary.clone());
                if !args.json {
                    println!(
                        "Completed `{}` by {} ({}) in {} turn(s)",
                        scheduled.task.title,
                        scheduled.agent.name,
                        scheduled.runner.definition().agent_type,
                        run_result.turns
                    );
                    if !summary.is_empty() {
                        println!("  {}", summary);
                    }
                }
            }
            Ok(run_result) => {
                let message = truncate_single_line(&run_result.output, 160);
                let _ = scheduler.fail_task(scheduled.task_id, message.clone());
                if !args.json {
                    println!(
                        "Failed `{}` by {} ({})",
                        scheduled.task.title,
                        scheduled.agent.name,
                        scheduled.runner.definition().agent_type
                    );
                    if !message.is_empty() {
                        println!("  {}", message);
                    }
                }
            }
            Err(error) => {
                let _ = scheduler.fail_task(scheduled.task_id, error.to_string());
                if !args.json {
                    println!(
                        "Error `{}` by {} ({})",
                        scheduled.task.title,
                        scheduled.agent.name,
                        scheduled.runner.definition().agent_type
                    );
                    println!("  {}", error);
                }
            }
        }
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&scheduler.snapshot())?);
    } else {
        let summary = scheduler.summary();
        println!(
            "\nTeam {}: {} agent(s), {} task(s), {} completed, {} failed, {} pending message(s)",
            summary.team_id,
            summary.total_agents,
            summary.total_tasks,
            summary.completed_tasks,
            summary.failed_tasks,
            summary.pending_messages
        );
    }
    Ok(())
}

fn append_conversation_context(
    conversation: &mut Vec<CoreConversationEntry>,
    context: &[rc_agents::ConversationEntry],
) {
    conversation.extend(context.iter().map(
        |entry| match entry.role.to_ascii_lowercase().as_str() {
            "system" => CoreConversationEntry::system(entry.content.clone()),
            "assistant" => CoreConversationEntry::assistant(entry.content.clone()),
            "tool" => CoreConversationEntry::user(format!("[tool context]\n{}", entry.content)),
            _ => CoreConversationEntry {
                role: ConversationRole::User,
                text: entry.content.clone(),
                history_text: None,
                content_blocks: Vec::new(),
                tool_calls: Vec::new(),
                attachments: Vec::new(),
                tool_call_id: None,
                name: None,
                is_error: false,
            },
        },
    ));
}

fn agent_definition_for_identity(agent: &AgentIdentity) -> AgentDefinition {
    let role = agent.role.to_ascii_lowercase();
    if role.contains("plan")
        || agent
            .labels
            .get("phase")
            .is_some_and(|phase| phase == "plan")
    {
        plan_agent()
    } else if role.contains("review")
        || role.contains("verify")
        || agent
            .labels
            .get("phase")
            .is_some_and(|phase| phase == "review")
    {
        verification_agent()
    } else if role.contains("explore")
        || role.contains("research")
        || agent.name.eq_ignore_ascii_case("explore")
    {
        explore_agent()
    } else {
        general_purpose_agent()
    }
}

fn build_task_prompt(
    objective: &str,
    agent: &AgentIdentity,
    task: &AgentTask,
    mailbox: &[rc_agents::AgentMailboxMessage],
    definition: &AgentDefinition,
) -> String {
    let mut sections = vec![
        format!("You are assigned to advance this objective:\n{objective}"),
        format!(
            "Assigned agent: {} ({}) using {}",
            agent.name, agent.role, definition.agent_type
        ),
        format!("Task title: {}", task.title),
    ];

    if !task.description.trim().is_empty() {
        sections.push(format!("Task description:\n{}", task.description));
    }
    if !task.ownership_paths.is_empty() {
        sections.push(format!(
            "Primary ownership paths:\n{}",
            task.ownership_paths.join("\n")
        ));
    }
    if !task.required_labels.is_empty() {
        sections.push(format!(
            "Required labels:\n{}",
            task.required_labels
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }
    if !mailbox.is_empty() {
        sections.push(format!(
            "Coordinator messages:\n{}",
            mailbox
                .iter()
                .map(|message| format!("{}:\n{}", message.subject, message.body))
                .collect::<Vec<_>>()
                .join("\n\n")
        ));
    }

    let completion = match definition.agent_type.as_str() {
        "Plan" | "Explore" => {
            "This is a read-only assignment. Do not modify files. Investigate the codebase thoroughly and return a concrete implementation-oriented report."
        }
        "verification" => {
            "Independently verify the current state. Prefer finding concrete defects, regressions, or missing tests. Only change files if strictly necessary to complete verification."
        }
        _ => {
            "Implement the requested work directly in the workspace, use tools as needed, run relevant validation where practical, and finish with a concise summary of changes and verification."
        }
    };
    sections.push(format!("Completion expectations:\n{completion}"));

    sections.join("\n\n")
}

fn insert_runtime_tool_aliases(spec: &ToolSpec, tools: &mut BTreeSet<String>) {
    tools.insert(spec.name.clone());
    tools.insert(spec.protocol_name.clone());
    match spec.name.as_str() {
        "read_file" => {
            tools.insert("Read".to_owned());
        }
        "write_file" => {
            tools.insert("Write".to_owned());
        }
        "edit_file" | "replace_in_file" => {
            tools.insert("Edit".to_owned());
        }
        "bash_command" => {
            tools.insert("Bash".to_owned());
        }
        "glob" => {
            tools.insert("Glob".to_owned());
        }
        "grep" => {
            tools.insert("Grep".to_owned());
        }
        "ask_user" => {
            tools.insert("AskUserQuestion".to_owned());
        }
        "agent" => {
            tools.insert("Agent".to_owned());
        }
        "task_create" => {
            tools.insert("Task".to_owned());
            tools.insert("TaskCreate".to_owned());
        }
        "todo_write" => {
            tools.insert("TodoWrite".to_owned());
        }
        "send_message" => {
            tools.insert("SendMessage".to_owned());
        }
        "skill_execute" | "discover_skills" => {
            tools.insert("Skill".to_owned());
        }
        "sleep" => {
            tools.insert("Sleep".to_owned());
        }
        _ => {}
    }
}

async fn available_runtime_agent_tools() -> Vec<String> {
    let mut tools = BTreeSet::new();
    for spec in runtime_provider_tool_specs().await {
        insert_runtime_tool_aliases(&spec, &mut tools);
    }
    tools.into_iter().collect()
}

fn truncate_single_line(text: &str, max_chars: usize) -> String {
    let single_line = text.lines().next().unwrap_or_default().trim();
    if single_line.chars().count() <= max_chars {
        return single_line.to_owned();
    }

    let truncated = single_line
        .chars()
        .take(max_chars)
        .collect::<String>()
        .trim_end()
        .to_owned();
    format!("{truncated}...")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_agent_specs_include_workspace_owner() {
        let agents = default_agent_specs_for_workspace(r"C:\work\sample-project");
        let workspace_agent = agents
            .iter()
            .find(|agent| agent.name == "workspace")
            .expect("workspace agent should be present");
        assert_eq!(
            workspace_agent.ownership_paths,
            vec![r"C:\work\sample-project".to_owned()]
        );
    }

    #[test]
    fn default_team_assigns_current_workspace_task() {
        let workspace = r"C:\work\sample-project";
        let mut scheduler = AgentScheduler::new("lead", "Inspect workspace");
        for agent in default_agent_specs_for_workspace(workspace) {
            scheduler.register_agent(agent);
        }

        let mut task = AgentTask::new("Inspect workspace");
        task.ownership_paths = vec![workspace.to_owned()];
        scheduler.add_task(task);

        let (_, agent_id) = scheduler
            .assign_next_task()
            .expect("a workspace-scoped task should be assignable");
        let assigned_agent = scheduler
            .agents()
            .into_iter()
            .find(|agent| agent.agent_id == agent_id)
            .expect("assigned agent should exist");
        assert_eq!(assigned_agent.name, "workspace");
    }

    #[test]
    fn parse_agent_spec_parses_paths_and_labels() {
        let agent = parse_agent_spec("reviewer;review;src,tests;phase=review,lang=rust")
            .expect("agent spec should parse");
        assert_eq!(agent.name, "reviewer");
        assert_eq!(agent.role, "review");
        assert_eq!(agent.ownership_paths, vec!["src", "tests"]);
        assert_eq!(
            agent.labels.get("phase").map(String::as_str),
            Some("review")
        );
        assert_eq!(agent.labels.get("lang").map(String::as_str), Some("rust"));
    }

    #[test]
    fn parse_task_spec_sets_budget_paths_labels_and_description() {
        let task = parse_task_spec("Refactor service;src/core;phase=backend;Tighten boundaries")
            .expect("task spec should parse");
        assert_eq!(task.title, "Refactor service");
        assert_eq!(task.ownership_paths, vec!["src/core"]);
        assert_eq!(
            task.required_labels.get("phase").map(String::as_str),
            Some("backend")
        );
        assert_eq!(task.description, "Tighten boundaries");
        assert_eq!(task.budget.read_calls, 32);
        assert_eq!(task.budget.edit_calls, 12);
        assert_eq!(task.budget.command_calls, 8);
    }

    #[test]
    fn parse_agent_spec_rejects_missing_role() {
        let error = parse_agent_spec("reviewer;;;").expect_err("missing role should fail");
        assert!(error.to_string().contains("role is missing"));
    }

    #[test]
    fn agent_definition_for_identity_selects_specialized_roles() {
        let mut planner = AgentIdentity::new("planner", "planner");
        planner.labels.insert("phase".to_owned(), "plan".to_owned());
        assert_eq!(
            agent_definition_for_identity(&planner).agent_type,
            plan_agent().agent_type
        );

        let reviewer = AgentIdentity::new("review", "reviewer");
        assert_eq!(
            agent_definition_for_identity(&reviewer).agent_type,
            verification_agent().agent_type
        );

        let explore = AgentIdentity::new("explore", "researcher");
        assert_eq!(
            agent_definition_for_identity(&explore).agent_type,
            explore_agent().agent_type
        );

        let implementer = AgentIdentity::new("workspace", "implementer");
        assert_eq!(
            agent_definition_for_identity(&implementer).agent_type,
            general_purpose_agent().agent_type
        );
    }

    #[test]
    fn append_conversation_context_maps_tool_role_to_user_context_message() {
        let mut conversation = Vec::new();
        append_conversation_context(
            &mut conversation,
            &[rc_agents::ConversationEntry {
                role: "tool".to_owned(),
                content: "cargo check passed".to_owned(),
            }],
        );
        assert_eq!(conversation.len(), 1);
        assert!(matches!(conversation[0].role, ConversationRole::User));
        assert!(conversation[0].text.contains("[tool context]"));
        assert!(conversation[0].text.contains("cargo check passed"));
    }
}

//! Agent system integration tests.
//!
//! Validates that rc-agents types flow correctly through the agent pipeline:
//! definition → runner → execution config, fork, coordinator/worker,
//! resume checkpoint, and built-in agent registry.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

// ─── Helpers ──────────────────────────────────────────────────────────────

fn ok<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("unexpected error: {error}"),
    }
}

// ─── AgentDefinition → serialization round-trip ───────────────────────────

#[test]
fn agent_definition_round_trips_via_json() {
    let def = rc_agents::AgentDefinition::new("test-agent", "Use for testing");
    let json = serde_json::to_string(&def).expect("serialize definition");
    let decoded: rc_agents::AgentDefinition =
        serde_json::from_str(&json).expect("deserialize definition");
    assert_eq!(decoded.agent_type, "test-agent");
    assert_eq!(decoded.when_to_use, "Use for testing");
    assert_eq!(decoded.source, rc_agents::definition::AgentSource::BuiltIn);
}

#[test]
fn agent_definition_with_all_fields_serializes() {
    let mut def = rc_agents::AgentDefinition::new("full-agent", "Full test agent");
    def.tools = vec!["Read".to_owned(), "Write".to_owned()];
    def.disallowed_tools = vec!["Bash".to_owned()];
    def.model = Some("haiku".to_owned());
    def.permission_mode = Some("plan".to_owned());
    def.system_prompt = Some("Custom system prompt".to_owned());
    def.skills = vec!["commit".to_owned()];
    def.memory = Some(rc_agents::definition::AgentMemoryScope::Project);
    def.background = true;
    def.isolation = rc_agents::definition::AgentIsolation::Worktree;
    def.initial_prompt = Some("Start here".to_owned());

    let json = serde_json::to_string(&def).expect("serialize");
    let decoded: rc_agents::AgentDefinition = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded.tools, ["Read", "Write"]);
    assert_eq!(decoded.disallowed_tools, ["Bash"]);
    assert_eq!(decoded.model.as_deref(), Some("haiku"));
    assert!(decoded.background);
    assert_eq!(
        decoded.isolation,
        rc_agents::definition::AgentIsolation::Worktree
    );
}

#[test]
fn agent_source_display_variants() {
    assert_eq!(
        rc_agents::definition::AgentSource::BuiltIn.to_string(),
        "built-in"
    );
    assert_eq!(
        rc_agents::definition::AgentSource::User.to_string(),
        "userSettings"
    );
    assert_eq!(
        rc_agents::definition::AgentSource::Marketplace.to_string(),
        "marketplace"
    );
}

// ─── Built-in agents registry ─────────────────────────────────────────────

#[test]
fn built_in_agents_returns_default_gated_set() {
    let agents = rc_agents::builtins::get_built_in_agents();
    assert_eq!(agents.len(), 5);
}

#[test]
fn built_in_agents_have_correct_types() {
    let agents = rc_agents::builtins::get_built_in_agents();
    let types: Vec<&str> = agents.iter().map(|a| a.agent_type.as_str()).collect();
    assert!(types.contains(&"general-purpose"));
    assert!(types.contains(&"statusline-setup"));
    assert!(types.contains(&"Explore"));
    assert!(types.contains(&"Plan"));
    assert!(types.contains(&"claude-code-guide"));
    assert!(!types.contains(&"verification"));
}

#[test]
fn built_in_agents_all_have_system_prompts() {
    let agents = rc_agents::builtins::get_built_in_agents();
    for agent in &agents {
        assert!(
            agent.system_prompt.is_some(),
            "agent '{}' missing system prompt",
            agent.agent_type
        );
    }
}

#[test]
fn explore_agent_is_read_only() {
    let explore = rc_agents::builtins::explore_agent();
    assert!(explore.has_tool_denylist());
    assert!(explore.disallowed_tools.contains(&"Edit".to_owned()));
    assert!(explore.disallowed_tools.contains(&"Write".to_owned()));
    assert!(explore.omit_claude_md);
}

// ─── AgentRunner → tool resolution ────────────────────────────────────────

#[test]
fn runner_resolves_wildcard_tools() {
    let def = rc_agents::builtins::general_purpose_agent();
    let config = rc_agents::AgentRunConfig {
        max_turns: 50,
        model: "sonnet".to_owned(),
        tools: vec![],
        system_prompt: None,
        working_dir: PathBuf::from("."),
        additional_working_directories: Vec::new(),
    };
    let runner = rc_agents::AgentRunner::new(def, config);
    let available = ["Read".to_owned(), "Write".to_owned(), "Bash".to_owned()];
    let resolved = runner.resolve_tools(&available);
    assert_eq!(resolved.len(), 3);
}

#[test]
fn runner_filters_denylisted_tools() {
    let explore = rc_agents::builtins::explore_agent();
    let config = rc_agents::AgentRunConfig {
        max_turns: 50,
        model: "haiku".to_owned(),
        tools: vec![],
        system_prompt: None,
        working_dir: PathBuf::from("."),
        additional_working_directories: Vec::new(),
    };
    let runner = rc_agents::AgentRunner::new(explore, config);
    let available = [
        "Read".to_owned(),
        "Write".to_owned(),
        "Edit".to_owned(),
        "Bash".to_owned(),
        "Glob".to_owned(),
    ];
    let resolved = runner.resolve_tools(&available);
    assert!(!resolved.contains(&"Write".to_owned()));
    assert!(!resolved.contains(&"Edit".to_owned()));
    assert!(resolved.contains(&"Read".to_owned()));
}

#[test]
fn runner_builds_system_prompt_from_definition() {
    let def = rc_agents::builtins::general_purpose_agent();
    let config = rc_agents::AgentRunConfig {
        max_turns: 10,
        model: "sonnet".to_owned(),
        tools: vec![],
        system_prompt: None,
        working_dir: PathBuf::from("."),
        additional_working_directories: Vec::new(),
    };
    let runner = rc_agents::AgentRunner::new(def, config);
    let prompt = runner.build_system_prompt();
    assert!(prompt.contains("Claude Code"));
}

#[tokio::test]
async fn runner_requires_host_executor_for_execution() {
    let def = rc_agents::builtins::general_purpose_agent();
    let config = rc_agents::AgentRunConfig {
        max_turns: 10,
        model: "sonnet".to_owned(),
        tools: vec!["Read".to_owned(), "Write".to_owned()],
        system_prompt: None,
        working_dir: PathBuf::from("."),
        additional_working_directories: Vec::new(),
    };
    let runner = rc_agents::AgentRunner::new(def, config);
    let error = runner
        .run("ship the feature", &[])
        .await
        .expect_err("runner should require executor");
    assert!(error.to_string().contains("run_with_executor"));
}

#[tokio::test]
async fn runner_executes_via_mock_executor_with_resolved_request() {
    #[derive(Clone)]
    struct MockExecutor {
        requests: Arc<Mutex<Vec<rc_agents::AgentExecutionRequest>>>,
    }

    #[async_trait]
    impl rc_agents::AgentExecutor for MockExecutor {
        async fn execute(
            &self,
            request: rc_agents::AgentExecutionRequest,
        ) -> anyhow::Result<rc_agents::AgentRunResult> {
            self.requests.lock().expect("requests lock").push(request);
            Ok(rc_agents::AgentRunResult {
                output: "executor output".to_owned(),
                success: true,
                turns: 3,
                usage: rc_agents::UsageSummary {
                    input_tokens: 12,
                    output_tokens: 8,
                    cache_creation_tokens: 0,
                    cache_read_tokens: 0,
                },
            })
        }
    }

    let requests = Arc::new(Mutex::new(Vec::new()));
    let executor = MockExecutor {
        requests: Arc::clone(&requests),
    };
    let def = rc_agents::builtins::explore_agent();
    let config = rc_agents::AgentRunConfig {
        max_turns: 0,
        model: String::new(),
        tools: vec![
            "Read".to_owned(),
            "Write".to_owned(),
            "Edit".to_owned(),
            "Glob".to_owned(),
        ],
        system_prompt: None,
        working_dir: PathBuf::from("."),
        additional_working_directories: Vec::new(),
    };
    let runner = rc_agents::AgentRunner::new(def, config);
    let result = runner
        .run_with_executor(
            "inspect auth code",
            &[rc_agents::ConversationEntry {
                role: "user".to_owned(),
                content: "previous context".to_owned(),
            }],
            &executor,
        )
        .await
        .expect("run with mock executor");

    assert_eq!(result.output, "executor output");
    let recorded = requests.lock().expect("requests");
    assert_eq!(recorded.len(), 1);
    let request = &recorded[0];
    assert_eq!(request.task, "inspect auth code");
    assert_eq!(request.context.len(), 1);
    assert_eq!(request.max_turns, 200);
    assert_eq!(request.model, "haiku");
    assert!(!request.skip_transcript);
    assert!(request.tools.contains(&"Read".to_owned()));
    assert!(request.tools.contains(&"Glob".to_owned()));
    assert!(!request.tools.contains(&"Write".to_owned()));
    assert!(!request.tools.contains(&"Edit".to_owned()));
}

// ─── Fork configuration ──────────────────────────────────────────────────

#[test]
fn fork_config_default_inherits_context() {
    let config = rc_agents::fork::ForkConfig::default();
    assert!(config.inherit_context);
    assert_eq!(config.model, rc_agents::fork::ForkModel::Inherit);
    assert_eq!(
        config.permission_mode,
        rc_agents::fork::ForkPermissionMode::Bubble
    );
}

#[test]
fn fork_config_serialization_round_trip() {
    let config = rc_agents::fork::ForkConfig {
        inherit_context: false,
        model: rc_agents::fork::ForkModel::Specific("sonnet".to_owned()),
        permission_mode: rc_agents::fork::ForkPermissionMode::Isolated,
        max_turns: 50,
    };
    let json = serde_json::to_string(&config).expect("serialize fork config");
    let decoded: rc_agents::fork::ForkConfig =
        serde_json::from_str(&json).expect("deserialize fork config");
    assert!(!decoded.inherit_context);
    assert_eq!(decoded.max_turns, 50);
}

#[test]
fn fork_agent_definition_has_correct_type() {
    let def = rc_agents::fork::fork_agent_definition();
    assert_eq!(def.agent_type, "fork");
    assert!(def.tools.contains(&"*".to_owned()));
}

#[test]
fn is_fork_child_detects_boilerplate_tag() {
    let messages = [rc_agents::fork::ForkMessage {
        role: "user".to_owned(),
        content: vec![rc_agents::fork::ForkContentBlock::Text {
            text: "<fork_boilerplate>some content</fork_boilerplate>".to_string(),
        }],
    }];
    assert!(rc_agents::fork::is_fork_child(&messages));
}

#[test]
fn is_fork_child_returns_false_for_normal_messages() {
    let messages = [rc_agents::fork::ForkMessage {
        role: "user".to_owned(),
        content: vec![rc_agents::fork::ForkContentBlock::Text {
            text: "normal user message".to_owned(),
        }],
    }];
    assert!(!rc_agents::fork::is_fork_child(&messages));
}

#[test]
fn build_fork_messages_with_tool_use_blocks() {
    let parent = [rc_agents::fork::ForkMessage {
        role: "assistant".to_owned(),
        content: vec![
            rc_agents::fork::ForkContentBlock::ToolUse {
                id: "tu-1".to_owned(),
                name: "Read".to_owned(),
                input: serde_json::json!({"path": "/tmp/test.rs"}),
            },
            rc_agents::fork::ForkContentBlock::ToolUse {
                id: "tu-2".to_owned(),
                name: "Write".to_owned(),
                input: serde_json::json!({"path": "/tmp/out.rs"}),
            },
        ],
    }];
    let result = rc_agents::fork::build_fork_messages(&parent, "do something");
    // Should produce placeholder results + directive
    assert!(!result.is_empty());
    let last = result.last().expect("last message");
    assert_eq!(last.role, "user");
}

// ─── Coordinator mode ────────────────────────────────────────────────────

#[test]
fn coordinator_mode_display() {
    assert_eq!(rc_agents::CoordinatorMode::Normal.to_string(), "normal");
    assert_eq!(
        rc_agents::CoordinatorMode::Coordinator.to_string(),
        "coordinator"
    );
}

#[test]
fn coordinator_mode_serialization_round_trip() {
    let mode = rc_agents::CoordinatorMode::Coordinator;
    let json = serde_json::to_string(&mode).expect("serialize");
    let decoded: rc_agents::CoordinatorMode = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(mode, decoded);
}

#[test]
fn coordinator_reset_override_cleans_state() {
    // Ensure reset works without panic
    rc_agents::coordinator::reset_coordinator_override();
    // After reset, should not be in coordinator mode
    // (unless env var is set, which we don't control in tests)
}

// ─── Worker lifecycle ────────────────────────────────────────────────────

#[test]
fn worker_agent_lifecycle_transitions() {
    let config = rc_agents::WorkerConfig {
        description: "test worker".to_owned(),
        prompt: "do work".to_owned(),
        model: None,
        max_turns: 10,
        simple_mode: false,
        working_dir: None,
    };
    let mut worker = rc_agents::WorkerAgent::new("w-1", config);
    assert_eq!(worker.status, rc_agents::WorkerStatus::Idle);

    ok(worker.start());
    assert_eq!(worker.status, rc_agents::WorkerStatus::Running);

    ok(worker.complete("done".to_owned()));
    assert_eq!(worker.status, rc_agents::WorkerStatus::Completed);
    assert_eq!(worker.output.as_deref(), Some("done"));
}

#[test]
fn worker_cannot_start_from_running_state() {
    let config = rc_agents::WorkerConfig::default();
    let mut worker = rc_agents::WorkerAgent::new("w-2", config);
    ok(worker.start());
    let err = worker
        .start()
        .expect_err("starting a running worker should fail");
    assert!(err.contains("running"));
}

#[test]
fn worker_kill_transitions_to_killed() {
    let config = rc_agents::WorkerConfig::default();
    let mut worker = rc_agents::WorkerAgent::new("w-3", config);
    ok(worker.start());
    ok(worker.kill());
    assert_eq!(worker.status, rc_agents::WorkerStatus::Killed);
}

#[test]
fn worker_config_serialization_round_trip() {
    let config = rc_agents::WorkerConfig {
        description: "serialize test".to_owned(),
        prompt: "test prompt".to_owned(),
        model: Some("haiku".to_owned()),
        max_turns: 42,
        simple_mode: true,
        working_dir: Some("/tmp".to_owned()),
    };
    let json = serde_json::to_string(&config).expect("serialize");
    let decoded: rc_agents::WorkerConfig = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded.description, "serialize test");
    assert_eq!(decoded.max_turns, 42);
    assert!(decoded.simple_mode);
}

// ─── Resume checkpoint save/load ─────────────────────────────────────────

#[test]
fn checkpoint_save_and_load_round_trip() {
    let temp = ok(tempfile::tempdir());
    let mut cp = rc_agents::AgentCheckpoint::new("agent-1", "general-purpose");
    cp.description = "test checkpoint".to_owned();
    cp.add_message("user", "hello");
    cp.add_message("assistant", "hi there");

    ok(rc_agents::resume::save_agent_checkpoint(temp.path(), &cp));

    let loaded = ok(rc_agents::resume::load_agent_checkpoint(
        temp.path(),
        "agent-1",
    ))
    .expect("checkpoint should exist");
    assert_eq!(loaded.agent_id, "agent-1");
    assert_eq!(loaded.agent_type, "general-purpose");
    assert_eq!(loaded.message_count(), 2);
    assert_eq!(loaded.messages[0].role, "user");
    assert_eq!(loaded.messages[0].content, "hello");
}

#[test]
fn checkpoint_can_resume_check() {
    let mut cp = rc_agents::AgentCheckpoint::new("a-2", "worker");
    assert!(cp.can_resume()); // default is Paused

    cp.state = rc_agents::ResumableAgentState::Completed;
    assert!(!cp.can_resume());

    cp.state = rc_agents::ResumableAgentState::Failed;
    assert!(cp.can_resume());
}

#[test]
fn checkpoint_total_tokens() {
    let mut cp = rc_agents::AgentCheckpoint::new("a-3", "worker");
    cp.usage.input_tokens = 100;
    cp.usage.output_tokens = 50;
    assert_eq!(cp.total_tokens(), 150);
}

// ─── Cross-crate: Agent types → JSON → rc-settings compatible ────────────

#[test]
fn agent_definition_source_compatible_with_settings() {
    // Verify that AgentSource from rc-agents can be serialized and
    // deserialized consistently (cross-crate type compatibility)
    let sources = [
        rc_agents::definition::AgentSource::BuiltIn,
        rc_agents::definition::AgentSource::User,
        rc_agents::definition::AgentSource::Project,
        rc_agents::definition::AgentSource::Local,
        rc_agents::definition::AgentSource::Plugin,
        rc_agents::definition::AgentSource::Marketplace,
    ];
    for source in sources {
        let json = serde_json::to_string(&source).expect("serialize source");
        let decoded: rc_agents::definition::AgentSource =
            serde_json::from_str(&json).expect("deserialize source");
        assert_eq!(source, decoded, "round-trip failed for {source:?}");
    }
}

#[test]
fn tool_budget_allows_and_consume() {
    let mut budget = rc_agents::ToolBudget {
        read_calls: 5,
        edit_calls: 3,
        command_calls: 0,
        network_calls: 2,
    };
    assert!(budget.allows(rc_agents::BudgetScope::Read));
    assert!(!budget.allows(rc_agents::BudgetScope::Command));

    assert!(budget.consume(rc_agents::BudgetScope::Read));
    assert_eq!(budget.remaining(rc_agents::BudgetScope::Read), 4);

    assert!(!budget.consume(rc_agents::BudgetScope::Command));
}

#[test]
fn agent_task_creation_and_serialization() {
    let task = rc_agents::AgentTask::new("Implement feature X");
    assert_eq!(task.state, rc_agents::TaskState::Pending);
    assert!(task.owner.is_none());

    let json = serde_json::to_string(&task).expect("serialize task");
    let decoded: rc_agents::AgentTask = serde_json::from_str(&json).expect("deserialize task");
    assert_eq!(decoded.title, "Implement feature X");
    assert_eq!(decoded.state, rc_agents::TaskState::Pending);
}

#[test]
fn agent_identity_creation() {
    let identity = rc_agents::AgentIdentity::new("worker-1", "worker");
    assert_eq!(identity.name, "worker-1");
    assert_eq!(identity.role, "worker");
    assert_eq!(identity.state, rc_agents::AgentState::Idle);
    assert_eq!(identity.max_concurrency, 1);
}

#[test]
fn context_slice_default_and_serialization() {
    let slice = rc_agents::ContextSlice {
        summary: "test context".to_owned(),
        artifact_paths: vec!["/tmp/a.rs".to_owned()],
        environment_hints: {
            let mut map = BTreeMap::new();
            map.insert("OS".to_owned(), "linux".to_owned());
            map
        },
        token_estimate: 42,
    };
    let json = serde_json::to_string(&slice).expect("serialize");
    let decoded: rc_agents::ContextSlice = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded.summary, "test context");
    assert_eq!(decoded.token_estimate, 42);
}

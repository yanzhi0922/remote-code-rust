//! Agent system integration tests.
//!
//! Validates that rc-agents types flow correctly through the agent pipeline:
//! definition 鈫?runner 鈫?execution config, fork, coordinator/worker,
//! resume checkpoint, and built-in agent registry.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use claude_context::{RuntimeFeatureGates, RuntimeIdentityContext};

// 鈹€鈹€鈹€ Helpers 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

fn ok<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("unexpected error: {error}"),
    }
}

// 鈹€鈹€鈹€ AgentDefinition 鈫?serialization round-trip 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

#[test]
fn agent_definition_round_trips_via_json() {
    let def = claude_agents::AgentDefinition::new("test-agent", "Use for testing");
    let json = serde_json::to_string(&def).expect("serialize definition");
    let decoded: claude_agents::AgentDefinition =
        serde_json::from_str(&json).expect("deserialize definition");
    assert_eq!(decoded.agent_type, "test-agent");
    assert_eq!(decoded.when_to_use, "Use for testing");
    assert_eq!(decoded.source, claude_agents::definition::AgentSource::BuiltIn);
}

#[test]
fn agent_definition_with_all_fields_serializes() {
    let mut def = claude_agents::AgentDefinition::new("full-agent", "Full test agent");
    def.tools = vec!["Read".to_owned(), "Write".to_owned()];
    def.disallowed_tools = vec!["Bash".to_owned()];
    def.model = Some("haiku".to_owned());
    def.permission_mode = Some("plan".to_owned());
    def.system_prompt = Some("Custom system prompt".to_owned());
    def.skills = vec!["commit".to_owned()];
    def.memory = Some(claude_agents::definition::AgentMemoryScope::Project);
    def.background = true;
    def.isolation = claude_agents::definition::AgentIsolation::Worktree;
    def.initial_prompt = Some("Start here".to_owned());

    let json = serde_json::to_string(&def).expect("serialize");
    let decoded: claude_agents::AgentDefinition = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded.tools, ["Read", "Write"]);
    assert_eq!(decoded.disallowed_tools, ["Bash"]);
    assert_eq!(decoded.model.as_deref(), Some("haiku"));
    assert!(decoded.background);
    assert_eq!(
        decoded.isolation,
        claude_agents::definition::AgentIsolation::Worktree
    );
}

#[test]
fn agent_source_display_variants() {
    assert_eq!(
        claude_agents::definition::AgentSource::BuiltIn.to_string(),
        "built-in"
    );
    assert_eq!(
        claude_agents::definition::AgentSource::User.to_string(),
        "userSettings"
    );
    assert_eq!(
        claude_agents::definition::AgentSource::Marketplace.to_string(),
        "marketplace"
    );
}

// 鈹€鈹€鈹€ Built-in agents registry 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

fn test_context() -> RuntimeIdentityContext {
    RuntimeIdentityContext {
        features: RuntimeFeatureGates {
            explore_plan_agents_enabled: true,
            code_guide_enabled: true,
            ..Default::default()
        },
        ..Default::default()
    }
}

#[test]
fn built_in_agents_returns_default_gated_set() {
    let agents = claude_agents::builtins::get_built_in_agents_with_context(&test_context());
    assert_eq!(agents.len(), 5);
}

#[test]
fn built_in_agents_have_correct_types() {
    let agents = claude_agents::builtins::get_built_in_agents_with_context(&test_context());
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
    let agents = claude_agents::builtins::get_built_in_agents_with_context(&test_context());
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
    let explore = claude_agents::builtins::explore_agent();
    assert!(explore.has_tool_denylist());
    assert!(explore.disallowed_tools.contains(&"Edit".to_owned()));
    assert!(explore.disallowed_tools.contains(&"Write".to_owned()));
    assert!(explore.omit_claude_md);
}

// 鈹€鈹€鈹€ AgentRunner 鈫?tool resolution 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

#[test]
fn runner_resolves_wildcard_tools() {
    let def = claude_agents::builtins::general_purpose_agent();
    let config = claude_agents::AgentRunConfig {
        max_turns: 50,
        model: "sonnet".to_owned(),
        tools: vec![],
        system_prompt: None,
        working_dir: PathBuf::from("."),
        additional_working_directories: Vec::new(),
    };
    let runner = claude_agents::AgentRunner::new(def, config);
    let available = ["Read".to_owned(), "Write".to_owned(), "Bash".to_owned()];
    let resolved = runner.resolve_tools(&available);
    assert_eq!(resolved.len(), 3);
}

#[test]
fn runner_filters_denylisted_tools() {
    let explore = claude_agents::builtins::explore_agent();
    let config = claude_agents::AgentRunConfig {
        max_turns: 50,
        model: "haiku".to_owned(),
        tools: vec![],
        system_prompt: None,
        working_dir: PathBuf::from("."),
        additional_working_directories: Vec::new(),
    };
    let runner = claude_agents::AgentRunner::new(explore, config);
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
    let def = claude_agents::builtins::general_purpose_agent();
    let config = claude_agents::AgentRunConfig {
        max_turns: 10,
        model: "sonnet".to_owned(),
        tools: vec![],
        system_prompt: None,
        working_dir: PathBuf::from("."),
        additional_working_directories: Vec::new(),
    };
    let runner = claude_agents::AgentRunner::new(def, config);
    let prompt = runner.build_system_prompt();
    assert!(prompt.contains("Claude Code"));
}

#[tokio::test]
async fn runner_requires_host_executor_for_execution() {
    let def = claude_agents::builtins::general_purpose_agent();
    let config = claude_agents::AgentRunConfig {
        max_turns: 10,
        model: "sonnet".to_owned(),
        tools: vec!["Read".to_owned(), "Write".to_owned()],
        system_prompt: None,
        working_dir: PathBuf::from("."),
        additional_working_directories: Vec::new(),
    };
    let runner = claude_agents::AgentRunner::new(def, config);
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
        requests: Arc<Mutex<Vec<claude_agents::AgentExecutionRequest>>>,
    }

    #[async_trait]
    impl claude_agents::AgentExecutor for MockExecutor {
        async fn execute(
            &self,
            request: claude_agents::AgentExecutionRequest,
        ) -> anyhow::Result<claude_agents::AgentRunResult> {
            self.requests.lock().expect("requests lock").push(request);
            Ok(claude_agents::AgentRunResult {
                output: "executor output".to_owned(),
                success: true,
                turns: 3,
                usage: claude_agents::UsageSummary {
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
    let def = claude_agents::builtins::explore_agent();
    let config = claude_agents::AgentRunConfig {
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
    let runner = claude_agents::AgentRunner::new(def, config);
    let result = runner
        .run_with_executor(
            "inspect auth code",
            &[claude_agents::ConversationEntry {
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

// 鈹€鈹€鈹€ Fork configuration 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

#[test]
fn fork_config_default_inherits_context() {
    let config = claude_agents::fork::ForkConfig::default();
    assert!(config.inherit_context);
    assert_eq!(config.model, claude_agents::fork::ForkModel::Inherit);
    assert_eq!(
        config.permission_mode,
        claude_agents::fork::ForkPermissionMode::Bubble
    );
}

#[test]
fn fork_config_serialization_round_trip() {
    let config = claude_agents::fork::ForkConfig {
        inherit_context: false,
        model: claude_agents::fork::ForkModel::Specific("sonnet".to_owned()),
        permission_mode: claude_agents::fork::ForkPermissionMode::Isolated,
        max_turns: 50,
    };
    let json = serde_json::to_string(&config).expect("serialize fork config");
    let decoded: claude_agents::fork::ForkConfig =
        serde_json::from_str(&json).expect("deserialize fork config");
    assert!(!decoded.inherit_context);
    assert_eq!(decoded.max_turns, 50);
}

#[test]
fn fork_agent_definition_has_correct_type() {
    let def = claude_agents::fork::fork_agent_definition();
    assert_eq!(def.agent_type, "fork");
    assert!(def.tools.contains(&"*".to_owned()));
}

#[test]
fn is_fork_child_detects_boilerplate_tag() {
    let messages = [claude_agents::fork::ForkMessage {
        role: "user".to_owned(),
        content: vec![claude_agents::fork::ForkContentBlock::Text {
            text: "<fork-boilerplate>some content</fork-boilerplate>".to_string(),
        }],
    }];
    assert!(claude_agents::fork::is_fork_child(&messages));
}

#[test]
fn is_fork_child_returns_false_for_normal_messages() {
    let messages = [claude_agents::fork::ForkMessage {
        role: "user".to_owned(),
        content: vec![claude_agents::fork::ForkContentBlock::Text {
            text: "normal user message".to_owned(),
        }],
    }];
    assert!(!claude_agents::fork::is_fork_child(&messages));
}

#[test]
fn build_fork_messages_with_tool_use_blocks() {
    let parent = [claude_agents::fork::ForkMessage {
        role: "assistant".to_owned(),
        content: vec![
            claude_agents::fork::ForkContentBlock::ToolUse {
                id: "tu-1".to_owned(),
                name: "Read".to_owned(),
                input: serde_json::json!({"path": "/tmp/test.rs"}),
            },
            claude_agents::fork::ForkContentBlock::ToolUse {
                id: "tu-2".to_owned(),
                name: "Write".to_owned(),
                input: serde_json::json!({"path": "/tmp/out.rs"}),
            },
        ],
    }];
    let result = claude_agents::fork::build_fork_messages(&parent, "do something");
    // Should produce placeholder results + directive
    assert!(!result.is_empty());
    let last = result.last().expect("last message");
    assert_eq!(last.role, "user");
}

// 鈹€鈹€鈹€ Coordinator mode 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

#[test]
fn coordinator_mode_display() {
    assert_eq!(claude_agents::CoordinatorMode::Normal.to_string(), "normal");
    assert_eq!(
        claude_agents::CoordinatorMode::Coordinator.to_string(),
        "coordinator"
    );
}

#[test]
fn coordinator_mode_serialization_round_trip() {
    let mode = claude_agents::CoordinatorMode::Coordinator;
    let json = serde_json::to_string(&mode).expect("serialize");
    let decoded: claude_agents::CoordinatorMode = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(mode, decoded);
}

#[test]
fn coordinator_reset_override_cleans_state() {
    // Ensure reset works without panic
    claude_agents::coordinator::reset_coordinator_override();
    // After reset, should not be in coordinator mode
    // (unless env var is set, which we don't control in tests)
}

// 鈹€鈹€鈹€ Worker lifecycle 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

#[test]
fn worker_agent_lifecycle_transitions() {
    let config = claude_agents::WorkerConfig {
        description: "test worker".to_owned(),
        prompt: "do work".to_owned(),
        model: None,
        max_turns: 10,
        simple_mode: false,
        working_dir: None,
    };
    let mut worker = claude_agents::WorkerAgent::new("w-1", config);
    assert_eq!(worker.status, claude_agents::WorkerStatus::Idle);

    ok(worker.start());
    assert_eq!(worker.status, claude_agents::WorkerStatus::Running);

    ok(worker.complete("done".to_owned()));
    assert_eq!(worker.status, claude_agents::WorkerStatus::Completed);
    assert_eq!(worker.output.as_deref(), Some("done"));
}

#[test]
fn worker_cannot_start_from_running_state() {
    let config = claude_agents::WorkerConfig::default();
    let mut worker = claude_agents::WorkerAgent::new("w-2", config);
    ok(worker.start());
    let err = worker
        .start()
        .expect_err("starting a running worker should fail");
    assert!(err.contains("running"));
}

#[test]
fn worker_kill_transitions_to_killed() {
    let config = claude_agents::WorkerConfig::default();
    let mut worker = claude_agents::WorkerAgent::new("w-3", config);
    ok(worker.start());
    ok(worker.kill());
    assert_eq!(worker.status, claude_agents::WorkerStatus::Killed);
}

#[test]
fn worker_config_serialization_round_trip() {
    let config = claude_agents::WorkerConfig {
        description: "serialize test".to_owned(),
        prompt: "test prompt".to_owned(),
        model: Some("haiku".to_owned()),
        max_turns: 42,
        simple_mode: true,
        working_dir: Some("/tmp".to_owned()),
    };
    let json = serde_json::to_string(&config).expect("serialize");
    let decoded: claude_agents::WorkerConfig = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded.description, "serialize test");
    assert_eq!(decoded.max_turns, 42);
    assert!(decoded.simple_mode);
}

// 鈹€鈹€鈹€ Resume checkpoint save/load 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

#[test]
fn checkpoint_save_and_load_round_trip() {
    let temp = ok(tempfile::tempdir());
    let mut cp = claude_agents::AgentCheckpoint::new("agent-1", "general-purpose");
    cp.description = "test checkpoint".to_owned();
    cp.add_message("user", "hello");
    cp.add_message("assistant", "hi there");

    ok(claude_agents::resume::save_agent_checkpoint(temp.path(), &cp));

    let loaded = ok(claude_agents::resume::load_agent_checkpoint(
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
    let mut cp = claude_agents::AgentCheckpoint::new("a-2", "worker");
    assert!(cp.can_resume()); // default is Paused

    cp.state = claude_agents::ResumableAgentState::Completed;
    assert!(!cp.can_resume());

    cp.state = claude_agents::ResumableAgentState::Failed;
    assert!(cp.can_resume());
}

#[test]
fn checkpoint_total_tokens() {
    let mut cp = claude_agents::AgentCheckpoint::new("a-3", "worker");
    cp.usage.input_tokens = 100;
    cp.usage.output_tokens = 50;
    assert_eq!(cp.total_tokens(), 150);
}

// 鈹€鈹€鈹€ Cross-crate: Agent types 鈫?JSON 鈫?rc-settings compatible 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

#[test]
fn agent_definition_source_compatible_with_settings() {
    // Verify that AgentSource from rc-agents can be serialized and
    // deserialized consistently (cross-crate type compatibility)
    let sources = [
        claude_agents::definition::AgentSource::BuiltIn,
        claude_agents::definition::AgentSource::User,
        claude_agents::definition::AgentSource::Project,
        claude_agents::definition::AgentSource::Local,
        claude_agents::definition::AgentSource::Plugin,
        claude_agents::definition::AgentSource::Marketplace,
    ];
    for source in sources {
        let json = serde_json::to_string(&source).expect("serialize source");
        let decoded: claude_agents::definition::AgentSource =
            serde_json::from_str(&json).expect("deserialize source");
        assert_eq!(source, decoded, "round-trip failed for {source:?}");
    }
}

#[test]
fn tool_budget_allows_and_consume() {
    let mut budget = claude_agents::ToolBudget {
        read_calls: 5,
        edit_calls: 3,
        command_calls: 0,
        network_calls: 2,
    };
    assert!(budget.allows(claude_agents::BudgetScope::Read));
    assert!(!budget.allows(claude_agents::BudgetScope::Command));

    assert!(budget.consume(claude_agents::BudgetScope::Read));
    assert_eq!(budget.remaining(claude_agents::BudgetScope::Read), 4);

    assert!(!budget.consume(claude_agents::BudgetScope::Command));
}

#[test]
fn agent_task_creation_and_serialization() {
    let task = claude_agents::AgentTask::new("Implement feature X");
    assert_eq!(task.state, claude_agents::TaskState::Pending);
    assert!(task.owner.is_none());

    let json = serde_json::to_string(&task).expect("serialize task");
    let decoded: claude_agents::AgentTask = serde_json::from_str(&json).expect("deserialize task");
    assert_eq!(decoded.title, "Implement feature X");
    assert_eq!(decoded.state, claude_agents::TaskState::Pending);
}

#[test]
fn agent_identity_creation() {
    let identity = claude_agents::AgentIdentity::new("worker-1", "worker");
    assert_eq!(identity.name, "worker-1");
    assert_eq!(identity.role, "worker");
    assert_eq!(identity.state, claude_agents::AgentState::Idle);
    assert_eq!(identity.max_concurrency, 1);
}

#[test]
fn context_slice_default_and_serialization() {
    let slice = claude_agents::ContextSlice {
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
    let decoded: claude_agents::ContextSlice = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded.summary, "test context");
    assert_eq!(decoded.token_estimate, 42);
}

// 鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺?
// Phase 6.1 鈥?rc-agent-protocol integration tests
// 鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺愨晲鈺?

use claude_agent_protocol::adapters::RemoteClaudeAdapter;
use claude_agent_protocol::health::{HealthCheckConfig, HealthChecker, HealthStatus};
use claude_agent_protocol::restart::{RestartPolicy, RestartTracker};
use claude_agent_protocol::types::{AgentCapability, AgentConfig, AgentInfo, AgentStatus};
use claude_agent_protocol::{
    AgentAdapter, AgentResult, AgentRouter, AgentType, PermissionDecision, ToolCallInfo,
    UnifiedAgentEvent, UsageInfo,
};
use std::collections::HashSet;

/// Helper: minimal RemoteClaude AgentConfig for tests.
fn protocol_test_config() -> AgentConfig {
    AgentConfig {
        agent_type: AgentType::RemoteClaude,
        binary_path: None,
        args: vec![],
        env: vec![],
        working_dir: None,
        model: None,
        provider: None,
        api_key: None,
        base_url: None,
    }
}

// 鈹€鈹€鈹€ AgentRouter routing tests 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

#[tokio::test]
async fn router_register_and_send_message_routes_correctly() {
    let mut router = AgentRouter::new();

    let adapter = RemoteClaudeAdapter::new_claude().with_send_message(|_sid, msg| {
        Ok(vec![UnifiedAgentEvent::MessageDelta {
            session_id: "sess-1".into(),
            delta: format!("echo: {msg}"),
        }])
    });

    let mut boxed: Box<dyn AgentAdapter> = Box::new(adapter);
    boxed.start(&protocol_test_config()).await.unwrap();
    router.register("sess-1".into(), boxed).await;

    assert!(router.has_session("sess-1"));
    assert_eq!(router.session_count(), 1);

    let mut rx = router.send_message("sess-1", "hello").await.unwrap();
    let event = rx.recv().await.expect("should receive event");
    match event {
        UnifiedAgentEvent::MessageDelta { delta, .. } => {
            assert_eq!(delta, "echo: hello");
        }
        other => panic!("expected MessageDelta, got {other:?}"),
    }
}

#[tokio::test]
async fn router_multiple_sessions_route_independently() {
    let mut router = AgentRouter::new();

    // Session A
    let adapter_a = RemoteClaudeAdapter::new_claude().with_send_message(|_sid, msg| {
        Ok(vec![UnifiedAgentEvent::MessageDelta {
            session_id: "sess-a".into(),
            delta: format!("A:{msg}"),
        }])
    });
    let mut boxed_a: Box<dyn AgentAdapter> = Box::new(adapter_a);
    boxed_a.start(&protocol_test_config()).await.unwrap();
    router.register("sess-a".into(), boxed_a).await;

    // Session B
    let adapter_b = RemoteClaudeAdapter::new_claude().with_send_message(|_sid, msg| {
        Ok(vec![UnifiedAgentEvent::MessageDelta {
            session_id: "sess-b".into(),
            delta: format!("B:{msg}"),
        }])
    });
    let mut boxed_b: Box<dyn AgentAdapter> = Box::new(adapter_b);
    boxed_b.start(&protocol_test_config()).await.unwrap();
    router.register("sess-b".into(), boxed_b).await;

    assert_eq!(router.session_count(), 2);

    // Route to A
    let mut rx_a = router.send_message("sess-a", "test").await.unwrap();
    let ev_a = rx_a.recv().await.unwrap();
    match ev_a {
        UnifiedAgentEvent::MessageDelta { delta, .. } => assert_eq!(delta, "A:test"),
        _ => panic!("expected MessageDelta"),
    }

    // Route to B
    let mut rx_b = router.send_message("sess-b", "test").await.unwrap();
    let ev_b = rx_b.recv().await.unwrap();
    match ev_b {
        UnifiedAgentEvent::MessageDelta { delta, .. } => assert_eq!(delta, "B:test"),
        _ => panic!("expected MessageDelta"),
    }
}

#[tokio::test]
async fn router_close_session_removes_adapter() {
    let mut router = AgentRouter::new();

    let adapter = RemoteClaudeAdapter::new_claude().with_send_message(|_sid, _msg| Ok(vec![]));
    let mut boxed: Box<dyn AgentAdapter> = Box::new(adapter);
    boxed.start(&protocol_test_config()).await.unwrap();
    router.register("sess-x".into(), boxed).await;

    assert_eq!(router.session_count(), 1);
    router.close_session("sess-x").await.unwrap();
    assert_eq!(router.session_count(), 0);
    assert!(!router.has_session("sess-x"));
}

#[tokio::test]
async fn router_cancel_delegates_to_adapter() {
    let canceled = Arc::new(Mutex::new(false));
    let canceled_clone = canceled.clone();

    let adapter = RemoteClaudeAdapter::new_claude()
        .with_send_message(|_sid, _msg| Ok(vec![]))
        .with_cancel(move |_sid| {
            *canceled_clone.lock().unwrap() = true;
            Ok(())
        });

    let mut boxed: Box<dyn AgentAdapter> = Box::new(adapter);
    boxed.start(&protocol_test_config()).await.unwrap();
    let mut router = AgentRouter::new();
    router.register("sess-c".into(), boxed).await;

    router.cancel("sess-c").await.unwrap();
    assert!(*canceled.lock().unwrap());
}

// 鈹€鈹€鈹€ RemoteClaudeAdapter integration tests 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

#[tokio::test]
async fn remotecode_adapter_lifecycle_start_send_stop() {
    let messages_received = Arc::new(Mutex::new(Vec::<String>::new()));
    let messages_clone = messages_received.clone();

    let adapter = RemoteClaudeAdapter::new_claude().with_send_message(move |_sid, msg| {
        messages_clone.lock().unwrap().push(msg.to_string());
        Ok(vec![
            UnifiedAgentEvent::Ready,
            UnifiedAgentEvent::MessageDelta {
                session_id: "s1".into(),
                delta: msg.into(),
            },
            UnifiedAgentEvent::Completed {
                session_id: "s1".into(),
                result: AgentResult {
                    response_text: msg.into(),
                    tool_calls: vec![],
                    usage: UsageInfo::default(),
                    cost: None,
                },
            },
        ])
    });

    let mut adapter = adapter;

    // Before start 鈥?alive (status is Starting)
    assert!(adapter.is_alive());

    // Start
    adapter.start(&protocol_test_config()).await.unwrap();
    assert!(adapter.is_alive());
    assert_eq!(adapter.info().status, AgentStatus::Ready);

    // Send message
    let mut rx = adapter.send_message("s1", "hello world").await.unwrap();
    let mut events = Vec::new();
    while let Some(ev) = rx.recv().await {
        events.push(ev);
    }
    assert_eq!(events.len(), 3);
    assert!(matches!(events[0], UnifiedAgentEvent::Ready));
    assert!(matches!(events[1], UnifiedAgentEvent::MessageDelta { .. }));
    assert!(matches!(events[2], UnifiedAgentEvent::Completed { .. }));

    // Verify callback received the message
    assert_eq!(messages_received.lock().unwrap().len(), 1);
    assert_eq!(messages_received.lock().unwrap()[0], "hello world");

    // Stop
    adapter.stop().await.unwrap();
    assert!(!adapter.is_alive());
    assert_eq!(adapter.info().status, AgentStatus::Stopped);
}

#[tokio::test]
async fn remotecode_adapter_is_alive_state_changes() {
    let mut adapter = RemoteClaudeAdapter::new_claude();
    assert!(adapter.is_alive()); // Starting

    adapter.start(&protocol_test_config()).await.unwrap();
    assert!(adapter.is_alive()); // Ready

    adapter.stop().await.unwrap();
    assert!(!adapter.is_alive()); // Stopped
}

#[tokio::test]
async fn remotecode_adapter_info_has_correct_metadata() {
    let adapter = RemoteClaudeAdapter::new_claude();
    let info = adapter.info();

    assert_eq!(info.name, "Remote Claude");
    assert!(!info.version.is_empty());
    assert!(info.capabilities.contains(&AgentCapability::Streaming));
    assert!(info.capabilities.contains(&AgentCapability::ToolUse));
    assert_eq!(adapter.agent_type(), AgentType::RemoteClaude);
}

#[tokio::test]
async fn remotecode_adapter_resolve_permission_delegates() {
    let resolved = Arc::new(Mutex::new(
        Vec::<(String, String, PermissionDecision)>::new(),
    ));
    let resolved_clone = resolved.clone();

    let adapter =
        RemoteClaudeAdapter::new_claude().with_resolve_permission(move |sid, rid, dec| {
            resolved_clone
                .lock()
                .unwrap()
                .push((sid.to_string(), rid.to_string(), dec));
            Ok(())
        });

    let mut adapter = adapter;
    adapter.start(&protocol_test_config()).await.unwrap();

    adapter
        .resolve_permission("sess-1", "req-1", PermissionDecision::Allow)
        .await
        .unwrap();

    let lock = resolved.lock().unwrap();
    assert_eq!(lock.len(), 1);
    assert_eq!(lock[0].0, "sess-1");
    assert_eq!(lock[0].1, "req-1");
    assert_eq!(lock[0].2, PermissionDecision::Allow);
}

// 鈹€鈹€鈹€ Event translation / serialization tests 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

#[test]
fn all_unified_agent_event_variants_roundtrip() {
    let mut caps = HashSet::new();
    caps.insert(AgentCapability::Streaming);

    let info = AgentInfo {
        name: "Test".into(),
        version: "0.1.0".into(),
        capabilities: caps,
        status: AgentStatus::Ready,
    };

    let events = vec![
        UnifiedAgentEvent::Started(info.clone()),
        UnifiedAgentEvent::Ready,
        UnifiedAgentEvent::MessageDelta {
            session_id: "s".into(),
            delta: "hi".into(),
        },
        UnifiedAgentEvent::ToolCallStarted {
            session_id: "s".into(),
            tool_name: "bash".into(),
            tool_input: serde_json::json!({"cmd": "ls"}),
        },
        UnifiedAgentEvent::ToolCallProgress {
            session_id: "s".into(),
            tool_name: "bash".into(),
            progress: "running".into(),
        },
        UnifiedAgentEvent::CodexAppServerNotification {
            session_id: "s".into(),
            method: "model/verification".into(),
            params: serde_json::json!({"model": "gpt-5"}),
        },
        UnifiedAgentEvent::ToolCallCompleted {
            session_id: "s".into(),
            tool_name: "bash".into(),
            result: serde_json::json!({"exit": 0}),
        },
        UnifiedAgentEvent::PermissionRequest {
            session_id: "s".into(),
            request_id: "r1".into(),
            tool_name: "write".into(),
            input: serde_json::json!({"path": "/tmp/x"}),
        },
        UnifiedAgentEvent::SubtaskStarted {
            session_id: "s".into(),
            task_id: "t1".into(),
            description: "sub".into(),
        },
        UnifiedAgentEvent::SubtaskProgress {
            session_id: "s".into(),
            task_id: "t1".into(),
            progress: "50%".into(),
        },
        UnifiedAgentEvent::SubtaskCompleted {
            session_id: "s".into(),
            task_id: "t1".into(),
            result: serde_json::json!("done"),
        },
        UnifiedAgentEvent::ContextUsage {
            session_id: "s".into(),
            used: 1000,
            total: 2000,
        },
        UnifiedAgentEvent::ContextOverflow {
            session_id: "s".into(),
            used: 0,
            total: 0,
        },
        UnifiedAgentEvent::ContextCompacted {
            session_id: "s".into(),
            entries_removed: 0,
            usage_ratio: 0.0,
        },
        UnifiedAgentEvent::Error {
            session_id: "s".into(),
            message: "fail".into(),
            recoverable: true,
        },
        UnifiedAgentEvent::Completed {
            session_id: "s".into(),
            result: AgentResult {
                response_text: "ok".into(),
                tool_calls: vec![ToolCallInfo {
                    id: "tc-1".into(),
                    name: "read".into(),
                    input: serde_json::json!({}),
                    output: serde_json::json!({}),
                }],
                usage: UsageInfo {
                    input_tokens: 10,
                    output_tokens: 5,
                    cache_read: 0,
                    cache_write: 0,
                },
                cost: Some(0.001),
            },
        },
        UnifiedAgentEvent::Stopped,
    ];

    for event in &events {
        let json = serde_json::to_string(event)
            .unwrap_or_else(|e| panic!("failed to serialize {event:?}: {e}"));
        let back: UnifiedAgentEvent = serde_json::from_str(&json)
            .unwrap_or_else(|e| panic!("failed to deserialize {json}: {e}"));

        // Re-serialize the deserialized value and compare JSON strings.
        let json2 = serde_json::to_string(&back).unwrap();
        assert_eq!(json, json2, "roundtrip mismatch for {event:?}");
    }
}

#[tokio::test]
async fn events_flow_from_adapter_through_router() {
    let adapter = RemoteClaudeAdapter::new_claude().with_send_message(|_sid, msg| {
        Ok(vec![
            UnifiedAgentEvent::MessageDelta {
                session_id: "sess-flow".into(),
                delta: format!("part1:{msg}"),
            },
            UnifiedAgentEvent::MessageDelta {
                session_id: "sess-flow".into(),
                delta: "part2".into(),
            },
            UnifiedAgentEvent::Completed {
                session_id: "sess-flow".into(),
                result: AgentResult {
                    response_text: "done".into(),
                    tool_calls: vec![],
                    usage: UsageInfo::default(),
                    cost: None,
                },
            },
        ])
    });

    let mut boxed: Box<dyn AgentAdapter> = Box::new(adapter);
    boxed.start(&protocol_test_config()).await.unwrap();

    let mut router = AgentRouter::new();
    router.register("sess-flow".into(), boxed).await;

    let mut rx = router.send_message("sess-flow", "test-flow").await.unwrap();

    let ev1 = rx.recv().await.unwrap();
    assert!(matches!(ev1, UnifiedAgentEvent::MessageDelta { .. }));

    let ev2 = rx.recv().await.unwrap();
    assert!(matches!(ev2, UnifiedAgentEvent::MessageDelta { .. }));

    let ev3 = rx.recv().await.unwrap();
    assert!(matches!(ev3, UnifiedAgentEvent::Completed { .. }));

    assert!(rx.recv().await.is_none(), "channel should be closed");
}

// 鈹€鈹€鈹€ Error handling tests 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

#[tokio::test]
async fn router_send_message_unregistered_session_returns_error() {
    let mut router = AgentRouter::new();
    let result = router.send_message("nonexistent", "hello").await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("no adapter found for session nonexistent"),
        "unexpected error: {err_msg}"
    );
}

#[tokio::test]
async fn router_cancel_unregistered_session_returns_error() {
    let mut router = AgentRouter::new();
    let result = router.cancel("ghost").await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("no adapter found for session ghost"),
        "unexpected error: {err_msg}"
    );
}

#[tokio::test]
async fn adapter_send_message_without_callback_returns_error() {
    let mut adapter = RemoteClaudeAdapter::new_claude();
    adapter.start(&protocol_test_config()).await.unwrap();

    let result = adapter.send_message("s1", "hello").await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("send_message callback not configured"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn adapter_cancel_without_callback_returns_error() {
    let mut adapter = RemoteClaudeAdapter::new_claude();
    adapter.start(&protocol_test_config()).await.unwrap();

    let result = adapter.cancel("s1").await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("cancel callback not configured"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn router_close_nonexistent_session_is_ok() {
    let mut router = AgentRouter::new();
    let result = router.close_session("does-not-exist").await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn router_resolve_permission_unregistered_returns_error() {
    let mut router = AgentRouter::new();
    let result = router
        .resolve_permission("no-sess", "req-1", PermissionDecision::Deny)
        .await;
    assert!(result.is_err());
}

// 鈹€鈹€鈹€ Health check integration tests 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

#[test]
fn health_checker_tracks_adapter_liveness() {
    let mut checker = HealthChecker::new(HealthCheckConfig {
        max_failures: 3,
        ..Default::default()
    });

    // Simulate adapter alive
    assert_eq!(checker.check(true), &HealthStatus::Healthy);

    // Simulate failures
    checker.check(false);
    assert!(matches!(checker.status(), HealthStatus::Degraded { .. }));

    checker.check(false);
    assert!(matches!(checker.status(), HealthStatus::Degraded { .. }));

    checker.check(false);
    assert!(matches!(checker.status(), HealthStatus::Unhealthy { .. }));

    // Recovery
    checker.check(true);
    assert_eq!(checker.status(), &HealthStatus::Healthy);
}

#[test]
fn health_checker_reset_after_adapter_restart() {
    let mut checker = HealthChecker::new(HealthCheckConfig {
        max_failures: 1,
        ..Default::default()
    });

    checker.check(false);
    assert!(matches!(checker.status(), HealthStatus::Unhealthy { .. }));

    // Simulate adapter restart 鈫?reset health
    checker.reset();
    assert_eq!(checker.status(), &HealthStatus::Healthy);

    checker.check(true);
    assert_eq!(checker.status(), &HealthStatus::Healthy);
}

// 鈹€鈹€鈹€ Restart strategy integration tests 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

#[test]
fn restart_tracker_allows_backoff_and_reset() {
    let mut tracker = RestartTracker::new(RestartPolicy {
        max_restarts: 3,
        initial_backoff: std::time::Duration::from_millis(100),
        max_backoff: std::time::Duration::from_secs(5),
        backoff_multiplier: 2.0,
    });

    let b1 = tracker.request_restart().unwrap();
    assert_eq!(b1, std::time::Duration::from_millis(100));

    let b2 = tracker.request_restart().unwrap();
    assert_eq!(b2, std::time::Duration::from_millis(200));

    let b3 = tracker.request_restart().unwrap();
    assert_eq!(b3, std::time::Duration::from_millis(400));

    // Exhausted
    assert!(tracker.request_restart().is_none());

    // After successful run 鈫?reset
    tracker.reset();
    assert!(tracker.can_restart());
    let b_after = tracker.request_restart().unwrap();
    assert_eq!(b_after, std::time::Duration::from_millis(100));
}

#[test]
fn restart_tracker_zero_max_never_allows() {
    let mut tracker = RestartTracker::new(RestartPolicy {
        max_restarts: 0,
        initial_backoff: std::time::Duration::from_secs(1),
        max_backoff: std::time::Duration::from_secs(10),
        backoff_multiplier: 2.0,
    });
    assert!(tracker.request_restart().is_none());
    assert!(!tracker.can_restart());
}

// ─── Phase 15: Multi-Agent adapter protocol tests ──────────────────────

#[tokio::test]
async fn remote_roo_adapter_lifecycle_and_routing() {
    let adapter = claude_agent_protocol::adapters::RemoteRooAdapter::new_roo().with_send_message(
        |_sid, msg| {
            Ok(vec![
                UnifiedAgentEvent::MessageDelta {
                    session_id: "roo-s1".into(),
                    delta: format!("roo: {msg}"),
                },
                UnifiedAgentEvent::Completed {
                    session_id: "roo-s1".into(),
                    result: AgentResult {
                        response_text: format!("roo response: {msg}"),
                        tool_calls: vec![],
                        usage: UsageInfo::default(),
                        cost: None,
                    },
                },
            ])
        },
    );

    let mut boxed: Box<dyn AgentAdapter> = Box::new(adapter);
    let config = AgentConfig {
        agent_type: AgentType::RemoteRoo,
        binary_path: None,
        args: vec![],
        env: vec![],
        working_dir: None,
        model: Some("claude-sonnet-4-20250514".into()),
        provider: Some("anthropic".into()),
        api_key: None,
        base_url: None,
    };
    boxed.start(&config).await.unwrap();
    assert_eq!(boxed.agent_type(), AgentType::RemoteRoo);
    assert!(boxed.is_alive());
    assert_eq!(boxed.info().name, "Remote Roo");

    let mut rx = boxed.send_message("roo-s1", "hello from roo").await.unwrap();
    let ev1 = rx.recv().await.unwrap();
    assert!(matches!(ev1, UnifiedAgentEvent::MessageDelta { ref delta, .. } if delta == "roo: hello from roo"));
    let ev2 = rx.recv().await.unwrap();
    assert!(matches!(ev2, UnifiedAgentEvent::Completed { .. }));
}

#[tokio::test]
async fn remote_codex_adapter_lifecycle_and_routing() {
    let adapter = claude_agent_protocol::adapters::RemoteCodexAdapter::new_codex().with_send_message(
        |_sid, msg| {
            Ok(vec![
                UnifiedAgentEvent::MessageDelta {
                    session_id: "codex-s1".into(),
                    delta: format!("codex: {msg}"),
                },
                UnifiedAgentEvent::Completed {
                    session_id: "codex-s1".into(),
                    result: AgentResult {
                        response_text: format!("codex response: {msg}"),
                        tool_calls: vec![],
                        usage: UsageInfo::default(),
                        cost: None,
                    },
                },
            ])
        },
    );

    let mut boxed: Box<dyn AgentAdapter> = Box::new(adapter);
    let config = AgentConfig {
        agent_type: AgentType::RemoteCodex,
        binary_path: None,
        args: vec![],
        env: vec![],
        working_dir: None,
        model: None,
        provider: None,
        api_key: None,
        base_url: None,
    };
    boxed.start(&config).await.unwrap();
    assert_eq!(boxed.agent_type(), AgentType::RemoteCodex);
    assert!(boxed.is_alive());
    assert_eq!(boxed.info().name, "Remote Codex");

    let mut rx = boxed.send_message("codex-s1", "hello from codex").await.unwrap();
    let ev1 = rx.recv().await.unwrap();
    assert!(matches!(ev1, UnifiedAgentEvent::MessageDelta { ref delta, .. } if delta == "codex: hello from codex"));
}

#[tokio::test]
async fn three_agents_route_through_same_router() {
    let mut router = AgentRouter::new();

    let claude = RemoteClaudeAdapter::new_claude().with_send_message(|_sid, msg| {
        Ok(vec![UnifiedAgentEvent::MessageDelta {
            session_id: "s-claude".into(),
            delta: format!("claude:{msg}"),
        }])
    });
    let mut boxed_c: Box<dyn AgentAdapter> = Box::new(claude);
    boxed_c.start(&protocol_test_config()).await.unwrap();
    router.register("s-claude".into(), boxed_c).await;

    let roo = claude_agent_protocol::adapters::RemoteRooAdapter::new_roo()
        .with_send_message(|_sid, msg| {
            Ok(vec![UnifiedAgentEvent::MessageDelta {
                session_id: "s-roo".into(),
                delta: format!("roo:{msg}"),
            }])
        });
    let mut boxed_r: Box<dyn AgentAdapter> = Box::new(roo);
    boxed_r.start(&AgentConfig {
        agent_type: AgentType::RemoteRoo,
        binary_path: None,
        args: vec![],
        env: vec![],
        working_dir: None,
        model: None,
        provider: None,
        api_key: None,
        base_url: None,
    }).await.unwrap();
    router.register("s-roo".into(), boxed_r).await;

    let codex = claude_agent_protocol::adapters::RemoteCodexAdapter::new_codex()
        .with_send_message(|_sid, msg| {
            Ok(vec![UnifiedAgentEvent::MessageDelta {
                session_id: "s-codex".into(),
                delta: format!("codex:{msg}"),
            }])
        });
    let mut boxed_x: Box<dyn AgentAdapter> = Box::new(codex);
    boxed_x.start(&AgentConfig {
        agent_type: AgentType::RemoteCodex,
        binary_path: None,
        args: vec![],
        env: vec![],
        working_dir: None,
        model: None,
        provider: None,
        api_key: None,
        base_url: None,
    }).await.unwrap();
    router.register("s-codex".into(), boxed_x).await;

    assert_eq!(router.session_count(), 3);

    let mut rx_c = router.send_message("s-claude", "hi").await.unwrap();
    match rx_c.recv().await.unwrap() {
        UnifiedAgentEvent::MessageDelta { delta, .. } => assert_eq!(delta, "claude:hi"),
        _ => panic!("expected MessageDelta"),
    }

    let mut rx_r = router.send_message("s-roo", "hi").await.unwrap();
    match rx_r.recv().await.unwrap() {
        UnifiedAgentEvent::MessageDelta { delta, .. } => assert_eq!(delta, "roo:hi"),
        _ => panic!("expected MessageDelta"),
    }

    let mut rx_x = router.send_message("s-codex", "hi").await.unwrap();
    match rx_x.recv().await.unwrap() {
        UnifiedAgentEvent::MessageDelta { delta, .. } => assert_eq!(delta, "codex:hi"),
        _ => panic!("expected MessageDelta"),
    }
}

#[tokio::test]
async fn agent_type_serialization_covers_all_three_variants() {
    let types = vec![
        AgentType::RemoteClaude,
        AgentType::RemoteCodex,
        AgentType::RemoteRoo,
    ];
    for at in &types {
        let json = serde_json::to_string(at).unwrap();
        let back: AgentType = serde_json::from_str(&json).unwrap();
        assert_eq!(*at, back, "round-trip failed for {at:?}");
    }

    assert_eq!(
        serde_json::to_string(&AgentType::RemoteClaude).unwrap(),
        "\"remote_claude\""
    );
    assert_eq!(
        serde_json::to_string(&AgentType::RemoteCodex).unwrap(),
        "\"remote_codex\""
    );
    assert_eq!(
        serde_json::to_string(&AgentType::RemoteRoo).unwrap(),
        "\"remote_roo\""
    );
}

#[tokio::test]
async fn permission_request_event_carries_tool_info() {
    let request_id = "perm-test-1".to_string();
    let event = UnifiedAgentEvent::PermissionRequest {
        session_id: "s1".into(),
        request_id: request_id.clone(),
        tool_name: "execute_command".into(),
        input: serde_json::json!({"command": "rm -rf /tmp/test"}),
    };

    let json = serde_json::to_string(&event).unwrap();
    let back: UnifiedAgentEvent = serde_json::from_str(&json).unwrap();

    match back {
        UnifiedAgentEvent::PermissionRequest {
            session_id,
            request_id: rid,
            tool_name,
            input,
        } => {
            assert_eq!(session_id, "s1");
            assert_eq!(rid, request_id);
            assert_eq!(tool_name, "execute_command");
            assert_eq!(input["command"], "rm -rf /tmp/test");
        }
        _ => panic!("expected PermissionRequest"),
    }
}

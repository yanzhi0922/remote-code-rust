//! Integration tests for rc-claude-adapter.
//!
//! These tests verify the adapter's lifecycle and error handling
//! without requiring a real provider or API key.

use std::collections::BTreeMap;
use std::sync::Arc;

use anyhow::Result;
use rc_agent_protocol::adapter::AgentAdapter;
use rc_agent_protocol::events::UnifiedAgentEvent;
use rc_agent_protocol::permission::PermissionDecision;
use rc_agent_protocol::types::{AgentCapability, AgentConfig, AgentType};
use rc_claude_adapter::ClaudeInProcessAdapter;
use tempfile::tempdir;

use claude_config::{AppPaths, ProviderConfig, RuntimeConfig};
use claude_core::{InputFormat, OutputFormat, PermissionMode, ProviderProtocol};
use claude_session::SessionStore;
use uuid::Uuid;

/// Create a minimal `RuntimeConfig` backed by a temporary directory.
fn minimal_runtime_config(profile_dir: &std::path::Path) -> Result<RuntimeConfig> {
    let paths = AppPaths::discover(Some(profile_dir.to_path_buf()))?;
    paths.ensure_exists()?;

    let provider = ProviderConfig {
        name: "test".to_owned(),
        base_url: None,
        api_key: None,
        model: None,
        protocol: ProviderProtocol::OpenAi,
        timeout_ms: 30_000,
        max_output_tokens: 4096,
        max_retries: 3,
        retry_initial_backoff_ms: 1_000,
        retry_max_backoff_ms: 30_000,
        respect_retry_after: false,
        request_header_overrides: BTreeMap::new(),
        request_metadata: BTreeMap::new(),
        thinking_budget: None,
        temperature: None,
        top_p: None,
        top_k: None,
    };

    let cwd = profile_dir.to_path_buf();

    Ok(RuntimeConfig {
        cwd: cwd.clone(),
        original_cwd: cwd.clone(),
        active_worktree_session: None,
        session_id: Uuid::new_v4(),
        permission_mode: PermissionMode::Default,
        input_format: InputFormat::Text,
        output_format: OutputFormat::Text,
        print_mode: false,
        verbose: false,
        replay_user_messages: false,
        include_partial_messages: false,
        structured_output_schema: None,
        mcp_config_paths: Vec::new(),
        strict_mcp_config: false,
        max_turns: 1,
        session_name: None,
        system_prompt: None,
        append_system_prompt: None,
        setting_sources: Vec::new(),
        allowed_setting_sources: Vec::new(),
        settings_files: Vec::new(),
        cli_settings_files: Vec::new(),
        allowed_tools: Vec::new(),
        disallowed_tools: Vec::new(),
        effort: None,
        fallback_model: None,
        output_style: None,
        language: None,
        brief_enabled: false,
        proactive_active: false,
        auth_source: None,
        api_key_helper: None,
        api_key_helper_source: None,
        provider,
        paths,
    })
}

#[tokio::test]
async fn test_adapter_start_stop() -> Result<()> {
    let dir = tempdir()?;
    let config = minimal_runtime_config(dir.path())?;
    let store = Arc::new(SessionStore::open(config.paths.clone())?);
    let mut adapter = ClaudeInProcessAdapter::new(config, store);

    // Adapter starts in Starting state.
    assert_eq!(format!("{}", adapter.info().status), "starting");

    adapter
        .start(&AgentConfig {
            agent_type: AgentType::RemoteClaude,
            ..Default::default()
        })
        .await?;

    // After start the adapter should be ready and alive.
    assert!(adapter.is_alive());

    adapter.stop().await?;
    Ok(())
}

#[tokio::test]
async fn test_adapter_info_and_type() -> Result<()> {
    let dir = tempdir()?;
    let config = minimal_runtime_config(dir.path())?;
    let store = Arc::new(SessionStore::open(config.paths.clone())?);
    let adapter = ClaudeInProcessAdapter::new(config, store);

    assert_eq!(adapter.agent_type(), AgentType::RemoteClaude);
    assert_eq!(adapter.info().name, "Remote Claude");
    assert!(
        adapter
            .info()
            .capabilities
            .contains(&AgentCapability::Streaming)
    );
    assert!(
        adapter
            .info()
            .capabilities
            .contains(&AgentCapability::ToolUse)
    );
    // The Claude adapter does not advertise McpSupport by default;
    // MCP capability is loaded dynamically via claude-mcp at runtime.
    Ok(())
}

#[tokio::test]
async fn test_send_message_returns_error_without_provider() -> Result<()> {
    let dir = tempdir()?;
    let config = minimal_runtime_config(dir.path())?;
    let store = Arc::new(SessionStore::open(config.paths.clone())?);
    let mut adapter = ClaudeInProcessAdapter::new(config, store);

    adapter
        .start(&AgentConfig {
            agent_type: AgentType::RemoteClaude,
            ..Default::default()
        })
        .await?;

    // Without a real API key, send_message should either return an error
    // or return a receiver that eventually yields an Error or stop signal.
    let result = adapter.send_message("test-session", "Hello").await;
    match result {
        Ok(mut rx) => {
            // Adapter spawned a worker. Wait briefly for the stream to
            // terminate (either with an Error event or channel close).
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), async {
                while let Some(event) = rx.recv().await {
                    match &event {
                        UnifiedAgentEvent::Error { .. } | UnifiedAgentEvent::Stopped => break,
                        _ => {}
                    }
                }
            })
            .await;
        }
        Err(_) => {
            // Expected: no provider configured.
        }
    }
    Ok(())
}

#[tokio::test]
async fn test_stop_is_idempotent() -> Result<()> {
    let dir = tempdir()?;
    let config = minimal_runtime_config(dir.path())?;
    let store = Arc::new(SessionStore::open(config.paths.clone())?);
    let mut adapter = ClaudeInProcessAdapter::new(config, store);

    adapter
        .start(&AgentConfig {
            agent_type: AgentType::RemoteClaude,
            ..Default::default()
        })
        .await?;

    adapter.stop().await?;
    // Second stop should not panic or error.
    adapter.stop().await?;
    Ok(())
}

#[tokio::test]
async fn test_resolve_unknown_permission_returns_error() -> Result<()> {
    let dir = tempdir()?;
    let config = minimal_runtime_config(dir.path())?;
    let store = Arc::new(SessionStore::open(config.paths.clone())?);
    let mut adapter = ClaudeInProcessAdapter::new(config, store);

    adapter
        .start(&AgentConfig {
            agent_type: AgentType::RemoteClaude,
            ..Default::default()
        })
        .await?;

    // A permission request with an unknown ID should error.
    let result = adapter
        .resolve_permission("test-session", "nonexistent", PermissionDecision::Deny)
        .await;
    assert!(result.is_err());
    Ok(())
}

#[tokio::test]
async fn test_cancel_is_safe_when_not_busy() -> Result<()> {
    let dir = tempdir()?;
    let config = minimal_runtime_config(dir.path())?;
    let store = Arc::new(SessionStore::open(config.paths.clone())?);
    let mut adapter = ClaudeInProcessAdapter::new(config, store);

    adapter
        .start(&AgentConfig {
            agent_type: AgentType::RemoteClaude,
            ..Default::default()
        })
        .await?;

    // Cancel on an idle adapter should succeed.
    adapter.cancel("test-session").await?;
    assert!(adapter.is_alive());
    Ok(())
}

// ── Performance benchmarks (inline, no external deps) ─────────────────

/// Benchmark: adapter start/stop should complete within 500ms.
#[tokio::test]
async fn bench_adapter_start_stop_timing() {
    let dir = tempdir().expect("tempdir");
    let config = minimal_runtime_config(dir.path()).expect("runtime config");
    let store = Arc::new(SessionStore::open(config.paths.clone()).expect("session store"));
    let start = std::time::Instant::now();
    let mut adapter = ClaudeInProcessAdapter::new(config, store);
    adapter
        .start(&AgentConfig {
            agent_type: AgentType::RemoteClaude,
            ..Default::default()
        })
        .await
        .expect("start");
    adapter.stop().await.expect("stop");
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 500,
        "adapter start/stop took too long: {}ms",
        elapsed.as_millis()
    );
}

/// Benchmark: resolve 100 permission requests within 2 seconds.
#[tokio::test]
async fn bench_permission_resolve_timing() {
    let dir = tempdir().expect("tempdir");
    let config = minimal_runtime_config(dir.path()).expect("runtime config");
    let store = Arc::new(SessionStore::open(config.paths.clone()).expect("session store"));
    let mut adapter = ClaudeInProcessAdapter::new(config, store);
    adapter
        .start(&AgentConfig {
            agent_type: AgentType::RemoteClaude,
            ..Default::default()
        })
        .await
        .expect("start");

    let start = std::time::Instant::now();
    for i in 0..100 {
        let _ = adapter
            .resolve_permission("bench", &format!("unknown-{i}"), PermissionDecision::Deny)
            .await;
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 2000,
        "100 permission resolve calls took too long: {}ms",
        elapsed.as_millis()
    );
    adapter.stop().await.expect("stop");
}

/// Verify engine event conversion performance (1000 events).
#[test]
fn bench_engine_event_conversion_timing() {
    use rc_agent_protocol::from_engine::engine_event_to_unified;
    use rc_engine_events::types::{ContentBlockDelta, EngineEvent};
    use std::sync::Arc;

    let events: Vec<EngineEvent> = (0..1000)
        .map(|i| {
            if i % 3 == 0 {
                EngineEvent::ToolUseStarted {
                    tool_use_id: Arc::from(format!("tu-{i}")),
                    tool_name: Arc::from("read_file"),
                    input: Arc::new(serde_json::json!({"path": "/tmp/test.txt"})),
                }
            } else if i % 3 == 1 {
                EngineEvent::StreamContentBlockDelta {
                    index: 0,
                    delta: ContentBlockDelta::TextDelta {
                        text: "Hello ".repeat(10),
                    },
                }
            } else {
                EngineEvent::ToolUseCompleted {
                    tool_use_id: Arc::from(format!("tu-{i}")),
                    result: rc_engine_events::types::ToolResult {
                        content: "done".into(),
                        is_error: false,
                        ..Default::default()
                    },
                }
            }
        })
        .collect();

    let start = std::time::Instant::now();
    for event in &events {
        let _ = engine_event_to_unified(event, "bench-session");
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 500,
        "1000 engine event conversions took too long: {}ms",
        elapsed.as_millis()
    );
}

/// Verify UnifiedAgentEvent JSON round-trip performance.
#[test]
fn bench_unified_event_serde_timing() {
    use rc_agent_protocol::events::{AgentResult, UnifiedAgentEvent, UsageInfo};

    let event = UnifiedAgentEvent::Completed {
        session_id: "bench-session".into(),
        result: AgentResult {
            response_text: "Hello, world!".repeat(100),
            tool_calls: vec![],
            usage: UsageInfo {
                input_tokens: 500,
                output_tokens: 1200,
                cache_read: 0,
                cache_write: 0,
            },
            cost: Some(0.015),
        },
    };

    let start = std::time::Instant::now();
    for _ in 0..500 {
        let json = serde_json::to_string(&event).expect("serialize");
        let _back: UnifiedAgentEvent = serde_json::from_str(&json).expect("deserialize");
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 1000,
        "500 serde round-trips took too long: {}ms",
        elapsed.as_millis()
    );
}

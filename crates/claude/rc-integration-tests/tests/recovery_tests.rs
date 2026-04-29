//! Recovery tests.
//!
//! Tests session recovery, MCP reconnection, and swarm teammate reconnection.

use std::collections::BTreeMap;

// ─── Session recovery ───────────────────────────────────────────────────────

#[tokio::test]
async fn session_recovery_via_transcript() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("recovery.jsonl");
    let storage = rc_transcript::TranscriptStorage::new(&path);
    let session_id = uuid::Uuid::new_v4();

    // Simulate a conversation that was interrupted
    storage
        .append(&rc_transcript::TranscriptEntry::conversation_now(
            session_id,
            rc_core::ConversationEntry::user("Write a hello world"),
        ))
        .await
        .expect("should write");

    storage
        .append(&rc_transcript::TranscriptEntry::conversation_now(
            session_id,
            rc_core::ConversationEntry::assistant("I'll create the file."),
        ))
        .await
        .expect("should write");

    storage
        .append(&rc_transcript::TranscriptEntry::named_event_now(
            session_id,
            "tool_call",
            Some(serde_json::json!({
                "id": "tc-1",
                "name": "write_file",
                "input": {"path": "/tmp/hello.rs", "content": "fn main() { println!(\"hello\"); }"}
            })),
        ))
        .await
        .expect("should write");

    // Simulate crash — no tool result was written

    // Recovery: read all entries
    let entries = storage.read_all().await.expect("should read");
    assert_eq!(entries.len(), 3);

    // Reconstruct conversation from entries
    let mut conversation = Vec::new();
    for entry in &entries {
        if let Some(conv_entry) = entry.as_conversation() {
            conversation.push(conv_entry.clone());
        }
    }
    assert_eq!(conversation.len(), 2);

    // Resume state should have pending tool calls
    let resume = rc_session::resume_state::ResumeState::from_pending_calls(vec![
        rc_session::resume_state::PendingToolCall {
            id: "tc-1".to_owned(),
            name: "write_file".to_owned(),
            input: serde_json::json!({"path": "/tmp/hello.rs"}),
        },
    ]);
    assert_eq!(resume.pending_tool_calls.len(), 1);
}

#[test]
fn session_recovery_full_cycle() {
    let dir = tempfile::tempdir().expect("tempdir");
    let paths = rc_config::AppPaths {
        profile_dir: dir.path().to_path_buf(),
        state_db_path: dir.path().join("state.db"),
        sessions_dir: dir.path().join("sessions"),
        artifacts_dir: dir.path().join("artifacts"),
        logs_dir: dir.path().join("logs"),
        profiles_dir: dir.path().join("profiles"),
        skills_dir: dir.path().join("skills"),
        plugins_dir: dir.path().join("plugins"),
    };

    let session_id = uuid::Uuid::new_v4();

    // Phase 1: Create session and add entries
    {
        let store = rc_session::SessionStore::open(paths.clone()).expect("store should open");
        store
            .ensure_session(
                session_id,
                std::path::Path::new("/tmp"),
                "anthropic",
                None,
                Some("Recovery Test"),
            )
            .expect("should create session");

        store
            .append_conversation_entry(session_id, &rc_core::ConversationEntry::user("hello"))
            .expect("should append");
        store
            .append_conversation_entry(session_id, &rc_core::ConversationEntry::assistant("hi"))
            .expect("should append");
    }

    // Phase 2: Reopen and verify recovery
    {
        let store = rc_session::SessionStore::open(paths.clone()).expect("store should reopen");
        let sessions = store.list_sessions().expect("should list");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, session_id);

        let conversation = store
            .load_conversation(session_id)
            .expect("should load conversation");
        assert_eq!(conversation.len(), 2);
    }
}

// ─── MCP reconnect ──────────────────────────────────────────────────────────

#[test]
fn mcp_reconnect_scheduler_recovery() {
    let mut scheduler = rc_mcp::ReconnectScheduler::with_params(5, 1, 1000);

    // Simulate connection failure
    let action = scheduler.schedule_reconnect("server-1".to_owned());
    assert!(matches!(action, rc_mcp::ReconnectAction::ConnectNow));

    // Simulate failure
    let backoff = scheduler.report_failure("server-1");
    assert!(backoff.is_some());

    // Simulate recovery — success
    let action = scheduler.schedule_reconnect("server-1".to_owned());
    assert!(matches!(
        action,
        rc_mcp::ReconnectAction::ConnectNow | rc_mcp::ReconnectAction::WaitFor(..)
    ));

    scheduler.report_success("server-1");
    assert!(!scheduler.is_reconnecting("server-1"));
}

#[test]
fn mcp_connection_state_recovery() {
    let config = rc_mcp::McpServerConfig {
        name: "test".to_owned(),
        enabled: true,
        transport: rc_mcp::McpTransportConfig::Stdio {
            command: "echo".to_owned(),
            args: vec![],
            cwd: None,
            env: BTreeMap::new(),
        },
        capabilities: rc_mcp::McpCapabilityMatrix::default(),
        startup_timeout_secs: None,
        request_timeout_secs: None,
        metadata: BTreeMap::new(),
    };
    let scoped =
        rc_mcp::scope::ScopedMcpServerConfig::new(config, rc_mcp::scope::ConfigScope::Local);

    // Simulate: was connected → failed → pending → reconnecting
    let failed = rc_mcp::McpServerConnection::Failed(rc_mcp::connection::FailedServer {
        name: "test".to_owned(),
        config: scoped.clone(),
        error: Some("connection reset".to_owned()),
    });

    // Serialize and deserialize (simulate persistence)
    let json = serde_json::to_string(&failed).expect("should serialize");
    let recovered: rc_mcp::McpServerConnection =
        serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(recovered.name(), "test");
    assert_eq!(recovered.connection_type(), "failed");

    // Transition to pending
    let pending = rc_mcp::McpServerConnection::Pending(rc_mcp::connection::PendingServer {
        name: "test".to_owned(),
        config: scoped.clone(),
        reconnect_attempt: Some(1),
        max_reconnect_attempts: Some(5),
    });
    assert_eq!(pending.connection_type(), "pending");

    // Transition to connected
    let connected = rc_mcp::McpServerConnection::Connected(rc_mcp::connection::ConnectedServer {
        name: "test".to_owned(),
        capabilities: rc_mcp::McpCapabilityMatrix::default(),
        server_info: None,
        instructions: None,
        config: scoped,
    });
    assert!(connected.is_connected());
}

// ─── Swarm teammate reconnection ────────────────────────────────────────────

#[tokio::test]
async fn swarm_teammate_reconnection() {
    let dir = tempfile::tempdir().expect("tempdir");
    rc_swarm::team_helpers::set_base_dir_override(Some(dir.path().to_path_buf()));

    let mut team = rc_swarm::TeamFile::new("reconnect-team", "lead-1");

    // Add a member with session ID (needed for reconnection)
    let mut member = rc_swarm::TeamMember::new("agent-1", "worker-1", "pane-0", "/tmp/workdir");
    member.session_id = Some("session-abc".to_owned());
    member.backend_type = Some(rc_swarm::BackendType::InProcess);
    member.is_active = Some(true);
    team.members.push(member);

    rc_swarm::team_helpers::create_team(&team)
        .await
        .expect("should create team");

    // Simulate disconnection
    rc_swarm::reconnection::mark_disconnected("reconnect-team", "worker-1")
        .await
        .expect("should mark disconnected");

    let team = rc_swarm::team_helpers::read_team("reconnect-team")
        .await
        .expect("should read team");
    let member = team.find_member("worker-1").expect("should find member");
    assert_eq!(member.is_active, Some(false));

    // Verify can reconnect
    assert!(rc_swarm::reconnection::can_reconnect("reconnect-team", "worker-1").await);

    // List reconnectable teammates
    let reconnectable = rc_swarm::reconnection::list_reconnectable("reconnect-team")
        .await
        .expect("should list reconnectable");
    assert_eq!(reconnectable.len(), 1);
    assert_eq!(reconnectable[0].name, "worker-1");

    // Reconnect
    let result = rc_swarm::reconnection::reconnect_teammate("reconnect-team", "worker-1")
        .await
        .expect("should reconnect");
    assert!(!result.was_active);
    assert_eq!(result.identity.name, "worker-1");

    // Mark as reconnected
    rc_swarm::reconnection::mark_reconnected("reconnect-team", "worker-1")
        .await
        .expect("should mark reconnected");

    let team = rc_swarm::team_helpers::read_team("reconnect-team")
        .await
        .expect("should read team");
    let member = team.find_member("worker-1").expect("should find member");
    assert_eq!(member.is_active, Some(true));

    rc_swarm::team_helpers::set_base_dir_override(None);
}

#[tokio::test]
async fn swarm_cannot_reconnect_nonexistent_member() {
    let dir = tempfile::tempdir().expect("tempdir");
    rc_swarm::team_helpers::set_base_dir_override(Some(dir.path().to_path_buf()));

    let team = rc_swarm::TeamFile::new("no-reconnect", "lead-1");
    rc_swarm::team_helpers::create_team(&team)
        .await
        .expect("should create team");

    // Cannot reconnect a member that doesn't exist
    assert!(!rc_swarm::reconnection::can_reconnect("no-reconnect", "ghost-worker").await);

    rc_swarm::team_helpers::set_base_dir_override(None);
}

// ─── Auth cache recovery ────────────────────────────────────────────────────

#[test]
fn auth_cache_recovery_after_clear() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut cache = rc_mcp::McpAuthCache::new(dir.path());

    // Mark several servers as needing auth
    cache.mark_needs_auth("server-1");
    cache.mark_needs_auth("server-2");
    assert_eq!(cache.len(), 2);

    // Clear and verify recovery state
    cache.clear_all();
    assert!(cache.is_empty());

    // Re-mark (simulating re-discovery)
    cache.mark_needs_auth("server-1");
    assert!(cache.is_cached("server-1"));
}

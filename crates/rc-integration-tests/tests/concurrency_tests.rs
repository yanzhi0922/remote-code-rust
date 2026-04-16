//! Concurrency tests.
//!
//! Tests concurrent operations across multiple agents including
//! mailbox operations, permission requests, and TeamFile operations.

use std::sync::Arc;

// ─── Concurrent mailbox message creation ────────────────────────────────────

#[tokio::test]
async fn concurrent_mailbox_message_creation() {
    let messages: Vec<rc_swarm::MailboxMessage> = (0..100)
        .map(|i| {
            rc_swarm::MailboxMessage::new(
                "lead",
                format!("worker-{i}"),
                rc_swarm::MailboxMessageType::TaskAssignment,
                format!("Task {i}"),
            )
        })
        .collect();

    assert_eq!(messages.len(), 100);

    // All messages should have unique IDs
    let ids: std::collections::HashSet<String> = messages.iter().map(|m| m.id.clone()).collect();
    assert_eq!(ids.len(), 100);
}

#[tokio::test]
async fn concurrent_mailbox_file_operations() {
    let dir = tempfile::tempdir().expect("tempdir");
    rc_swarm::team_helpers::set_base_dir_override(Some(dir.path().to_path_buf()));

    let team = rc_swarm::TeamFile::new("concurrent-mail", "lead-1");
    rc_swarm::team_helpers::create_team(&team)
        .await
        .expect("should create team");

    // Send messages from multiple "agents" concurrently
    let mut handles = vec![];
    for i in 0..10 {
        let team_name = "concurrent-mail".to_owned();
        handles.push(tokio::spawn(async move {
            let msg = rc_swarm::MailboxMessage::new(
                format!("sender-{i}"),
                "receiver",
                rc_swarm::MailboxMessageType::Text,
                format!("Hello from sender {i}"),
            );
            rc_swarm::mailbox::send_message(&team_name, &msg).await
        }));
    }

    // Wait for all sends
    for handle in handles {
        handle.await.expect("task should complete").expect("send should succeed");
    }

    // Verify all messages were received
    let messages = rc_swarm::mailbox::read_messages("concurrent-mail", "receiver")
        .await
        .expect("should read messages");
    assert_eq!(messages.len(), 10);

    rc_swarm::team_helpers::set_base_dir_override(None);
}

// ─── Concurrent permission requests ─────────────────────────────────────────

#[tokio::test]
async fn concurrent_permission_requests() {
    let dir = tempfile::tempdir().expect("tempdir");
    rc_swarm::team_helpers::set_base_dir_override(Some(dir.path().to_path_buf()));

    let team = rc_swarm::TeamFile::new("concurrent-perm", "lead-1");
    rc_swarm::team_helpers::create_team(&team)
        .await
        .expect("should create team");

    // Write permission requests concurrently
    let mut handles = vec![];
    for i in 0..10 {
        let team_name = "concurrent-perm".to_owned();
        handles.push(tokio::spawn(async move {
            let request = rc_swarm::SwarmPermissionRequest::new(
                &team_name,
                format!("worker-{i}"),
                "write_file",
                serde_json::json!({"path": format!("/tmp/file-{i}.rs")}),
            );
            rc_swarm::permission_sync::write_request(&team_name, &request).await
        }));
    }

    for handle in handles {
        handle.await.expect("task should complete").expect("write should succeed");
    }

    // Verify all requests were written
    let pending = rc_swarm::permission_sync::list_pending_requests("concurrent-perm")
        .await
        .expect("should list pending");
    assert_eq!(pending.len(), 10);

    rc_swarm::team_helpers::set_base_dir_override(None);
}

// ─── Concurrent credential pool access ──────────────────────────────────────

#[test]
fn concurrent_credential_pool_rotation() {
    let pool = Arc::new(rc_provider::credential_pool::CredentialPool::from_keys(vec![
        "key-1".to_owned(),
        "key-2".to_owned(),
        "key-3".to_owned(),
    ]));

    let mut handles = vec![];
    for _ in 0..10 {
        let pool = Arc::clone(&pool);
        handles.push(std::thread::spawn(move || {
            let mut results = Vec::new();
            for _ in 0..100 {
                let cred = pool.next().expect("should have credential");
                results.push(cred.api_key.clone());
            }
            results
        }));
    }

    let all_results: Vec<Vec<String>> = handles
        .into_iter()
        .map(|h| h.join().expect("thread should succeed"))
        .collect();

    // Verify all threads got valid keys
    for results in &all_results {
        assert_eq!(results.len(), 100);
        for key in results {
            assert!(key == "key-1" || key == "key-2" || key == "key-3");
        }
    }
}

// ─── Concurrent circuit breaker access ──────────────────────────────────────

#[test]
fn concurrent_circuit_breaker_access() {
    let cb = Arc::new(rc_provider::CircuitBreaker::new(rc_provider::CircuitBreakerConfig {
        failure_threshold: 100,
        recovery_timeout: std::time::Duration::from_secs(60),
        half_open_max_probes: 10,
    }));

    let mut handles = vec![];
    for _ in 0..10 {
        let cb = Arc::clone(&cb);
        handles.push(std::thread::spawn(move || {
            let mut successes = 0;
            let mut failures = 0;
            for i in 0..100 {
                if i % 3 == 0 {
                    cb.record_failure();
                    failures += 1;
                } else {
                    cb.record_success();
                    successes += 1;
                }
            }
            (successes, failures)
        }));
    }

    for handle in handles {
        let (successes, failures) = handle.join().expect("thread should succeed");
        assert!(successes > 0);
        assert!(failures > 0);
    }

    // Circuit should still be closed (threshold is 100, we recorded ~333 failures but also successes)
    assert!(cb.allow_request().is_ok());
}

// ─── Concurrent MCP discovery ───────────────────────────────────────────────

#[test]
fn concurrent_mcp_discovery_access() {
    let discovery = Arc::new(std::sync::Mutex::new(rc_mcp::McpDiscovery::new()));

    let mut handles = vec![];
    for i in 0..10 {
        let discovery = Arc::clone(&discovery);
        handles.push(std::thread::spawn(move || {
            let mut disc = discovery.lock().expect("should lock");
            disc.store(
                &format!("server-{i}"),
                vec![rc_mcp::McpToolDescriptor {
                    name: format!("tool-{i}"),
                    title: None,
                    description: Some(format!("Tool {i}")),
                    input_schema: serde_json::Value::Null,
                    annotations: serde_json::Value::Null,
                }],
                vec![],
                None,
            );
        }));
    }

    for handle in handles {
        handle.join().expect("thread should succeed");
    }

    let disc = discovery.lock().expect("should lock");
    assert_eq!(disc.server_count(), 10);
    assert_eq!(disc.total_tool_count(), 10);
}

// ─── Concurrent TeamFile read/write ─────────────────────────────────────────

#[tokio::test]
async fn concurrent_teamfile_operations() {
    let dir = tempfile::tempdir().expect("tempdir");
    rc_swarm::team_helpers::set_base_dir_override(Some(dir.path().to_path_buf()));

    // Create team
    let team = rc_swarm::TeamFile::new("concurrent-team", "lead-1");
    rc_swarm::team_helpers::create_team(&team)
        .await
        .expect("should create team");

    // Add members sequentially (file-based, so sequential is safer)
    for i in 0..5 {
        let member = rc_swarm::TeamMember::new(
            format!("agent-{i}"),
            format!("worker-{i}"),
            format!("pane-{i}"),
            "/tmp/workdir",
        );
        rc_swarm::team_helpers::add_member("concurrent-team", member)
            .await
            .expect("should add member");
    }

    // Read from multiple concurrent tasks
    let mut handles = vec![];
    for _ in 0..5 {
        handles.push(tokio::spawn(async {
            rc_swarm::team_helpers::read_team("concurrent-team").await
        }));
    }

    for handle in handles {
        let team = handle.await.expect("task should complete").expect("should read team");
        assert_eq!(team.members.len(), 5);
    }

    rc_swarm::team_helpers::set_base_dir_override(None);
}

// ─── Concurrent session store access ────────────────────────────────────────

#[test]
fn concurrent_session_store_writes() {
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

    let store = Arc::new(rc_session::SessionStore::open(paths).expect("store should open"));

    // Create sessions concurrently
    let mut handles = vec![];
    for i in 0..5 {
        let store = Arc::clone(&store);
        handles.push(std::thread::spawn(move || {
            let session_id = uuid::Uuid::new_v4();
            store
                .ensure_session(
                    session_id,
                    std::path::Path::new("/tmp"),
                    "anthropic",
                    None,
                    Some(&format!("Concurrent Session {i}")),
                )
                .expect("should create session");
            session_id
        }));
    }

    let session_ids: Vec<uuid::Uuid> = handles
        .into_iter()
        .map(|h| h.join().expect("thread should succeed"))
        .collect();

    assert_eq!(session_ids.len(), 5);

    // Verify all sessions exist
    let sessions = store.list_sessions().expect("should list");
    assert_eq!(sessions.len(), 5);
}

// ─── Concurrent denial tracker ──────────────────────────────────────────────

#[test]
fn concurrent_denial_tracker() {
    let tracker = Arc::new(std::sync::Mutex::new(rc_permissions::DenialTracker::new()));

    let mut handles = vec![];
    for i in 0..10 {
        let tracker = Arc::clone(&tracker);
        handles.push(std::thread::spawn(move || {
            let mut t = tracker.lock().expect("should lock");
            t.record_denial(&format!("tool-{i}"), "test denial");
        }));
    }

    for handle in handles {
        handle.join().expect("thread should succeed");
    }

    let t = tracker.lock().expect("should lock");
    assert_eq!(t.total_denials(), 10);
}

//! Stress tests for performance validation.
//!
//! All tests are marked #[ignore] to avoid CI timeouts.
//! Run locally with: cargo test -p rc-integration-tests -- --ignored

use std::collections::BTreeMap;

// ─── Large TeamFile member management ───────────────────────────────────────

#[test]
#[ignore]
fn stress_large_team_member_management() {
    let mut team = claude_swarm::TeamFile::new("stress-team", "lead-1");

    // Add 1000 members
    for i in 0..1000 {
        let member = claude_swarm::TeamMember::new(
            format!("agent-{i}"),
            format!("worker-{i}"),
            format!("pane-{i}"),
            "/tmp/workdir",
        );
        team.members.push(member);
    }

    assert_eq!(team.members.len(), 1000);

    // Verify lookup performance
    assert!(team.has_member("worker-500"));
    assert!(!team.has_member("worker-9999"));

    // Verify find
    let found = team.find_member("worker-250");
    assert!(found.is_some());
    assert_eq!(found.expect("should find").agent_id, "agent-250");

    // Remove half the members
    for i in 0..500 {
        let removed = team.remove_member(&format!("worker-{i}"));
        assert!(removed.is_some());
    }
    assert_eq!(team.members.len(), 500);
}

// ─── Large mailbox message processing ───────────────────────────────────────

#[test]
#[ignore]
fn stress_large_mailbox_messages() {
    // Create 10,000 messages
    let mut messages: Vec<claude_swarm::MailboxMessage> = Vec::with_capacity(10000);
    for i in 0..10000 {
        let msg = claude_swarm::MailboxMessage::new(
            "lead",
            "worker-1",
            claude_swarm::MailboxMessageType::Text,
            format!("Message number {i}"),
        );
        messages.push(msg);
    }

    assert_eq!(messages.len(), 10000);

    // Serialize all messages
    let mut json_size = 0usize;
    for msg in &messages {
        let json = serde_json::to_string(msg).expect("should serialize");
        json_size += json.len();
    }
    assert!(json_size > 0);

    // Deserialize all messages
    for msg in &messages {
        let json = serde_json::to_string(msg).expect("should serialize");
        let _: claude_swarm::MailboxMessage = serde_json::from_str(&json).expect("should deserialize");
    }
}

// ─── Large permission request batch ─────────────────────────────────────────

#[test]
#[ignore]
fn stress_large_permission_request_batch() {
    let mut requests: Vec<claude_swarm::SwarmPermissionRequest> = Vec::with_capacity(1000);
    for i in 0..1000 {
        let req = claude_swarm::SwarmPermissionRequest::new(
            "stress-team",
            "worker-1",
            "write_file",
            serde_json::json!({"path": format!("/tmp/file-{i}.rs"), "content": "fn main() {}"}),
        );
        requests.push(req);
    }

    assert_eq!(requests.len(), 1000);

    // Resolve all requests
    for req in &mut requests {
        req.resolve(claude_swarm::PermissionDecision::Allow, None);
        assert!(req.is_resolved());
    }
}

// ─── Session large message serialization ────────────────────────────────────

#[test]
#[ignore]
fn stress_session_large_conversation_serialization() {
    let mut entries: Vec<claude_core::ConversationEntry> = Vec::with_capacity(10000);

    // Create 10,000 conversation entries
    for i in 0..5000 {
        entries.push(claude_core::ConversationEntry::user(format!(
            "User message {i}"
        )));
        entries.push(claude_core::ConversationEntry::assistant(format!(
            "Assistant response {i}"
        )));
    }

    assert_eq!(entries.len(), 10000);

    // Serialize the entire conversation
    let json = serde_json::to_string(&entries).expect("should serialize 10k entries");
    assert!(!json.is_empty());

    // Deserialize
    let decoded: Vec<claude_core::ConversationEntry> =
        serde_json::from_str(&json).expect("should deserialize 10k entries");
    assert_eq!(decoded.len(), 10000);
}

// ─── Large transcript storage ───────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn stress_transcript_large_write_read() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("stress-test.jsonl");
    let storage = claude_transcript::TranscriptStorage::new(&path);
    let session_id = uuid::Uuid::new_v4();

    // Write 5000 entries
    for i in 0..5000 {
        let entry = if i % 3 == 0 {
            claude_transcript::TranscriptEntry::conversation_now(
                session_id,
                claude_core::ConversationEntry::user(format!("Message {i}")),
            )
        } else if i % 3 == 1 {
            claude_transcript::TranscriptEntry::named_event_now(
                session_id,
                "tool_call",
                Some(serde_json::json!({"index": i})),
            )
        } else {
            claude_transcript::TranscriptEntry::compact_boundary_now(
                session_id,
                claude_transcript::CompactBoundary::new(
                    claude_transcript::CompactTrigger::Auto,
                    100000 + i,
                ),
            )
        };
        storage.append(&entry).await.expect("should append entry");
    }

    // Read all entries back
    let entries = storage.read_all().await.expect("should read all entries");
    assert_eq!(entries.len(), 5000);
}

// ─── Large MCP config ───────────────────────────────────────────────────────

#[test]
#[ignore]
fn stress_large_mcp_config() {
    let mut servers = BTreeMap::new();
    for i in 0..100 {
        servers.insert(
            format!("server-{i}"),
            claude_mcp::McpServerConfig {
                name: format!("server-{i}"),
                enabled: true,
                transport: claude_mcp::McpTransportConfig::Stdio {
                    command: format!("cmd-{i}"),
                    args: vec![],
                    cwd: None,
                    env: BTreeMap::new(),
                },
                capabilities: claude_mcp::McpCapabilityMatrix {
                    supports_tools: true,
                    supports_prompts: i % 2 == 0,
                    supports_resources: i % 3 == 0,
                    supports_sampling: false,
                    supports_roots: false,
                },
                startup_timeout_secs: None,
                request_timeout_secs: None,
                oauth: None,
                metadata: BTreeMap::new(),
            },
        );
    }

    let config = claude_mcp::McpConfig { servers };
    assert_eq!(config.servers.len(), 100);

    // Serialize and deserialize
    let json = serde_json::to_string(&config).expect("should serialize large config");
    let decoded: claude_mcp::McpConfig =
        serde_json::from_str(&json).expect("should deserialize large config");
    assert_eq!(decoded.servers.len(), 100);
}

// ─── Large provider response with many tool calls ───────────────────────────

#[test]
#[ignore]
fn stress_large_provider_response() {
    let mut tool_calls = Vec::with_capacity(1000);
    for i in 0..1000 {
        tool_calls.push(claude_core::ToolCall {
            id: format!("tc-{i}"),
            name: "read_file".to_owned(),
            input: serde_json::json!({"path": format!("/tmp/file-{i}.rs")}),
        });
    }

    let response = claude_core::ProviderResponse {
        text: "Processing files".to_owned(),
        history_text: None,
        thinking: None,
        content_blocks: vec![],
        tool_calls,
        request_id: Some("req-stress".to_owned()),
        usage: claude_core::UsageSummary {
            input_tokens: 100000,
            output_tokens: 50000,
            cache_read_input_tokens: 10000,
            cache_creation_input_tokens: 5000,
            ..Default::default()
        },
        stop_reason: "tool_use".to_owned(),
        research: None,
    };

    let json = serde_json::to_string(&response).expect("should serialize");
    let decoded: claude_core::ProviderResponse =
        serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(decoded.tool_calls.len(), 1000);
}

// ─── Credential pool stress ─────────────────────────────────────────────────

#[test]
#[ignore]
fn stress_credential_pool_rotation() {
    let keys: Vec<String> = (0..100).map(|i| format!("key-{i}")).collect();
    let pool = claude_provider::credential_pool::CredentialPool::from_keys(keys);

    // Rotate 100,000 times
    for _ in 0..100_000 {
        let cred = pool.next().expect("should have credential");
        assert!(!cred.api_key.is_empty());
    }
}

// ─── State machine stress ───────────────────────────────────────────────────

#[test]
#[ignore]
fn stress_state_machine_transitions() {
    let mut sm = claude_query_engine::state_machine::StateMachine::new();

    // Run 1000 full lifecycles
    for _ in 0..1000 {
        sm.transition(claude_query_engine::state_machine::EnginePhase::Initializing)
            .expect("ok");
        sm.transition(claude_query_engine::state_machine::EnginePhase::BuildingPrompt)
            .expect("ok");
        sm.transition(claude_query_engine::state_machine::EnginePhase::CallingProvider)
            .expect("ok");
        sm.transition(claude_query_engine::state_machine::EnginePhase::ProcessingResponse)
            .expect("ok");
        sm.transition(claude_query_engine::state_machine::EnginePhase::Finalizing)
            .expect("ok");
        sm.transition(claude_query_engine::state_machine::EnginePhase::Idle)
            .expect("ok");
    }

    // Transitions should have 6000 entries (6 transitions × 1000 cycles)
    assert_eq!(sm.transitions().len(), 6000);
}

// ─── Failure tracker stress ─────────────────────────────────────────────────

#[test]
#[ignore]
fn stress_failure_tracker() {
    let mut tracker = claude_query_engine::failure_tracker::FailureTracker::new(
        100,
        std::time::Duration::from_secs(1),
    );

    // Record 10,000 failures and successes
    for i in 0..10000 {
        if i % 2 == 0 {
            tracker.record_failure();
        } else {
            tracker.record_success();
        }
    }

    assert_eq!(tracker.total_failures(), 5000);
    assert_eq!(tracker.total_successes(), 5000);
}
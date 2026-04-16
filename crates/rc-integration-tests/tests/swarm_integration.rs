//! Swarm integration tests.
//!
//! Tests TeamFile CRUD, permission sync, mailbox operations,
//! backend registry, leader bridge, and reconnection.


// ─── TeamFile CRUD ──────────────────────────────────────────────────────────

#[tokio::test]
async fn team_file_create_read_update_delete() {
    let dir = tempfile::tempdir().expect("tempdir");
    rc_swarm::team_helpers::set_base_dir_override(Some(dir.path().to_path_buf()));

    let team = rc_swarm::TeamFile::new("test-team", "lead-1");

    // Create
    rc_swarm::team_helpers::create_team(&team)
        .await
        .expect("should create team");

    // Read
    let loaded = rc_swarm::team_helpers::read_team("test-team")
        .await
        .expect("should read team");
    assert_eq!(loaded.name, "test-team");
    assert_eq!(loaded.lead_agent_id, "lead-1");

    // Update (add member)
    let member = rc_swarm::TeamMember::new(
        "agent-1",
        "worker-1",
        "pane-0",
        "/tmp/workdir",
    );
    rc_swarm::team_helpers::add_member("test-team", member)
        .await
        .expect("should add member");

    let updated = rc_swarm::team_helpers::read_team("test-team")
        .await
        .expect("should read updated team");
    assert_eq!(updated.members.len(), 1);
    assert!(updated.has_member("worker-1"));

    // Delete
    rc_swarm::team_helpers::delete_team("test-team")
        .await
        .expect("should delete team");

    let result = rc_swarm::team_helpers::read_team("test-team").await;
    assert!(result.is_err());

    rc_swarm::team_helpers::set_base_dir_override(None);
}

#[tokio::test]
async fn team_file_member_management() {
    let dir = tempfile::tempdir().expect("tempdir");
    rc_swarm::team_helpers::set_base_dir_override(Some(dir.path().to_path_buf()));

    let team = rc_swarm::TeamFile::new("member-test", "lead-1");
    rc_swarm::team_helpers::create_team(&team)
        .await
        .expect("should create team");

    // Add multiple members
    for i in 0..3 {
        let member = rc_swarm::TeamMember::new(
            format!("agent-{i}"),
            format!("worker-{i}"),
            format!("pane-{i}"),
            "/tmp/workdir",
        );
        rc_swarm::team_helpers::add_member("member-test", member)
            .await
            .expect("should add member");
    }

    let loaded = rc_swarm::team_helpers::read_team("member-test")
        .await
        .expect("should read team");
    assert_eq!(loaded.members.len(), 3);

    // Remove a member
    rc_swarm::team_helpers::remove_member("member-test", "worker-1")
        .await
        .expect("should remove member");

    let loaded = rc_swarm::team_helpers::read_team("member-test")
        .await
        .expect("should read team");
    assert_eq!(loaded.members.len(), 2);
    assert!(!loaded.has_member("worker-1"));

    rc_swarm::team_helpers::set_base_dir_override(None);
}

// ─── Team name validation ───────────────────────────────────────────────────

#[test]
fn team_name_validation() {
    assert!(rc_swarm::team_helpers::validate_team_name("my-team").is_ok());
    assert!(rc_swarm::team_helpers::validate_team_name("team_123").is_ok());
    assert!(rc_swarm::team_helpers::validate_team_name("a").is_ok());
    assert!(rc_swarm::team_helpers::validate_team_name("").is_err());
    assert!(rc_swarm::team_helpers::validate_team_name("-invalid").is_err());
    assert!(rc_swarm::team_helpers::validate_team_name(&"x".repeat(65)).is_err());
}

#[test]
fn team_name_sanitization() {
    assert_eq!(rc_swarm::team_helpers::sanitize_team_name("my team"), "my_team");
    assert_eq!(rc_swarm::team_helpers::sanitize_team_name("test/path"), "test_path");
    assert_eq!(rc_swarm::team_helpers::sanitize_team_name("valid-name"), "valid-name");
}

// ─── Permission sync ────────────────────────────────────────────────────────

#[tokio::test]
async fn permission_request_response_cycle() {
    let dir = tempfile::tempdir().expect("tempdir");
    rc_swarm::team_helpers::set_base_dir_override(Some(dir.path().to_path_buf()));

    let team = rc_swarm::TeamFile::new("perm-test", "lead-1");
    rc_swarm::team_helpers::create_team(&team)
        .await
        .expect("should create team");

    // Write a permission request
    let request = rc_swarm::SwarmPermissionRequest::new(
        "perm-test",
        "worker-1",
        "write_file",
        serde_json::json!({"path": "/tmp/test.rs"}),
    );
    rc_swarm::permission_sync::write_request("perm-test", &request)
        .await
        .expect("should write request");

    // Read the request back
    let loaded = rc_swarm::permission_sync::read_request("perm-test", &request.request_id)
        .await
        .expect("should read request");
    assert_eq!(loaded.tool_name, "write_file");
    assert!(!loaded.is_resolved());

    // Write a response (allow)
    rc_swarm::permission_sync::write_response(
        "perm-test",
        &request.request_id,
        rc_swarm::PermissionDecision::Allow,
        Some("safe file".to_owned()),
    )
    .await
    .expect("should write response");

    // Read the response
    let response = rc_swarm::permission_sync::read_response("perm-test", &request.request_id)
        .await
        .expect("should read response");
    assert!(response.is_resolved());
    assert_eq!(response.decision, Some(rc_swarm::PermissionDecision::Allow));

    rc_swarm::team_helpers::set_base_dir_override(None);
}

// ─── Mailbox operations ─────────────────────────────────────────────────────

#[tokio::test]
async fn mailbox_send_read_mark_delete() {
    let dir = tempfile::tempdir().expect("tempdir");
    rc_swarm::team_helpers::set_base_dir_override(Some(dir.path().to_path_buf()));

    let team = rc_swarm::TeamFile::new("mail-test", "lead-1");
    rc_swarm::team_helpers::create_team(&team)
        .await
        .expect("should create team");

    // Send messages
    let msg1 = rc_swarm::MailboxMessage::new(
        "lead",
        "worker-1",
        rc_swarm::MailboxMessageType::TaskAssignment,
        "Fix the bug",
    );
    let msg2 = rc_swarm::MailboxMessage::new(
        "lead",
        "worker-1",
        rc_swarm::MailboxMessageType::Text,
        "How's it going?",
    );

    rc_swarm::mailbox::send_message("mail-test", &msg1)
        .await
        .expect("should send msg1");
    rc_swarm::mailbox::send_message("mail-test", &msg2)
        .await
        .expect("should send msg2");

    // Read all messages
    let messages = rc_swarm::mailbox::read_messages("mail-test", "worker-1")
        .await
        .expect("should read messages");
    assert_eq!(messages.len(), 2);

    // Read unread messages
    let unread = rc_swarm::mailbox::read_unread_messages("mail-test", "worker-1")
        .await
        .expect("should read unread");
    assert_eq!(unread.len(), 2);

    // Mark as read
    rc_swarm::mailbox::mark_message_read("mail-test", "worker-1", &msg1.id)
        .await
        .expect("should mark read");

    let unread = rc_swarm::mailbox::read_unread_messages("mail-test", "worker-1")
        .await
        .expect("should read unread");
    assert_eq!(unread.len(), 1);

    // Delete message
    rc_swarm::mailbox::delete_message("mail-test", "worker-1", &msg2.id)
        .await
        .expect("should delete message");

    let messages = rc_swarm::mailbox::read_messages("mail-test", "worker-1")
        .await
        .expect("should read messages");
    assert_eq!(messages.len(), 1);

    rc_swarm::team_helpers::set_base_dir_override(None);
}

// ─── Backend registry ───────────────────────────────────────────────────────

#[test]
fn backend_registry_with_defaults() {
    let registry = rc_swarm::backends::registry::BackendRegistry::with_defaults("test-team");
    let names = registry.backend_names();
    assert!(names.contains(&"in_process"));
    assert!(registry.count() >= 1);
}

#[test]
fn backend_registry_find_by_type() {
    let registry = rc_swarm::backends::registry::BackendRegistry::with_defaults("test-team");
    let found = registry.find(rc_swarm::BackendType::InProcess);
    assert!(found.is_some());
}

#[test]
fn backend_type_from_str() {
    assert_eq!(
        rc_swarm::BackendType::from_str_opt("in_process"),
        Some(rc_swarm::BackendType::InProcess)
    );
    assert_eq!(
        rc_swarm::BackendType::from_str_opt("tmux"),
        Some(rc_swarm::BackendType::Tmux)
    );
    assert_eq!(
        rc_swarm::BackendType::from_str_opt("iterm2"),
        Some(rc_swarm::BackendType::ITerm2)
    );
    assert_eq!(rc_swarm::BackendType::from_str_opt("unknown"), None);
}

// ─── Leader bridge ──────────────────────────────────────────────────────────

#[test]
fn leader_bridge_auto_approve_rules() {
    // Read-only tools should be auto-approved
    assert!(rc_swarm::leader_bridge::should_auto_approve("read"));
    assert!(rc_swarm::leader_bridge::should_auto_approve("search"));
    assert!(rc_swarm::leader_bridge::should_auto_approve("glob"));
    assert!(rc_swarm::leader_bridge::should_auto_approve("list"));
    assert!(rc_swarm::leader_bridge::should_auto_approve("info"));
    assert!(rc_swarm::leader_bridge::should_auto_approve("status"));

    // Write tools should not be auto-approved
    assert!(!rc_swarm::leader_bridge::should_auto_approve("write"));
    assert!(!rc_swarm::leader_bridge::should_auto_approve("bash"));
    assert!(!rc_swarm::leader_bridge::should_auto_approve("edit"));
}

#[test]
fn leader_bridge_default_decision() {
    let request = rc_swarm::SwarmPermissionRequest::new(
        "team",
        "worker",
        "read",
        serde_json::json!({}),
    );
    let (decision, _reason) = rc_swarm::leader_bridge::default_decision(&request);
    assert_eq!(decision, rc_swarm::PermissionDecision::Allow);

    let write_request = rc_swarm::SwarmPermissionRequest::new(
        "team",
        "worker",
        "write",
        serde_json::json!({}),
    );
    let (decision, _reason) = rc_swarm::leader_bridge::default_decision(&write_request);
    assert_eq!(decision, rc_swarm::PermissionDecision::Deny);
}

#[test]
fn bridge_status_default() {
    let status = rc_swarm::leader_bridge::BridgeStatus::default();
    assert_eq!(status.pending_count, 0);
}

// ─── Terminal detection ─────────────────────────────────────────────────────

#[test]
fn terminal_environment_detect() {
    let env = rc_swarm::detection::TerminalEnvironment::detect();
    // Just verify it doesn't panic and returns a valid description
    let desc = env.description();
    assert!(!desc.is_empty());
}

// ─── Teammate state ─────────────────────────────────────────────────────────

#[test]
fn teammate_identity_serialization() {
    let identity = rc_swarm::TeammateIdentity {
        agent_id: "agent-1".to_owned(),
        name: "worker-1".to_owned(),
        team_name: "test-team".to_owned(),
        is_lead: false,
        lead_agent_id: "lead-1".to_owned(),
        backend_type: rc_swarm::BackendType::InProcess,
    };
    let json = serde_json::to_string(&identity).expect("serialize");
    let decoded: rc_swarm::TeammateIdentity =
        serde_json::from_str(&json).expect("deserialize");
    assert_eq!(decoded.agent_id, "agent-1");
    assert!(!decoded.is_lead);
}

// ─── Spawn config ───────────────────────────────────────────────────────────

#[test]
fn spawn_config_construction() {
    let config = rc_swarm::SpawnConfig {
        agent_id: "agent-1".to_owned(),
        agent_name: "worker-1".to_owned(),
        team_name: "team-1".to_owned(),
        model: None,
        cwd: "/tmp".to_owned(),
        backend_type: rc_swarm::BackendType::InProcess,
        env_vars: vec![],
        permission_mode: None,
        worktree_path: None,
    };
    assert_eq!(config.agent_name, "worker-1");
    assert_eq!(config.backend_type, rc_swarm::BackendType::InProcess);

    // Verify serialization round-trip
    let json = serde_json::to_string(&config).expect("should serialize");
    let decoded: rc_swarm::SpawnConfig =
        serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(decoded, config);
}

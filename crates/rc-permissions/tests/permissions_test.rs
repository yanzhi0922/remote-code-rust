use rc_permissions::{
    LayeredPermissionBroker, PermissionBroker, PermissionClass, PermissionDecision,
    PermissionRequest, RuleAction, RuleSource, SourceAwarePermissionRule, StaticPermissionBroker,
    classify_tool, rule_matches_pattern,
};

// ── classify_tool tests ───────────────────────────────────────

#[test]
fn classify_tool_returns_read_for_read() {
    assert_eq!(classify_tool("Read"), PermissionClass::Read);
}

#[test]
fn classify_tool_returns_read_for_glob() {
    assert_eq!(classify_tool("Glob"), PermissionClass::Read);
}

#[test]
fn classify_tool_returns_read_for_ls() {
    assert_eq!(classify_tool("LS"), PermissionClass::Read);
}

#[test]
fn classify_tool_returns_edit_for_edit() {
    assert_eq!(classify_tool("Edit"), PermissionClass::Edit);
}

#[test]
fn classify_tool_returns_edit_for_write() {
    assert_eq!(classify_tool("Write"), PermissionClass::Edit);
}

#[test]
fn classify_tool_returns_edit_for_multi_edit() {
    assert_eq!(classify_tool("MultiEdit"), PermissionClass::Edit);
}

#[test]
fn classify_tool_returns_bash_for_bash() {
    assert_eq!(classify_tool("Bash"), PermissionClass::Bash);
}

#[test]
fn classify_tool_returns_mcp_for_mcp() {
    assert_eq!(classify_tool("mcp"), PermissionClass::Mcp);
}

#[test]
fn classify_tool_returns_agent_for_agent() {
    assert_eq!(classify_tool("Agent"), PermissionClass::Agent);
}

#[test]
fn classify_tool_returns_read_for_unknown() {
    assert_eq!(classify_tool("custom_tool"), PermissionClass::Read);
}

// ── PermissionDecision tests ──────────────────────────────────

#[test]
fn permission_decision_allow() {
    let decision = PermissionDecision::allow();
    assert!(decision.allowed);
    assert!(decision.message.is_none());
}

#[test]
fn permission_decision_deny() {
    let decision = PermissionDecision::deny("test reason");
    assert!(!decision.allowed);
    assert_eq!(decision.message.as_deref(), Some("test reason"));
}

// ── StaticPermissionBroker tests ──────────────────────────────

#[tokio::test]
async fn static_broker_allow_all_permits_read() {
    let broker = StaticPermissionBroker::new(true);
    let decision = broker
        .decide(PermissionRequest {
            tool_name: "Read".to_owned(),
            tool_input: serde_json::json!({"path": "/tmp/a"}),
            working_directory: None,
            tool_use_id: None,
            title: None,
            description: None,
            blocked_path: None,
        })
        .await;
    assert!(decision.allowed);
}

#[tokio::test]
async fn static_broker_deny_all_rejects_edit() {
    let broker = StaticPermissionBroker::new(false);
    let decision = broker
        .decide(PermissionRequest {
            tool_name: "Edit".to_owned(),
            tool_input: serde_json::json!({"path": "/tmp/a"}),
            working_directory: None,
            tool_use_id: None,
            title: None,
            description: None,
            blocked_path: None,
        })
        .await;
    assert!(!decision.allowed);
}

#[tokio::test]
async fn static_broker_allow_all_permits_bash() {
    let broker = StaticPermissionBroker::new(true);
    let decision = broker
        .decide(PermissionRequest {
            tool_name: "Bash".to_owned(),
            tool_input: serde_json::json!({"command": "ls"}),
            working_directory: None,
            tool_use_id: None,
            title: None,
            description: None,
            blocked_path: None,
        })
        .await;
    assert!(decision.allowed);
}

// ── LayeredPermissionBroker tests ─────────────────────────────

#[tokio::test]
async fn layered_broker_falls_through_to_fallback() {
    let fallback = StaticPermissionBroker::new(true);
    let layered = LayeredPermissionBroker::new(fallback, vec![]);
    let decision = layered
        .decide(PermissionRequest {
            tool_name: "Read".to_owned(),
            tool_input: serde_json::json!({"path": "/tmp/a"}),
            working_directory: None,
            tool_use_id: None,
            title: None,
            description: None,
            blocked_path: None,
        })
        .await;
    assert!(decision.allowed);
}

#[tokio::test]
async fn layered_broker_deny_rule_overrides_fallback() {
    let fallback = StaticPermissionBroker::new(true);
    let rules = vec![SourceAwarePermissionRule {
        tool_pattern: "Bash".to_owned(),
        action: RuleAction::Deny,
        source: RuleSource::Project,
    }];
    let layered = LayeredPermissionBroker::new(fallback, rules);
    let decision = layered
        .decide(PermissionRequest {
            tool_name: "Bash".to_owned(),
            tool_input: serde_json::json!({"command": "ls"}),
            working_directory: None,
            tool_use_id: None,
            title: None,
            description: None,
            blocked_path: None,
        })
        .await;
    assert!(!decision.allowed);
}

#[tokio::test]
async fn layered_broker_allow_rule_overrides_deny_fallback() {
    let fallback = StaticPermissionBroker::new(false);
    let rules = vec![SourceAwarePermissionRule {
        tool_pattern: "Read".to_owned(),
        action: RuleAction::Allow,
        source: RuleSource::User,
    }];
    let layered = LayeredPermissionBroker::new(fallback, rules);
    let decision = layered
        .decide(PermissionRequest {
            tool_name: "Read".to_owned(),
            tool_input: serde_json::json!({"path": "/tmp/a"}),
            working_directory: None,
            tool_use_id: None,
            title: None,
            description: None,
            blocked_path: None,
        })
        .await;
    assert!(decision.allowed);
}

#[tokio::test]
async fn layered_broker_session_rule_highest_priority() {
    let fallback = StaticPermissionBroker::new(false);
    // Persistent rule allows Bash
    let rules = vec![SourceAwarePermissionRule {
        tool_pattern: "Bash".to_owned(),
        action: RuleAction::Allow,
        source: RuleSource::User,
    }];
    let layered = LayeredPermissionBroker::new(fallback, rules);
    // Session rule denies Bash
    layered
        .add_session_rule(RuleAction::Deny, "Bash".to_owned())
        .unwrap();
    let decision = layered
        .decide(PermissionRequest {
            tool_name: "Bash".to_owned(),
            tool_input: serde_json::json!({"command": "ls"}),
            working_directory: None,
            tool_use_id: None,
            title: None,
            description: None,
            blocked_path: None,
        })
        .await;
    assert!(!decision.allowed);
}

#[test]
fn layered_broker_clear_session_rules_returns_count() {
    let fallback = StaticPermissionBroker::new(true);
    let layered = LayeredPermissionBroker::new(fallback, vec![]);
    layered
        .add_session_rule(RuleAction::Allow, "Read".to_owned())
        .unwrap();
    layered
        .add_session_rule(RuleAction::Deny, "Bash".to_owned())
        .unwrap();
    let count = layered.clear_session_rules().unwrap();
    assert_eq!(count, 2);
}

#[tokio::test]
async fn layered_broker_tracks_audit_records() {
    let fallback = StaticPermissionBroker::new(true);
    let rules = vec![SourceAwarePermissionRule {
        tool_pattern: "Read".to_owned(),
        action: RuleAction::Allow,
        source: RuleSource::Project,
    }];
    let layered = LayeredPermissionBroker::new(fallback, rules);
    let _ = layered
        .decide(PermissionRequest {
            tool_name: "Read".to_owned(),
            tool_input: serde_json::json!({"path": "/tmp"}),
            working_directory: None,
            tool_use_id: None,
            title: None,
            description: None,
            blocked_path: None,
        })
        .await;
    let records = layered.audit_records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].tool_name, "Read");
    assert!(records[0].final_allowed);
}

// ── rule_matches_pattern tests ────────────────────────────────

#[test]
fn rule_matches_pattern_exact_match() {
    assert!(rule_matches_pattern("Bash", "Bash"));
    assert!(!rule_matches_pattern("Bash", "Read"));
}

#[test]
fn rule_matches_pattern_star_matches_everything() {
    assert!(rule_matches_pattern("*", "anything"));
}

#[test]
fn rule_matches_pattern_prefix_wildcard() {
    assert!(rule_matches_pattern("Read*", "ReadFile"));
    assert!(!rule_matches_pattern("Read*", "WriteFile"));
}

#[test]
fn rule_matches_pattern_suffix_wildcard() {
    assert!(rule_matches_pattern("*File", "ReadFile"));
    assert!(!rule_matches_pattern("*File", "ReadDir"));
}

#[test]
fn rule_matches_pattern_middle_wildcard() {
    assert!(rule_matches_pattern("Read*File", "ReadMyFile"));
    assert!(!rule_matches_pattern("Read*File", "WriteMyFile"));
}

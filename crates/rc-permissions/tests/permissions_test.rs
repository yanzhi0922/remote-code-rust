use rc_core::PermissionMode;
use rc_permissions::{
    PermissionBroker, PermissionClass, PermissionDecision, StaticPermissionBroker, auto_allows,
    classify_tool,
};

#[test]
fn classify_tool_returns_read_for_list_directory() {
    assert_eq!(classify_tool("list_directory"), PermissionClass::Read);
}

#[test]
fn classify_tool_returns_read_for_read_file() {
    assert_eq!(classify_tool("read_file"), PermissionClass::Read);
}

#[test]
fn classify_tool_returns_read_for_search_text() {
    assert_eq!(classify_tool("search_text"), PermissionClass::Read);
}

#[test]
fn classify_tool_returns_edit_for_write_file() {
    assert_eq!(classify_tool("write_file"), PermissionClass::Edit);
}

#[test]
fn classify_tool_returns_edit_for_replace_in_file() {
    assert_eq!(classify_tool("replace_in_file"), PermissionClass::Edit);
}

#[test]
fn classify_tool_returns_edit_for_edit_file() {
    assert_eq!(classify_tool("edit_file"), PermissionClass::Edit);
}

#[test]
fn classify_tool_returns_command_for_bash() {
    assert_eq!(classify_tool("bash_command"), PermissionClass::Command);
}

#[test]
fn classify_tool_returns_command_for_arbitrary_name() {
    assert_eq!(classify_tool("custom_tool"), PermissionClass::Command);
}

#[test]
fn auto_allows_bypass_permissions_allows_everything() {
    let mode = PermissionMode::BypassPermissions;
    assert!(auto_allows(mode, PermissionClass::Read));
    assert!(auto_allows(mode, PermissionClass::Edit));
    assert!(auto_allows(mode, PermissionClass::Command));
}

#[test]
fn auto_allows_accept_edits_allows_read_and_edit() {
    let mode = PermissionMode::AcceptEdits;
    assert!(auto_allows(mode, PermissionClass::Read));
    assert!(auto_allows(mode, PermissionClass::Edit));
    assert!(!auto_allows(mode, PermissionClass::Command));
}

#[test]
fn auto_allows_default_only_allows_read() {
    let mode = PermissionMode::Default;
    assert!(auto_allows(mode, PermissionClass::Read));
    assert!(!auto_allows(mode, PermissionClass::Edit));
    assert!(!auto_allows(mode, PermissionClass::Command));
}

#[test]
fn auto_allows_dont_ask_only_allows_read() {
    let mode = PermissionMode::DontAsk;
    assert!(auto_allows(mode, PermissionClass::Read));
    assert!(!auto_allows(mode, PermissionClass::Edit));
    assert!(!auto_allows(mode, PermissionClass::Command));
}

#[test]
fn auto_allows_plan_only_allows_read() {
    let mode = PermissionMode::Plan;
    assert!(auto_allows(mode, PermissionClass::Read));
    assert!(!auto_allows(mode, PermissionClass::Edit));
    assert!(!auto_allows(mode, PermissionClass::Command));
}

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

#[tokio::test]
async fn static_broker_auto_allows_read_in_default_mode() {
    let broker = StaticPermissionBroker::new(PermissionMode::Default);
    assert_eq!(broker.mode(), PermissionMode::Default);
    let decision = broker
        .decide(rc_permissions::PermissionRequest {
            tool_name: "read_file".to_owned(),
            tool_use_id: "test-id".to_owned(),
            title: "Read".to_owned(),
            description: "Read a file".to_owned(),
            input: serde_json::json!({}),
            blocked_path: None,
        })
        .await;
    assert!(decision.allowed);
}

#[tokio::test]
async fn static_broker_denies_edit_in_default_mode() {
    let broker = StaticPermissionBroker::new(PermissionMode::Default);
    let decision = broker
        .decide(rc_permissions::PermissionRequest {
            tool_name: "write_file".to_owned(),
            tool_use_id: "test-id".to_owned(),
            title: "Write".to_owned(),
            description: "Write a file".to_owned(),
            input: serde_json::json!({}),
            blocked_path: None,
        })
        .await;
    assert!(!decision.allowed);
}

#[tokio::test]
async fn static_broker_allows_all_in_bypass_mode() {
    let broker = StaticPermissionBroker::new(PermissionMode::BypassPermissions);
    let decision = broker
        .decide(rc_permissions::PermissionRequest {
            tool_name: "bash_command".to_owned(),
            tool_use_id: "test-id".to_owned(),
            title: "Bash".to_owned(),
            description: "Run command".to_owned(),
            input: serde_json::json!({}),
            blocked_path: None,
        })
        .await;
    assert!(decision.allowed);
}

#[tokio::test]
async fn static_broker_allows_edit_in_accept_edits_mode() {
    let broker = StaticPermissionBroker::new(PermissionMode::AcceptEdits);
    let decision = broker
        .decide(rc_permissions::PermissionRequest {
            tool_name: "write_file".to_owned(),
            tool_use_id: "test-id".to_owned(),
            title: "Write".to_owned(),
            description: "Write a file".to_owned(),
            input: serde_json::json!({}),
            blocked_path: None,
        })
        .await;
    assert!(decision.allowed);
}

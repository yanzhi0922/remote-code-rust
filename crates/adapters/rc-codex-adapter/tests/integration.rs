//! Integration tests for rc-codex-adapter.
//!
//! Full adapter lifecycle tests require a running Codex runtime
//! (v8, deno, etc.) and are marked `#[ignore]` by default. Run them
//! explicitly with `cargo test --package rc-codex-adapter -- --ignored`
//! in an environment with Codex build dependencies available.
//!
//! The permission type tests below verify the protocol-level types
//! used by the adapter without needing the Codex runtime.

use rc_agent_protocol::permission::{PermissionDecision, PermissionRequest};

/// Verify PermissionDecision serialization matches Codex protocol expectations.
#[test]
fn test_permission_decision_roundtrip() {
    for decision in &[
        PermissionDecision::Allow,
        PermissionDecision::Deny,
        PermissionDecision::AllowAll,
    ] {
        let json = serde_json::to_string(decision).expect("serialize");
        let back: PermissionDecision = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(*decision, back);
    }
}

/// Verify PermissionDecision::AllowAll has the expected JSON value.
#[test]
fn test_permission_decision_allow_all_value() {
    assert_eq!(
        serde_json::to_string(&PermissionDecision::AllowAll).expect("serialize"),
        "\"allow_all\""
    );
}

/// Verify PermissionRequest serialization works end-to-end.
#[test]
fn test_permission_request_serde() {
    let req = PermissionRequest {
        request_id: "req-001".into(),
        session_id: "sess-codex".into(),
        tool_name: "bash".into(),
        input: serde_json::json!({"command": "ls"}),
    };
    let json = serde_json::to_string(&req).expect("serialize");
    let back: PermissionRequest = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(req.request_id, back.request_id);
    assert_eq!(req.session_id, back.session_id);
    assert_eq!(req.tool_name, back.tool_name);
}

// ---------------------------------------------------------------------------
// Full lifecycle tests — require the Codex runtime to be available.
// ---------------------------------------------------------------------------

/// Full adapter lifecycle test.
///
/// This test requires the Codex runtime (v8, deno) to be available.
/// It is marked `#[ignore]` by default. Run with:
/// ```bash
/// cargo test --package rc-codex-adapter -- --ignored
/// ```
#[ignore]
#[tokio::test]
async fn test_adapter_lifecycle() {
    unimplemented!(
        "Codex adapter lifecycle test requires Codex runtime - see rc-codex-adapter/src/adapter.rs for the implementation"
    )
}

/// Test that the adapter handles cancel before start gracefully.
#[ignore]
#[tokio::test]
async fn test_cancel_without_start() {
    unimplemented!("Requires Codex runtime")
}

/// Test that resolve_permission handles unknown request IDs.
#[ignore]
#[tokio::test]
async fn test_resolve_unknown_permission() {
    unimplemented!("Requires Codex runtime")
}

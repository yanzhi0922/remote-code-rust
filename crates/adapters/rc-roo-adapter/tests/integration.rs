//! Integration tests for rc-roo-adapter.
//!
//! Tests the adapter lifecycle and error handling without requiring
//! a real provider or API key. The adapter's RooInProcessAdapter
//! supports 26 provider backends.

use anyhow::Result;
use rc_agent_protocol::adapter::AgentAdapter;
use rc_agent_protocol::permission::PermissionDecision;
use rc_agent_protocol::types::{AgentCapability, AgentConfig, AgentType};
use rc_roo_adapter::RooInProcessAdapter;

/// Verify default adapter metadata is correct for an unstarted adapter.
#[test]
fn test_adapter_info_and_type() {
    let adapter = RooInProcessAdapter::new();

    assert_eq!(adapter.agent_type(), AgentType::RemoteRoo);
    assert_eq!(adapter.info().name, "Roo In-Process");
    assert!(adapter.info().capabilities.contains(&AgentCapability::Streaming));
    assert!(adapter.info().capabilities.contains(&AgentCapability::ToolUse));
    assert!(adapter.info().capabilities.contains(&AgentCapability::Subtasks));
    assert!(adapter.info().capabilities.contains(&AgentCapability::McpSupport));
}

/// Verify that start() fails gracefully without a real API key.
#[tokio::test]
async fn test_start_fails_without_api_key() {
    let mut adapter = RooInProcessAdapter::new();
    let result = adapter
        .start(&AgentConfig {
            agent_type: AgentType::RemoteRoo,
            ..Default::default()
        })
        .await;
    assert!(result.is_err(), "expected start to fail without provider config");
}

/// Verify that stop is safe on an unstarted adapter (no panic).
#[tokio::test]
async fn test_stop_without_start_is_safe() -> Result<()> {
    let mut adapter = RooInProcessAdapter::new();
    // Stopping an unstarted adapter should not panic.
    adapter.stop().await?;
    adapter.stop().await?; // Idempotent.
    Ok(())
}

/// Verify that send_message without a configured provider returns an error.
#[tokio::test]
async fn test_send_message_without_provider_returns_error() -> Result<()> {
    let mut adapter = RooInProcessAdapter::new();
    let result = adapter.send_message("test-session", "Hello").await;
    assert!(result.is_err(), "expected error when no provider is configured");
    Ok(())
}

/// Verify that cancel on an idle adapter is safe (no panic).
#[tokio::test]
async fn test_cancel_without_start_is_safe() -> Result<()> {
    let mut adapter = RooInProcessAdapter::new();
    adapter.cancel("test-session").await?;
    Ok(())
}

/// Verify that is_alive returns true before any lifecycle call.
#[test]
fn test_is_alive_before_start() {
    let adapter = RooInProcessAdapter::new();
    assert!(adapter.is_alive());
}

/// Verify the adapter advertises McpSupport capability.
#[test]
fn test_adapter_has_mcp_capability() {
    let adapter = RooInProcessAdapter::new();
    assert!(
        adapter.info().capabilities.contains(&AgentCapability::McpSupport),
        "Roo adapter should advertise McpSupport"
    );
}

// ── Performance benchmarks ────────────────────────────────────────────

/// Benchmark: verify RooInProcessAdapter::new() is fast.
#[test]
fn bench_adapter_creation_timing() {
    let start = std::time::Instant::now();
    for _ in 0..1000 {
        let _adapter = RooInProcessAdapter::new();
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 500,
        "1000 adapter creations took too long: {}ms",
        elapsed.as_millis()
    );
}

/// Benchmark: stop on unstarted adapter should be instant.
#[tokio::test]
async fn bench_stop_timing() -> Result<()> {
    let start = std::time::Instant::now();
    for _ in 0..100 {
        let mut adapter = RooInProcessAdapter::new();
        adapter.stop().await?;
    }
    let elapsed = start.elapsed();
    assert!(
        elapsed.as_millis() < 500,
        "100 stop calls took too long: {}ms",
        elapsed.as_millis()
    );
    Ok(())
}

//! Provider integration tests.
//!
//! Tests provider client construction, circuit breaker state transitions,
//! credential pool rotation, and streaming callback structures.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

// ─── Circuit breaker state transitions ──────────────────────────────────────

#[test]
fn circuit_breaker_starts_closed() {
    let cb = claude_provider::CircuitBreaker::new_default();
    assert!(cb.allow_request().is_ok());
}

#[test]
fn circuit_breaker_transitions_to_open_after_failures() {
    let config = claude_provider::CircuitBreakerConfig {
        failure_threshold: 3,
        recovery_timeout: Duration::from_secs(60),
        half_open_max_probes: 1,
    };
    let cb = claude_provider::CircuitBreaker::new(config);

    // Record failures up to threshold
    cb.record_failure();
    cb.record_failure();
    cb.record_failure();

    // Should now be open
    let result = cb.allow_request();
    let error = result.expect_err("circuit breaker should be open");
    assert_eq!(error, claude_provider::CircuitState::Open);
}

#[test]
fn circuit_breaker_transitions_to_half_open_after_timeout() {
    let config = claude_provider::CircuitBreakerConfig {
        failure_threshold: 1,
        recovery_timeout: Duration::from_millis(1), // very short timeout
        half_open_max_probes: 1,
    };
    let cb = claude_provider::CircuitBreaker::new(config);

    // Open the circuit
    cb.record_failure();
    assert!(cb.allow_request().is_err());

    // Wait for recovery timeout
    std::thread::sleep(Duration::from_millis(5));

    // Should transition to half-open
    assert!(cb.allow_request().is_ok());
}

#[test]
fn circuit_breaker_closes_on_success_from_half_open() {
    let config = claude_provider::CircuitBreakerConfig {
        failure_threshold: 1,
        recovery_timeout: Duration::from_millis(1),
        half_open_max_probes: 1,
    };
    let cb = claude_provider::CircuitBreaker::new(config);

    // Open the circuit
    cb.record_failure();
    assert!(cb.allow_request().is_err());

    // Wait for recovery
    std::thread::sleep(Duration::from_millis(5));

    // Allow probe (half-open)
    assert!(cb.allow_request().is_ok());

    // Record success → should close
    cb.record_success();
    assert!(cb.allow_request().is_ok());
}

#[test]
fn circuit_breaker_re_opens_on_half_open_failure() {
    let config = claude_provider::CircuitBreakerConfig {
        failure_threshold: 1,
        recovery_timeout: Duration::from_millis(1),
        half_open_max_probes: 1,
    };
    let cb = claude_provider::CircuitBreaker::new(config);

    // Open the circuit
    cb.record_failure();
    assert!(cb.allow_request().is_err());

    // Wait for recovery
    std::thread::sleep(Duration::from_millis(5));

    // Allow probe (half-open)
    assert!(cb.allow_request().is_ok());

    // Record failure → should re-open
    cb.record_failure();
    assert!(cb.allow_request().is_err());
}

// ─── Credential pool rotation ───────────────────────────────────────────────

#[test]
fn credential_pool_rotates_round_robin() {
    let pool = claude_provider::credential_pool::CredentialPool::from_keys(vec![
        "key-1".to_owned(),
        "key-2".to_owned(),
        "key-3".to_owned(),
    ]);

    assert_eq!(pool.len(), 3);

    let first = pool.next().expect("should have credential");
    assert_eq!(first.api_key, "key-1");

    let second = pool.next().expect("should have credential");
    assert_eq!(second.api_key, "key-2");

    let third = pool.next().expect("should have credential");
    assert_eq!(third.api_key, "key-3");

    // Should wrap around
    let wraps = pool.next().expect("should have credential");
    assert_eq!(wraps.api_key, "key-1");
}

#[test]
fn credential_pool_single_key() {
    let pool = claude_provider::credential_pool::CredentialPool::single("only-key");
    assert_eq!(pool.len(), 1);

    let first = pool.next().expect("should have credential");
    let second = pool.next().expect("should have credential");
    assert_eq!(first.api_key, second.api_key);
}

#[test]
fn credential_pool_empty() {
    let pool = claude_provider::credential_pool::CredentialPool::new(vec![]);
    assert!(pool.is_empty());
    assert!(pool.next().is_none());
}

#[test]
fn credential_entry_with_model() {
    let entry = claude_provider::credential_pool::CredentialEntry::with_model(
        "sk-test-key",
        "claude-sonnet-4-20250514",
    );
    assert_eq!(entry.api_key, "sk-test-key");
    assert_eq!(entry.model.as_deref(), Some("claude-sonnet-4-20250514"));
}

#[test]
fn credential_pool_thread_safety() {
    let pool = Arc::new(claude_provider::credential_pool::CredentialPool::from_keys(
        vec!["key-a".to_owned(), "key-b".to_owned()],
    ));
    let counter = Arc::new(AtomicUsize::new(0));

    let mut handles = vec![];
    for _ in 0..4 {
        let pool = Arc::clone(&pool);
        let counter = Arc::clone(&counter);
        handles.push(std::thread::spawn(move || {
            let cred = pool.next().expect("should have credential");
            counter.fetch_add(1, Ordering::Relaxed);
            cred.api_key.clone()
        }));
    }

    let results: Vec<String> = handles
        .into_iter()
        .map(|h| h.join().expect("thread should succeed"))
        .collect();

    assert_eq!(counter.load(Ordering::Relaxed), 4);
    // All results should be valid keys
    for key in &results {
        assert!(key == "key-a" || key == "key-b");
    }
}

// ─── Provider client construction ───────────────────────────────────────────

#[test]
fn provider_client_can_be_created() {
    let client = claude_provider::ProviderClient::new();
    assert!(client.is_ok());
}

// ─── Retry configuration ────────────────────────────────────────────────────

#[test]
fn retry_config_default_values() {
    let config = claude_provider::RetryConfig::default();
    assert!(config.max_retries > 0);
}

// ─── Streaming callbacks structure ──────────────────────────────────────────

#[test]
fn streaming_callbacks_can_be_created() {
    let text_count = Arc::new(AtomicUsize::new(0));
    let tc = Arc::clone(&text_count);

    let callbacks = claude_provider::StreamingCallbacks {
        on_text_delta: Some(Box::new(move |_delta| {
            tc.fetch_add(1, Ordering::Relaxed);
        })),
        on_tool_call_start: None,
        on_tool_call_delta: None,
        on_usage: None,
        on_thinking_delta: None,
        on_lifecycle_event: None,
    };

    // Invoke the callback
    if let Some(ref cb) = callbacks.on_text_delta {
        cb("hello");
        cb("world");
    }

    assert_eq!(text_count.load(Ordering::Relaxed), 2);
}

#[test]
fn streaming_callbacks_usage_callback() {
    let usage_calls = Arc::new(AtomicUsize::new(0));
    let uc = Arc::clone(&usage_calls);

    let callbacks = claude_provider::StreamingCallbacks {
        on_text_delta: None,
        on_tool_call_start: None,
        on_tool_call_delta: None,
        on_usage: Some(Box::new(move |_update| {
            uc.fetch_add(1, Ordering::Relaxed);
        })),
        on_thinking_delta: None,
        on_lifecycle_event: None,
    };

    if let Some(ref cb) = callbacks.on_usage {
        cb(claude_provider::streaming::StreamingUsageUpdate {
            input_tokens: 100,
            output_tokens: 50,
            ..Default::default()
        });
        cb(claude_provider::streaming::StreamingUsageUpdate {
            input_tokens: 200,
            output_tokens: 100,
            ..Default::default()
        });
    }

    assert_eq!(usage_calls.load(Ordering::Relaxed), 2);
}

// ─── Provider protocol request building (structural) ────────────────────────

#[test]
fn anthropic_protocol_conversation_format() {
    let entries = [
        claude_core::ConversationEntry::system("You are a helpful assistant."),
        claude_core::ConversationEntry::user("What is 2+2?"),
        claude_core::ConversationEntry::assistant("The answer is 4."),
    ];

    assert_eq!(entries[0].role, claude_core::ConversationRole::System);
    assert_eq!(entries[1].role, claude_core::ConversationRole::User);
    assert_eq!(entries[2].role, claude_core::ConversationRole::Assistant);

    for entry in &entries {
        let json = serde_json::to_string(entry).expect("entry should serialize");
        assert!(!json.is_empty());
    }
}

#[test]
fn openai_protocol_conversation_format() {
    let entries = [
        claude_core::ConversationEntry::system("You are a coding assistant."),
        claude_core::ConversationEntry::user("Write a hello world in Rust."),
    ];

    assert_eq!(entries[0].role, claude_core::ConversationRole::System);
    assert_eq!(entries[1].role, claude_core::ConversationRole::User);
}

#[test]
fn provider_response_with_tool_calls() {
    let response = claude_core::ProviderResponse {
        text: String::new(),
        history_text: None,
        thinking: None,
        content_blocks: vec![],
        tool_calls: vec![
            claude_core::ToolCall {
                id: "tc-1".to_owned(),
                name: "read_file".to_owned(),
                input: serde_json::json!({"path": "/tmp/test.rs"}),
            },
            claude_core::ToolCall {
                id: "tc-2".to_owned(),
                name: "write_file".to_owned(),
                input: serde_json::json!({"path": "/tmp/output.rs", "content": "fn main() {}"}),
            },
        ],
        request_id: Some("req-001".to_owned()),
        usage: claude_core::UsageSummary {
            input_tokens: 500,
            output_tokens: 200,
            cache_read_input_tokens: 50,
            cache_creation_input_tokens: 25,
            ..Default::default()
        },
        stop_reason: "tool_use".to_owned(),
        research: None,
    };

    let json = serde_json::to_string(&response).expect("should serialize");
    let decoded: claude_core::ProviderResponse =
        serde_json::from_str(&json).expect("should deserialize");
    assert_eq!(decoded.tool_calls.len(), 2);
    assert_eq!(decoded.tool_calls[0].name, "read_file");
    assert_eq!(decoded.tool_calls[1].name, "write_file");
}

#[test]
fn circuit_breaker_reset_returns_to_closed() {
    let config = claude_provider::CircuitBreakerConfig {
        failure_threshold: 1,
        recovery_timeout: Duration::from_secs(60),
        half_open_max_probes: 1,
    };
    let cb = claude_provider::CircuitBreaker::new(config);

    cb.record_failure();
    assert!(cb.allow_request().is_err());

    cb.reset();
    assert!(cb.allow_request().is_ok());
}

#[test]
fn circuit_breaker_success_resets_failure_count() {
    let config = claude_provider::CircuitBreakerConfig {
        failure_threshold: 3,
        recovery_timeout: Duration::from_secs(60),
        half_open_max_probes: 1,
    };
    let cb = claude_provider::CircuitBreaker::new(config);

    cb.record_failure();
    cb.record_failure();
    // Success resets the failure count
    cb.record_success();
    // Now need 3 more failures to open
    cb.record_failure();
    cb.record_failure();
    assert!(cb.allow_request().is_ok()); // still closed (only 2 consecutive failures)
}
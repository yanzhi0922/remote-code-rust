//! MCP integration tests.
//!
//! Tests MCP config loading, transport creation, reconnect scheduler,
//! OAuth PKCE parameter generation, and JSON-RPC message structures.

use std::collections::BTreeMap;

// ─── MCP config loading and parsing ─────────────────────────────────────────

#[test]
fn mcp_config_from_toml() {
    let toml = r#"
[mcp_servers.filesystem]
command = "npx"
args = ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"]
enabled = true

[mcp_servers.github]
url = "https://github-mcp.example.com/sse"
enabled = false
"#;
    let config = rc_mcp::McpConfig::from_toml_str(toml).expect("should parse TOML config");
    assert_eq!(config.servers.len(), 2);

    let fs_config = config.servers.get("filesystem").expect("filesystem server");
    assert!(fs_config.enabled);
    assert!(matches!(
        fs_config.transport,
        rc_mcp::McpTransportConfig::Stdio { .. }
    ));

    let gh_config = config.servers.get("github").expect("github server");
    assert!(!gh_config.enabled);
}

#[test]
fn mcp_config_empty() {
    let config = rc_mcp::McpConfig::default();
    assert!(config.servers.is_empty());
}

#[test]
fn mcp_config_save_and_load() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("mcp.toml");

    let config = rc_mcp::McpConfig {
        servers: {
            let mut map = BTreeMap::new();
            map.insert(
                "test-server".to_owned(),
                rc_mcp::McpServerConfig {
                    name: "test-server".to_owned(),
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
                },
            );
            map
        },
    };

    config.save(&path).expect("should save config");
    let loaded = rc_mcp::McpConfig::load(&path).expect("should load config");
    assert_eq!(loaded.servers.len(), 1);
    assert!(loaded.servers.contains_key("test-server"));
}

// ─── Transport types ────────────────────────────────────────────────────────

#[test]
fn transport_config_stdio() {
    let config = rc_mcp::McpTransportConfig::Stdio {
        command: "npx".to_owned(),
        args: vec!["-y".to_owned(), "@mcp/server".to_owned()],
        cwd: Some("/tmp".into()),
        env: BTreeMap::new(),
    };
    assert_eq!(config.kind(), rc_mcp::transport::McpTransport::Stdio);
}

#[test]
fn transport_config_http() {
    let config = rc_mcp::McpTransportConfig::Http {
        url: "https://mcp.example.com/sse".to_owned(),
        headers: BTreeMap::new(),
    };
    assert_eq!(config.kind(), rc_mcp::transport::McpTransport::Http);
}

#[test]
fn transport_config_websocket() {
    let config = rc_mcp::McpTransportConfig::WebSocket {
        url: "wss://mcp.example.com/ws".to_owned(),
        headers: BTreeMap::new(),
    };
    assert_eq!(config.kind(), rc_mcp::transport::McpTransport::WebSocket);
}

// ─── Reconnect scheduler ────────────────────────────────────────────────────

#[test]
fn reconnect_scheduler_starts_idle() {
    let scheduler = rc_mcp::ReconnectScheduler::new();
    assert_eq!(scheduler.pending_count(), 0);
    assert!(!scheduler.is_reconnecting("test-server"));
}

#[test]
fn reconnect_scheduler_schedules_reconnect() {
    let mut scheduler = rc_mcp::ReconnectScheduler::new();
    let action = scheduler.schedule_reconnect("test-server".to_owned());

    match action {
        rc_mcp::ReconnectAction::ConnectNow => {}
        rc_mcp::ReconnectAction::WaitFor { .. } => {}
        rc_mcp::ReconnectAction::GiveUp => {
            panic!("first reconnect should not give up");
        }
    }

    assert!(scheduler.is_reconnecting("test-server"));
    assert_eq!(scheduler.pending_count(), 1);
}

#[test]
fn reconnect_scheduler_report_success() {
    let mut scheduler = rc_mcp::ReconnectScheduler::new();
    scheduler.schedule_reconnect("test-server".to_owned());
    scheduler.report_success("test-server");

    assert!(!scheduler.is_reconnecting("test-server"));
    assert_eq!(scheduler.pending_count(), 0);
}

#[test]
fn reconnect_scheduler_cancel() {
    let mut scheduler = rc_mcp::ReconnectScheduler::new();
    scheduler.schedule_reconnect("test-server".to_owned());
    scheduler.cancel("test-server");

    assert!(!scheduler.is_reconnecting("test-server"));
}

#[test]
fn reconnect_scheduler_cancel_all() {
    let mut scheduler = rc_mcp::ReconnectScheduler::new();
    scheduler.schedule_reconnect("server-1".to_owned());
    scheduler.schedule_reconnect("server-2".to_owned());
    scheduler.cancel_all();

    assert_eq!(scheduler.pending_count(), 0);
}

#[test]
fn reconnect_scheduler_max_attempts() {
    let mut scheduler = rc_mcp::ReconnectScheduler::with_params(2, 1, 1000);

    // First attempt
    let action1 = scheduler.schedule_reconnect("server".to_owned());
    assert!(matches!(action1, rc_mcp::ReconnectAction::ConnectNow));

    // Simulate failure and second attempt
    let _ = scheduler.report_failure("server");
    let _action2 = scheduler.schedule_reconnect("server".to_owned());

    // After max attempts, should give up
    let _ = scheduler.report_failure("server");
    let action3 = scheduler.schedule_reconnect("server".to_owned());
    assert!(matches!(action3, rc_mcp::ReconnectAction::GiveUp));
}

// ─── OAuth PKCE parameter generation ────────────────────────────────────────

#[test]
fn pkce_params_are_generated() {
    let pkce = rc_mcp::oauth::McpOAuthFlow::generate_pkce();
    assert!(!pkce.code_verifier.is_empty());
    assert!(!pkce.code_challenge.is_empty());
    assert_eq!(pkce.code_challenge_method, "S256");

    // Verifier should be at least 43 characters (RFC 7636)
    assert!(pkce.code_verifier.len() >= 43);
}

#[test]
fn oauth_flow_construction() {
    let flow = rc_mcp::oauth::McpOAuthFlow::new(&rc_mcp::transport::McpOAuthConfig {
        client_id: Some("test-client".to_owned()),
        callback_port: None,
        auth_server_metadata_url: None,
        xaa: None,
    });
    assert!(!flow.xaa_enabled());
}

#[test]
fn oauth_token_store() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut store = rc_mcp::OAuthTokenStore::new(dir.path());

    let tokens = rc_mcp::OAuthTokens {
        access_token: "at-123".to_owned(),
        refresh_token: Some("rt-456".to_owned()),
        expires_at: Some(chrono::Utc::now().timestamp() + 3600),
        token_type: "Bearer".to_owned(),
        scope: Some("openid".to_owned()),
    };

    store.save_token("server-1", tokens.clone());
    assert!(store.contains("server-1"));
    assert_eq!(store.len(), 1);

    let retrieved = store.get_token("server-1").expect("should have token");
    assert_eq!(retrieved.access_token, "at-123");

    store.remove_token("server-1");
    assert!(!store.contains("server-1"));
    assert!(store.is_empty());
}

#[test]
fn oauth_token_expired_detection() {
    let expired = rc_mcp::OAuthTokens {
        access_token: "at-expired".to_owned(),
        refresh_token: None,
        expires_at: Some(chrono::Utc::now().timestamp() - 3600), // 1 hour ago
        token_type: "Bearer".to_owned(),
        scope: None,
    };
    assert!(rc_mcp::oauth::McpOAuthFlow::is_token_expired(&expired));

    let valid = rc_mcp::OAuthTokens {
        access_token: "at-valid".to_owned(),
        refresh_token: None,
        expires_at: Some(chrono::Utc::now().timestamp() + 3600), // 1 hour from now
        token_type: "Bearer".to_owned(),
        scope: None,
    };
    assert!(!rc_mcp::oauth::McpOAuthFlow::is_token_expired(&valid));
}

// ─── Auth cache ─────────────────────────────────────────────────────────────

#[test]
fn auth_cache_operations() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut cache = rc_mcp::McpAuthCache::new(dir.path());

    assert!(!cache.is_cached("server-1"));

    cache.mark_needs_auth("server-1");
    assert!(cache.is_cached("server-1"));

    cache.clear_server("server-1");
    assert!(!cache.is_cached("server-1"));

    cache.mark_needs_auth("server-2");
    cache.mark_needs_auth("server-3");
    assert_eq!(cache.len(), 2);

    cache.clear_all();
    assert!(cache.is_empty());
}

// ─── Connection state machine ───────────────────────────────────────────────

#[test]
fn connection_state_transitions() {
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

    // Connected
    let connected = rc_mcp::McpServerConnection::Connected(rc_mcp::connection::ConnectedServer {
        name: "test".to_owned(),
        capabilities: rc_mcp::McpCapabilityMatrix::default(),
        server_info: None,
        instructions: None,
        config: scoped.clone(),
    });
    assert!(connected.is_connected());
    assert_eq!(connected.name(), "test");
    assert_eq!(connected.connection_type(), "connected");

    // Failed
    let failed = rc_mcp::McpServerConnection::Failed(rc_mcp::connection::FailedServer {
        name: "test".to_owned(),
        config: scoped.clone(),
        error: Some("connection refused".to_owned()),
    });
    assert!(!failed.is_connected());
    assert_eq!(failed.connection_type(), "failed");

    // Pending
    let pending = rc_mcp::McpServerConnection::Pending(rc_mcp::connection::PendingServer {
        name: "test".to_owned(),
        config: scoped.clone(),
        reconnect_attempt: Some(1),
        max_reconnect_attempts: Some(5),
    });
    assert!(!pending.is_connected());
    assert_eq!(pending.connection_type(), "pending");

    // Needs auth
    let needs_auth = rc_mcp::McpServerConnection::NeedsAuth(rc_mcp::connection::NeedsAuthServer {
        name: "test".to_owned(),
        config: scoped.clone(),
    });
    assert!(!needs_auth.is_connected());
    assert_eq!(needs_auth.connection_type(), "needs-auth");

    // Disabled
    let disabled = rc_mcp::McpServerConnection::Disabled(rc_mcp::connection::DisabledServer {
        name: "test".to_owned(),
        config: scoped,
    });
    assert!(!disabled.is_connected());
    assert_eq!(disabled.connection_type(), "disabled");
}

// ─── Discovery ──────────────────────────────────────────────────────────────

#[test]
fn discovery_cache_operations() {
    let mut discovery = rc_mcp::McpDiscovery::new();
    assert_eq!(discovery.server_count(), 0);
    assert_eq!(discovery.total_tool_count(), 0);

    discovery.store(
        "server-1",
        vec![rc_mcp::McpToolDescriptor {
            name: "tool-1".to_owned(),
            title: None,
            description: Some("Test tool".to_owned()),
            input_schema: serde_json::Value::Null,
            annotations: serde_json::Value::Null,
        }],
        vec![],
        vec![],
        None,
    );

    assert_eq!(discovery.server_count(), 1);
    assert_eq!(discovery.total_tool_count(), 1);
    assert!(discovery.tools("server-1").is_some());

    discovery.clear_server("server-1");
    assert_eq!(discovery.server_count(), 0);

    discovery.clear_all();
    assert_eq!(discovery.total_tool_count(), 0);
}

// ─── Batch update queue ─────────────────────────────────────────────────────

#[test]
fn batch_queue_operations() {
    let mut queue = rc_mcp::BatchedUpdateQueue::new();
    assert!(!queue.has_pending());

    let config = rc_mcp::McpServerConfig {
        name: "server-1".to_owned(),
        enabled: false,
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
        rc_mcp::scope::ScopedMcpServerConfig::new(config, rc_mcp::scope::ConfigScope::User);

    queue.enqueue(rc_mcp::BatchUpdate {
        server_name: "server-1".to_owned(),
        connection: rc_mcp::McpServerConnection::Disabled(rc_mcp::connection::DisabledServer {
            name: "server-1".to_owned(),
            config: scoped,
        }),
        tools: None,
        resources: None,
    });

    assert!(queue.has_pending());
    assert_eq!(queue.pending_count(), 1);

    let flushed = queue.flush();
    assert_eq!(flushed.len(), 1);
    assert!(!queue.has_pending());
}

// ─── Channel permissions ────────────────────────────────────────────────────

#[test]
fn channel_allowlist_operations() {
    let mut allowlist = rc_mcp::ChannelAllowlist::new();
    assert!(allowlist.is_empty());

    allowlist.add("server-1".to_owned());
    assert!(allowlist.is_allowed("server-1"));
    assert!(!allowlist.is_allowed("server-2"));
    assert_eq!(allowlist.len(), 1);

    allowlist.remove("server-1");
    assert!(!allowlist.is_allowed("server-1"));
}

#[test]
fn channel_message_construction() {
    let msg = rc_mcp::ChannelMessage::new(
        "server-1".to_owned(),
        "notifications".to_owned(),
        "Hello from MCP".to_owned(),
    );
    assert_eq!(msg.server_name, "server-1");
    assert_eq!(msg.channel, "notifications");
    assert_eq!(msg.content, "Hello from MCP");
}

// ─── Name normalization ─────────────────────────────────────────────────────

#[test]
fn mcp_name_normalization() {
    let info = rc_mcp::normalization::mcp_info_from_string("mcp__filesystem__read_file");
    assert!(info.is_some());
    let info = info.expect("should parse");
    assert_eq!(info.server_name, "filesystem");
    assert_eq!(info.tool_name, "read_file");

    let tool_name = rc_mcp::normalization::build_mcp_tool_name("filesystem", "read_file");
    assert_eq!(tool_name, "mcp__filesystem__read_file");
}

// ─── Config validation ──────────────────────────────────────────────────────

#[test]
fn config_validator_checks_server_name() {
    assert!(rc_mcp::McpConfigValidator::validate_server_name(
        "valid-name"
    ));
    assert!(rc_mcp::McpConfigValidator::validate_server_name(
        "my_server"
    ));
    assert!(!rc_mcp::McpConfigValidator::validate_server_name(""));
    assert!(!rc_mcp::McpConfigValidator::validate_server_name(
        &"x".repeat(65)
    ));
}

#[test]
fn config_validator_checks_url() {
    assert!(rc_mcp::McpConfigValidator::validate_url(
        "https://example.com"
    ));
    assert!(rc_mcp::McpConfigValidator::validate_url(
        "http://localhost:8080"
    ));
    assert!(!rc_mcp::McpConfigValidator::validate_url("not-a-url"));
}

// ─── Official registry ──────────────────────────────────────────────────────

#[test]
fn official_registry_operations() {
    let mut registry = rc_mcp::OfficialMcpRegistry::new();
    assert!(!registry.is_loaded());

    registry.load_from_urls(&["https://github.com/modelcontextprotocol/servers"]);
    assert!(registry.is_loaded());
    assert_eq!(registry.count(), 1);
    assert!(registry.is_official("https://github.com/modelcontextprotocol/servers"));

    registry.clear();
    assert_eq!(registry.count(), 0);
}

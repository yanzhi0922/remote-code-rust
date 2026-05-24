//! HTTP/WS server for Claude agent — hub for IM adapters.
//!
//! Provides a REST API for session CRUD and a WebSocket endpoint for
//! real-time streaming chat. IM adapters (Telegram, Feishu, DingTalk,
//! WeChat) connect via WebSocket and use the REST API for session management.

pub mod app;
pub mod auth;
pub mod error;
pub mod events;
pub mod routes;
pub mod state;
pub mod tool_runner;
pub mod ws;

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Configuration for the claude-server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Bind address, e.g. "127.0.0.1:9090".
    pub bind: SocketAddr,
    /// Bearer token for API authentication.
    pub auth_token: Option<String>,
    /// Allow running without API authentication. Intended only for isolated local development.
    #[serde(default)]
    pub allow_unauthenticated: bool,
    /// Override profile directory. If None, auto-discovered.
    pub profile_dir: Option<PathBuf>,
    /// Default working directory for new sessions.
    pub default_work_dir: PathBuf,
    /// Default model identifier.
    pub default_model: String,
    /// Default provider name.
    pub default_provider: String,
    /// API key for the default provider.
    pub api_key: Option<String>,
    /// Broadcast channel buffer size for per-session events.
    #[serde(default = "default_buffer_size")]
    pub event_buffer_size: usize,
}

fn default_buffer_size() -> usize {
    256
}

const DEFAULT_BIND_ADDR: &str = "127.0.0.1:9090";

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind: DEFAULT_BIND_ADDR
                .parse()
                .expect("default bind address is valid"),
            auth_token: None,
            allow_unauthenticated: false,
            profile_dir: None,
            default_work_dir: std::env::current_dir().unwrap_or_default(),
            default_model: "claude-sonnet-4-20250514".to_owned(),
            default_provider: "anthropic".to_owned(),
            api_key: None,
            event_buffer_size: 256,
        }
    }
}

/// Start the claude-server and block until shutdown.
pub async fn serve(config: ServerConfig) -> Result<()> {
    let paths = claude_config::AppPaths::discover(config.profile_dir.clone())?;
    paths.ensure_exists()?;
    let session_store = claude_session::SessionStore::open(paths)?;

    let provider_config = claude_config::ProviderConfig {
        name: config.default_provider.clone(),
        base_url: None,
        api_key: config.api_key.clone(),
        model: Some(config.default_model.clone()),
        protocol: claude_core::ProviderProtocol::Anthropic,
        timeout_ms: 120_000,
        max_output_tokens: 16_384,
        max_retries: 2,
        retry_initial_backoff_ms: 1_000,
        retry_max_backoff_ms: 10_000,
        respect_retry_after: true,
        request_header_overrides: Default::default(),
        request_metadata: Default::default(),
        thinking_budget: None,
        temperature: None,
        top_p: None,
        top_k: None,
    };
    let provider_client = Arc::new(claude_provider::ProviderClient::new()?);
    let backend = Arc::new(claude_provider::ProviderCompatBackend::new(
        provider_client,
        &provider_config,
    ));
    let tool_runner = Arc::new(tool_runner::ServerToolRunner::new());

    let state = state::ServerState::new(config.clone(), session_store, backend, tool_runner);
    let app = app::build_router(state);

    if config.auth_token.is_none() && !config.allow_unauthenticated {
        anyhow::bail!(
            "claude-server requires authentication; set CLAUDE_SERVER_AUTH_TOKEN or pass --allow-unauthenticated for isolated local development"
        );
    }

    if config.auth_token.is_none() && !config.bind.ip().is_loopback() {
        anyhow::bail!("unauthenticated claude-server may only bind to a loopback address");
    }

    if config.auth_token.is_none() {
        tracing::warn!(
            "Server starting without authentication because --allow-unauthenticated was set."
        );
    }

    let listener = tokio::net::TcpListener::bind(&config.bind).await?;
    tracing::info!("claude-server listening on {}", config.bind);
    tracing::warn!(
        "claude-server is using BypassPermissions mode — all tool calls are auto-approved. \
         Ensure this server is only accessible to trusted clients."
    );
    axum::serve(listener, app).await?;
    Ok(())
}

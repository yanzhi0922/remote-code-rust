use std::net::SocketAddr;
use std::path::PathBuf;

use clap::Parser;
use tracing_subscriber::EnvFilter;

/// HTTP/WS server for Claude agent — hub for IM adapters.
#[derive(Debug, Parser)]
#[command(name = "claude-server", version, about)]
struct Cli {
    /// Bind address.
    #[arg(long, default_value = "127.0.0.1:9090")]
    bind: SocketAddr,

    /// Bearer token for API authentication. Required unless --allow-unauthenticated is set.
    #[arg(long, env = "CLAUDE_SERVER_AUTH_TOKEN")]
    auth_token: Option<String>,

    /// Allow unauthenticated loopback-only local development.
    #[arg(long)]
    allow_unauthenticated: bool,

    /// API key for the LLM provider.
    /// TODO: In production deployments, this should only be read from an env var
    /// (or a secrets manager) and never accepted as a plain CLI arg to avoid
    /// leaking the key in process listings.
    #[arg(long, env = "ANTHROPIC_API_KEY")]
    api_key: Option<String>,

    /// Override profile directory.
    #[arg(long)]
    profile_dir: Option<PathBuf>,

    /// Default working directory for new sessions.
    #[arg(long, default_value = ".")]
    work_dir: PathBuf,

    /// Default model identifier.
    #[arg(long, default_value = "claude-sonnet-4-20250514")]
    model: String,

    /// Default provider name.
    #[arg(long, default_value = "anthropic")]
    provider: String,

    /// Log filter (RUST_LOG style).
    #[arg(long, default_value = "claude_server=info")]
    log_filter: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(&cli.log_filter)),
        )
        .init();

    let config = claude_server::ServerConfig {
        bind: cli.bind,
        auth_token: cli.auth_token,
        allow_unauthenticated: cli.allow_unauthenticated,
        profile_dir: cli.profile_dir,
        default_work_dir: cli.work_dir,
        default_model: cli.model,
        default_provider: cli.provider,
        api_key: cli.api_key,
        event_buffer_size: 256,
    };

    tracing::info!("starting claude-server on {}", config.bind);

    tokio::select! {
        result = claude_server::serve(config) => result,
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("received CTRL+C, shutting down");
            Ok(())
        }
    }
}

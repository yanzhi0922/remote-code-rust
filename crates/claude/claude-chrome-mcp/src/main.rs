//! Chrome MCP server binary entry point.
//!
//! Launches the MCP server communicating over stdin/stdout.
//! Intended to be spawned as a child process by the MCP connection manager.

use claude_chrome_mcp::mcp_server;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_writer(std::io::stderr)
        .init();

    if let Err(e) = mcp_server::run_stdio_server().await {
        tracing::error!("Chrome MCP server error: {e}");
        std::process::exit(1);
    }
}

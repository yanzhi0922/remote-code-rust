use claude_computer_use::mcp_server;

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
        tracing::error!("computer-use MCP server error: {e}");
        std::process::exit(1);
    }
}

use clap::Parser;
use tracing_subscriber::EnvFilter;

use claude_api_proxy::config::ProxyConfigFile;

#[derive(Debug, Parser)]
#[command(
    name = "claude-api-proxy",
    about = "Multi-provider API proxy with OpenAI/Anthropic passthrough"
)]
struct Cli {
    #[arg(long, default_value = "proxy-config.toml")]
    config: String,
    #[arg(long)]
    bind: Option<std::net::SocketAddr>,
    #[arg(long)]
    auth_token: Option<String>,
    #[arg(long, default_value = "info")]
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

    let config = ProxyConfigFile::load(&cli.config)?;
    claude_api_proxy::serve(config, cli.auth_token, cli.bind).await
}

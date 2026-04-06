use std::net::SocketAddr;

use anyhow::Result;
use clap::{Parser, Subcommand};
use rc_control_plane::{
    ControlPlaneConfigOverrides, ControlPlaneService, describe_status, load_control_plane_config,
};
use rc_telemetry::install_tracing;

#[derive(Parser, Debug)]
#[command(
    name = "remote-code-control-plane",
    version,
    about = "Rust control-plane service"
)]
struct Cli {
    #[arg(long, env = "REMOTE_CODE_CONTROL_PLANE_BIND")]
    bind: Option<SocketAddr>,

    #[arg(long, env = "REMOTE_CODE_CONTROL_PLANE_PUBLIC_BASE_URL")]
    public_base_url: Option<String>,

    #[arg(long, env = "REMOTE_CODE_CONTROL_PLANE_SERVICE_NAME")]
    service_name: Option<String>,

    #[arg(long, env = "REMOTE_CODE_RUNNER_LEASE_TTL_SECS")]
    runner_lease_ttl_secs: Option<u64>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    Doctor,
    Serve,
}

#[tokio::main]
async fn main() -> Result<()> {
    install_tracing("remote_code_control_plane", false)?;
    let cli = Cli::parse();
    let config = load_control_plane_config(ControlPlaneConfigOverrides {
        bind: cli.bind,
        public_base_url: cli.public_base_url,
        service_name: cli.service_name,
        runner_lease_ttl_secs: cli.runner_lease_ttl_secs,
    })?;

    match cli.command.unwrap_or(Command::Serve) {
        Command::Doctor => println!(
            "{}",
            serde_json::to_string_pretty(&describe_status(&config))?
        ),
        Command::Serve => {
            let bind = config.bind;
            let app = ControlPlaneService::new(config, env!("CARGO_PKG_VERSION")).router();
            let listener = tokio::net::TcpListener::bind(bind).await?;
            axum::serve(listener, app).await?;
        }
    }
    Ok(())
}

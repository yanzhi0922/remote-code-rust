use std::net::SocketAddr;

use anyhow::Result;
use clap::Parser;
use rc_control_plane::{ControlPlaneMeta, router};
use rc_telemetry::install_tracing;

#[derive(Parser, Debug)]
#[command(
    name = "remote-code-control-plane",
    version,
    about = "Rust control plane skeleton"
)]
struct Cli {
    #[arg(long, default_value = "127.0.0.1:8787")]
    bind: SocketAddr,
}

#[tokio::main]
async fn main() -> Result<()> {
    install_tracing("remote_code_control_plane", false)?;
    let cli = Cli::parse();
    let app = router(ControlPlaneMeta {
        service: "remote-code-control-plane".to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        phase: "phase0-skeleton".to_owned(),
    });
    let listener = tokio::net::TcpListener::bind(cli.bind).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

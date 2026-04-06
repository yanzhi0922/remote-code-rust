use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand};
use rc_runner::{
    RunnerApi, RunnerConfigOverrides, describe_status, load_runner_config,
    register_with_control_plane, send_heartbeat,
};
use rc_telemetry::install_tracing;
use tracing::warn;

#[derive(Parser, Debug)]
#[command(name = "remote-code-runner", version, about = "Rust runner service")]
struct Cli {
    #[arg(long, env = "REMOTE_CODE_RUNNER_ID")]
    runner_id: Option<String>,

    #[arg(long, env = "REMOTE_CODE_CONTROL_PLANE_URL")]
    control_plane_url: Option<String>,

    #[arg(long, env = "REMOTE_CODE_RUNNER_BIND")]
    bind: Option<SocketAddr>,

    #[arg(long, env = "REMOTE_CODE_RUNNER_PUBLIC_BASE_URL")]
    public_base_url: Option<String>,

    #[arg(long, env = "REMOTE_CODE_RUNNER_HEARTBEAT_SECS")]
    heartbeat_interval_secs: Option<u64>,

    #[arg(long, env = "REMOTE_CODE_RUNNER_MAX_PARALLEL_SESSIONS")]
    max_parallel_sessions: Option<u16>,

    #[arg(long, env = "REMOTE_CODE_PROFILE_DIR")]
    profile_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    Doctor,
    PrintConfig,
    Serve,
}

#[tokio::main]
async fn main() -> Result<()> {
    install_tracing("remote_code_runner", false)?;
    let cli = Cli::parse();
    let config = load_runner_config(
        cli.profile_dir,
        RunnerConfigOverrides {
            runner_id: cli.runner_id,
            control_plane_url: cli.control_plane_url,
            bind: cli.bind,
            public_base_url: cli.public_base_url,
            heartbeat_interval_secs: cli.heartbeat_interval_secs,
            max_parallel_sessions: cli.max_parallel_sessions,
            ..RunnerConfigOverrides::default()
        },
    )?;

    match cli.command.unwrap_or(Command::Doctor) {
        Command::Doctor => println!(
            "{}",
            serde_json::to_string_pretty(&describe_status(&config)?)?
        ),
        Command::PrintConfig => println!("{}", serde_json::to_string_pretty(&config)?),
        Command::Serve => {
            let bind = config.bind;
            let api = RunnerApi::new(
                config.clone(),
                "remote-code-runner",
                env!("CARGO_PKG_VERSION"),
            );
            if let Some(control_plane_url) = config.control_plane_url.clone() {
                let registration = config.registration_request();
                let lease = register_with_control_plane(&control_plane_url, &registration).await?;
                let heartbeat_api = api.clone();
                tokio::spawn(async move {
                    let interval_secs = (lease.lease_ttl_secs / 2).max(1);
                    let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
                    interval.tick().await;
                    loop {
                        interval.tick().await;
                        let heartbeat = heartbeat_api.heartbeat().await;
                        if let Err(error) = send_heartbeat(&control_plane_url, &heartbeat).await {
                            warn!("failed to send heartbeat to control plane: {error}");
                        }
                    }
                });
            }
            let app = api.router();
            let listener = tokio::net::TcpListener::bind(bind).await?;
            axum::serve(listener, app).await?;
        }
    }
    Ok(())
}

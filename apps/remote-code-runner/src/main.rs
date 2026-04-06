use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use rc_runner::{RunnerApi, RunnerConfigOverrides, describe_status, load_runner_config};
use rc_telemetry::install_tracing;

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
            let app =
                RunnerApi::new(config, "remote-code-runner", env!("CARGO_PKG_VERSION")).router();
            let listener = tokio::net::TcpListener::bind(bind).await?;
            axum::serve(listener, app).await?;
        }
    }
    Ok(())
}

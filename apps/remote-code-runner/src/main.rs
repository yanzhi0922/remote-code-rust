use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use rc_config::AppPaths;
use rc_runner::{RunnerConfig, describe_status};
use rc_telemetry::install_tracing;

#[derive(Parser, Debug)]
#[command(name = "remote-code-runner", version, about = "Rust runner skeleton")]
struct Cli {
    #[arg(long, default_value = "local-runner")]
    runner_id: String,

    #[arg(long)]
    control_plane_url: Option<String>,

    #[arg(long)]
    profile_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand, Debug)]
enum Command {
    Doctor,
}

fn main() -> Result<()> {
    install_tracing("remote_code_runner", false)?;
    let cli = Cli::parse();
    let paths = AppPaths::discover(cli.profile_dir)?;
    let status = describe_status(&RunnerConfig {
        runner_id: cli.runner_id,
        control_plane_url: cli.control_plane_url,
        profile_dir: paths,
    })?;
    match cli.command.unwrap_or(Command::Doctor) {
        Command::Doctor => println!("{}", serde_json::to_string_pretty(&status)?),
    }
    Ok(())
}

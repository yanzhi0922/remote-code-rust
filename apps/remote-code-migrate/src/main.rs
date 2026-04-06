use std::path::PathBuf;

use anyhow::Result;
use clap::{Parser, Subcommand};
use rc_config::{ProviderOverrides, import_legacy_profile, load_runtime_config};
use rc_core::{InputFormat, OutputFormat, PermissionMode};
use rc_session::SessionStore;
use rc_telemetry::install_tracing;

#[derive(Parser, Debug)]
#[command(
    name = "remote-code-migrate",
    version,
    about = "Legacy profile importer"
)]
struct Cli {
    #[arg(long)]
    profile_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug)]
enum Command {
    Import {
        #[arg(long)]
        source: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    install_tracing("remote_code_migrate", false)?;
    let cli = Cli::parse();
    let config = load_runtime_config(
        None,
        cli.profile_dir,
        None,
        PermissionMode::Default,
        InputFormat::Text,
        OutputFormat::Text,
        false,
        false,
        false,
        false,
        1,
        ProviderOverrides::default(),
    )?;
    let store = SessionStore::open(config.paths.clone())?;
    match cli.command {
        Command::Import { source } => {
            let summary = import_legacy_profile(source, store.paths())?;
            println!("{}", serde_json::to_string_pretty(&summary)?);
        }
    }
    Ok(())
}

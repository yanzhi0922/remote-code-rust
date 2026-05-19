use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use claude_control_plane::{
    ControlPlaneConfigOverrides, ControlPlaneService, describe_status, load_control_plane_config,
    quic::{QuicServerConfig, start_quic_listener},
};
use claude_telemetry::install_tracing;

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

    #[arg(long, env = "REMOTE_CODE_PROFILE_DIR")]
    profile_dir: Option<PathBuf>,

    #[arg(long, env = "REMOTE_CODE_CONTROL_PLANE_AUTH_TOKEN")]
    auth_token: Option<String>,

    #[arg(long, env = "REMOTE_CODE_CONTROL_PLANE_BOOTSTRAP_SECRET")]
    bootstrap_secret: Option<String>,

    #[arg(long, env = "REMOTE_CODE_CONTROL_PLANE_QUIC_BIND")]
    quic_bind: Option<SocketAddr>,

    #[arg(long, env = "REMOTE_CODE_CONTROL_PLANE_QUIC_CERT")]
    quic_cert_pem: Option<PathBuf>,

    #[arg(long, env = "REMOTE_CODE_CONTROL_PLANE_QUIC_KEY")]
    quic_key_pem: Option<PathBuf>,

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
        profile_dir: cli.profile_dir,
        auth_token: cli.auth_token,
        bootstrap_secret: cli.bootstrap_secret,
        downloads_dir: None,
        quic_bind: cli.quic_bind,
        quic_cert_pem: cli.quic_cert_pem,
        quic_key_pem: cli.quic_key_pem,
    })?;

    match cli.command.unwrap_or(Command::Serve) {
        Command::Doctor => println!(
            "{}",
            serde_json::to_string_pretty(&describe_status(&config))?
        ),
        Command::Serve => {
            let status = describe_status(&config);
            if !status.ok {
                bail!(status.issues.join("; "));
            }
            let bind = config.bind;
            let service = ControlPlaneService::new(config.clone(), env!("CARGO_PKG_VERSION"));

            // QUIC is an explicit opt-in listener. It stays disabled by
            // default even when bind/cert/key are configured, because opening
            // a UDP listener changes the deployment's exposed surface.
            if let (Some(quic_bind), Some(cert_path), Some(key_path)) = (
                &config.quic_bind,
                &config.quic_cert_pem,
                &config.quic_key_pem,
            ) {
                if quic_enabled() {
                    let cert_pem = std::fs::read(cert_path).with_context(|| {
                        format!("reading QUIC cert from {}", cert_path.display())
                    })?;
                    let key_pem = std::fs::read(key_path)
                        .with_context(|| format!("reading QUIC key from {}", key_path.display()))?;
                    let quic_config = QuicServerConfig {
                        listen_addr: *quic_bind,
                        cert_pem,
                        key_pem,
                    };
                    let quic_service = std::sync::Arc::new(service.clone());
                    tokio::spawn(async move {
                        if let Err(e) = start_quic_listener(quic_service, quic_config).await {
                            eprintln!("QUIC listener failed: {e:#}");
                        }
                    });
                } else {
                    eprintln!(
                        "QUIC config is present but disabled; set REMOTE_CODE_CONTROL_PLANE_QUIC_ENABLE=1 to enable"
                    );
                }
            }

            let app = service.router();
            let listener = tokio::net::TcpListener::bind(bind).await?;
            axum::serve(listener, app).await?;
        }
    }
    Ok(())
}

fn env_flag_enabled(name: &str) -> bool {
    matches!(
        std::env::var(name)
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str(),
        "1" | "true" | "yes" | "on"
    )
}

fn quic_enabled() -> bool {
    env_flag_enabled("REMOTE_CODE_CONTROL_PLANE_QUIC_ENABLE")
        || env_flag_enabled("REMOTE_CODE_CONTROL_PLANE_QUIC_EXPERIMENTAL")
}

#[cfg(test)]
mod tests {
    use super::{env_flag_enabled, quic_enabled};

    #[test]
    fn quic_enable_uses_formal_env_name() {
        unsafe {
            std::env::remove_var("REMOTE_CODE_CONTROL_PLANE_QUIC_EXPERIMENTAL");
            std::env::set_var("REMOTE_CODE_CONTROL_PLANE_QUIC_ENABLE", "true");
            std::env::set_var("REMOTE_CODE_TEST_FLAG", "on");
        }
        assert!(quic_enabled());
        assert!(env_flag_enabled("REMOTE_CODE_TEST_FLAG"));
        unsafe {
            std::env::remove_var("REMOTE_CODE_CONTROL_PLANE_QUIC_ENABLE");
            std::env::remove_var("REMOTE_CODE_TEST_FLAG");
        }
    }
}

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use claude_telemetry::install_tracing;
use rc_control_plane::{
    ControlPlaneConfigOverrides, ControlPlaneService, describe_status, load_control_plane_config,
    quic::{QuicServerConfig, start_quic_listener},
};

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

            // Initialise the Prometheus metric registry so all metrics appear
            // in the /metrics output from the first scrape, even if no
            // observations have been recorded yet.
            let _ = rc_control_plane::metrics::encode_metrics();

            let bind = config.bind;
            let service = ControlPlaneService::new(config.clone(), env!("CARGO_PKG_VERSION"));

            // QUIC listener: auto-starts when cert/key are configured.
            // Opening a UDP listener changes the deployment's network surface,
            // so `REMOTE_CODE_CONTROL_PLANE_QUIC_DISABLE=1` can explicitly
            // suppress it even when cert/key are present.
            if let (Some(quic_bind), Some(cert_path), Some(key_path)) = (
                &config.quic_bind,
                &config.quic_cert_pem,
                &config.quic_key_pem,
            ) {
                if !quic_disabled() {
                    let cert_path = cert_path.clone();
                    let key_path = key_path.clone();
                    let (cert_pem, key_pem) = tokio::task::spawn_blocking(move || -> Result<_> {
                        let cert_pem = std::fs::read(&cert_path).with_context(|| {
                            format!("reading QUIC cert from {}", cert_path.display())
                        })?;
                        let key_pem = std::fs::read(&key_path).with_context(|| {
                            format!("reading QUIC key from {}", key_path.display())
                        })?;
                        Ok((cert_pem, key_pem))
                    })
                    .await
                    .context("QUIC cert/key read task panicked")??;
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
                    eprintln!("QUIC is disabled by REMOTE_CODE_CONTROL_PLANE_QUIC_DISABLE=1");
                }
            }

            let app = service.router();
            let listener = tokio::net::TcpListener::bind(bind).await?;
            axum::serve(listener, app).await?;
        }
    }
    Ok(())
}

fn env_flag(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|v| v.trim().to_ascii_lowercase())
}

fn env_flag_enabled(name: &str) -> bool {
    matches!(env_flag(name).as_deref(), Some("1" | "true" | "yes" | "on"))
}

/// QUIC is enabled by default (auto-starts when cert/key are configured).
/// Explicitly disable via `REMOTE_CODE_CONTROL_PLANE_QUIC_DISABLE=1`.
fn quic_disabled() -> bool {
    env_flag_enabled("REMOTE_CODE_CONTROL_PLANE_QUIC_DISABLE")
}

#[cfg(test)]
mod tests {
    use super::{env_flag_enabled, quic_disabled};

    #[test]
    fn quic_default_enabled_unless_disabled() {
        // SAFETY: `std::env::set_var` / `std::env::remove_var` are unsafe because the underlying
        // C runtime is not thread-safe and concurrent reads/writes can race.
        // This call is serialized by the surrounding guard (OnceLock, Mutex, or
        // single-threaded test context) so no other thread is reading the
        // variable concurrently.

        // Without QUIC_DISABLE, QUIC is not disabled (i.e. enabled by default).
        assert!(!quic_disabled());
        assert!(env_flag_enabled("REMOTE_CODE_TEST_FLAG"));
        // SAFETY: `std::env::set_var` / `std::env::remove_var` are unsafe because the underlying
        // C runtime is not thread-safe and concurrent reads/writes can race.
        // This call is serialized by the surrounding guard (OnceLock, Mutex, or
        // single-threaded test context) so no other thread is reading the
        // variable concurrently.

        // With QUIC_DISABLE, QUIC is disabled.
        assert!(quic_disabled());
        // SAFETY: `std::env::set_var` / `std::env::remove_var` are unsafe because the underlying
        // C runtime is not thread-safe and concurrent reads/writes can race.
        // This call is serialized by the surrounding guard (OnceLock, Mutex, or
        // single-threaded test context) so no other thread is reading the
        // variable concurrently.

    }
}

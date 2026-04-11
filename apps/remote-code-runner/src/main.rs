use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use clap::{Parser, Subcommand};
use rc_runner::{
    RunnerApi, RunnerConfig, RunnerConfigOverrides, describe_status, load_runner_config,
    register_with_control_plane, send_heartbeat,
};
use rc_telemetry::install_tracing;
use tokio::sync::watch;
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

fn effective_heartbeat_interval(configured_interval_secs: u64, lease_ttl_secs: u64) -> Duration {
    Duration::from_secs(
        configured_interval_secs
            .max(1)
            .min((lease_ttl_secs / 2).max(1)),
    )
}

fn next_retry_delay(current: Duration) -> Duration {
    current.saturating_mul(2).min(Duration::from_secs(30))
}

async fn wait_for_shutdown_or_timeout(
    duration: Duration,
    shutdown: &mut watch::Receiver<bool>,
) -> bool {
    tokio::select! {
        () = tokio::time::sleep(duration) => false,
        changed = shutdown.changed() => changed.is_err() || *shutdown.borrow(),
    }
}

async fn run_control_plane_sync(
    api: RunnerApi,
    config: RunnerConfig,
    mut shutdown: watch::Receiver<bool>,
) {
    let Some(control_plane_url) = config.control_plane_url.clone() else {
        return;
    };
    let registration = config.registration_request();
    let configured_interval_secs = config.heartbeat_interval_secs;
    let mut retry_delay = Duration::from_secs(1);

    loop {
        if *shutdown.borrow() {
            return;
        }

        match register_with_control_plane(&control_plane_url, &registration).await {
            Ok(lease) => {
                retry_delay = Duration::from_secs(1);
                let mut interval = tokio::time::interval(effective_heartbeat_interval(
                    configured_interval_secs,
                    lease.lease_ttl_secs,
                ));

                loop {
                    tokio::select! {
                        changed = shutdown.changed() => {
                            if changed.is_err() || *shutdown.borrow() {
                                return;
                            }
                        }
                        _ = interval.tick() => {
                            let heartbeat = api.heartbeat().await;
                            if let Err(error) = send_heartbeat(&control_plane_url, &heartbeat).await {
                                warn!("failed to send heartbeat to control plane: {error}");
                                break;
                            }
                        }
                    }
                }
            }
            Err(error) => warn!("failed to register runner with control plane: {error}"),
        }

        if wait_for_shutdown_or_timeout(retry_delay, &mut shutdown).await {
            return;
        }
        retry_delay = next_retry_delay(retry_delay);
    }
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
            let (_shutdown_tx, shutdown_rx) = watch::channel(false);
            if config.control_plane_url.is_some() {
                tokio::spawn(run_control_plane_sync(
                    api.clone(),
                    config.clone(),
                    shutdown_rx,
                ));
            }
            let app = api.router();
            let listener = tokio::net::TcpListener::bind(bind).await?;
            axum::serve(listener, app).await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use axum::{
        Json, Router,
        extract::{Path as AxumPath, State},
        http::StatusCode,
        response::IntoResponse,
        routing::post,
    };
    use chrono::Utc;
    use rc_runner::{
        RunnerHeartbeat, RunnerRegistrationLease, RunnerRegistrationRequest, RunnerSnapshot,
        RunnerState, RunnerWorkspace,
    };
    use tempfile::tempdir;

    #[derive(Clone, Default)]
    struct FakeControlPlaneState {
        register_count: Arc<AtomicUsize>,
        heartbeat_count: Arc<AtomicUsize>,
        registration: Arc<tokio::sync::RwLock<Option<RunnerRegistrationRequest>>>,
    }

    #[test]
    fn effective_heartbeat_interval_uses_config_without_exceeding_lease_half() {
        assert_eq!(effective_heartbeat_interval(15, 6), Duration::from_secs(3));
        assert_eq!(effective_heartbeat_interval(1, 60), Duration::from_secs(1));
        assert_eq!(effective_heartbeat_interval(0, 1), Duration::from_secs(1));
    }

    #[test]
    fn retry_delay_caps_at_thirty_seconds() {
        assert_eq!(
            next_retry_delay(Duration::from_secs(1)),
            Duration::from_secs(2)
        );
        assert_eq!(
            next_retry_delay(Duration::from_secs(16)),
            Duration::from_secs(30)
        );
        assert_eq!(
            next_retry_delay(Duration::from_secs(30)),
            Duration::from_secs(30)
        );
    }

    #[tokio::test]
    async fn control_plane_sync_re_registers_after_heartbeat_failure() {
        let state = FakeControlPlaneState::default();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener should bind");
        let address = listener.local_addr().expect("address should be readable");
        let server_state = state.clone();
        let server = tokio::spawn(async move {
            let app = Router::new()
                .route("/v1/runners/register", post(fake_register_runner))
                .route("/v1/runners/{runner_id}/heartbeat", post(fake_heartbeat))
                .with_state(server_state);
            axum::serve(listener, app).await.expect("server should run");
        });

        let profile = tempdir().expect("tempdir should exist");
        let config = load_runner_config(
            Some(profile.path().join("profile")),
            RunnerConfigOverrides {
                runner_id: Some("runner-loop".to_owned()),
                control_plane_url: Some(format!("http://{address}")),
                public_base_url: Some("http://127.0.0.1:9999".to_owned()),
                heartbeat_interval_secs: Some(1),
                workspaces: Some(vec![RunnerWorkspace {
                    workspace_id: "default".to_owned(),
                    root_dir: profile.path().join("workspace"),
                    writable: true,
                }]),
                ..RunnerConfigOverrides::default()
            },
        )
        .expect("config should load");
        let api = RunnerApi::new(config.clone(), "remote-code-runner", "0.1.0");
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let sync_task = tokio::spawn(run_control_plane_sync(api, config, shutdown_rx));

        tokio::time::timeout(Duration::from_secs(8), async {
            loop {
                if state.register_count.load(Ordering::SeqCst) >= 2
                    && state.heartbeat_count.load(Ordering::SeqCst) >= 2
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        })
        .await
        .expect("runner should re-register before timeout");

        shutdown_tx.send(true).expect("shutdown should send");
        tokio::time::timeout(Duration::from_secs(5), sync_task)
            .await
            .expect("sync task should stop")
            .expect("sync task should join");

        assert!(state.register_count.load(Ordering::SeqCst) >= 2);
        assert!(state.heartbeat_count.load(Ordering::SeqCst) >= 2);

        server.abort();
        let _ = server.await;
    }

    async fn fake_register_runner(
        State(state): State<FakeControlPlaneState>,
        Json(request): Json<RunnerRegistrationRequest>,
    ) -> Json<RunnerRegistrationLease> {
        state.register_count.fetch_add(1, Ordering::SeqCst);
        *state.registration.write().await = Some(request.clone());
        let now = Utc::now();
        Json(RunnerRegistrationLease {
            runner_id: request.runner_id.clone(),
            registered_at: now,
            lease_ttl_secs: 2,
            snapshot: RunnerSnapshot {
                registration: request,
                state: RunnerState::Idle,
                active_sessions: 0,
                queued_sessions: 0,
                registered_at: now,
                last_seen_at: now,
            },
        })
    }

    async fn fake_heartbeat(
        State(state): State<FakeControlPlaneState>,
        AxumPath(runner_id): AxumPath<String>,
        Json(heartbeat): Json<RunnerHeartbeat>,
    ) -> impl IntoResponse {
        let heartbeat_count = state.heartbeat_count.fetch_add(1, Ordering::SeqCst) + 1;
        if heartbeat_count == 1 {
            return StatusCode::NOT_FOUND.into_response();
        }

        let registration = state
            .registration
            .read()
            .await
            .clone()
            .expect("runner should be registered");
        let snapshot = RunnerSnapshot {
            registration,
            state: heartbeat.state,
            active_sessions: heartbeat.active_sessions,
            queued_sessions: heartbeat.queued_sessions,
            registered_at: Utc::now(),
            last_seen_at: heartbeat.timestamp,
        };
        debug_assert_eq!(runner_id, snapshot.registration.runner_id);
        Json(snapshot).into_response()
    }
}

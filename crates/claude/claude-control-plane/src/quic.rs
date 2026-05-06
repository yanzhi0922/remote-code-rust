//! QUIC server endpoint for the control plane.
//!
//! Accepts QUIC connections from mobile clients and streams timeline events
//! via unidirectional QUIC streams. Events flow server→client, commands
//! can flow client→server on separate streams.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::io::AsyncWriteExt;
use tokio::sync::broadcast;
use tokio::task::JoinSet;
use uuid::Uuid;

use crate::helpers;
use crate::state::ControlPlaneService;
use crate::types::{ApiError, TimelineEvent};

/// QUIC server configuration.
pub struct QuicServerConfig {
    pub listen_addr: SocketAddr,
    pub cert_pem: Vec<u8>,
    pub key_pem: Vec<u8>,
}

/// Start the QUIC listener alongside the HTTP server.
pub async fn start_quic_listener(
    service: Arc<ControlPlaneService>,
    config: QuicServerConfig,
) -> anyhow::Result<()> {
    let tls_config = build_quic_server_tls(&config.cert_pem, &config.key_pem)?;
    let server_config = quinn::ServerConfig::with_crypto(Arc::new(tls_config));

    let endpoint = quinn::Endpoint::server(server_config, config.listen_addr)?;
    let local_addr = endpoint.local_addr()?;
    tracing::info!("QUIC server listening on {local_addr}");

    let mut tasks: JoinSet<()> = JoinSet::new();

    loop {
        match endpoint.accept().await {
            Some(incoming) => {
                let service = service.clone();
                tasks.spawn(async move {
                    if let Err(e) = handle_quic_connection(incoming, service).await {
                        tracing::debug!("QUIC connection error: {e}");
                    }
                });
            }
            None => break,
        }

        while tasks.try_join_next().is_some() {}
    }

    Ok(())
}

async fn handle_quic_connection(
    incoming: quinn::Incoming,
    service: Arc<ControlPlaneService>,
) -> anyhow::Result<()> {
    let conn = incoming.await?;

    // Accept the first unidirectional stream — expect an auth message.
    let mut auth_stream = conn.accept_uni().await?;
    let mut len_buf = [0u8; 4];
    auth_stream.read_exact(&mut len_buf).await?;
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut auth_buf = vec![0u8; len];
    auth_stream.read_exact(&mut auth_buf).await?;

    let auth: QuicAuthMessage = serde_json::from_slice(&auth_buf)?;

    // Validate auth token — QUIC requires auth (no anonymous access).
    let expected = service.auth_token.as_deref().ok_or_else(|| {
        anyhow::anyhow!("QUIC server requires auth_token to be configured")
    })?;
    if !constant_time_eq(&auth.token, expected) {
        conn.close(1u32.into(), b"auth failed");
        return Err(anyhow::anyhow!("QUIC auth token mismatch"));
    }

    let target_session: Option<Uuid> = auth.session_id.parse().ok();
    tracing::debug!(
        "QUIC client authenticated for session {}",
        target_session.map_or("all".to_owned(), |s| s.to_string())
    );

    // Subscribe to events via the shared timeline broadcast channel.
    let mut event_rx = service.timeline.subscribe();

    loop {
        tokio::select! {
            event = event_rx.recv() => {
                match event {
                    Ok(event) => {
                        let matches_session = event.session_id == target_session
                            || event.session_id.is_none();
                        if matches_session {
                            if let Err(e) = send_quic_event(&conn, &event).await {
                                tracing::debug!("QUIC event send error: {e}");
                                break;
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("QUIC client lagged {n} events");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            // Accept command streams from client.
            cmd_stream = conn.accept_uni() => {
                match cmd_stream {
                    Ok(mut stream) => {
                        let mut len_buf = [0u8; 4];
                        if let Err(e) = stream.read_exact(&mut len_buf).await {
                            tracing::debug!("QUIC command read error: {e}");
                            break;
                        }
                        let len = u32::from_le_bytes(len_buf) as usize;
                        let mut buf = vec![0u8; len];
                        if let Err(e) = stream.read_exact(&mut buf).await {
                            tracing::debug!("QUIC command payload read error: {e}");
                            break;
                        }

                        // Dispatch the command to the runner queue.
                        if let Err(e) = dispatch_quic_command(&service, &buf).await {
                            tracing::warn!("QUIC command dispatch error: {e}");
                        }
                    }
                    Err(quinn::ConnectionError::ApplicationClosed(_)) => break,
                    Err(e) => {
                        tracing::debug!("QUIC command accept error: {e}");
                        break;
                    }
                }
            }
        }
    }

    conn.close(0u32.into(), b"done");
    Ok(())
}

/// Deserialize a QUIC command and enqueue it for the appropriate runner.
async fn dispatch_quic_command(
    service: &ControlPlaneService,
    payload: &[u8],
) -> anyhow::Result<()> {
    let cmd: QuicClientCommand = serde_json::from_slice(payload)?;

    match cmd {
        QuicClientCommand::SessionCommand { session_id, request } => {
            // Look up session → runner → enqueue.
            let runner_id = {
                let registry = service.registry.read().await;
                let session = registry
                    .get_session(session_id)
                    .map_err(|e: ApiError| anyhow::anyhow!("{}", e.message))?;
                session.owner_runner_id.ok_or_else(|| {
                    anyhow::anyhow!("session `{session_id}` is not assigned to a runner")
                })?
            };

            let runner = {
                let registry = service.registry.read().await;
                registry.runners.get(&runner_id).cloned().ok_or_else(|| {
                    anyhow::anyhow!("runner `{runner_id}` was not found")
                })?
            };

            if helpers::runner_uses_pull_commands(&runner) {
                let body = crate::types::RunnerQueuedCommandBody::SessionCommand {
                    session_id,
                    request,
                };
                let mut registry = service.registry.write().await;
                registry
                    .enqueue_runner_command(&runner_id, body)
                    .map_err(|e: ApiError| anyhow::anyhow!("{}", e.message))?;
            } else {
                helpers::dispatch_session_command_to_runner(
                    &service.http_client,
                    &runner,
                    session_id,
                    &request,
                )
                .await
                .map_err(|e: ApiError| {
                    anyhow::anyhow!(
                        "runner command relay failed for session {session_id}: {}",
                        e.message
                    )
                })?;
            }

            if let Err(e) = service.persist_state().await {
                tracing::error!("Failed to persist control plane state: {e:#}");
            }
            tracing::debug!(
                "QUIC command dispatched to runner {runner_id} for session {session_id}"
            );
        }
        QuicClientCommand::ApprovalDecision { approval_id, request } => {
            let (runner_id, _session_id) = {
                let registry = service.registry.read().await;
                let approval = registry.approvals.get(&approval_id).ok_or_else(|| {
                    anyhow::anyhow!("approval `{approval_id}` was not found")
                })?;
                let session_id = approval.session_id;
                let session = registry
                    .get_session(session_id)
                    .map_err(|e: ApiError| anyhow::anyhow!("{}", e.message))?;
                let runner_id = session.owner_runner_id.ok_or_else(|| {
                    anyhow::anyhow!("session `{session_id}` is not assigned to a runner")
                })?;
                (runner_id, session_id)
            };

            let body = crate::types::RunnerQueuedCommandBody::ApplyApprovalDecision {
                approval_id,
                request,
            };
            let mut registry = service.registry.write().await;
            registry
                .enqueue_runner_command(&runner_id, body)
                .map_err(|e: ApiError| anyhow::anyhow!("{}", e.message))?;

            if let Err(e) = service.persist_state().await {
                tracing::error!("Failed to persist control plane state: {e:#}");
            }
            tracing::debug!("QUIC approval decision dispatched to runner {runner_id}");
        }
    }

    Ok(())
}

async fn send_quic_event(
    conn: &quinn::Connection,
    event: &TimelineEvent,
) -> anyhow::Result<()> {
    let payload = serde_json::to_vec(event)?;
    let len = (payload.len() as u32).to_le_bytes();

    let mut stream = conn.open_uni().await?;
    stream.write_all(&len).await?;
    stream.write_all(&payload).await?;
    stream.shutdown().await?;
    Ok(())
}

fn build_quic_server_tls(
    cert_pem: &[u8],
    key_pem: &[u8],
) -> anyhow::Result<quinn::crypto::rustls::QuicServerConfig> {
    let cert = rustls_pemfile::certs(&mut std::io::Cursor::new(cert_pem))
        .collect::<Result<Vec<_>, _>>()?;
    let key = rustls_pemfile::private_key(&mut std::io::Cursor::new(key_pem))
        .map_err(|e| anyhow::anyhow!("read private key: {e}"))?
        .ok_or_else(|| anyhow::anyhow!("no private key found"))?;

    let mut tls_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert, key)
        .map_err(|e| anyhow::anyhow!("TLS config: {e}"))?;
    tls_config.alpn_protocols = vec![b"rc-quic/1".to_vec()];

    quinn::crypto::rustls::QuicServerConfig::try_from(Arc::new(tls_config))
        .map_err(|e| anyhow::anyhow!("QUIC server TLS: {e}"))
}

#[derive(serde::Deserialize)]
struct QuicAuthMessage {
    token: String,
    session_id: String,
}

/// Commands that QUIC clients can send to the control plane.
#[derive(serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum QuicClientCommand {
    SessionCommand {
        session_id: Uuid,
        #[serde(flatten)]
        request: claude_runner::RunnerSessionCommandRequest,
    },
    ApprovalDecision {
        approval_id: Uuid,
        request: claude_runner::ApprovalDecisionRequest,
    },
}

/// Constant-time comparison for auth tokens.
fn constant_time_eq(a: &str, b: &str) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result = 0u8;
    for (x, y) in a.bytes().zip(b.bytes()) {
        result |= x ^ y;
    }
    result == 0
}
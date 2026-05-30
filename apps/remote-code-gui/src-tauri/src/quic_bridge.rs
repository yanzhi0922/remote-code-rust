//! Tauri QUIC bridge — connects to QUIC server and forwards events to the frontend.

use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use tokio::sync::Mutex;

use rc_remote_transport::QuicTransport;
use rc_remote_transport::{RemoteTransport, TransportCommand, TransportConfig, TransportStrategy};

type SharedQuicState = Arc<Mutex<Option<QuicBridge>>>;

pub(crate) struct QuicBridge {
    pub(crate) transport: QuicTransport,
}

pub struct QuicBridgeState(pub SharedQuicState);

impl QuicBridgeState {
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(None)))
    }
}

#[tauri::command]
pub async fn quic_connect(
    app: AppHandle,
    state: State<'_, QuicBridgeState>,
    url: String,
    token: String,
    session_id: String,
    server_cert_fingerprint: Option<String>,
) -> std::result::Result<(), String> {
    let config = TransportConfig {
        strategy: TransportStrategy::Quic {
            server_url: url,
            server_cert_fingerprint,
        },
        auth_token: token,
        session_id,
        after_sequence: 0,
        tls: rc_remote_transport::TlsConfig::default(),
        reconnect: rc_remote_transport::ReconnectPolicy::default(),
    };

    let mut transport = QuicTransport::new(rc_remote_transport::ReconnectPolicy::default());
    transport.connect(config).await.map_err(|e| {
        let msg = format!("{e:#}");
        tracing::warn!(error = %msg, "command error");
        msg
    })?;

    // Take the event receiver and forward events to the Tauri frontend.
    if let Some(mut event_rx) = transport.take_event_receiver() {
        let app_handle = app.clone();
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                let payload = serde_json::to_value(&event).unwrap_or_default();
                let _ = app_handle.emit("quic-event", payload);
            }
        });
    }

    // Disconnect any existing connection before replacing it.
    {
        let mut guard = state.0.lock().await;
        if let Some(mut old_bridge) = guard.take() {
            tracing::info!("Disconnecting previous QUIC connection before establishing new one");
            if let Err(e) = old_bridge.transport.disconnect().await {
                tracing::warn!("Error disconnecting previous QUIC connection: {e:#}");
            }
        }
        *guard = Some(QuicBridge { transport });
    }

    Ok(())
}

#[tauri::command]
pub async fn quic_send_command(
    state: State<'_, QuicBridgeState>,
    command: String,
) -> std::result::Result<String, String> {
    let guard = state.0.lock().await;
    let bridge = guard.as_ref().ok_or("QUIC not connected")?;

    let cmd: TransportCommand = serde_json::from_str(&command).map_err(|e| {
        let msg = format!("{e}");
        tracing::warn!(error = %msg, "command error");
        msg
    })?;
    let ack = bridge.transport.send_command(cmd).await.map_err(|e| {
        let msg = format!("{e}");
        tracing::warn!(error = %msg, "command error");
        msg
    })?;
    serde_json::to_string(&ack).map_err(|e| {
        let msg = format!("{e}");
        tracing::warn!(error = %msg, "command error");
        msg
    })
}

#[tauri::command]
pub async fn quic_disconnect(state: State<'_, QuicBridgeState>) -> std::result::Result<(), String> {
    let mut guard = state.0.lock().await;
    if let Some(mut bridge) = guard.take() {
        bridge.transport.disconnect().await.map_err(|e| {
            let msg = format!("{e}");
            tracing::warn!(error = %msg, "command error");
            msg
        })?;
    }
    Ok(())
}

#[tauri::command]
pub async fn quic_state(state: State<'_, QuicBridgeState>) -> std::result::Result<String, String> {
    let guard = state.0.lock().await;
    match guard.as_ref() {
        Some(bridge) => {
            let s = bridge.transport.state();
            serde_json::to_string(&s).map_err(|e| {
                let msg = format!("{e}");
                tracing::warn!(error = %msg, "command error");
                msg
            })
        }
        None => Ok("\"disconnected\"".to_owned()),
    }
}

/// Used by the mobile entry point; desktop builds don't reference this directly.
#[tauri::command]
#[allow(dead_code)]
pub async fn quic_health_probe(
    state: State<'_, QuicBridgeState>,
) -> std::result::Result<String, String> {
    let guard = state.0.lock().await;
    match guard.as_ref() {
        Some(bridge) => {
            let health = bridge.transport.health_probe().await;
            serde_json::to_string(&health).map_err(|e| {
                let msg = format!("{e}");
                tracing::warn!(error = %msg, "command error");
                msg
            })
        }
        None => Ok("null".to_owned()),
    }
}

/// Used by the mobile entry point; desktop builds don't reference this directly.
#[tauri::command]
#[allow(dead_code)]
pub async fn quic_get_metrics(
    state: State<'_, QuicBridgeState>,
) -> std::result::Result<String, String> {
    let guard = state.0.lock().await;
    match guard.as_ref() {
        Some(bridge) => {
            let metrics = bridge.transport.metrics();
            serde_json::to_string(&metrics).map_err(|e| {
                let msg = format!("{e}");
                tracing::warn!(error = %msg, "command error");
                msg
            })
        }
        None => Ok("null".to_owned()),
    }
}

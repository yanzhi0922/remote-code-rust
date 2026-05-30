//! Strategy 3: Outbound polling (Anthropic mode).
//!
//! The runner polls the control plane for commands; the mobile app
//! connects to the control plane for events via SSE/long-poll.
//! No inbound ports needed on the runner.

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::reconnect::ReconnectPolicy;
use crate::transport::{CommandAck, HealthStatus, RemoteTransport, TransportCommand};
use crate::{ConnectionState, TransportConfig, TransportEvent, TransportMetrics};

/// Outbound polling transport — mobile connects to control plane,
/// runner pulls commands via long-poll.
pub struct OutboundPollTransport {
    state: ConnectionState,
    config: Option<TransportConfig>,
    metrics: Arc<std::sync::Mutex<TransportMetrics>>,
    event_rx: Option<mpsc::Receiver<TransportEvent>>,
    client: reqwest::Client,
    #[allow(dead_code)]
    reconnect: ReconnectPolicy,
    cancel: tokio::sync::watch::Sender<bool>,
}

impl OutboundPollTransport {
    pub fn new(reconnect: ReconnectPolicy) -> Self {
        let (cancel, _) = tokio::sync::watch::channel(false);
        Self {
            state: ConnectionState::Disconnected,
            config: None,
            metrics: Arc::new(std::sync::Mutex::new(TransportMetrics::default())),
            event_rx: None,
            client: reqwest::Client::new(),
            reconnect,
            cancel,
        }
    }

    /// Create with a shared HTTP client to reuse TCP connections and TLS sessions.
    pub fn with_client(reconnect: ReconnectPolicy, client: reqwest::Client) -> Self {
        let (cancel, _) = tokio::sync::watch::channel(false);
        Self {
            state: ConnectionState::Disconnected,
            config: None,
            metrics: Arc::new(std::sync::Mutex::new(TransportMetrics::default())),
            event_rx: None,
            client,
            reconnect,
            cancel,
        }
    }
}

#[async_trait]
impl RemoteTransport for OutboundPollTransport {
    async fn connect(&mut self, config: TransportConfig) -> anyhow::Result<()> {
        let (cp_url, poll_ms) = match &config.strategy {
            crate::TransportStrategy::OutboundPolling {
                control_plane_url,
                poll_interval_ms,
            } => (control_plane_url.clone(), *poll_interval_ms),
            _ => anyhow::bail!("OutboundPollTransport requires OutboundPolling strategy"),
        };

        let is_reconnect = self.config.is_some();
        self.config = Some(config.clone());
        self.state = ConnectionState::Connecting;

        // Enforce HTTPS if the TLS config requires it.
        if config.tls.enforce_https {
            let lowered = cp_url.to_ascii_lowercase();
            if !lowered.starts_with("https://") {
                self.state = ConnectionState::Error("HTTPS enforced but URL is not secure".into());
                anyhow::bail!(
                    "HTTPS is enforced but URL is not secure: {}",
                    cp_url
                );
            }
        }

        let sse_url = format!(
            "{cp_url}/v1/sessions/{}/events/stream?after={}",
            config.session_id, config.after_sequence
        );

        let (tx, rx) = mpsc::channel(256);
        self.event_rx = Some(rx);

        let cancel_rx = self.cancel.subscribe();
        let metrics_clone = self.metrics.clone();
        tokio::spawn(poll_events_loop(
            sse_url,
            poll_ms,
            tx,
            cancel_rx,
            self.client.clone(),
            config.auth_token.clone(),
            metrics_clone,
        ));

        {
            let mut m = self.metrics.lock().unwrap();
            if is_reconnect {
                m.reconnect_count += 1;
            }
            m.latency_ms = poll_ms;
            m.last_event_at = Some(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
            );
        }

        self.state = ConnectionState::Open {
            active_strategy: "outbound_polling".into(),
            latency_ms: poll_ms,
        };
        Ok(())
    }

    async fn send_command(&self, command: TransportCommand) -> anyhow::Result<CommandAck> {
        let config = self
            .config
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("not connected"))?;
        let cp_url = match &config.strategy {
            crate::TransportStrategy::OutboundPolling {
                control_plane_url, ..
            } => control_plane_url,
            _ => anyhow::bail!("internal error: strategy mismatch, expected OutboundPolling"),
        };

        let (path, body) = super::direct_ws::command_to_request(&command, &config.session_id);
        let url = format!("{cp_url}{path}");

        let body_size = body.to_string().len() as u64;

        let response = self
            .client
            .post(&url)
            .bearer_auth(&config.auth_token)
            .json(&body)
            .send()
            .await?;

        {
            let mut m = self.metrics.lock().unwrap();
            m.bytes_sent += body_size;
        }

        if response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            {
                let mut m = self.metrics.lock().unwrap();
                m.bytes_received += text.len() as u64;
            }
            Ok(CommandAck {
                accepted: true,
                message: "ok".into(),
            })
        } else {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            Ok(CommandAck {
                accepted: false,
                message: format!("HTTP {status}: {text}"),
            })
        }
    }

    async fn health_probe(&self) -> HealthStatus {
        let config = self.config.as_ref();
        if let Some(config) = config {
            let cp_url = match &config.strategy {
                crate::TransportStrategy::OutboundPolling {
                    control_plane_url, ..
                } => control_plane_url,
                _ => {
                    return HealthStatus {
                        endpoints: vec![],
                        recommended_strategy: None,
                    };
                }
            };
            let health_url = format!("{cp_url}/healthz");
            let health = crate::health::probe_endpoint(
                &health_url,
                Some(&config.auth_token),
                std::time::Duration::from_secs(5),
            )
            .await;
            HealthStatus {
                endpoints: vec![health],
                recommended_strategy: Some("outbound_polling".into()),
            }
        } else {
            HealthStatus {
                endpoints: vec![],
                recommended_strategy: None,
            }
        }
    }

    async fn disconnect(&mut self) -> anyhow::Result<()> {
        let _ = self.cancel.send(true);
        self.event_rx = None;
        self.state = ConnectionState::Closed;
        Ok(())
    }

    fn state(&self) -> ConnectionState {
        self.state.clone()
    }

    fn active_strategy(&self) -> &str {
        "outbound_polling"
    }

    fn metrics(&self) -> TransportMetrics {
        self.metrics.lock().unwrap().clone()
    }
}

/// Long-poll event loop: repeatedly GETs the SSE endpoint,
/// parses events, tracks the highest sequence, and forwards
/// them to the channel. Updates the `after=` query parameter
/// so each poll only fetches new events.
async fn poll_events_loop(
    mut sse_url: String,
    poll_interval_ms: u32,
    tx: mpsc::Sender<TransportEvent>,
    mut cancel: tokio::sync::watch::Receiver<bool>,
    client: reqwest::Client,
    auth_token: String,
    metrics: Arc<std::sync::Mutex<TransportMetrics>>,
) {
    let interval = std::time::Duration::from_millis(poll_interval_ms as u64);

    loop {
        if *cancel.borrow() {
            break;
        }

        match client.get(&sse_url).bearer_auth(&auth_token).send().await {
            Ok(response) => {
                if response.status().is_success() {
                    // Limit response body to 64 MiB to prevent unbounded memory
                    // allocation from a malicious or misbehaving SSE endpoint.
                    const MAX_SSE_BODY_BYTES: u64 = 64 * 1024 * 1024;
                    let body = match response.content_length() {
                        Some(len) if len > MAX_SSE_BODY_BYTES => {
                            tracing::warn!("SSE response body too large ({len} bytes), skipping");
                            continue;
                        }
                        _ => {
                            let bytes = response.bytes().await.unwrap_or_default();
                            if bytes.len() as u64 > MAX_SSE_BODY_BYTES {
                                tracing::warn!(
                                    "SSE response body exceeded {} bytes after read, skipping",
                                    MAX_SSE_BODY_BYTES
                                );
                                continue;
                            }
                            // Convert to String and immediately drop the Bytes
                            // to avoid holding both allocations simultaneously.
                            let text = String::from_utf8_lossy(&bytes).into_owned();
                            drop(bytes);
                            text
                        }
                    };
                    let mut max_seq: u64 = 0;
                    for line in body.lines() {
                        let event = if let Some(data) = line.strip_prefix("data: ") {
                            serde_json::from_str::<TransportEvent>(data)
                        } else {
                            serde_json::from_str::<TransportEvent>(line)
                        };
                        match event {
                            Ok(ev) => {
                                if ev.sequence > max_seq {
                                    max_seq = ev.sequence;
                                }
                                {
                                    let mut m = metrics.lock().unwrap();
                                    m.events_received += 1;
                                    m.last_event_at = Some(
                                        std::time::SystemTime::now()
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .unwrap_or_default()
                                            .as_secs() as i64,
                                    );
                                }
                                if tx.send(ev).await.is_err() {
                                    return;
                                }
                            }
                            Err(e) => {
                                if !line.is_empty() {
                                    tracing::debug!("poll event parse error: {e}");
                                }
                            }
                        }
                    }
                    // Update the after= parameter for the next poll so we
                    // only fetch events after the highest sequence seen.
                    if max_seq > 0 {
                        sse_url = update_after_param(&sse_url, max_seq);
                    }
                }
            }
            Err(e) => {
                tracing::debug!("poll error: {e}");
            }
        }

        tokio::select! {
            _ = tokio::time::sleep(interval) => {},
            _ = cancel.changed() => {
                if *cancel.borrow() {
                    break;
                }
            }
        }
    }
}

/// Replace or append the `after=` query parameter in a URL.
fn update_after_param(url: &str, new_after: u64) -> String {
    let (base, query) = url.split_once('?').unwrap_or((url, ""));
    if query.contains("after=") {
        let updated = query
            .split('&')
            .map(|pair| {
                if pair.starts_with("after=") {
                    format!("after={new_after}")
                } else {
                    pair.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("&");
        format!("{base}?{updated}")
    } else if query.is_empty() {
        format!("{base}?after={new_after}")
    } else {
        format!("{url}&after={new_after}")
    }
}

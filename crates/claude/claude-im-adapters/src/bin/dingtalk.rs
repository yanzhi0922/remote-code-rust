use std::sync::{Arc, OnceLock};

use anyhow::{Result, anyhow};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Json, Router};
use clap::Parser;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing_subscriber::EnvFilter;

use claude_im_adapters::webhook_auth::{validate_webhook_secret, verify_webhook_secret};
use claude_im_adapters::{ImBridge, ImMessage, ImResponse, ImSender};

/// Shared HTTP client for DingTalk API requests.
static DINGTALK_CLIENT: OnceLock<Client> = OnceLock::new();

fn shared_client() -> Client {
    DINGTALK_CLIENT.get_or_init(Client::new).clone()
}

#[derive(Debug, Deserialize)]
struct DingCallback {
    msgtype: Option<String>,
    text: Option<DingText>,
    sender_nick: Option<String>,
    conversation_id: Option<String>,
    chatgroup_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DingText {
    content: Option<String>,
}

#[derive(Debug, Serialize)]
struct DingSendRequest {
    msgtype: String,
    text: DingSendText,
}

#[derive(Debug, Serialize)]
struct DingSendText {
    content: String,
}

struct DingTalkSender {
    client: Client,
    access_token: String,
    webhook_url: Option<String>,
}

impl DingTalkSender {
    fn new(access_token: String, webhook_url: Option<String>) -> Self {
        Self {
            client: shared_client(),
            access_token,
            webhook_url,
        }
    }
}

#[async_trait::async_trait]
impl ImSender for DingTalkSender {
    async fn send(&self, response: ImResponse) -> Result<()> {
        let body = DingSendRequest {
            msgtype: "text".to_owned(),
            text: DingSendText {
                content: response.text,
            },
        };
        match &self.webhook_url {
            Some(url) => self.client.post(url).json(&body).send().await?,
            None => {
                let url = format!(
                    "https://oapi.dingtalk.com/robot/send?access_token={}",
                    self.access_token
                );
                self.client.post(&url).json(&body).send().await?
            }
        };
        Ok(())
    }
}

async fn webhook(
    Path(secret): Path<String>,
    State((bridge, expected_secret)): State<(Arc<ImBridge>, String)>,
    Json(cb): Json<DingCallback>,
) -> StatusCode {
    if !verify_webhook_secret(&secret, &expected_secret) {
        return StatusCode::UNAUTHORIZED;
    }

    if cb.msgtype.as_deref() != Some("text") {
        return StatusCode::OK;
    }
    let text = cb
        .text
        .and_then(|t| t.content)
        .unwrap_or_default()
        .trim()
        .to_owned();
    if text.is_empty() {
        return StatusCode::OK;
    }
    let chat_id = cb.conversation_id.or(cb.chatgroup_id).unwrap_or_default();
    if chat_id.is_empty() {
        return StatusCode::OK;
    }

    if let Err(e) = bridge
        .on_message(ImMessage {
            chat_id,
            text,
            sender_name: cb.sender_nick,
        })
        .await
    {
        tracing::error!("dingtalk message error: {e}");
    }
    StatusCode::OK
}

#[derive(Debug, Parser)]
#[command(
    name = "claude-im-dingtalk",
    about = "DingTalk adapter for claude-server"
)]
struct Cli {
    #[arg(long, env = "DINGTALK_ACCESS_TOKEN", default_value = "")]
    access_token: String,
    #[arg(long, env = "DINGTALK_WEBHOOK_URL")]
    webhook_url: Option<String>,
    #[arg(long, default_value = "http://127.0.0.1:9090")]
    server_url: String,
    #[arg(long)]
    auth_token: Option<String>,
    #[arg(long, env = "CLAUDE_IM_WEBHOOK_SECRET")]
    webhook_secret: Option<String>,
    #[arg(long, default_value = "127.0.0.1:3002")]
    bind: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env().add_directive("claude_im_adapters=info".parse()?),
        )
        .init();

    let bridge = Arc::new(ImBridge::new(
        cli.server_url.clone(),
        cli.auth_token.clone(),
        Arc::new(DingTalkSender::new(cli.access_token, cli.webhook_url)),
    ));
    bridge.spawn_response_dispatcher();

    let webhook_secret = cli
        .webhook_secret
        .as_deref()
        .ok_or_else(|| anyhow!("--webhook-secret is required"))?;
    validate_webhook_secret(webhook_secret)?;

    let app = Router::new()
        .route("/webhook/{secret}", axum::routing::post(webhook))
        .with_state((bridge, webhook_secret.to_owned()));
    let listener = tokio::net::TcpListener::bind(&cli.bind).await?;
    tracing::info!("dingtalk adapter listening on {}", cli.bind);
    axum::serve(listener, app).await?;
    Ok(())
}

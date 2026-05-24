use std::sync::Arc;

use anyhow::{Result, anyhow};
use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use clap::Parser;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing_subscriber::EnvFilter;

use claude_im_adapters::webhook_auth::{validate_webhook_secret, verify_webhook_secret};
use claude_im_adapters::{ImBridge, ImMessage, ImResponse, ImSender};

#[derive(Debug, Deserialize)]
struct FeishuUrlVerification {
    challenge: String,
}

#[derive(Debug, Deserialize)]
struct FeishuMessageEvent {
    chat_id: Option<String>,
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct FeishuTextContent {
    text: Option<String>,
}

#[derive(Debug, Serialize)]
struct FeishuSendRequest {
    receive_id: String,
    msg_type: String,
    content: String,
}

#[derive(Debug, Serialize)]
struct FeishuTextPayload {
    text: String,
}

#[derive(Debug, Deserialize)]
struct FeishuTokenResponse {
    tenant_access_token: Option<String>,
}

struct FeishuClient {
    http: Client,
    app_id: String,
    app_secret: String,
    token: tokio::sync::RwLock<Option<String>>,
}

impl FeishuClient {
    fn new(app_id: String, app_secret: String) -> Self {
        Self {
            http: Client::new(),
            app_id,
            app_secret,
            token: tokio::sync::RwLock::new(None),
        }
    }

    async fn get_token(&self) -> Result<String> {
        {
            let guard = self.token.read().await;
            if let Some(t) = guard.as_ref() {
                return Ok(t.clone());
            }
        }
        let resp: FeishuTokenResponse = self
            .http
            .post("https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal")
            .json(&serde_json::json!({ "app_id": self.app_id, "app_secret": self.app_secret }))
            .send()
            .await?
            .json()
            .await?;
        let token = resp
            .tenant_access_token
            .ok_or_else(|| anyhow::anyhow!("missing tenant_access_token"))?;
        *self.token.write().await = Some(token.clone());
        Ok(token)
    }
}

#[async_trait::async_trait]
impl ImSender for FeishuClient {
    async fn send(&self, response: ImResponse) -> Result<()> {
        let token = self.get_token().await?;
        let payload = FeishuTextPayload {
            text: response.text,
        };
        self.http
            .post("https://open.feishu.cn/open-apis/im/v1/messages?receive_id_type=chat_id")
            .bearer_auth(&token)
            .json(&FeishuSendRequest {
                receive_id: response.chat_id,
                msg_type: "text".to_owned(),
                content: serde_json::to_string(&payload)?,
            })
            .send()
            .await?;
        Ok(())
    }
}

async fn webhook(
    Path(secret): Path<String>,
    State((bridge, expected_secret)): State<(Arc<ImBridge>, String)>,
    body: String,
) -> (StatusCode, String) {
    if !verify_webhook_secret(&secret, &expected_secret) {
        return (StatusCode::UNAUTHORIZED, "{}".to_owned());
    }

    // URL verification challenge.
    if let Ok(v) = serde_json::from_str::<FeishuUrlVerification>(&body) {
        tracing::info!("feishu URL verification");
        return (
            StatusCode::OK,
            serde_json::json!({ "challenge": v.challenge }).to_string(),
        );
    }

    // Event callback.
    if let Ok(wrapper) = serde_json::from_str::<serde_json::Value>(&body) {
        if let Some(event) = wrapper.get("event") {
            if let Ok(msg) = serde_json::from_value::<FeishuMessageEvent>(event.clone()) {
                let chat_id = msg.chat_id.unwrap_or_default();
                let text = msg
                    .content
                    .and_then(|c| serde_json::from_str::<FeishuTextContent>(&c).ok())
                    .and_then(|c| c.text)
                    .unwrap_or_default();
                if !text.is_empty() && !chat_id.is_empty() {
                    if let Err(e) = bridge
                        .on_message(ImMessage {
                            chat_id,
                            text,
                            sender_name: None,
                        })
                        .await
                    {
                        tracing::error!("feishu message error: {e}");
                    }
                }
            }
        }
    }
    (StatusCode::OK, "{}".to_owned())
}

#[derive(Debug, Parser)]
#[command(
    name = "claude-im-feishu",
    about = "Feishu/Lark adapter for claude-server"
)]
struct Cli {
    #[arg(long, env = "FEISHU_APP_ID")]
    app_id: String,
    #[arg(long, env = "FEISHU_APP_SECRET")]
    app_secret: String,
    #[arg(long, default_value = "http://127.0.0.1:9090")]
    server_url: String,
    #[arg(long)]
    auth_token: Option<String>,
    #[arg(long, env = "CLAUDE_IM_WEBHOOK_SECRET")]
    webhook_secret: Option<String>,
    #[arg(long, default_value = "127.0.0.1:3001")]
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

    let sender: Arc<dyn ImSender> = Arc::new(FeishuClient::new(cli.app_id, cli.app_secret));
    let bridge = Arc::new(ImBridge::new(
        cli.server_url.clone(),
        cli.auth_token.clone(),
        sender,
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
    tracing::info!("feishu adapter listening on {}", cli.bind);
    axum::serve(listener, app).await?;
    Ok(())
}

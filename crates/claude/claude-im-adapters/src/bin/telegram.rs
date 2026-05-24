use std::sync::Arc;

use anyhow::{Result, anyhow};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Json, Router};
use clap::Parser;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing_subscriber::EnvFilter;

use claude_im_adapters::webhook_auth::{
    append_secret_to_webhook_url, validate_webhook_secret, verify_webhook_secret,
};
use claude_im_adapters::{ImBridge, ImMessage, ImResponse, ImSender};

#[derive(Debug, Deserialize)]
struct Update {
    update_id: i64,
    message: Option<TelegramMessage>,
}

#[derive(Debug, Deserialize)]
struct TelegramMessage {
    chat: TelegramChat,
    text: Option<String>,
    from: Option<TelegramUser>,
}

#[derive(Debug, Deserialize)]
struct TelegramChat {
    id: i64,
}

#[derive(Debug, Deserialize)]
struct TelegramUser {
    username: Option<String>,
}

#[derive(Debug, Serialize)]
struct SendMessageRequest {
    chat_id: i64,
    text: String,
}

#[derive(Debug, Serialize)]
struct SetWebhookRequest {
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    secret_token: Option<String>,
}

struct TelegramSender {
    client: Client,
    bot_token: String,
}

impl TelegramSender {
    fn new(bot_token: String) -> Self {
        Self {
            client: Client::new(),
            bot_token,
        }
    }
    fn api_url(&self, method: &str) -> String {
        format!("https://api.telegram.org/bot{}/{}", self.bot_token, method)
    }
}

#[async_trait::async_trait]
impl ImSender for TelegramSender {
    async fn send(&self, response: ImResponse) -> Result<()> {
        let chat_id: i64 = response.chat_id.parse()?;
        self.client
            .post(self.api_url("sendMessage"))
            .json(&SendMessageRequest {
                chat_id,
                text: response.text,
            })
            .send()
            .await?;
        Ok(())
    }
}

async fn webhook(
    Path(secret): Path<String>,
    State((bridge, expected_secret)): State<(Arc<ImBridge>, String)>,
    Json(update): Json<Update>,
) -> StatusCode {
    if !verify_webhook_secret(&secret, &expected_secret) {
        return StatusCode::UNAUTHORIZED;
    }

    if let Some(msg) = update.message {
        if let Some(text) = msg.text {
            let im_msg = ImMessage {
                chat_id: msg.chat.id.to_string(),
                text,
                sender_name: msg.from.as_ref().and_then(|u| u.username.clone()),
            };
            if let Err(e) = bridge.on_message(im_msg).await {
                tracing::error!("telegram message error: {e}");
            }
        }
    }
    StatusCode::OK
}

#[derive(Debug, Parser)]
#[command(
    name = "claude-im-telegram",
    about = "Telegram adapter for claude-server"
)]
struct Cli {
    #[arg(long, env = "TELEGRAM_BOT_TOKEN")]
    bot_token: String,
    #[arg(long, default_value = "http://127.0.0.1:9090")]
    server_url: String,
    #[arg(long)]
    auth_token: Option<String>,
    #[arg(long)]
    webhook_url: Option<String>,
    #[arg(long, env = "CLAUDE_IM_WEBHOOK_SECRET")]
    webhook_secret: Option<String>,
    #[arg(long, default_value = "127.0.0.1:3000")]
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
        Arc::new(TelegramSender::new(cli.bot_token.clone())),
    ));
    bridge.spawn_response_dispatcher();

    if let Some(webhook_url) = &cli.webhook_url {
        let webhook_secret = cli
            .webhook_secret
            .as_deref()
            .ok_or_else(|| anyhow!("--webhook-secret is required in webhook mode"))?;
        validate_webhook_secret(webhook_secret)?;
        let registered_url = append_secret_to_webhook_url(webhook_url, webhook_secret)?;

        let sender = TelegramSender::new(cli.bot_token.clone());
        sender
            .client
            .post(sender.api_url("setWebhook"))
            .json(&SetWebhookRequest {
                url: registered_url,
                secret_token: Some(webhook_secret.to_owned()),
            })
            .send()
            .await?;
        tracing::info!("webhook registered");

        let app = Router::new()
            .route("/webhook/{secret}", axum::routing::post(webhook))
            .with_state((bridge, webhook_secret.to_owned()));
        let listener = tokio::net::TcpListener::bind(&cli.bind).await?;
        tracing::info!("telegram webhook listening on {}", cli.bind);
        axum::serve(listener, app).await?;
    } else {
        tracing::info!("telegram adapter running in long-polling mode");
        let sender = TelegramSender::new(cli.bot_token.clone());
        let mut offset: Option<i64> = None;
        loop {
            let mut body = serde_json::json!({ "timeout": 30 });
            if let Some(off) = offset {
                body["offset"] = serde_json::json!(off + 1);
            }

            let resp = match sender
                .client
                .post(sender.api_url("getUpdates"))
                .json(&body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!("poll error: {e}");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    continue;
                }
            };

            if let Some(updates) = resp.json::<serde_json::Value>().await?["result"].as_array() {
                for val in updates {
                    if let Ok(update) = serde_json::from_value::<Update>(val.clone()) {
                        offset = Some(update.update_id);
                        if let Some(msg) = &update.message {
                            if let Some(text) = &msg.text {
                                let im_msg = ImMessage {
                                    chat_id: msg.chat.id.to_string(),
                                    text: text.clone(),
                                    sender_name: msg.from.as_ref().and_then(|u| u.username.clone()),
                                };
                                if let Err(e) = bridge.on_message(im_msg).await {
                                    tracing::error!("message error: {e}");
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

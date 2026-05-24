use std::sync::{Arc, OnceLock};

use anyhow::{Result, anyhow};
use axum::Router;
use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use clap::Parser;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing_subscriber::EnvFilter;

use claude_im_adapters::webhook_auth::{validate_webhook_secret, verify_webhook_secret};
use claude_im_adapters::{ImBridge, ImMessage, ImResponse, ImSender};

/// Shared HTTP client for WeChat API requests.
static WECHAT_CLIENT: OnceLock<Client> = OnceLock::new();

fn shared_client() -> Client {
    WECHAT_CLIENT.get_or_init(Client::new).clone()
}

#[derive(Debug, Deserialize)]
struct VerifyQuery {
    echostr: Option<String>,
}

#[derive(Debug, Serialize)]
struct WechatSendRequest {
    touser: String,
    msgtype: String,
    agentid: i64,
    text: WechatSendText,
}

#[derive(Debug, Serialize)]
struct WechatSendText {
    content: String,
}

#[derive(Debug, Deserialize)]
struct WechatTokenResponse {
    access_token: Option<String>,
    errcode: Option<i64>,
    errmsg: Option<String>,
}

struct WechatSender {
    http: Client,
    corp_id: String,
    corp_secret: String,
    agent_id: i64,
    token: tokio::sync::RwLock<Option<String>>,
}

impl WechatSender {
    fn new(corp_id: String, corp_secret: String, agent_id: i64) -> Self {
        Self {
            http: shared_client(),
            corp_id,
            corp_secret,
            agent_id,
            token: tokio::sync::RwLock::new(None),
        }
    }

    async fn get_token(&self) -> Result<String> {
        let mut guard = self.token.write().await;
        if let Some(t) = guard.as_ref() {
            return Ok(t.clone());
        }
        let url = format!(
            "https://qyapi.weixin.qq.com/cgi-bin/gettoken?corpid={}&corpsecret={}",
            self.corp_id, self.corp_secret,
        );
        let resp: WechatTokenResponse = self.http.get(&url).send().await?.json().await?;
        if let Some(code) = resp.errcode {
            if code != 0 {
                let message = resp.errmsg.as_deref().unwrap_or("unknown error");
                anyhow::bail!("wechat token error {code}: {message}");
            }
        }
        let token = resp
            .access_token
            .ok_or_else(|| anyhow::anyhow!("missing access_token"))?;
        *guard = Some(token.clone());
        Ok(token)
    }
}

#[async_trait::async_trait]
impl ImSender for WechatSender {
    async fn send(&self, response: ImResponse) -> Result<()> {
        let token = self.get_token().await?;
        let url = format!("https://qyapi.weixin.qq.com/cgi-bin/message/send?access_token={token}");
        self.http
            .post(&url)
            .json(&WechatSendRequest {
                touser: response.chat_id,
                msgtype: "text".to_owned(),
                agentid: self.agent_id,
                text: WechatSendText {
                    content: response.text,
                },
            })
            .send()
            .await?;
        Ok(())
    }
}

async fn verify(
    Path(secret): Path<String>,
    State((_, expected_secret)): State<(Arc<ImBridge>, String)>,
    Query(query): Query<VerifyQuery>,
) -> (StatusCode, String) {
    if !verify_webhook_secret(&secret, &expected_secret) {
        return (StatusCode::UNAUTHORIZED, String::new());
    }

    if let Some(echostr) = query.echostr {
        tracing::info!("wechat URL verification");
        return (StatusCode::OK, echostr);
    }
    (StatusCode::BAD_REQUEST, String::new())
}

async fn webhook(
    Path(secret): Path<String>,
    State((bridge, expected_secret)): State<(Arc<ImBridge>, String)>,
    body: Bytes,
) -> StatusCode {
    if !verify_webhook_secret(&secret, &expected_secret) {
        return StatusCode::UNAUTHORIZED;
    }

    let xml = String::from_utf8_lossy(&body);
    let content = extract_xml(&xml, "Content");
    let from_user = extract_xml(&xml, "FromUserName");
    let msg_type = extract_xml(&xml, "MsgType");

    if msg_type.as_deref() != Some("text") {
        return StatusCode::OK;
    }
    let text = content.unwrap_or_default();
    let chat_id = from_user.unwrap_or_default();
    if text.is_empty() || chat_id.is_empty() {
        return StatusCode::OK;
    }

    if let Err(e) = bridge
        .on_message(ImMessage {
            chat_id,
            text,
            sender_name: None,
        })
        .await
    {
        tracing::error!("wechat message error: {e}");
    }
    StatusCode::OK
}

fn extract_xml(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)?;
    let content_start = start + open.len();
    let end = xml[content_start..].find(&close)?;
    Some(xml[content_start..content_start + end].to_owned())
}

#[derive(Debug, Parser)]
#[command(
    name = "claude-im-wechat",
    about = "WeChat Work adapter for claude-server"
)]
struct Cli {
    #[arg(long, env = "WECHAT_CORP_ID")]
    corp_id: String,
    #[arg(long, env = "WECHAT_CORP_SECRET")]
    corp_secret: String,
    #[arg(long, env = "WECHAT_AGENT_ID", default_value = "0")]
    agent_id: i64,
    #[arg(long, default_value = "http://127.0.0.1:9090")]
    server_url: String,
    #[arg(long)]
    auth_token: Option<String>,
    #[arg(long, env = "CLAUDE_IM_WEBHOOK_SECRET")]
    webhook_secret: Option<String>,
    #[arg(long, default_value = "127.0.0.1:3003")]
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
        Arc::new(WechatSender::new(
            cli.corp_id,
            cli.corp_secret,
            cli.agent_id,
        )),
    ));
    bridge.spawn_response_dispatcher();

    let webhook_secret = cli
        .webhook_secret
        .as_deref()
        .ok_or_else(|| anyhow!("--webhook-secret is required"))?;
    validate_webhook_secret(webhook_secret)?;

    let app = Router::new()
        .route(
            "/webhook/{secret}",
            axum::routing::get(verify).post(webhook),
        )
        .with_state((bridge, webhook_secret.to_owned()));
    let listener = tokio::net::TcpListener::bind(&cli.bind).await?;
    tracing::info!("wechat work adapter listening on {}", cli.bind);
    axum::serve(listener, app).await?;
    Ok(())
}

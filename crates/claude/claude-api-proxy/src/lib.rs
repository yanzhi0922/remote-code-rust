pub mod auth;
pub mod config;
pub mod error;
pub mod router;
pub mod sse;
pub mod state;

mod anthropic;
mod health;
mod openai;
mod redaction;

use std::net::SocketAddr;

pub async fn serve(
    config: config::ProxyConfigFile,
    auth_token_overwrite: Option<String>,
    bind_overwrite: Option<SocketAddr>,
) -> anyhow::Result<()> {
    let model_index = config.build_model_index();
    let raw_auth = auth_token_overwrite.or(config.proxy.auth_token);
    let auth_token = raw_auth.filter(|t| !t.is_empty());
    let settings = config::ProxySettings {
        bind: bind_overwrite.unwrap_or(config.proxy.bind),
        auth_token,
        allow_unauthenticated: config.proxy.allow_unauthenticated,
    };

    tracing::info!("providers:");
    for (model, provider) in model_index.iter() {
        tracing::info!(
            "  {model} -> {} ({})",
            provider.name,
            provider.anthropic_url
        );
    }

    let bind_addr = settings.bind;
    if settings.auth_token.is_none() && !settings.allow_unauthenticated {
        anyhow::bail!(
            "claude-api-proxy requires authentication; pass --auth-token or set allow_unauthenticated = true for isolated local development"
        );
    }
    if settings.auth_token.is_none() && !settings.bind.ip().is_loopback() {
        anyhow::bail!("unauthenticated claude-api-proxy may only bind to a loopback address");
    }

    let state = state::ProxyState::new(settings, model_index);
    let app = router::build_router(state);

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    tracing::info!("proxy listening on {}", listener.local_addr()?);
    axum::serve(listener, app).await?;
    Ok(())
}

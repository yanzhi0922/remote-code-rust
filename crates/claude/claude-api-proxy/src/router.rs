use axum::Router;
use axum::middleware;
use axum::routing::{get, post};

use crate::auth;
use crate::state::ProxyState;

pub fn build_router(state: ProxyState) -> Router {
    let protected = Router::new()
        .route("/v1/messages", post(crate::anthropic::handle_anthropic))
        .route("/v1/chat/completions", post(crate::openai::handle_openai))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_auth,
        ));

    Router::new()
        .route("/healthz", get(crate::health::health_check))
        .merge(protected)
        .with_state(state)
}

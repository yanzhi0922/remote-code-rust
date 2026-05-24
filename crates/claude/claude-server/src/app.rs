use axum::Router;
use axum::middleware;
use axum::routing::get;

use crate::auth;
use crate::routes;
use crate::state::ServerState;
use crate::ws;

pub fn build_router(state: ServerState) -> Router {
    let protected = Router::new()
        .route(
            "/v1/sessions",
            get(routes::sessions::list_sessions).post(routes::sessions::create_session),
        )
        .route(
            "/v1/sessions/{id}",
            get(routes::sessions::get_session).delete(routes::sessions::delete_session),
        )
        .route(
            "/v1/sessions/{id}/messages",
            get(routes::sessions::get_messages),
        )
        .route("/v1/sessions/{id}/ws", get(ws::handler::ws_upgrade))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            auth::require_auth,
        ));

    Router::new()
        .route("/healthz", get(routes::health::health_check))
        .merge(protected)
        .with_state(state)
}

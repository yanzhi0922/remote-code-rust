use axum::Json;
use axum::extract::State;
use serde::Serialize;

use crate::state::ServerState;

#[derive(Serialize)]
pub struct HealthResponse {
    pub ok: bool,
    pub service: &'static str,
    pub version: &'static str,
}

pub async fn health_check(State(_state): State<ServerState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        ok: true,
        service: "claude-server",
        version: env!("CARGO_PKG_VERSION"),
    })
}

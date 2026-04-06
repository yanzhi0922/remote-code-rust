use axum::{Json, Router, routing::get};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ControlPlaneMeta {
    pub service: String,
    pub version: String,
    pub phase: String,
}

pub fn router(meta: ControlPlaneMeta) -> Router {
    Router::new()
        .route(
            "/healthz",
            get(|| async { Json(serde_json::json!({"ok": true})) }),
        )
        .route(
            "/v1/meta",
            get(move || {
                let meta = meta.clone();
                async move { Json(meta) }
            }),
        )
        .route(
            "/v1/sessions",
            get(|| async { Json(serde_json::json!({"items": []})) }),
        )
}

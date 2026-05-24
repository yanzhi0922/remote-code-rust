use axum::Json;
use axum::extract::State;
use serde_json::Value;

use crate::error::ApiError;
use crate::redaction::upstream_error_summary;
use crate::sse;
use crate::state::ProxyState;

pub async fn handle_anthropic(
    State(state): State<ProxyState>,
    Json(mut body): Json<Value>,
) -> Result<axum::response::Response, ApiError> {
    let model = body
        .get("model")
        .and_then(|m| m.as_str())
        .unwrap_or("")
        .to_owned();

    let provider = state
        .model_index
        .get(&model)
        .ok_or_else(|| ApiError::not_found(format!("unknown model: {model}")))?
        .clone();

    tracing::info!(model = %model, provider = %provider.name, "anthropic request");

    // Override model with provider's model name (in case client sent an alias).
    if let Some(obj) = body.as_object_mut() {
        obj.insert("model".to_owned(), Value::String(provider.model.clone()));
    }

    let is_stream = body
        .get("stream")
        .and_then(|s| s.as_bool())
        .unwrap_or(false);

    let url = format!(
        "{}/v1/messages",
        provider.anthropic_url.trim_end_matches('/')
    );
    let mut req = state
        .http
        .post(&url)
        .header("content-type", "application/json")
        .header("x-api-key", &provider.api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&body);

    if is_stream {
        req = req.header("accept", "text/event-stream");
    }

    let upstream = req
        .send()
        .await
        .map_err(|e| ApiError::upstream(format!("upstream request failed: {e}")))?;

    if !upstream.status().is_success() {
        let status = upstream.status();
        let body_text = upstream.text().await.unwrap_or_default();
        let body_summary = upstream_error_summary(&body_text);
        tracing::warn!(%status, body = %body_summary, "upstream error");
        return Err(ApiError::upstream(format!(
            "upstream {status}: {body_summary}"
        )));
    }

    if is_stream {
        Ok(sse::sse_response(upstream))
    } else {
        Ok(sse::json_response(upstream).await)
    }
}

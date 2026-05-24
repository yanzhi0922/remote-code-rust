use axum::body::Body;
use axum::http::{Response, StatusCode};
use axum::response::IntoResponse;
use futures::StreamExt;

pub fn sse_response(upstream: reqwest::Response) -> axum::response::Response {
    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let stream = upstream
        .bytes_stream()
        .map(|result| result.map_err(std::io::Error::other));
    Response::builder()
        .status(status)
        .header("content-type", "text/event-stream")
        .header("cache-control", "no-cache")
        .header("connection", "keep-alive")
        .body(Body::from_stream(stream))
        .unwrap()
}

pub async fn json_response(upstream: reqwest::Response) -> axum::response::Response {
    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let body = upstream.bytes().await;
    match body {
        Ok(bytes) => (
            status,
            [("content-type", "application/json")],
            bytes.to_vec(),
        )
            .into_response(),
        Err(e) => {
            crate::error::ApiError::upstream(format!("failed to read upstream response: {e}"))
                .into_response()
        }
    }
}

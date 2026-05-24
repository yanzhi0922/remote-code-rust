use axum::extract::{Request, State};
use axum::http::header;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use constant_time_eq::constant_time_eq_32;
use sha2::{Digest, Sha256};

use crate::error::ApiError;
use crate::state::ProxyState;

pub async fn require_auth(
    State(state): State<ProxyState>,
    request: Request,
    next: Next,
) -> Response {
    let Some(expected) = state.settings.auth_token.as_deref() else {
        return next.run(request).await;
    };

    if request_has_valid_bearer(&request, expected) {
        return next.run(request).await;
    }

    ApiError::unauthorized("missing or invalid bearer token".into()).into_response()
}

fn request_has_valid_bearer(request: &Request, expected: &str) -> bool {
    request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(str::trim)
        .is_some_and(|provided| constant_time_token_eq(provided, expected))
}

fn constant_time_token_eq(provided: &str, expected: &str) -> bool {
    let provided_digest: [u8; 32] = Sha256::digest(provided.as_bytes()).into();
    let expected_digest: [u8; 32] = Sha256::digest(expected.as_bytes()).into();
    constant_time_eq_32(&provided_digest, &expected_digest)
}

#[cfg(test)]
mod tests {
    use super::request_has_valid_bearer;
    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::header;

    #[test]
    fn query_tokens_do_not_authenticate_proxy_requests() {
        let request = Request::builder()
            .uri("/v1/messages?token=secret")
            .body(Body::empty())
            .expect("request should build");
        assert!(!request_has_valid_bearer(&request, "secret"));
    }

    #[test]
    fn bearer_header_authenticates_proxy_requests() {
        let request = Request::builder()
            .uri("/v1/messages?token=wrong")
            .header(header::AUTHORIZATION, "Bearer secret")
            .body(Body::empty())
            .expect("request should build");
        assert!(request_has_valid_bearer(&request, "secret"));
    }
}

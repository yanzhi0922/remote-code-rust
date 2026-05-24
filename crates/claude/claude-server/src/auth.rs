use axum::extract::{Request, State};
use axum::http::{Method, Uri, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use constant_time_eq::constant_time_eq_32;
use sha2::{Digest, Sha256};

use crate::error::ApiError;
use crate::state::ServerState;

/// Bearer token authentication middleware.
///
/// If `config.auth_token` is None, all requests pass through only when the
/// server was explicitly started in unauthenticated mode.
/// Otherwise, requires `Authorization: Bearer <token>` with constant-time comparison.
pub async fn require_auth(
    State(state): State<ServerState>,
    mut request: Request,
    next: Next,
) -> Response {
    let Some(expected) = state.config.auth_token.as_deref() else {
        return next.run(request).await;
    };

    if request_has_valid_bearer(&request, expected) {
        return next.run(request).await;
    }

    // Fallback: query-param auth for WebSocket connections (browsers can't set headers).
    let query_ok = request_allows_query_auth(&request)
        && request
            .uri()
            .query()
            .and_then(extract_auth_token_from_query)
            .is_some_and(|provided| constant_time_token_eq(&provided, expected));

    if query_ok {
        strip_auth_from_request_uri(&mut request);
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

fn request_allows_query_auth(request: &Request) -> bool {
    request.method() == Method::GET
        && request.uri().path().starts_with("/v1/sessions/")
        && request.uri().path().ends_with("/ws")
}

fn extract_auth_token_from_query(query: &str) -> Option<String> {
    for pair in query.split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let key = urlencoding::decode(key).ok()?;
        if key == "token" && !value.is_empty() {
            return urlencoding::decode(value).ok().map(|v| v.into_owned());
        }
    }
    None
}

fn strip_auth_from_request_uri(request: &mut Request) {
    let uri = request.uri().clone();
    let Some(query) = uri.query() else {
        return;
    };

    let cleaned: String = query
        .split('&')
        .filter(|pair| {
            let key = pair.split_once('=').map(|(key, _)| key).unwrap_or(*pair);
            let decoded_key = urlencoding::decode(key).unwrap_or_default();
            !matches!(decoded_key.as_ref(), "token" | "access_token")
        })
        .collect::<Vec<_>>()
        .join("&");

    let mut parts = uri.into_parts();
    let path = parts
        .path_and_query
        .as_ref()
        .map(|pq| pq.path())
        .unwrap_or("/");

    let new_path_and_query = if cleaned.is_empty() {
        path.to_owned()
    } else {
        format!("{path}?{cleaned}")
    };

    if let Ok(path_and_query) = new_path_and_query.parse() {
        parts.path_and_query = Some(path_and_query);
        if let Ok(cleaned_uri) = Uri::from_parts(parts) {
            *request.uri_mut() = cleaned_uri;
        }
    }
}

fn constant_time_token_eq(provided: &str, expected: &str) -> bool {
    let provided_digest: [u8; 32] = Sha256::digest(provided.as_bytes()).into();
    let expected_digest: [u8; 32] = Sha256::digest(expected.as_bytes()).into();
    constant_time_eq_32(&provided_digest, &expected_digest)
}

#[cfg(test)]
mod tests {
    use super::{request_allows_query_auth, strip_auth_from_request_uri};
    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::Method;

    fn request(method: Method, uri: &str) -> Request {
        Request::builder()
            .method(method)
            .uri(uri)
            .body(Body::empty())
            .expect("request should build")
    }

    #[test]
    fn query_auth_is_limited_to_session_websocket_gets() {
        assert!(request_allows_query_auth(&request(
            Method::GET,
            "/v1/sessions/550e8400-e29b-41d4-a716-446655440000/ws?token=secret",
        )));
        assert!(!request_allows_query_auth(&request(
            Method::POST,
            "/v1/sessions?token=secret",
        )));
        assert!(!request_allows_query_auth(&request(
            Method::GET,
            "/v1/sessions/550e8400-e29b-41d4-a716-446655440000/messages?token=secret",
        )));
    }

    #[test]
    fn strip_auth_removes_query_tokens_from_uri() {
        let mut req = request(
            Method::GET,
            "/v1/sessions/550e8400-e29b-41d4-a716-446655440000/ws?token=secret&keep=1&access_token=legacy",
        );
        strip_auth_from_request_uri(&mut req);
        assert_eq!(
            req.uri().to_string(),
            "/v1/sessions/550e8400-e29b-41d4-a716-446655440000/ws?keep=1"
        );
    }
}

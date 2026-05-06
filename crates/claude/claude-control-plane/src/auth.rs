//! Authentication middleware, CORS configuration, and token validation.

use axum::extract::{Request, State};
use axum::http::{
    Method,
    header::{AUTHORIZATION, CONTENT_DISPOSITION, CONTENT_TYPE},
};
use axum::middleware::Next;
use axum::response::IntoResponse;
use tower_http::cors::CorsLayer;

use crate::state::ControlPlaneService;
use crate::types::ApiError;
use crate::AuthPrincipal;

// ---------------------------------------------------------------------------
// CORS
// ---------------------------------------------------------------------------

pub(crate) fn build_cors_layer() -> CorsLayer {
    use axum::http::HeaderValue;

    let raw_origins = std::env::var("REMOTE_CODE_CORS_ORIGINS")
        .unwrap_or_else(|_| "http://localhost:*".to_owned());
    let origins: Vec<HeaderValue> = raw_origins
        .split(',')
        .filter_map(|o| o.trim().parse::<HeaderValue>().ok())
        .collect();

    let any_localhost = raw_origins.contains("http://localhost:*");
    if any_localhost {
        CorsLayer::new()
            .allow_origin(tower_http::cors::AllowOrigin::any())
            .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
            .allow_headers([CONTENT_TYPE, AUTHORIZATION, axum::http::header::ACCEPT])
            .expose_headers([CONTENT_DISPOSITION, CONTENT_TYPE])
    } else if origins.is_empty() {
        CorsLayer::new()
            .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
            .allow_headers([CONTENT_TYPE, AUTHORIZATION, axum::http::header::ACCEPT])
            .expose_headers([CONTENT_DISPOSITION, CONTENT_TYPE])
    } else {
        CorsLayer::new()
            .allow_origin(origins)
            .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
            .allow_headers([CONTENT_TYPE, AUTHORIZATION, axum::http::header::ACCEPT])
            .expose_headers([CONTENT_DISPOSITION, CONTENT_TYPE])
    }
}

// ---------------------------------------------------------------------------
// Secret hashing
// ---------------------------------------------------------------------------

pub(crate) fn hash_secret_value(raw: &str) -> String {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        #[allow(clippy::format_push_string)]
        encoded.push_str(&format!("{byte:02x}"));
    }
    encoded
}

// ---------------------------------------------------------------------------
// Auth middleware
// ---------------------------------------------------------------------------

pub(crate) async fn require_api_auth(
    State(service): State<ControlPlaneService>,
    mut request: Request,
    next: Next,
) -> axum::response::Response {
    // Explicit disable via env var (local dev).
    if std::env::var("REMOTE_CODE_REQUIRE_AUTH")
        .as_deref()
        .is_ok_and(|v| v.eq_ignore_ascii_case("false"))
    {
        return next.run(request).await;
    }

    // Bootstrap mode: no shared token, no bootstrap secret, AND no trusted
    // devices registered — allow open access.  This covers fresh instances
    // that have no auth configuration at all (e.g. unit tests).
    // When a bootstrap_secret IS configured, the instance is in "waiting for
    // owner claim" mode and protected routes still require auth.
    // IMPORTANT: drop the read lock before calling next.run() to avoid
    // deadlocking with handlers that need a write lock on the registry.
    if service.auth_token.is_none() && service.bootstrap_secret_hash.is_none() {
        let is_empty = {
            let registry = service.registry.read().await;
            registry.trusted_devices.is_empty()
        };
        if is_empty {
            return next.run(request).await;
        }
    }

    let Some(provided) = extract_request_auth_token(&mut request) else {
        return ApiError::unauthorized(
            "missing or invalid control plane bearer token".to_owned(),
        )
        .into_response();
    };

    if service
        .auth_token
        .as_deref()
        .is_some_and(|expected| constant_time_token_eq(&provided, expected))
    {
        request.extensions_mut().insert(AuthPrincipal::SharedToken);
        return next.run(request).await;
    }

    let authenticated_device = {
        let mut registry = service.registry.write().await;
        registry.authenticate_device_token(&provided)
    };
    if let Some((device, _is_access_token)) = authenticated_device {
        request
            .extensions_mut()
            .insert(AuthPrincipal::Device(device));
        return next.run(request).await;
    }

    ApiError::unauthorized("missing or invalid control plane bearer token".to_owned())
        .into_response()
}

fn extract_request_auth_token(request: &mut Request) -> Option<String> {
    let bearer = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    if bearer.is_some() {
        return bearer;
    }
    if !request_allows_query_auth(request) {
        return None;
    }
    let token = request
        .uri()
        .query()
        .and_then(extract_auth_token_from_query);
    // Strip token from URI to prevent it from appearing in access logs
    if token.is_some() {
        strip_auth_from_request_uri(request);
    }
    token
}

fn request_allows_query_auth(request: &Request) -> bool {
    // Only allow query-string auth for WebSocket upgrade endpoints
    let is_stream_path = request.uri().path().ends_with("/stream");
    if !is_stream_path {
        return false;
    }
    // Must be a WebSocket upgrade or a normal GET (for SSE)
    let is_ws_upgrade = request
        .headers()
        .get("upgrade")
        .is_some_and(|v| v.to_str().is_ok_and(|v| v.eq_ignore_ascii_case("websocket")));
    let is_get = request.method() == Method::GET;
    is_ws_upgrade || is_get
}

fn extract_auth_token_from_query(query: &str) -> Option<String> {
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next()?.trim();
        let value = parts.next().unwrap_or_default().trim();
        if matches!(key, "token" | "access_token") && !value.is_empty() {
            return Some(percent_decode_query_value(value));
        }
    }
    None
}

fn percent_decode_query_value(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => {
                decoded.push(b' ');
                index += 1;
            }
            b'%' if index + 2 < bytes.len() => {
                let high = bytes[index + 1] as char;
                let low = bytes[index + 2] as char;
                if let (Some(high), Some(low)) = (high.to_digit(16), low.to_digit(16)) {
                    decoded.push(((high << 4) | low) as u8);
                    index += 3;
                } else {
                    decoded.push(bytes[index]);
                    index += 1;
                }
            }
            byte => {
                decoded.push(byte);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&decoded).into_owned()
}

/// Strip access_token and token query parameters from the request URI
/// so they don't appear in access logs or error messages.
fn strip_auth_from_request_uri(request: &mut Request) {
    let uri = request.uri().clone();
    let Some(query) = uri.query() else {
        return;
    };
    let cleaned: String = query
        .split('&')
        .filter(|pair| {
            let key = pair.split('=').next().unwrap_or("").trim();
            !matches!(key, "token" | "access_token")
        })
        .collect::<Vec<_>>()
        .join("&");
    let new_uri = if cleaned.is_empty() {
        format!("{}?", uri.path())
    } else {
        format!("{}?{cleaned}", uri.path())
    };
    if let Ok(parsed) = new_uri.parse::<axum::http::Uri>() {
        *request.uri_mut() = parsed;
    }
}

fn constant_time_token_eq(provided: &str, expected: &str) -> bool {
    use sha2::{Digest, Sha256};

    let provided_digest: [u8; 32] = Sha256::digest(provided.as_bytes()).into();
    let expected_digest: [u8; 32] = Sha256::digest(expected.as_bytes()).into();
    constant_time_eq::constant_time_eq_32(&provided_digest, &expected_digest)
}
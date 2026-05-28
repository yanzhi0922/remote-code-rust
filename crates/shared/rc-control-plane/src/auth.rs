//! Authentication middleware, CORS configuration, and token validation.

use axum::extract::{Request, State};
use axum::http::{
    HeaderValue, Method,
    header::{AUTHORIZATION, CONTENT_DISPOSITION, CONTENT_TYPE},
};
use axum::middleware::Next;
use axum::response::IntoResponse;
use tower_http::cors::{AllowOrigin, CorsLayer};

use crate::AuthPrincipal;
use crate::metrics;
use crate::state::ControlPlaneService;
use crate::types::ApiError;

// Re-export percent_decode_query_value from rc-runner (dedup M10).
pub(crate) use rc_runner::percent_decode_query_value;

// ---------------------------------------------------------------------------
// CORS
// ---------------------------------------------------------------------------

pub(crate) fn build_cors_layer(public_base_url: Option<&str>) -> CorsLayer {
    let policy = CorsOriginPolicy::from_env(public_base_url);

    CorsLayer::new()
        .allow_origin(AllowOrigin::predicate(
            move |origin: &HeaderValue, _request_parts: &axum::http::request::Parts| {
                policy.allows(origin)
            },
        ))
        .allow_methods([Method::GET, Method::POST, Method::DELETE, Method::OPTIONS])
        .allow_headers([CONTENT_TYPE, AUTHORIZATION, axum::http::header::ACCEPT])
        .expose_headers([CONTENT_DISPOSITION, CONTENT_TYPE])
}

#[derive(Clone, Debug)]
struct CorsOriginPolicy {
    exact_origins: Vec<HeaderValue>,
    allow_http_loopback: bool,
    allow_https_loopback: bool,
}

impl CorsOriginPolicy {
    fn from_env(public_base_url: Option<&str>) -> Self {
        let mut policy = Self {
            exact_origins: Vec::new(),
            allow_http_loopback: true,
            allow_https_loopback: false,
        };

        if let Some(origin) = public_base_url.and_then(origin_from_url) {
            policy.add_exact_origin(&origin);
        }

        // The desktop WebView uses a non-HTTP origin.  It still needs a bearer
        // token; CORS just decides whether browser-style clients may send it.
        policy.add_exact_origin("tauri://localhost");
        policy.add_exact_origin("http://tauri.localhost");

        if let Ok(raw_origins) = std::env::var("REMOTE_CODE_CORS_ORIGINS") {
            policy.apply_origin_list(&raw_origins);
        }

        policy
    }

    fn apply_origin_list(&mut self, raw_origins: &str) {
        self.exact_origins.clear();
        self.allow_http_loopback = false;
        self.allow_https_loopback = false;

        for origin in raw_origins.split(',').map(str::trim) {
            if origin.is_empty() {
                continue;
            }
            match origin {
                "*" => tracing::warn!(
                    "Ignoring REMOTE_CODE_CORS_ORIGINS=*; configure explicit trusted origins"
                ),
                "http://localhost:*" | "http://127.0.0.1:*" | "http://[::1]:*" => {
                    self.allow_http_loopback = true;
                }
                "https://localhost:*" | "https://127.0.0.1:*" | "https://[::1]:*" => {
                    self.allow_https_loopback = true;
                }
                _ => self.add_exact_origin(origin),
            }
        }
    }

    fn add_exact_origin(&mut self, origin: &str) {
        match origin.parse::<HeaderValue>() {
            Ok(header) if !self.exact_origins.contains(&header) => {
                self.exact_origins.push(header);
            }
            Ok(_) => {}
            Err(_) => tracing::warn!(origin, "Ignoring invalid CORS origin"),
        }
    }

    fn allows(&self, origin: &HeaderValue) -> bool {
        if self.exact_origins.iter().any(|allowed| allowed == origin) {
            return true;
        }

        let Ok(raw_origin) = origin.to_str() else {
            return false;
        };
        is_allowed_loopback_origin(
            raw_origin,
            self.allow_http_loopback,
            self.allow_https_loopback,
        )
    }
}

fn origin_from_url(raw_url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(raw_url).ok()?;
    let host = parsed.host_str()?;
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    Some(match parsed.port() {
        Some(port) => format!("{}://{}:{port}", parsed.scheme(), host),
        None => format!("{}://{}", parsed.scheme(), host),
    })
}

fn is_allowed_loopback_origin(raw_origin: &str, allow_http: bool, allow_https: bool) -> bool {
    let Ok(parsed) = reqwest::Url::parse(raw_origin) else {
        return false;
    };

    match parsed.scheme() {
        "http" if allow_http => {}
        "https" if allow_https => {}
        _ => return false,
    }

    match parsed.host_str() {
        Some("localhost") => true,
        Some(host) => {
            // reqwest::Url may return IPv6 addresses with or without brackets.
            let host = host.trim_start_matches('[').trim_end_matches(']');
            host.parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
        }
        None => false,
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
    hex::encode(digest)
}

pub(crate) fn derived_user_id_from_key(raw: &str) -> String {
    format!("sha256:{}", hash_secret_value(raw))
}

// ---------------------------------------------------------------------------
// Auth middleware
// ---------------------------------------------------------------------------

pub(crate) async fn require_api_auth(
    State(service): State<ControlPlaneService>,
    mut request: Request,
    next: Next,
) -> axum::response::Response {
    // Security: auth bypass is compile-time gated to debug builds via
    // cfg!(debug_assertions). In release mode this branch is eliminated entirely
    // and REMOTE_CODE_REQUIRE_AUTH=false has no effect.
    if cfg!(debug_assertions)
        && std::env::var("REMOTE_CODE_REQUIRE_AUTH")
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
            request.extensions_mut().insert(AuthPrincipal::SharedToken);
            return next.run(request).await;
        }
    }

    if let Some(principal) = consume_stream_ticket(&service, &mut request).await {
        metrics::record_auth("stream_ticket", true);
        request.extensions_mut().insert(principal);
        return next.run(request).await;
    }

    let Some(provided) = extract_request_auth_token(&mut request) else {
        metrics::record_auth("none", false);
        return ApiError::unauthorized("missing or invalid control plane bearer token".to_owned())
            .into_response();
    };

    if service
        .auth_token
        .as_deref()
        .is_some_and(|expected| constant_time_value_eq(&provided, expected))
    {
        metrics::record_auth("shared_token", true);
        request.extensions_mut().insert(AuthPrincipal::SharedToken);
        return next.run(request).await;
    }

    let authenticated_device = {
        let mut registry = service.registry.write().await;
        registry.authenticate_device_token(&provided)
    };
    if let Some((device, is_access_token)) = authenticated_device {
        if !is_access_token {
            metrics::record_auth("device_refresh_token", false);
            return ApiError::unauthorized(
                "refresh tokens must be exchanged at /v1/auth/refresh before calling protected APIs"
                    .to_owned(),
            )
            .into_response();
        }
        metrics::record_auth("device_token", true);
        request
            .extensions_mut()
            .insert(AuthPrincipal::Device(device));
        return next.run(request).await;
    }

    if request_allows_tenant_user_auth(&request) && service.accepts_derived_user_key(&provided) {
        metrics::record_auth("user_key", true);
        request.extensions_mut().insert(AuthPrincipal::User {
            user_id: derived_user_id_from_key(&provided),
        });
        return next.run(request).await;
    }

    metrics::record_auth("invalid_token", false);
    ApiError::unauthorized("missing or invalid control plane bearer token".to_owned())
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::{
        constant_time_value_eq, derived_user_id_from_key, extract_auth_token_from_query,
        extract_request_auth_token, extract_stream_ticket_from_query, hash_secret_value,
        is_allowed_loopback_origin, origin_from_url, request_allows_query_auth,
        strip_auth_from_request_uri,
    };
    use axum::body::Body;
    use axum::http::{Method, Request, Uri, header::AUTHORIZATION};
    use std::env;

    // -----------------------------------------------------------------------
    // hash_secret_value / derived_user_id_from_key
    // -----------------------------------------------------------------------

    #[test]
    fn derived_user_id_does_not_store_raw_user_key() {
        let raw_key = "user-key-with-enough-entropy";
        let derived = derived_user_id_from_key(raw_key);

        assert_ne!(derived, raw_key);
        assert!(derived.starts_with("sha256:"));
        assert_eq!(derived, derived_user_id_from_key(raw_key));
    }

    #[test]
    fn hash_secret_value_is_deterministic() {
        let a = hash_secret_value("hello");
        let b = hash_secret_value("hello");
        assert_eq!(a, b);
        // Different inputs must produce different hashes.
        assert_ne!(a, hash_secret_value("world"));
    }

    #[test]
    fn hash_secret_value_returns_hex_sha256() {
        // SHA-256 of empty string is well-known.
        let empty_hash = hash_secret_value("");
        assert_eq!(
            empty_hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(empty_hash.len(), 64);
    }

    // -----------------------------------------------------------------------
    // constant_time_value_eq
    // -----------------------------------------------------------------------

    #[test]
    fn constant_time_eq_same_value() {
        assert!(constant_time_value_eq("secret", "secret"));
    }

    #[test]
    fn constant_time_eq_different_values() {
        assert!(!constant_time_value_eq("secret", "different"));
    }

    #[test]
    fn constant_time_eq_empty_strings() {
        assert!(constant_time_value_eq("", ""));
    }

    #[test]
    fn constant_time_eq_one_empty_one_not() {
        assert!(!constant_time_value_eq("", "non-empty"));
        assert!(!constant_time_value_eq("non-empty", ""));
    }

    #[test]
    fn constant_time_eq_different_lengths() {
        // Timing-safe: different lengths must still return false.
        assert!(!constant_time_value_eq("short", "a-much-longer-value"));
        assert!(!constant_time_value_eq("a-much-longer-value", "short"));
    }

    #[test]
    fn constant_time_eq_case_sensitive() {
        assert!(!constant_time_value_eq("Secret", "secret"));
        assert!(!constant_time_value_eq("SECRET", "secret"));
    }

    #[test]
    fn constant_time_eq_unicode_values() {
        let s = "ünïcödé-tökën-🔐";
        assert!(constant_time_value_eq(s, s));
        assert!(!constant_time_value_eq(s, "ünïcödé-tökën-🔒"));
    }

    // -----------------------------------------------------------------------
    // extract_auth_token_from_query
    // -----------------------------------------------------------------------

    #[test]
    fn extract_token_from_query_token_key() {
        assert_eq!(
            extract_auth_token_from_query("token=abc123"),
            Some("abc123".to_owned())
        );
    }

    #[test]
    fn extract_token_from_query_access_token_key() {
        assert_eq!(
            extract_auth_token_from_query("access_token=abc123"),
            Some("abc123".to_owned())
        );
    }

    #[test]
    fn extract_token_from_query_no_token_param() {
        assert_eq!(extract_auth_token_from_query("foo=bar&baz=qux"), None);
    }

    #[test]
    fn extract_token_from_query_empty_value() {
        // Empty value should be ignored.
        assert_eq!(extract_auth_token_from_query("token="), None);
        assert_eq!(extract_auth_token_from_query("token"), None);
    }

    #[test]
    fn extract_token_from_query_multiple_params() {
        assert_eq!(
            extract_auth_token_from_query("foo=bar&token=secret123&baz=qux"),
            Some("secret123".to_owned())
        );
    }

    #[test]
    fn extract_token_from_query_first_match_wins() {
        // When both token and access_token appear, first match wins.
        let result = extract_auth_token_from_query("token=first&access_token=second");
        assert_eq!(result, Some("first".to_owned()));
    }

    #[test]
    fn extract_token_from_query_percent_encoded() {
        // The function delegates to percent_decode_query_value.
        assert_eq!(
            extract_auth_token_from_query("token=abc%20def"),
            Some("abc def".to_owned())
        );
    }

    // -----------------------------------------------------------------------
    // extract_stream_ticket_from_query
    // -----------------------------------------------------------------------

    #[test]
    fn extract_stream_ticket_present() {
        assert_eq!(
            extract_stream_ticket_from_query("stream_ticket=ticket-123"),
            Some("ticket-123".to_owned())
        );
    }

    #[test]
    fn extract_stream_ticket_missing() {
        assert_eq!(extract_stream_ticket_from_query("token=abc"), None);
    }

    #[test]
    fn extract_stream_ticket_empty_value() {
        assert_eq!(extract_stream_ticket_from_query("stream_ticket="), None);
    }

    #[test]
    fn extract_stream_ticket_with_other_params() {
        assert_eq!(
            extract_stream_ticket_from_query("foo=bar&stream_ticket=tk-42&baz=qux"),
            Some("tk-42".to_owned())
        );
    }

    // -----------------------------------------------------------------------
    // strip_auth_from_request_uri
    // -----------------------------------------------------------------------

    fn request_with_uri(uri: &str) -> axum::extract::Request {
        let parsed: Uri = uri.parse().expect("valid URI");
        Request::builder().uri(parsed).body(Body::empty()).unwrap()
    }

    #[test]
    fn strip_auth_removes_token_param() {
        let mut req = request_with_uri("/v1/stream?token=secret123");
        strip_auth_from_request_uri(&mut req);
        assert_eq!(req.uri().to_string(), "/v1/stream");
    }

    #[test]
    fn strip_auth_removes_access_token_param() {
        let mut req = request_with_uri("/v1/stream?access_token=secret");
        strip_auth_from_request_uri(&mut req);
        assert_eq!(req.uri().to_string(), "/v1/stream");
    }

    #[test]
    fn strip_auth_removes_stream_ticket_param() {
        let mut req = request_with_uri("/v1/stream?stream_ticket=tk-1");
        strip_auth_from_request_uri(&mut req);
        assert_eq!(req.uri().to_string(), "/v1/stream");
    }

    #[test]
    fn strip_auth_preserves_non_auth_params() {
        let mut req = request_with_uri("/v1/stream?foo=bar&token=secret&baz=qux");
        strip_auth_from_request_uri(&mut req);
        assert_eq!(req.uri().to_string(), "/v1/stream?foo=bar&baz=qux");
    }

    #[test]
    fn strip_auth_no_query_string() {
        let mut req = request_with_uri("/v1/stream");
        strip_auth_from_request_uri(&mut req);
        assert_eq!(req.uri().to_string(), "/v1/stream");
    }

    #[test]
    fn strip_auth_only_auth_params() {
        // When all params are auth params, query is removed entirely.
        let mut req = request_with_uri("/v1/stream?token=secret&access_token=other");
        strip_auth_from_request_uri(&mut req);
        assert_eq!(req.uri().to_string(), "/v1/stream");
    }

    // -----------------------------------------------------------------------
    // extract_request_auth_token
    // -----------------------------------------------------------------------

    #[test]
    fn extract_request_auth_token_bearer_header() {
        let mut req = Request::builder()
            .header(AUTHORIZATION, "Bearer my-token-123")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            extract_request_auth_token(&mut req),
            Some("my-token-123".to_owned())
        );
    }

    #[test]
    fn extract_request_auth_token_bearer_with_whitespace() {
        let mut req = Request::builder()
            .header(AUTHORIZATION, "Bearer   spaced-token   ")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            extract_request_auth_token(&mut req),
            Some("spaced-token".to_owned())
        );
    }

    #[test]
    fn extract_request_auth_token_no_header() {
        let mut req = Request::builder().body(Body::empty()).unwrap();
        // Without REMOTE_CODE_ALLOW_QUERY_ACCESS_TOKEN and no Authorization
        // header, result is None.
        assert_eq!(extract_request_auth_token(&mut req), None);
    }

    #[test]
    fn extract_request_auth_token_empty_bearer() {
        let mut req = Request::builder()
            .header(AUTHORIZATION, "Bearer ")
            .body(Body::empty())
            .unwrap();
        assert_eq!(extract_request_auth_token(&mut req), None);
    }

    #[test]
    fn extract_request_auth_token_wrong_scheme() {
        let mut req = Request::builder()
            .header(AUTHORIZATION, "Basic dXNlcjpwYXNz")
            .body(Body::empty())
            .unwrap();
        assert_eq!(extract_request_auth_token(&mut req), None);
    }

    #[test]
    fn extract_request_auth_token_query_fallback() {
        // Set the env var to enable legacy query auth, build a GET request
        // to a /stream path, and verify token is extracted from query.
        // SAFETY: test-only, single-threaded env mutation. Guard restores on drop.
        struct EnvVarGuard(&'static str, Option<std::ffi::OsString>);
        impl Drop for EnvVarGuard {
            fn drop(&mut self) {
                match &self.1 {
                    Some(v) => unsafe { env::set_var(self.0, v) },
                    None => unsafe { env::remove_var(self.0) },
                }
            }
        }
        let prev = env::var_os("REMOTE_CODE_ALLOW_QUERY_ACCESS_TOKEN");
        unsafe { env::set_var("REMOTE_CODE_ALLOW_QUERY_ACCESS_TOKEN", "true") };
        let _guard = EnvVarGuard("REMOTE_CODE_ALLOW_QUERY_ACCESS_TOKEN", prev);

        let mut req = Request::builder()
            .method(Method::GET)
            .uri("/v1/sessions/abc/stream?token=query-token")
            .body(Body::empty())
            .unwrap();
        let result = extract_request_auth_token(&mut req);
        assert_eq!(result, Some("query-token".to_owned()));
        // After extraction, auth param should be stripped from the URI.
        assert_eq!(req.uri().to_string(), "/v1/sessions/abc/stream");
    }

    // -----------------------------------------------------------------------
    // request_allows_query_auth
    // -----------------------------------------------------------------------

    #[test]
    fn query_auth_allows_get_to_stream() {
        let req = Request::builder()
            .method(Method::GET)
            .uri("/v1/sessions/abc/stream")
            .body(Body::empty())
            .unwrap();
        assert!(request_allows_query_auth(&req));
    }

    #[test]
    fn query_auth_allows_ws_upgrade_to_stream() {
        let req = Request::builder()
            .method(Method::GET)
            .uri("/v1/sessions/abc/stream")
            .header("upgrade", "websocket")
            .body(Body::empty())
            .unwrap();
        assert!(request_allows_query_auth(&req));
    }

    #[test]
    fn query_auth_rejects_non_stream_path() {
        let req = Request::builder()
            .method(Method::GET)
            .uri("/v1/devices")
            .body(Body::empty())
            .unwrap();
        assert!(!request_allows_query_auth(&req));
    }

    #[test]
    fn query_auth_rejects_post_to_stream() {
        // POST without upgrade header is not allowed for query auth.
        let req = Request::builder()
            .method(Method::POST)
            .uri("/v1/sessions/abc/stream")
            .body(Body::empty())
            .unwrap();
        assert!(!request_allows_query_auth(&req));
    }

    #[test]
    fn query_auth_rejects_non_ws_upgrade() {
        let req = Request::builder()
            .method(Method::GET)
            .uri("/v1/sessions/abc/stream")
            .header("upgrade", "h2c")
            .body(Body::empty())
            .unwrap();
        // Non-websocket upgrade on GET is still OK (GET is allowed).
        assert!(request_allows_query_auth(&req));
    }

    #[test]
    fn query_auth_rejects_post_with_non_ws_upgrade() {
        let req = Request::builder()
            .method(Method::POST)
            .uri("/v1/sessions/abc/stream")
            .header("upgrade", "h2c")
            .body(Body::empty())
            .unwrap();
        // POST + non-ws upgrade = not allowed.
        assert!(!request_allows_query_auth(&req));
    }

    // -----------------------------------------------------------------------
    // origin_from_url
    // -----------------------------------------------------------------------

    #[test]
    fn origin_from_simple_url() {
        assert_eq!(
            origin_from_url("https://example.com/path"),
            Some("https://example.com".to_owned())
        );
    }

    #[test]
    fn origin_from_url_with_port() {
        assert_eq!(
            origin_from_url("http://localhost:3000/api"),
            Some("http://localhost:3000".to_owned())
        );
    }

    #[test]
    fn origin_from_url_with_ipv6() {
        assert_eq!(
            origin_from_url("http://[::1]:8080/path"),
            Some("http://[::1]:8080".to_owned())
        );
    }

    #[test]
    fn origin_from_url_invalid() {
        assert_eq!(origin_from_url("not-a-url"), None);
    }

    // -----------------------------------------------------------------------
    // is_allowed_loopback_origin
    // -----------------------------------------------------------------------

    #[test]
    fn loopback_allows_http_localhost() {
        assert!(is_allowed_loopback_origin(
            "http://localhost:3000",
            true,
            false
        ));
    }

    #[test]
    fn loopback_rejects_http_localhost_when_disabled() {
        assert!(!is_allowed_loopback_origin(
            "http://localhost:3000",
            false,
            false
        ));
    }

    #[test]
    fn loopback_allows_https_localhost() {
        assert!(is_allowed_loopback_origin(
            "https://localhost:443",
            false,
            true
        ));
    }

    #[test]
    fn loopback_allows_ipv4_loopback() {
        assert!(is_allowed_loopback_origin(
            "http://127.0.0.1:8080",
            true,
            false
        ));
    }

    #[test]
    fn loopback_allows_ipv6_loopback() {
        assert!(is_allowed_loopback_origin("http://[::1]:8080", true, false));
    }

    #[test]
    fn loopback_rejects_non_loopback_ip() {
        assert!(!is_allowed_loopback_origin(
            "http://192.168.1.1:8080",
            true,
            false
        ));
    }

    #[test]
    fn loopback_rejects_external_host() {
        assert!(!is_allowed_loopback_origin("http://evil.com", true, false));
    }

    #[test]
    fn loopback_rejects_invalid_url() {
        assert!(!is_allowed_loopback_origin("not-a-url", true, true));
    }
}

async fn consume_stream_ticket(
    service: &ControlPlaneService,
    request: &mut Request,
) -> Option<AuthPrincipal> {
    if !request_allows_query_auth(request) {
        return None;
    }
    let ticket = request
        .uri()
        .query()
        .and_then(extract_stream_ticket_from_query)?;
    strip_auth_from_request_uri(request);
    service
        .consume_stream_ticket(&ticket, request.uri().path())
        .await
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
    if !legacy_query_access_tokens_enabled() || !request_allows_query_auth(request) {
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
    let is_ws_upgrade = request.headers().get("upgrade").is_some_and(|v| {
        v.to_str()
            .is_ok_and(|v| v.eq_ignore_ascii_case("websocket"))
    });
    let is_get = request.method() == Method::GET;
    is_ws_upgrade || is_get
}

fn request_allows_tenant_user_auth(request: &Request) -> bool {
    let path = request.uri().path();
    let method = request.method();

    if path == "/v1/devices/push-token" && method == Method::POST {
        return true;
    }
    if path == "/v1/stream-ticket" && method == Method::POST {
        return true;
    }
    // Runner sub-paths — scoped to the minimum set actually needed.
    if runner_id_subpath_allows_tenant_user(path, method) {
        return true;
    }
    if path.starts_with("/v1/sessions") {
        return true;
    }
    if path.starts_with("/v1/approvals") {
        return true;
    }
    if path.starts_with("/v1/artifacts") {
        return true;
    }
    if path == "/v1/events" || path == "/v1/events/stream" {
        return true;
    }

    false
}

/// Explicit allowlist of runner sub-paths accessible to tenant users.
/// New runner sub-resources are NOT automatically reachable.
fn runner_id_subpath_allows_tenant_user(path: &str, method: &Method) -> bool {
    // POST /v1/runners (register)
    if path == "/v1/runners" && method == Method::POST {
        return true;
    }
    // GET /v1/runners (list)
    if path == "/v1/runners" && method == Method::GET {
        return true;
    }
    // /v1/runners/{id}/heartbeat
    if path.ends_with("/heartbeat") && method == Method::POST {
        return true;
    }
    // /v1/runners/{id}/sessions
    if path.ends_with("/sessions") && method == Method::POST {
        return true;
    }
    // /v1/runners/{id}/sessions/{sid}/command
    if path.ends_with("/command") && method == Method::POST {
        return true;
    }
    // /v1/runners/{id}/events
    if path.ends_with("/events") && method == Method::POST {
        return true;
    }
    // /v1/runners/{id}/poll-commands
    if path.ends_with("/poll-commands") && method == Method::POST {
        return true;
    }
    false
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

fn extract_stream_ticket_from_query(query: &str) -> Option<String> {
    for pair in query.split('&') {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next()?.trim();
        let value = parts.next().unwrap_or_default().trim();
        if key == "stream_ticket" && !value.is_empty() {
            return Some(percent_decode_query_value(value));
        }
    }
    None
}

/// Strip auth query parameters from the request URI
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
            !matches!(key, "token" | "access_token" | "stream_ticket")
        })
        .collect::<Vec<_>>()
        .join("&");
    let new_uri = if cleaned.is_empty() {
        uri.path().to_owned()
    } else {
        format!("{}?{cleaned}", uri.path())
    };
    if let Ok(parsed) = new_uri.parse::<axum::http::Uri>() {
        *request.uri_mut() = parsed;
    }
}

fn legacy_query_access_tokens_enabled() -> bool {
    std::env::var("REMOTE_CODE_ALLOW_QUERY_ACCESS_TOKEN")
        .as_deref()
        .is_ok_and(|value| value.eq_ignore_ascii_case("true"))
}

/// Constant-time comparison for secret values.
///
/// Hashes both values with SHA-256 first so that timing cannot reveal
/// length differences between the provided and expected values.
pub(crate) fn constant_time_value_eq(a: &str, b: &str) -> bool {
    use sha2::{Digest, Sha256};

    let a_digest: [u8; 32] = Sha256::digest(a.as_bytes()).into();
    let b_digest: [u8; 32] = Sha256::digest(b.as_bytes()).into();
    constant_time_eq::constant_time_eq_32(&a_digest, &b_digest)
}

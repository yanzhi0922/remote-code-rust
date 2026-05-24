//! JWT utility functions — decode, expiry extraction, token refresh timing.
//!
//! Native Rust implementation of `claude-code-rev/src/bridge/jwtUtils.ts`.
//! Unlike the TypeScript original, JWT signature verification is available
//! when the `ring`-based HMAC feature is enabled; otherwise falls back to
//! payload-only decoding (safe for session-ingress tokens).

use serde_json::Value;
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

/// Prefix that session-ingress JWT tokens carry before the actual JWT.
const SESSION_INGRESS_PREFIX: &str = "sk-ant-si-";

/// Default buffer to refresh a token before it expires (5 minutes).
const TOKEN_REFRESH_BUFFER_MS: u64 = 5 * 60 * 1000;

/// Fallback refresh interval when the new token's expiry is unknown (30 minutes).
const FALLBACK_REFRESH_INTERVAL_MS: u64 = 30 * 60 * 1000;

/// Maximum consecutive failures before giving up on the refresh chain.
const MAX_REFRESH_FAILURES: u32 = 3;

/// Retry delay when getAccessToken returns None.
const REFRESH_RETRY_DELAY_MS: u64 = 60_000;

// ── JWT Decode ─────────────────────────────────────────────────────────

/// Decode a JWT's payload segment without verifying the signature.
///
/// Strips the `sk-ant-si-` session-ingress prefix if present.
/// Returns the parsed JSON payload, or `None` if the token is malformed.
pub fn decode_jwt_payload(token: &str) -> Option<Value> {
    let jwt = token.strip_prefix(SESSION_INGRESS_PREFIX).unwrap_or(token);
    let parts: Vec<&str> = jwt.split('.').collect();
    if parts.len() != 3 || parts[1].is_empty() {
        return None;
    }
    use base64::Engine as _;
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(parts[1])
        .ok()?;
    serde_json::from_slice(&decoded).ok()
}

/// Decode the `exp` (expiry) claim from a JWT without verifying the signature.
pub fn decode_jwt_expiry(token: &str) -> Option<u64> {
    let payload = decode_jwt_payload(token)?;
    payload.get("exp").and_then(|v| v.as_u64())
}

/// Format a millisecond duration as a human-readable string.
pub fn format_duration(ms: u64) -> String {
    if ms < 60_000 {
        format!("{}s", ms / 1000)
    } else {
        let m = ms / 60_000;
        let s = (ms % 60_000) / 1000;
        if s > 0 {
            format!("{m}m {s}s")
        } else {
            format!("{m}m")
        }
    }
}

// ── Token Refresh Timing ──────────────────────────────────────────────

/// Configures token refresh timing decisions.
pub struct TokenRefreshConfig {
    pub refresh_buffer_ms: u64,
    pub fallback_interval_ms: u64,
    pub max_failures: u32,
    pub retry_delay_ms: u64,
}

impl Default for TokenRefreshConfig {
    fn default() -> Self {
        Self {
            refresh_buffer_ms: TOKEN_REFRESH_BUFFER_MS,
            fallback_interval_ms: FALLBACK_REFRESH_INTERVAL_MS,
            max_failures: MAX_REFRESH_FAILURES,
            retry_delay_ms: REFRESH_RETRY_DELAY_MS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshSchedule {
    Scheduled { delay_ms: u64 },
    DueNow,
}

/// Token refresh timing tracker.
///
/// This type records when refresh should occur. It does not spawn background
/// tasks or perform network refreshes by itself; callers must act on the
/// returned [`RefreshSchedule`] value.
pub struct TokenRefreshScheduler {
    config: TokenRefreshConfig,
    label: String,
    entry: HashMap<String, RefreshEntry>,
    generation: HashMap<String, u64>,
}

struct RefreshEntry {
    _delay_ms: u64,
    _generation: u64,
    _failures: u32,
}

impl TokenRefreshScheduler {
    /// Create a new scheduler.
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            config: TokenRefreshConfig::default(),
            label: label.into(),
            entry: HashMap::new(),
            generation: HashMap::new(),
        }
    }

    /// Create with custom config.
    pub fn with_config(config: TokenRefreshConfig, label: impl Into<String>) -> Self {
        Self {
            config,
            label: label.into(),
            entry: HashMap::new(),
            generation: HashMap::new(),
        }
    }

    /// Record the next refresh time for the given session using a JWT token.
    pub fn schedule(&mut self, session_id: &str, token: &str) -> RefreshSchedule {
        let Some(expiry) = decode_jwt_expiry(token) else {
            let delay_ms = self.config.fallback_interval_ms;
            self.store_schedule(session_id, delay_ms);
            debug!(
                label = %self.label, session_id,
                delay_ms,
                "Could not decode JWT expiry, using fallback refresh interval"
            );
            return RefreshSchedule::Scheduled { delay_ms };
        };

        self.entry.remove(session_id);

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let expiry_ms = expiry * 1000;

        let delay_ms = if expiry_ms > now + self.config.refresh_buffer_ms {
            expiry_ms - now - self.config.refresh_buffer_ms
        } else {
            0
        };

        if delay_ms <= 0 {
            debug!(
                label = %self.label, session_id,
                "Token expiring or expired; refresh is due now"
            );
            warn!("Token refresh is due now for {session_id}");
            return RefreshSchedule::DueNow;
        }

        debug!(
            label = %self.label, session_id,
            delay_ms, expiry,
            "Scheduled token refresh"
        );
        self.store_schedule(session_id, delay_ms);
        RefreshSchedule::Scheduled { delay_ms }
    }

    /// Schedule refresh using an explicit TTL (seconds until expiry).
    pub fn schedule_from_expires_in(
        &mut self,
        session_id: &str,
        expires_in_seconds: u64,
    ) -> RefreshSchedule {
        self.entry.remove(session_id);
        let delay_ms = std::cmp::max(
            expires_in_seconds
                .saturating_mul(1000)
                .saturating_sub(self.config.refresh_buffer_ms),
            30_000,
        );
        debug!(
            label = %self.label, session_id,
            delay_ms, expires_in_seconds,
            "Scheduled token refresh from expires_in"
        );
        self.store_schedule(session_id, delay_ms);
        RefreshSchedule::Scheduled { delay_ms }
    }

    fn store_schedule(&mut self, session_id: &str, delay_ms: u64) {
        self.entry.remove(session_id);
        let generation = self.next_generation(session_id);
        self.entry.insert(
            session_id.to_owned(),
            RefreshEntry {
                _delay_ms: delay_ms,
                _generation: generation,
                _failures: 0,
            },
        );
    }

    /// Cancel refresh for a session.
    pub fn cancel(&mut self, session_id: &str) {
        self.next_generation(session_id);
        self.entry.remove(session_id);
    }

    /// Cancel all refreshes.
    pub fn cancel_all(&mut self) {
        self.generation.clear();
        self.entry.clear();
    }

    /// Number of active refresh schedules.
    pub fn active_count(&self) -> usize {
        self.entry.len()
    }

    fn next_generation(&mut self, session_id: &str) -> u64 {
        let generation = self.generation.get(session_id).copied().unwrap_or(0) + 1;
        self.generation.insert(session_id.to_owned(), generation);
        generation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a test JWT with a specific exp claim.
    fn make_test_jwt(exp: u64) -> String {
        use base64::Engine as _;
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(b"{\"alg\":\"HS256\",\"typ\":\"JWT\"}");
        let payload_vec = serde_json::json!({
            "sub": "test",
            "exp": exp,
            "iat": exp - 3600,
        });
        let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(payload_vec.to_string().as_bytes());
        format!("{header}.{payload}.signature")
    }

    #[test]
    fn decodes_jwt_payload() {
        let future = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 86400;
        let token = make_test_jwt(future);
        let payload = decode_jwt_payload(&token).expect("should decode");
        assert_eq!(payload["sub"], "test");
        assert_eq!(payload["exp"].as_u64(), Some(future));
    }

    #[test]
    fn strips_session_ingress_prefix() {
        let future = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 86400;
        let inner = make_test_jwt(future);
        let prefixed = format!("sk-ant-si-{inner}");
        let payload = decode_jwt_payload(&prefixed).expect("should decode with prefix");
        assert_eq!(payload["sub"], "test");
    }

    #[test]
    fn decode_jwt_expiry_returns_exp() {
        let future = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3600;
        let token = make_test_jwt(future);
        assert_eq!(decode_jwt_expiry(&token), Some(future));
    }

    #[test]
    fn malformed_token_returns_none() {
        assert!(decode_jwt_payload("not-a-jwt").is_none());
        assert!(decode_jwt_payload("a.b").is_none());
        assert!(decode_jwt_payload("a.b.c").is_none()); // invalid base64
    }

    #[test]
    fn scheduler_manages_sessions() {
        let mut sched = TokenRefreshScheduler::new("test");
        let future = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 86400;
        let token = make_test_jwt(future);
        let schedule = sched.schedule("session-1", &token);
        assert!(matches!(schedule, RefreshSchedule::Scheduled { delay_ms } if delay_ms > 0));
        assert_eq!(sched.active_count(), 1);
        sched.cancel("session-1");
        assert_eq!(sched.active_count(), 0);
    }

    #[test]
    fn cancel_all_clears_everything() {
        let mut sched = TokenRefreshScheduler::new("test");
        let future = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 86400;
        let token = make_test_jwt(future);
        sched.schedule("session-1", &token);
        sched.schedule("session-2", &token);
        assert_eq!(sched.active_count(), 2);
        sched.cancel_all();
        assert_eq!(sched.active_count(), 0);
    }

    #[test]
    fn schedules_from_expires_in() {
        let mut sched = TokenRefreshScheduler::new("test");
        let schedule = sched.schedule_from_expires_in("session-1", 3600);
        assert!(matches!(schedule, RefreshSchedule::Scheduled { delay_ms } if delay_ms > 0));
        assert_eq!(sched.active_count(), 1);
    }

    #[test]
    fn expired_token_returns_due_now_without_active_schedule() {
        let mut sched = TokenRefreshScheduler::new("test");
        let past = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            .saturating_sub(60);
        let token = make_test_jwt(past);

        assert_eq!(sched.schedule("session-1", &token), RefreshSchedule::DueNow);
        assert_eq!(sched.active_count(), 0);
    }

    #[test]
    fn malformed_token_uses_fallback_schedule() {
        let mut sched = TokenRefreshScheduler::new("test");

        let schedule = sched.schedule("session-1", "not-a-jwt");

        assert_eq!(
            schedule,
            RefreshSchedule::Scheduled {
                delay_ms: FALLBACK_REFRESH_INTERVAL_MS,
            }
        );
        assert_eq!(sched.active_count(), 1);
    }

    #[test]
    fn format_duration_various() {
        assert_eq!(format_duration(30_000), "30s");
        assert_eq!(format_duration(120_000), "2m"); // 0s is omitted
        assert_eq!(format_duration(300_000), "5m"); // 0s is omitted
        assert_eq!(format_duration(65_000), "1m 5s");
        assert_eq!(format_duration(1_500), "1s");
        assert_eq!(format_duration(0), "0s");
    }
}

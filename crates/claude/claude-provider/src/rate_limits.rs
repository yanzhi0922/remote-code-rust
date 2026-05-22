//! Rate-limit header parsing and quota status extraction.
//!
//! Mirrors Claude Code's `services/claudeAiLimits.ts` —
//! `computeNewLimitsFromHeaders()` and `extractQuotaStatusFromHeaders()`.
//!
//! Parses `anthropic-ratelimit-unified-*` headers into structured types for
//! UI display, early-warning logic, and overage detection.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Quota status reported by the `anthropic-ratelimit-unified-status` header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuotaStatus {
    /// Request was allowed within quota.
    Allowed,
    /// Request was allowed but a usage warning threshold was surpassed.
    AllowedWarning,
    /// Request was rejected due to quota exhaustion.
    Rejected,
}

impl QuotaStatus {
    fn from_header(value: Option<&str>) -> Self {
        match value {
            Some("allowed_warning") => Self::AllowedWarning,
            Some("rejected") => Self::Rejected,
            _ => Self::Allowed,
        }
    }
}

/// Which rate-limit window triggered the rejection/warning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RateLimitType {
    /// 5-hour rolling window.
    FiveHour,
    /// 7-day rolling window (generic).
    SevenDay,
    /// 7-day Opus-specific window.
    SevenDayOpus,
    /// 7-day Sonnet-specific window.
    SevenDaySonnet,
    /// Overage / extra-usage window.
    Overage,
}

impl RateLimitType {
    fn from_claim_header(value: Option<&str>) -> Option<Self> {
        match value {
            Some("5h") => Some(Self::FiveHour),
            Some("7d") => Some(Self::SevenDay),
            Some("7d_opus") => Some(Self::SevenDayOpus),
            Some("7d_sonnet") => Some(Self::SevenDaySonnet),
            Some("overage") => Some(Self::Overage),
            _ => None,
        }
    }

    /// Human-readable display name matching TS `RATE_LIMIT_DISPLAY_NAMES`.
    #[must_use]
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::FiveHour => "session limit",
            Self::SevenDay => "weekly limit",
            Self::SevenDayOpus => "Opus limit",
            Self::SevenDaySonnet => "Sonnet limit",
            Self::Overage => "extra usage limit",
        }
    }
}

/// Reason overage billing is disabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverageDisabledReason {
    /// Organization has a spending cap that has been reached.
    OutOfCredits,
    /// Org-level overage disabled until the next billing period.
    OrgLevelDisabledUntil,
    /// Org service has a zero credit limit.
    OrgServiceZeroCreditLimit,
}

impl OverageDisabledReason {
    fn from_header(value: Option<&str>) -> Option<Self> {
        match value {
            Some("out_of_credits") => Some(Self::OutOfCredits),
            Some("org_level_disabled_until") => Some(Self::OrgLevelDisabledUntil),
            Some("org_service_zero_credit_limit") => Some(Self::OrgServiceZeroCreditLimit),
            _ => None,
        }
    }
}

/// Parsed rate-limit state extracted from response headers.
///
/// Mirrors TS `ClaudeAILimits` — the subset of fields relevant to
/// non-interactive / CLI usage (no React hooks or UI state).
#[derive(Debug, Clone, PartialEq)]
pub struct RateLimitState {
    /// Primary quota status.
    pub status: QuotaStatus,
    /// Unix timestamp (seconds) when the current limit window resets.
    pub resets_at: Option<f64>,
    /// Whether a fallback rate-limit tier is available.
    pub fallback_available: bool,
    /// Which rate-limit type triggered the current status.
    pub rate_limit_type: Option<RateLimitType>,
    /// Overage quota status (if overage headers are present).
    pub overage_status: Option<QuotaStatus>,
    /// Unix timestamp (seconds) when the overage window resets.
    pub overage_resets_at: Option<f64>,
    /// Reason overage is disabled (if present).
    pub overage_disabled_reason: Option<OverageDisabledReason>,
    /// Whether the user is currently consuming overage credits
    /// (standard limits rejected but overage allowed).
    pub is_using_overage: bool,
    /// 5-hour utilization percentage (0–100).
    pub utilization_5h: Option<f64>,
    /// 7-day utilization percentage (0–100).
    pub utilization_7d: Option<f64>,
    /// Overage utilization percentage (0–100).
    pub overage_utilization: Option<f64>,
}

impl Default for RateLimitState {
    fn default() -> Self {
        Self {
            status: QuotaStatus::Allowed,
            resets_at: None,
            fallback_available: false,
            rate_limit_type: None,
            overage_status: None,
            overage_resets_at: None,
            overage_disabled_reason: None,
            is_using_overage: false,
            utilization_5h: None,
            utilization_7d: None,
            overage_utilization: None,
        }
    }
}

impl RateLimitState {
    /// Compute the time remaining until the primary limit resets.
    ///
    /// Returns `None` if `resets_at` is not set or is in the past.
    #[must_use]
    pub fn time_until_reset(&self) -> Option<Duration> {
        let reset_ts = self.resets_at?;
        let now_ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_secs_f64();
        let remaining = (reset_ts - now_ts).max(0.0);
        if remaining > 0.0 {
            Some(Duration::from_secs_f64(remaining))
        } else {
            None
        }
    }

    /// Whether the user is currently rate-limited (rejected status).
    #[must_use]
    pub fn is_rate_limited(&self) -> bool {
        self.status == QuotaStatus::Rejected
    }

    /// Whether an early warning should be shown (allowed but near limit).
    #[must_use]
    pub fn is_warning(&self) -> bool {
        self.status == QuotaStatus::AllowedWarning
    }

    /// Human-readable text when the user is consuming overage credits.
    /// Matches TS `getUsingOverageText()`.
    #[must_use]
    pub fn overage_text(&self) -> Option<String> {
        if self.is_using_overage {
            Some(
                "You are currently using your overages to power your Claude Code usage. \
                 We will automatically switch you back to your subscription rate limits when they reset."
                    .to_owned(),
            )
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Header parsing
// ---------------------------------------------------------------------------

/// Extract rate-limit state from HTTP response headers.
///
/// Mirrors TS `computeNewLimitsFromHeaders()`.
pub fn extract_rate_limit_state(headers: &reqwest::header::HeaderMap) -> RateLimitState {
    let status = QuotaStatus::from_header(
        headers
            .get("anthropic-ratelimit-unified-status")
            .and_then(|v| v.to_str().ok()),
    );

    let resets_at = headers
        .get("anthropic-ratelimit-unified-reset")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<f64>().ok());

    let fallback_available = headers
        .get("anthropic-ratelimit-unified-fallback")
        .and_then(|v| v.to_str().ok())
        == Some("available");

    let rate_limit_type = RateLimitType::from_claim_header(
        headers
            .get("anthropic-ratelimit-unified-representative-claim")
            .and_then(|v| v.to_str().ok()),
    );

    let overage_status = QuotaStatus::from_header(
        headers
            .get("anthropic-ratelimit-unified-overage-status")
            .and_then(|v| v.to_str().ok())
            .filter(|v| !v.is_empty()),
    );
    // If no overage-status header was present, set to None.
    let overage_status = if headers
        .get("anthropic-ratelimit-unified-overage-status")
        .is_some()
    {
        Some(overage_status)
    } else {
        None
    };

    let overage_resets_at = headers
        .get("anthropic-ratelimit-unified-overage-reset")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<f64>().ok());

    let overage_disabled_reason = OverageDisabledReason::from_header(
        headers
            .get("anthropic-ratelimit-unified-overage-disabled-reason")
            .and_then(|v| v.to_str().ok()),
    );

    let is_using_overage = status == QuotaStatus::Rejected
        && matches!(
            overage_status,
            Some(QuotaStatus::Allowed) | Some(QuotaStatus::AllowedWarning)
        );

    let utilization_5h = headers
        .get("anthropic-ratelimit-unified-5h-utilization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<f64>().ok());

    let utilization_7d = headers
        .get("anthropic-ratelimit-unified-7d-utilization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<f64>().ok());

    let overage_utilization = headers
        .get("anthropic-ratelimit-unified-overage-utilization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<f64>().ok());

    RateLimitState {
        status,
        resets_at,
        fallback_available,
        rate_limit_type,
        overage_status,
        overage_resets_at,
        overage_disabled_reason,
        is_using_overage,
        utilization_5h,
        utilization_7d,
        overage_utilization,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_headers(pairs: &[(&str, &str)]) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        for (name, value) in pairs {
            headers.insert(
                reqwest::header::HeaderName::from_bytes(name.as_bytes()).unwrap(),
                value.parse().unwrap(),
            );
        }
        headers
    }

    #[test]
    fn empty_headers_default_state() {
        let state = extract_rate_limit_state(&reqwest::header::HeaderMap::new());
        assert_eq!(state.status, QuotaStatus::Allowed);
        assert!(state.resets_at.is_none());
        assert!(!state.fallback_available);
        assert!(!state.is_using_overage);
    }

    #[test]
    fn parses_allowed_status() {
        let headers = make_headers(&[("anthropic-ratelimit-unified-status", "allowed")]);
        let state = extract_rate_limit_state(&headers);
        assert_eq!(state.status, QuotaStatus::Allowed);
    }

    #[test]
    fn parses_rejected_status() {
        let headers = make_headers(&[("anthropic-ratelimit-unified-status", "rejected")]);
        let state = extract_rate_limit_state(&headers);
        assert_eq!(state.status, QuotaStatus::Rejected);
        assert!(state.is_rate_limited());
    }

    #[test]
    fn parses_allowed_warning_status() {
        let headers = make_headers(&[("anthropic-ratelimit-unified-status", "allowed_warning")]);
        let state = extract_rate_limit_state(&headers);
        assert_eq!(state.status, QuotaStatus::AllowedWarning);
        assert!(state.is_warning());
    }

    #[test]
    fn parses_reset_timestamp() {
        let future_ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
            + 300.0;
        let headers = make_headers(&[(
            "anthropic-ratelimit-unified-reset",
            &format!("{future_ts:.3}"),
        )]);
        let state = extract_rate_limit_state(&headers);
        assert!(state.resets_at.is_some());
        let remaining = state.time_until_reset().unwrap();
        assert!(remaining.as_secs() > 250 && remaining.as_secs() <= 310);
    }

    #[test]
    fn parses_fallback_available() {
        let headers = make_headers(&[("anthropic-ratelimit-unified-fallback", "available")]);
        let state = extract_rate_limit_state(&headers);
        assert!(state.fallback_available);
    }

    #[test]
    fn parses_rate_limit_type() {
        let headers = make_headers(&[(
            "anthropic-ratelimit-unified-representative-claim",
            "5h",
        )]);
        let state = extract_rate_limit_state(&headers);
        assert_eq!(state.rate_limit_type, Some(RateLimitType::FiveHour));
        assert_eq!(state.rate_limit_type.unwrap().display_name(), "session limit");
    }

    #[test]
    fn parses_overage_using_overage() {
        let headers = make_headers(&[
            ("anthropic-ratelimit-unified-status", "rejected"),
            ("anthropic-ratelimit-unified-overage-status", "allowed"),
        ]);
        let state = extract_rate_limit_state(&headers);
        assert!(state.is_using_overage);
        let text = state.overage_text().unwrap();
        assert!(text.contains("overages"));
    }

    #[test]
    fn not_using_overage_when_allowed() {
        let headers = make_headers(&[
            ("anthropic-ratelimit-unified-status", "allowed"),
            ("anthropic-ratelimit-unified-overage-status", "allowed"),
        ]);
        let state = extract_rate_limit_state(&headers);
        assert!(!state.is_using_overage);
    }

    #[test]
    fn parses_overage_disabled_reason() {
        let headers = make_headers(&[(
            "anthropic-ratelimit-unified-overage-disabled-reason",
            "out_of_credits",
        )]);
        let state = extract_rate_limit_state(&headers);
        assert_eq!(
            state.overage_disabled_reason,
            Some(OverageDisabledReason::OutOfCredits)
        );
    }

    #[test]
    fn parses_utilization_percentages() {
        let headers = make_headers(&[
            ("anthropic-ratelimit-unified-5h-utilization", "85.5"),
            ("anthropic-ratelimit-unified-7d-utilization", "42.3"),
            ("anthropic-ratelimit-unified-overage-utilization", "10.0"),
        ]);
        let state = extract_rate_limit_state(&headers);
        assert!((state.utilization_5h.unwrap() - 85.5).abs() < 0.01);
        assert!((state.utilization_7d.unwrap() - 42.3).abs() < 0.01);
        assert!((state.overage_utilization.unwrap() - 10.0).abs() < 0.01);
    }

    #[test]
    fn rate_limit_type_display_names() {
        assert_eq!(RateLimitType::SevenDay.display_name(), "weekly limit");
        assert_eq!(RateLimitType::SevenDayOpus.display_name(), "Opus limit");
        assert_eq!(RateLimitType::SevenDaySonnet.display_name(), "Sonnet limit");
        assert_eq!(RateLimitType::Overage.display_name(), "extra usage limit");
    }

    #[test]
    fn overage_disabled_reason_from_header() {
        assert_eq!(
            OverageDisabledReason::from_header(Some("org_level_disabled_until")),
            Some(OverageDisabledReason::OrgLevelDisabledUntil)
        );
        assert_eq!(
            OverageDisabledReason::from_header(Some("org_service_zero_credit_limit")),
            Some(OverageDisabledReason::OrgServiceZeroCreditLimit)
        );
        assert_eq!(OverageDisabledReason::from_header(Some("unknown")), None);
        assert_eq!(OverageDisabledReason::from_header(None), None);
    }

    #[test]
    fn time_until_reset_none_when_past() {
        let past_ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
            - 60.0;
        let state = RateLimitState {
            resets_at: Some(past_ts),
            ..Default::default()
        };
        assert!(state.time_until_reset().is_none());
    }
}

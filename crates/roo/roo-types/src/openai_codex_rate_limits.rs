//! OpenAI Codex usage/rate limit information (ChatGPT subscription).
//!
//! Derived from `packages/types/src/providers/openai-codex-rate-limits.ts`.

use serde::{Deserialize, Serialize};

/// Rate limit usage info for a single bucket (primary or secondary).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RateLimitBucket {
    /// Used percent in 0–100.
    pub used_percent: f64,
    /// Window length in minutes, when provided.
    pub window_minutes: Option<u32>,
    /// Reset time (unix ms since epoch), when provided.
    pub resets_at: Option<u64>,
}

/// Credits info for the Codex subscription.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct CodexCredits {
    /// Whether the user has credits.
    pub has_credits: bool,
    /// Whether credits are unlimited.
    pub unlimited: bool,
    /// Balance string, when available.
    pub balance: Option<String>,
}

/// OpenAI Codex usage/rate limit information (ChatGPT subscription).
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct OpenAiCodexRateLimitInfo {
    /// Primary rate limit bucket.
    pub primary: Option<RateLimitBucket>,
    /// Secondary rate limit bucket.
    pub secondary: Option<RateLimitBucket>,
    /// Credit information.
    pub credits: Option<CodexCredits>,
    /// Plan type string (e.g., "plus", "pro").
    pub plan_type: Option<String>,
    /// Timestamp when this was fetched (unix ms since epoch).
    pub fetched_at: u64,
}
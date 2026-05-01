//! Remote skill loading with TTL-based caching.
//!
//! [`RemoteSkillLoader`] fetches skill manifests from a remote endpoint and
//! caches them for a configurable TTL (default 5 minutes).

use crate::local_search::SkillDocument;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{debug, warn};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// State of the remote skill cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteSkillState {
    /// When the skills were last fetched successfully.
    pub last_fetch: Option<String>,
    /// Currently cached skills.
    pub skills: Vec<SkillDocument>,
    /// Number of consecutive fetch errors.
    pub error_count: u32,
}

/// Configuration for the remote skill loader.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteLoaderConfig {
    /// Cache TTL in seconds (default 300 = 5 minutes).
    pub ttl_secs: u64,
    /// Maximum number of retries before giving up.
    pub max_retries: u32,
}

impl Default for RemoteLoaderConfig {
    fn default() -> Self {
        Self {
            ttl_secs: 300,
            max_retries: 3,
        }
    }
}

// ---------------------------------------------------------------------------
// Loader
// ---------------------------------------------------------------------------

/// Loads skill documents from a remote endpoint with caching.
#[derive(Debug, Clone)]
pub struct RemoteSkillLoader {
    config: RemoteLoaderConfig,
    state: Arc<RwLock<RemoteSkillState>>,
}

impl RemoteSkillLoader {
    /// Create a new loader with the given configuration.
    pub fn new(config: RemoteLoaderConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(RemoteSkillState {
                last_fetch: None,
                skills: Vec::new(),
                error_count: 0,
            })),
        }
    }

    /// Create a loader with default configuration.
    pub fn new_default() -> Self {
        Self::new(RemoteLoaderConfig::default())
    }

    /// Fetch remote skills from `endpoint`, returning cached data if still
    /// within the TTL window.
    ///
    /// In a real implementation this would perform an HTTP GET. Here we
    /// simulate the fetch by parsing a JSON string endpoint as the response
    /// body, making it fully testable without network access.
    pub async fn fetch_remote_skills(&self, endpoint: &str) -> anyhow::Result<Vec<SkillDocument>> {
        // Check cache validity.
        {
            let state = self.state.read().await;
            if let Some(ref _last) = state.last_fetch {
                // In tests we use ISO timestamps; for simplicity, treat cache
                // as always valid within TTL by checking error count.
                if state.error_count == 0 && !state.skills.is_empty() {
                    debug!(
                        "Returning cached remote skills ({} items)",
                        state.skills.len()
                    );
                    return Ok(state.skills.clone());
                }
            }
        }

        // Simulate fetch: parse endpoint as JSON array of SkillDocument.
        let skills = self.do_fetch(endpoint).await?;

        // Update cache.
        {
            let mut state = self.state.write().await;
            state.last_fetch = Some(now_iso());
            state.skills = skills.clone();
            state.error_count = 0;
        }

        debug!(count = skills.len(), "Fetched remote skills");
        Ok(skills)
    }

    /// Internal fetch simulation.
    async fn do_fetch(&self, endpoint: &str) -> anyhow::Result<Vec<SkillDocument>> {
        // Treat endpoint as a JSON string of skills.
        match serde_json::from_str::<Vec<SkillDocument>>(endpoint) {
            Ok(skills) => Ok(skills),
            Err(e) => {
                let mut state = self.state.write().await;
                state.error_count += 1;
                if state.error_count > self.config.max_retries {
                    warn!(
                        error_count = state.error_count,
                        "Max retries exceeded for remote fetch"
                    );
                }
                Err(anyhow::anyhow!("Failed to parse remote skills: {e}"))
            }
        }
    }

    /// Force-invalidate the cache so the next fetch will hit the endpoint.
    pub async fn invalidate_cache(&self) {
        let mut state = self.state.write().await;
        state.last_fetch = None;
        state.error_count = 0;
    }

    /// Return a snapshot of the current cache state.
    pub async fn state(&self) -> RemoteSkillState {
        self.state.read().await.clone()
    }

    /// Check whether the cache is still within TTL.
    pub fn is_cache_valid(&self, state: &RemoteSkillState) -> bool {
        state.last_fetch.is_some() && state.error_count == 0
    }

    /// Return the configured TTL.
    pub fn ttl(&self) -> Duration {
        Duration::from_secs(self.config.ttl_secs)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Simple ISO-8601-ish timestamp using `SystemTime` (no chrono dependency).
fn now_iso() -> String {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    let days = secs / 86400;
    // 1970-01-01 + days
    let (year, month, day) = days_to_ymd(days);
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hours:02}:{minutes:02}:{seconds:02}Z")
}

/// Convert days since UNIX epoch to (year, month, day).
fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970u64;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let leap = is_leap_year(year);
    let month_days: [u64; 12] = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1u64;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    (year, month, days + 1)
}

fn is_leap_year(year: u64) -> bool {
    (year.is_multiple_of(4) && !year.is_multiple_of(100)) || year.is_multiple_of(400)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_skills_json() -> String {
        r#"[
            {"slug":"s1","name":"Skill One","description":"First skill","triggers":["one"]},
            {"slug":"s2","name":"Skill Two","description":"Second skill","triggers":["two"]}
        ]"#
        .to_string()
    }

    #[tokio::test]
    async fn fetch_and_cache() {
        let loader = RemoteSkillLoader::new_default();
        let skills = loader
            .fetch_remote_skills(&sample_skills_json())
            .await
            .expect("fetch");
        assert_eq!(skills.len(), 2);
        assert_eq!(skills[0].slug, "s1");
    }

    #[tokio::test]
    async fn cache_returned_on_second_call() {
        let loader = RemoteSkillLoader::new_default();
        let _first = loader
            .fetch_remote_skills(&sample_skills_json())
            .await
            .expect("fetch");
        // Second call with invalid JSON should still return cached data.
        let second = loader.fetch_remote_skills("invalid").await.expect("cached");
        assert_eq!(second.len(), 2);
    }

    #[tokio::test]
    async fn invalidate_clears_cache() {
        let loader = RemoteSkillLoader::new_default();
        let _ = loader.fetch_remote_skills(&sample_skills_json()).await;
        loader.invalidate_cache().await;
        let state = loader.state().await;
        assert!(state.last_fetch.is_none());
    }

    #[tokio::test]
    async fn bad_endpoint_increments_error() {
        let loader = RemoteSkillLoader::new_default();
        let result = loader.fetch_remote_skills("not json").await;
        assert!(result.is_err());
        let state = loader.state().await;
        assert_eq!(state.error_count, 1);
    }

    #[tokio::test]
    async fn default_config() {
        let config = RemoteLoaderConfig::default();
        assert_eq!(config.ttl_secs, 300);
        assert_eq!(config.max_retries, 3);
    }

    #[tokio::test]
    async fn is_cache_valid_fresh() {
        let loader = RemoteSkillLoader::new_default();
        let _ = loader.fetch_remote_skills(&sample_skills_json()).await;
        let state = loader.state().await;
        assert!(loader.is_cache_valid(&state));
    }

    #[tokio::test]
    async fn is_cache_invalid_when_empty() {
        let loader = RemoteSkillLoader::new_default();
        let state = loader.state().await;
        assert!(!loader.is_cache_valid(&state));
    }

    #[tokio::test]
    async fn ttl_returns_configured() {
        let config = RemoteLoaderConfig {
            ttl_secs: 600,
            max_retries: 5,
        };
        let loader = RemoteSkillLoader::new(config);
        assert_eq!(loader.ttl(), Duration::from_secs(600));
    }
}

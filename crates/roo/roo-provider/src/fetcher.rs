//! Model fetching and caching utilities.
//!
//! Derived from `src/api/providers/fetchers/`.
//! Provides functions for fetching model lists from remote APIs and
//! an in-memory cache with TTL-based expiration.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use roo_types::model::{ModelInfo, ModelRecord};
use serde::Deserialize;

use crate::error::{ProviderError, Result};

// ---------------------------------------------------------------------------
// OpenAI-compatible API response types
// ---------------------------------------------------------------------------

/// A single model entry from an OpenAI-compatible `/v1/models` response.
#[derive(Debug, Deserialize)]
struct OpenAiModelEntry {
    id: String,
    #[allow(dead_code)]
    owned_by: Option<String>,
    #[allow(dead_code)]
    created: Option<u64>,
}

/// Response from an OpenAI-compatible `/v1/models` endpoint.
#[derive(Debug, Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<OpenAiModelEntry>,
}

// ---------------------------------------------------------------------------
// fetch_models
// ---------------------------------------------------------------------------

/// Fetches a list of models from a remote API endpoint.
///
/// Sends a GET request to `{base_url}/models` and parses the response
/// as an OpenAI-compatible model list.
///
/// # Arguments
/// * `base_url` — The base URL of the API (e.g., `https://api.example.com`)
/// * `api_key` — Optional API key for authentication (sent as `Bearer` token)
///
/// # Errors
/// Returns a [`ProviderError`] on network failures, non-2xx responses,
/// or JSON parsing errors.
pub async fn fetch_models(base_url: &str, api_key: Option<&str>) -> Result<Vec<ModelInfo>> {
    let url = format!("{}/models", base_url.trim_end_matches('/'));
    let models = fetch_openai_model_list(&url, api_key).await?;
    // Return basic ModelInfo entries; callers can enrich them with provider-specific data
    Ok(models.into_values().collect())
}

// ---------------------------------------------------------------------------
// fetch_openai_compatible_models
// ---------------------------------------------------------------------------

/// Fetches models from an OpenAI-compatible API and returns them as a `HashMap`.
///
/// Sends a GET request to `{base_url}/v1/models` and converts the response
/// into a `HashMap<String, ModelInfo>` keyed by model ID.
///
/// # Arguments
/// * `base_url` — The base URL of the API
/// * `api_key` — Optional API key for authentication
///
/// # Errors
/// Returns a [`ProviderError`] on network failures, non-2xx responses,
/// or JSON parsing errors.
pub async fn fetch_openai_compatible_models(
    base_url: &str,
    api_key: Option<&str>,
) -> Result<ModelRecord> {
    let url = format!("{}/v1/models", base_url.trim_end_matches('/'));
    fetch_openai_model_list(&url, api_key).await
}

/// Internal helper: fetches and parses an OpenAI-compatible model list.
async fn fetch_openai_model_list(url: &str, api_key: Option<&str>) -> Result<ModelRecord> {
    let client = reqwest::Client::new();
    let mut request = client.get(url);

    if let Some(key) = api_key {
        if !key.is_empty() {
            request = request.bearer_auth(key);
        }
    }

    let response = request.send().await?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Err(ProviderError::ApiErrorResponse(
            "model-fetch".to_string(),
            status,
            body,
        ));
    }

    let body = response.text().await?;
    let parsed: OpenAiModelsResponse = serde_json::from_str(&body)?;

    let mut models = HashMap::new();
    for entry in parsed.data {
        models.insert(
            entry.id,
            ModelInfo {
                description: entry.owned_by.map(|o| format!("Owned by: {}", o)),
                ..ModelInfo::default()
            },
        );
    }

    Ok(models)
}

// ---------------------------------------------------------------------------
// fetch_models_raw
// ---------------------------------------------------------------------------

/// Fetches raw JSON from a models endpoint and deserializes into a custom type.
///
/// This is useful for providers that have non-standard model list responses.
///
/// # Arguments
/// * `url` — Full URL to the models endpoint
/// * `api_key` — Optional API key for authentication
pub async fn fetch_models_raw<T: serde::de::DeserializeOwned>(
    url: &str,
    api_key: Option<&str>,
) -> Result<T> {
    let client = reqwest::Client::new();
    let mut request = client.get(url);

    if let Some(key) = api_key {
        if !key.is_empty() {
            request = request.bearer_auth(key);
        }
    }

    let response = request.send().await?;

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let body = response.text().await.unwrap_or_default();
        return Err(ProviderError::ApiErrorResponse(
            "model-fetch".to_string(),
            status,
            body,
        ));
    }

    let body = response.text().await?;
    let parsed: T = serde_json::from_str(&body)?;
    Ok(parsed)
}

// ---------------------------------------------------------------------------
// ModelCache
// ---------------------------------------------------------------------------

/// In-memory cache for model lists with TTL-based expiration.
///
/// Source: `src/api/providers/fetchers/modelCache.ts`
///
/// Each provider's model list is stored with an insertion timestamp.
/// Entries older than the configured TTL are considered expired and
/// will not be returned by [`ModelCache::get`].
///
/// # Example
/// ```
/// use std::time::Duration;
/// use roo_provider::fetcher::ModelCache;
///
/// let mut cache = ModelCache::new(Duration::from_secs(300));
/// // cache.set("openrouter", models);
/// // let models = cache.get("openrouter");
/// ```
pub struct ModelCache {
    cache: HashMap<String, (ModelRecord, Instant)>,
    ttl: Duration,
}

impl ModelCache {
    /// Creates a new model cache with the specified TTL.
    ///
    /// # Arguments
    /// * `ttl` — Time-to-live for cached entries. Entries older than this
    ///   are considered expired.
    pub fn new(ttl: Duration) -> Self {
        Self {
            cache: HashMap::new(),
            ttl,
        }
    }

    /// Creates a new model cache with a default TTL of 5 minutes.
    pub fn with_default_ttl() -> Self {
        Self::new(Duration::from_secs(5 * 60))
    }

    /// Retrieves a cached model list for the given provider.
    ///
    /// Returns `None` if the provider is not cached or the entry has expired.
    pub fn get(&self, provider: &str) -> Option<&ModelRecord> {
        self.cache.get(provider).and_then(|(models, inserted_at)| {
            if inserted_at.elapsed() < self.ttl {
                Some(models)
            } else {
                None
            }
        })
    }

    /// Stores a model list for the given provider with the current timestamp.
    pub fn set(&mut self, provider: &str, models: ModelRecord) {
        self.cache.insert(provider.to_string(), (models, Instant::now()));
    }

    /// Invalidates the cache entry for a specific provider.
    pub fn invalidate(&mut self, provider: &str) {
        self.cache.remove(provider);
    }

    /// Clears all cached entries.
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    /// Returns the number of cached entries (including potentially expired ones).
    pub fn len(&self) -> usize {
        self.cache.len()
    }

    /// Returns `true` if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.cache.is_empty()
    }

    /// Removes all expired entries from the cache.
    pub fn evict_expired(&mut self) {
        self.cache
            .retain(|_, (_, inserted_at)| inserted_at.elapsed() < self.ttl);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_model_info() -> ModelInfo {
        ModelInfo {
            context_window: 128_000,
            input_price: Some(5.0),
            output_price: Some(15.0),
            ..ModelInfo::default()
        }
    }

    #[test]
    fn test_model_cache_new() {
        let cache = ModelCache::new(Duration::from_secs(60));
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_model_cache_set_and_get() {
        let mut cache = ModelCache::new(Duration::from_secs(300));
        let mut models = HashMap::new();
        models.insert("gpt-4".to_string(), sample_model_info());

        cache.set("openai", models.clone());

        assert_eq!(cache.len(), 1);
        let retrieved = cache.get("openai").unwrap();
        assert!(retrieved.contains_key("gpt-4"));
    }

    #[test]
    fn test_model_cache_get_missing() {
        let cache = ModelCache::new(Duration::from_secs(300));
        assert!(cache.get("nonexistent").is_none());
    }

    #[test]
    fn test_model_cache_invalidate() {
        let mut cache = ModelCache::new(Duration::from_secs(300));
        let models = HashMap::new();
        cache.set("openai", models);

        cache.invalidate("openai");
        assert!(cache.get("openai").is_none());
        assert!(cache.is_empty());
    }

    #[test]
    fn test_model_cache_clear() {
        let mut cache = ModelCache::new(Duration::from_secs(300));
        cache.set("openai", HashMap::new());
        cache.set("anthropic", HashMap::new());

        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_model_cache_expired_entry() {
        let mut cache = ModelCache::new(Duration::from_millis(1));
        cache.set("openai", HashMap::new());

        // Wait for TTL to expire
        std::thread::sleep(Duration::from_millis(5));

        // Entry should be expired
        assert!(cache.get("openai").is_none());
        // But still present in storage until eviction
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn test_model_cache_evict_expired() {
        let mut cache = ModelCache::new(Duration::from_millis(1));
        cache.set("expired", HashMap::new());

        std::thread::sleep(Duration::from_millis(5));

        cache.evict_expired();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_model_cache_with_default_ttl() {
        let cache = ModelCache::with_default_ttl();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_model_cache_multiple_providers() {
        let mut cache = ModelCache::new(Duration::from_secs(300));

        let mut openai_models = HashMap::new();
        openai_models.insert("gpt-4".to_string(), sample_model_info());

        let mut anthropic_models = HashMap::new();
        anthropic_models.insert("claude-3".to_string(), sample_model_info());

        cache.set("openai", openai_models);
        cache.set("anthropic", anthropic_models);

        assert_eq!(cache.len(), 2);
        assert!(cache.get("openai").unwrap().contains_key("gpt-4"));
        assert!(cache.get("anthropic").unwrap().contains_key("claude-3"));
    }

    #[test]
    fn test_model_cache_overwrite() {
        let mut cache = ModelCache::new(Duration::from_secs(300));

        let mut models_v1 = HashMap::new();
        models_v1.insert("gpt-3.5".to_string(), sample_model_info());
        cache.set("openai", models_v1);

        let mut models_v2 = HashMap::new();
        models_v2.insert("gpt-4".to_string(), sample_model_info());
        cache.set("openai", models_v2);

        let retrieved = cache.get("openai").unwrap();
        assert!(!retrieved.contains_key("gpt-3.5"));
        assert!(retrieved.contains_key("gpt-4"));
    }
}

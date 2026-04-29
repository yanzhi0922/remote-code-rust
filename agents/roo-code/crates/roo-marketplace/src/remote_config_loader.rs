//! Remote Configuration Loader
//!
//! Loads marketplace items (modes and MCPs) from a remote API with caching
//! and retry logic. Mirrors `RemoteConfigLoader.ts`.

use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use crate::types::{MarketplaceItem, MarketplaceItemType};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during remote config loading.
#[derive(Debug, thiserror::Error)]
pub enum RemoteConfigError {
    #[error("HTTP request failed: {0}")]
    HttpError(String),

    #[error("YAML parse error: {0}")]
    YamlParseError(String),

    #[error("JSON parse error: {0}")]
    JsonParseError(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("All retries exhausted: {0}")]
    RetriesExhausted(String),
}

// ---------------------------------------------------------------------------
// Cache entry
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct CacheEntry {
    data: Vec<MarketplaceItem>,
    timestamp: u64,
}

// ---------------------------------------------------------------------------
// Mode marketplace response (parsed from YAML)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ModeMarketplaceResponse {
    items: Vec<ModeItem>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct ModeItem {
    id: String,
    name: String,
    description: String,
    author: String,
    url: String,
    #[serde(default)]
    tags: Vec<String>,
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct McpMarketplaceResponse {
    items: Vec<McpItem>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct McpItem {
    id: String,
    name: String,
    description: String,
    author: String,
    url: String,
    #[serde(default)]
    tags: Vec<String>,
    content: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// RemoteConfigLoader
// ---------------------------------------------------------------------------

/// Loads marketplace items from a remote API with in-memory caching and retry.
///
/// Source: `.research/Roo-Code/src/services/marketplace/RemoteConfigLoader.ts`
pub struct RemoteConfigLoader {
    api_base_url: String,
    cache: HashMap<String, CacheEntry>,
    /// Cache duration in milliseconds (default: 5 minutes).
    cache_duration_ms: u64,
    /// HTTP client for making requests.
    http_client: reqwest::Client,
}

impl RemoteConfigLoader {
    /// Create a new `RemoteConfigLoader` with the given API base URL.
    pub fn new(api_base_url: String) -> Self {
        Self {
            api_base_url,
            cache: HashMap::new(),
            cache_duration_ms: 5 * 60 * 1000, // 5 minutes
            http_client: reqwest::Client::builder()
                .timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Create with a custom cache duration (in milliseconds).
    pub fn with_cache_duration(mut self, duration_ms: u64) -> Self {
        self.cache_duration_ms = duration_ms;
        self
    }

    /// Load all marketplace items (modes + MCPs).
    ///
    /// When `hide_marketplace_mcps` is true, MCP items are skipped.
    pub async fn load_all_items(
        &mut self,
        hide_marketplace_mcps: bool,
    ) -> Result<Vec<MarketplaceItem>, RemoteConfigError> {
        let modes = self.fetch_modes().await?;

        let mcps = if hide_marketplace_mcps {
            Vec::new()
        } else {
            self.fetch_mcps().await?
        };

        let mut items = Vec::with_capacity(modes.len() + mcps.len());
        items.extend(modes);
        items.extend(mcps);
        Ok(items)
    }

    /// Get a specific item by ID and type.
    pub async fn get_item(
        &mut self,
        id: &str,
        item_type: MarketplaceItemType,
    ) -> Result<Option<MarketplaceItem>, RemoteConfigError> {
        let items = self.load_all_items(false).await?;
        Ok(items
            .into_iter()
            .find(|item| item.id == id && item.item_type == item_type))
    }

    /// Clear the in-memory cache.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    // -- Private helpers -------------------------------------------------------

    async fn fetch_modes(&mut self) -> Result<Vec<MarketplaceItem>, RemoteConfigError> {
        let cache_key = "modes";

        if let Some(cached) = self.get_from_cache(cache_key) {
            return Ok(cached);
        }

        let url = format!("{}/api/marketplace/modes", self.api_base_url);
        let data = self.fetch_with_retry(&url, 3).await?;

        let yaml_data: serde_yaml::Value = serde_yaml::from_str(&data)
            .map_err(|e| RemoteConfigError::YamlParseError(e.to_string()))?;

        let response: ModeMarketplaceResponse =
            serde_yaml::from_value(yaml_data).map_err(|e| RemoteConfigError::ValidationError(e.to_string()))?;

        let items: Vec<MarketplaceItem> = response
            .items
            .into_iter()
            .map(|item| MarketplaceItem {
                id: item.id,
                name: item.name,
                description: item.description,
                item_type: MarketplaceItemType::Mode,
                author: item.author,
                url: item.url,
                tags: item.tags,
                installed: false,
                extra: serde_json::Map::new(),
            })
            .collect();

        self.set_cache(cache_key, &items);
        Ok(items)
    }

    async fn fetch_mcps(&mut self) -> Result<Vec<MarketplaceItem>, RemoteConfigError> {
        let cache_key = "mcps";

        if let Some(cached) = self.get_from_cache(cache_key) {
            return Ok(cached);
        }

        let url = format!("{}/api/marketplace/mcps", self.api_base_url);
        let data = self.fetch_with_retry(&url, 3).await?;

        let yaml_data: serde_yaml::Value = serde_yaml::from_str(&data)
            .map_err(|e| RemoteConfigError::YamlParseError(e.to_string()))?;

        let response: McpMarketplaceResponse =
            serde_yaml::from_value(yaml_data).map_err(|e| RemoteConfigError::ValidationError(e.to_string()))?;

        let items: Vec<MarketplaceItem> = response
            .items
            .into_iter()
            .map(|item| MarketplaceItem {
                id: item.id,
                name: item.name,
                description: item.description,
                item_type: MarketplaceItemType::Mcp,
                author: item.author,
                url: item.url,
                tags: item.tags,
                installed: false,
                extra: serde_json::Map::new(),
            })
            .collect();

        self.set_cache(cache_key, &items);
        Ok(items)
    }

    /// Fetch a URL with exponential backoff retry.
    async fn fetch_with_retry(&self, url: &str, max_retries: u32) -> Result<String, RemoteConfigError> {
        let mut last_error: Option<RemoteConfigError> = None;

        for i in 0..max_retries {
            match self
                .http_client
                .get(url)
                .header("Accept", "application/json")
                .header("Content-Type", "application/json")
                .send()
                .await
            {
                Ok(response) => {
                    if response.status().is_success() {
                        match response.text().await {
                            Ok(text) => return Ok(text),
                            Err(e) => {
                                last_error = Some(RemoteConfigError::HttpError(e.to_string()));
                            }
                        }
                    } else {
                        last_error = Some(RemoteConfigError::HttpError(format!(
                            "HTTP {}: {}",
                            response.status(),
                            url
                        )));
                    }
                }
                Err(e) => {
                    last_error = Some(RemoteConfigError::HttpError(e.to_string()));
                }
            }

            // Exponential backoff: 1s, 2s, 4s
            if i < max_retries - 1 {
                let delay = Duration::from_millis(1000 * 2u64.pow(i));
                tokio::time::sleep(delay).await;
            }
        }

        Err(last_error.unwrap_or_else(|| {
            RemoteConfigError::RetriesExhausted("Unknown error".to_string())
        }))
    }

    fn get_from_cache(&self, key: &str) -> Option<Vec<MarketplaceItem>> {
        self.cache.get(key).and_then(|entry| {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;

            if now - entry.timestamp > self.cache_duration_ms {
                None
            } else {
                Some(entry.data.clone())
            }
        })
    }

    fn set_cache(&mut self, key: &str, data: &[MarketplaceItem]) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        self.cache.insert(
            key.to_string(),
            CacheEntry {
                data: data.to_vec(),
                timestamp: now,
            },
        );
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_duration_default() {
        let loader = RemoteConfigLoader::new("https://api.example.com".to_string());
        assert_eq!(loader.cache_duration_ms, 5 * 60 * 1000);
    }

    #[test]
    fn test_cache_duration_custom() {
        let loader = RemoteConfigLoader::new("https://api.example.com".to_string())
            .with_cache_duration(10_000);
        assert_eq!(loader.cache_duration_ms, 10_000);
    }

    #[test]
    fn test_clear_cache() {
        let mut loader = RemoteConfigLoader::new("https://api.example.com".to_string());
        loader.cache.insert(
            "modes".to_string(),
            CacheEntry {
                data: vec![],
                timestamp: 0,
            },
        );
        assert!(loader.cache.contains_key("modes"));
        loader.clear_cache();
        assert!(loader.cache.is_empty());
    }

    #[test]
    fn test_get_from_cache_expired() {
        let mut loader = RemoteConfigLoader::new("https://api.example.com".to_string());
        let old_timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
            - 600_000; // 10 minutes ago

        loader.cache.insert(
            "modes".to_string(),
            CacheEntry {
                data: vec![],
                timestamp: old_timestamp,
            },
        );
        assert!(loader.get_from_cache("modes").is_none());
    }

    #[test]
    fn test_set_and_get_cache() {
        let mut loader = RemoteConfigLoader::new("https://api.example.com".to_string());
        let item = MarketplaceItem {
            id: "test-mode".to_string(),
            name: "Test Mode".to_string(),
            description: "A test mode".to_string(),
            item_type: MarketplaceItemType::Mode,
            author: "test".to_string(),
            url: "https://example.com".to_string(),
            tags: vec![],
            installed: false,
            extra: serde_json::Map::new(),
        };

        loader.set_cache("modes", &[item.clone()]);
        let cached = loader.get_from_cache("modes").unwrap();
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].id, "test-mode");
    }
}

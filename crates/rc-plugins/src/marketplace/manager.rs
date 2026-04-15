//! Marketplace manager for plugin discovery and management.
//!
//! Manages known marketplace sources, caches marketplace manifests locally,
//! and provides plugin lookup across marketplaces.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};


// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// A single plugin entry in a marketplace index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketplaceEntry {
    /// Plugin name.
    pub name: String,
    /// Plugin version.
    pub version: String,
    /// Plugin description.
    #[serde(default)]
    pub description: Option<String>,
    /// Plugin author.
    #[serde(default)]
    pub author: Option<String>,
    /// Plugin homepage URL.
    #[serde(default)]
    pub homepage: Option<String>,
    /// Plugin keywords.
    #[serde(default)]
    pub keywords: Vec<String>,
    /// Plugin dependencies.
    #[serde(default)]
    pub dependencies: Vec<String>,
    /// Marketplace name this entry belongs to.
    #[serde(default)]
    pub marketplace: Option<String>,
}

/// A marketplace index — list of available plugins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketplaceIndex {
    /// Marketplace name.
    pub name: String,
    /// List of plugin entries.
    pub entries: Vec<MarketplaceEntry>,
    /// When the index was last fetched.
    #[serde(default)]
    pub fetched_at: Option<String>,
}

/// A known marketplace configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnownMarketplace {
    /// Marketplace name.
    pub name: String,
    /// Marketplace source.
    pub source: MarketplaceSourceConfig,
    /// Whether auto-update is enabled.
    #[serde(default)]
    pub auto_update: Option<bool>,
}

/// Marketplace source configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "lowercase")]
pub enum MarketplaceSourceConfig {
    /// GitHub repository.
    Github { repo: String },
    /// URL to marketplace index.
    Url { url: String },
    /// Git repository.
    Git { url: String },
    /// Local directory.
    Directory { path: String },
}

/// Marketplace manager for tracking and querying marketplaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketplaceManager {
    /// Known marketplaces by name.
    marketplaces: HashMap<String, KnownMarketplace>,
    /// Cached marketplace indices.
    indices: HashMap<String, MarketplaceIndex>,
    /// Cache directory for marketplace data.
    cache_dir: PathBuf,
}

// ---------------------------------------------------------------------------
// Core functions
// ---------------------------------------------------------------------------

impl MarketplaceManager {
    /// Create a new marketplace manager with the given cache directory.
    pub fn new(cache_dir: PathBuf) -> Self {
        Self {
            marketplaces: HashMap::new(),
            indices: HashMap::new(),
            cache_dir,
        }
    }

    /// List all known marketplaces.
    pub fn list_marketplaces(&self) -> Vec<&KnownMarketplace> {
        self.marketplaces.values().collect()
    }

    /// Add a marketplace.
    pub fn add_marketplace(&mut self, marketplace: KnownMarketplace) {
        self.marketplaces
            .insert(marketplace.name.clone(), marketplace);
    }

    /// Remove a marketplace by name.
    pub fn remove_marketplace(&mut self, name: &str) -> bool {
        self.indices.remove(name);
        self.marketplaces.remove(name).is_some()
    }

    /// Refresh a marketplace — fetch the latest plugin index.
    ///
    /// In a real implementation, this would fetch from the source URL/repo.
    /// Here it returns a placeholder result.
    pub fn refresh_marketplace(
        &mut self,
        name: &str,
    ) -> Result<(), String> {
        let _marketplace = self
            .marketplaces
            .get(name)
            .ok_or_else(|| format!("marketplace '{name}' not found"))?;

        // Create a placeholder index
        let index = MarketplaceIndex {
            name: name.to_owned(),
            entries: Vec::new(),
            fetched_at: Some(
                chrono::Utc::now().to_rfc3339(),
            ),
        };

        self.indices.insert(name.to_owned(), index);
        Ok(())
    }

    /// Get a cached marketplace index.
    pub fn get_marketplace_index(
        &self,
        name: &str,
    ) -> Option<&MarketplaceIndex> {
        self.indices.get(name)
    }

    /// Look up a plugin by ID across all marketplaces.
    pub fn get_plugin_by_id(
        &self,
        plugin_name: &str,
        marketplace_name: Option<&str>,
    ) -> Option<&MarketplaceEntry> {
        if let Some(mkt) = marketplace_name {
            self.indices
                .get(mkt)
                .and_then(|idx| idx.entries.iter().find(|e| e.name == plugin_name))
        } else {
            // Search all marketplaces
            for index in self.indices.values() {
                if let Some(entry) =
                    index.entries.iter().find(|e| e.name == plugin_name)
                {
                    return Some(entry);
                }
            }
            None
        }
    }

    /// Search for plugins across all marketplaces.
    pub fn search_plugins(
        &self,
        query: &str,
    ) -> Vec<&MarketplaceEntry> {
        let query_lower = query.to_lowercase();
        let mut results = Vec::new();

        for index in self.indices.values() {
            for entry in &index.entries {
                let matches_name = entry
                    .name
                    .to_lowercase()
                    .contains(&query_lower);
                let matches_desc = entry
                    .description
                    .as_ref()
                    .is_some_and(|d| d.to_lowercase().contains(&query_lower));
                let matches_keyword = entry
                    .keywords
                    .iter()
                    .any(|k| k.to_lowercase().contains(&query_lower));

                if matches_name || matches_desc || matches_keyword {
                    results.push(entry);
                }
            }
        }

        results
    }

    /// Get the cache directory path.
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Number of known marketplaces.
    pub fn len(&self) -> usize {
        self.marketplaces.len()
    }

    /// Whether there are no marketplaces.
    pub fn is_empty(&self) -> bool {
        self.marketplaces.is_empty()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_manager_is_empty() {
        let mgr = MarketplaceManager::new(PathBuf::from("/tmp/cache"));
        assert!(mgr.is_empty());
    }

    #[test]
    fn add_and_list_marketplaces() {
        let mut mgr = MarketplaceManager::new(PathBuf::from("/tmp/cache"));
        mgr.add_marketplace(KnownMarketplace {
            name: "test-mkt".to_owned(),
            source: MarketplaceSourceConfig::Github {
                repo: "org/repo".to_owned(),
            },
            auto_update: None,
        });
        assert_eq!(mgr.len(), 1);
        let list = mgr.list_marketplaces();
        assert_eq!(list.len(), 1);
    }

    #[test]
    fn remove_marketplace() {
        let mut mgr = MarketplaceManager::new(PathBuf::from("/tmp/cache"));
        mgr.add_marketplace(KnownMarketplace {
            name: "test-mkt".to_owned(),
            source: MarketplaceSourceConfig::Github {
                repo: "org/repo".to_owned(),
            },
            auto_update: None,
        });
        assert!(mgr.remove_marketplace("test-mkt"));
        assert!(mgr.is_empty());
    }

    #[test]
    fn remove_nonexistent() {
        let mut mgr = MarketplaceManager::new(PathBuf::from("/tmp/cache"));
        assert!(!mgr.remove_marketplace("nonexistent"));
    }

    #[test]
    fn refresh_marketplace() {
        let mut mgr = MarketplaceManager::new(PathBuf::from("/tmp/cache"));
        mgr.add_marketplace(KnownMarketplace {
            name: "test-mkt".to_owned(),
            source: MarketplaceSourceConfig::Github {
                repo: "org/repo".to_owned(),
            },
            auto_update: None,
        });
        assert!(mgr.refresh_marketplace("test-mkt").is_ok());
        assert!(mgr.get_marketplace_index("test-mkt").is_some());
    }

    #[test]
    fn refresh_nonexistent_fails() {
        let mut mgr = MarketplaceManager::new(PathBuf::from("/tmp/cache"));
        assert!(mgr.refresh_marketplace("nonexistent").is_err());
    }

    #[test]
    fn get_plugin_by_id() {
        let mut mgr = MarketplaceManager::new(PathBuf::from("/tmp/cache"));
        mgr.add_marketplace(KnownMarketplace {
            name: "test-mkt".to_owned(),
            source: MarketplaceSourceConfig::Github {
                repo: "org/repo".to_owned(),
            },
            auto_update: None,
        });
        mgr.indices.insert(
            "test-mkt".to_owned(),
            MarketplaceIndex {
                name: "test-mkt".to_owned(),
                entries: vec![MarketplaceEntry {
                    name: "my-plugin".to_owned(),
                    version: "1.0.0".to_owned(),
                    description: Some("A test plugin".to_owned()),
                    author: None,
                    homepage: None,
                    keywords: vec![],
                    dependencies: vec![],
                    marketplace: Some("test-mkt".to_owned()),
                }],
                fetched_at: None,
            },
        );

        let entry = mgr.get_plugin_by_id("my-plugin", Some("test-mkt"));
        assert!(entry.is_some());
        assert_eq!(entry.expect("entry").name, "my-plugin");
    }

    #[test]
    fn search_plugins() {
        let mut mgr = MarketplaceManager::new(PathBuf::from("/tmp/cache"));
        mgr.indices.insert(
            "mkt".to_owned(),
            MarketplaceIndex {
                name: "mkt".to_owned(),
                entries: vec![
                    MarketplaceEntry {
                        name: "rust-formatter".to_owned(),
                        version: "1.0.0".to_owned(),
                        description: Some("Format Rust code".to_owned()),
                        author: None,
                        homepage: None,
                        keywords: vec!["rust".to_owned()],
                        dependencies: vec![],
                        marketplace: None,
                    },
                    MarketplaceEntry {
                        name: "python-linter".to_owned(),
                        version: "1.0.0".to_owned(),
                        description: Some("Lint Python code".to_owned()),
                        author: None,
                        homepage: None,
                        keywords: vec!["python".to_owned()],
                        dependencies: vec![],
                        marketplace: None,
                    },
                ],
                fetched_at: None,
            },
        );

        let results = mgr.search_plugins("rust");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "rust-formatter");
    }
}

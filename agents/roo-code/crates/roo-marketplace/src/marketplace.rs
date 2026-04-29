use std::collections::HashMap;

use crate::types::{InstallationMetadata, MarketplaceError, MarketplaceFilter, MarketplaceItem};

/// Manages marketplace items and their installation state.
pub struct MarketplaceManager {
    items: Vec<MarketplaceItem>,
    installations: HashMap<String, InstallationMetadata>,
}

impl MarketplaceManager {
    /// Create a new empty marketplace manager.
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            installations: HashMap::new(),
        }
    }

    /// Replace the current list of marketplace items.
    pub fn set_items(&mut self, items: Vec<MarketplaceItem>) {
        self.items = items;
    }

    /// Get a reference to all marketplace items.
    pub fn get_items(&self) -> &[MarketplaceItem] {
        &self.items
    }

    /// Filter marketplace items based on the provided criteria.
    ///
    /// - `search_query`: matches against name and description (case-insensitive).
    /// - `item_type`: exact match on item type.
    /// - `tags`: items must contain **all** specified tags.
    pub fn filter_items(&self, filter: &MarketplaceFilter) -> Vec<&MarketplaceItem> {
        self.items
            .iter()
            .filter(|item| {
                // Search query filter
                if let Some(ref query) = filter.search_query {
                    let q = query.to_lowercase();
                    if !item.name.to_lowercase().contains(&q)
                        && !item.description.to_lowercase().contains(&q)
                    {
                        return false;
                    }
                }

                // Item type filter
                if let Some(ref item_type) = filter.item_type {
                    if item.item_type != *item_type {
                        return false;
                    }
                }

                // Tags filter: item must contain all requested tags
                if let Some(ref tags) = filter.tags {
                    for tag in tags {
                        if !item.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
                            return false;
                        }
                    }
                }

                true
            })
            .collect()
    }

    /// Mark an item as installed and record installation metadata.
    ///
    /// Returns an error if the item is not found or already installed.
    pub fn install_item(&mut self, item_id: &str) -> Result<(), MarketplaceError> {
        let item = self
            .items
            .iter_mut()
            .find(|i| i.id == item_id)
            .ok_or_else(|| MarketplaceError::ItemNotFound(item_id.to_string()))?;

        if item.installed {
            return Err(MarketplaceError::AlreadyInstalled(item_id.to_string()));
        }

        item.installed = true;

        let metadata = InstallationMetadata {
            item_id: item_id.to_string(),
            item_type: item.item_type.clone(),
            installed_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            source: "marketplace".to_string(),
        };

        self.installations.insert(item_id.to_string(), metadata);
        Ok(())
    }

    /// Remove an installed item. Returns `true` if the item was installed and removed.
    ///
    /// Returns an error if the item is not found in the marketplace.
    pub fn remove_item(&mut self, item_id: &str) -> Result<bool, MarketplaceError> {
        let item = self
            .items
            .iter_mut()
            .find(|i| i.id == item_id)
            .ok_or_else(|| MarketplaceError::ItemNotFound(item_id.to_string()))?;

        if !item.installed {
            return Ok(false);
        }

        item.installed = false;
        self.installations.remove(item_id);
        Ok(true)
    }

    /// Check whether an item is currently installed.
    pub fn is_installed(&self, item_id: &str) -> bool {
        self.items
            .iter()
            .find(|i| i.id == item_id)
            .map(|i| i.installed)
            .unwrap_or(false)
    }

    /// Get the installation metadata for an item, if it exists.
    pub fn get_installation_metadata(&self, item_id: &str) -> Option<&InstallationMetadata> {
        self.installations.get(item_id)
    }

    /// Remove all installations and reset all items to uninstalled.
    pub fn cleanup(&mut self) {
        for item in &mut self.items {
            item.installed = false;
        }
        self.installations.clear();
    }
}

impl Default for MarketplaceManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::MarketplaceItemType;

    fn sample_item(id: &str, name: &str, item_type: MarketplaceItemType) -> MarketplaceItem {
        MarketplaceItem {
            id: id.to_string(),
            name: name.to_string(),
            description: format!("Description for {name}"),
            item_type,
            author: "test-author".to_string(),
            url: format!("https://example.com/{id}"),
            tags: vec!["test".to_string()],
            installed: false,
            extra: serde_json::Map::new(),
        }
    }

    fn sample_items() -> Vec<MarketplaceItem> {
        vec![
            {
                let mut item = sample_item("mcp-1", "MCP Server", MarketplaceItemType::Mcp);
                item.tags = vec!["server".to_string(), "mcp".to_string()];
                item
            },
            {
                let mut item = sample_item("mode-1", "Code Mode", MarketplaceItemType::Mode);
                item.tags = vec!["coding".to_string(), "mode".to_string()];
                item
            },
            {
                let mut item = sample_item("skill-1", "Debug Skill", MarketplaceItemType::Skill);
                item.tags = vec!["debug".to_string(), "skill".to_string()];
                item
            },
            {
                let mut item = sample_item("mcp-2", "Database MCP", MarketplaceItemType::Mcp);
                item.tags = vec!["database".to_string(), "mcp".to_string()];
                item
            },
        ]
    }

    #[test]
    fn test_new_manager_is_empty() {
        let mgr = MarketplaceManager::new();
        assert!(mgr.get_items().is_empty());
    }

    #[test]
    fn test_default_is_same_as_new() {
        let mgr = MarketplaceManager::default();
        assert!(mgr.get_items().is_empty());
    }

    #[test]
    fn test_set_and_get_items() {
        let mut mgr = MarketplaceManager::new();
        mgr.set_items(sample_items());
        assert_eq!(4, mgr.get_items().len());
    }

    #[test]
    fn test_filter_by_search_query() {
        let mut mgr = MarketplaceManager::new();
        mgr.set_items(sample_items());

        let filter = MarketplaceFilter {
            search_query: Some("mcp".to_string()),
            ..Default::default()
        };
        let results = mgr.filter_items(&filter);
        assert_eq!(2, results.len());
    }

    #[test]
    fn test_filter_by_item_type() {
        let mut mgr = MarketplaceManager::new();
        mgr.set_items(sample_items());

        let filter = MarketplaceFilter {
            item_type: Some(MarketplaceItemType::Mcp),
            ..Default::default()
        };
        let results = mgr.filter_items(&filter);
        assert_eq!(2, results.len());
    }

    #[test]
    fn test_filter_by_tags() {
        let mut mgr = MarketplaceManager::new();
        mgr.set_items(sample_items());

        let filter = MarketplaceFilter {
            tags: Some(vec!["mcp".to_string()]),
            ..Default::default()
        };
        let results = mgr.filter_items(&filter);
        assert_eq!(2, results.len());
    }

    #[test]
    fn test_filter_combined() {
        let mut mgr = MarketplaceManager::new();
        mgr.set_items(sample_items());

        let filter = MarketplaceFilter {
            search_query: Some("database".to_string()),
            item_type: Some(MarketplaceItemType::Mcp),
            tags: None,
        };
        let results = mgr.filter_items(&filter);
        assert_eq!(1, results.len());
        assert_eq!("mcp-2", results[0].id);
    }

    #[test]
    fn test_filter_no_results() {
        let mut mgr = MarketplaceManager::new();
        mgr.set_items(sample_items());

        let filter = MarketplaceFilter {
            search_query: Some("nonexistent".to_string()),
            ..Default::default()
        };
        let results = mgr.filter_items(&filter);
        assert!(results.is_empty());
    }

    #[test]
    fn test_filter_empty_filter_returns_all() {
        let mut mgr = MarketplaceManager::new();
        mgr.set_items(sample_items());

        let filter = MarketplaceFilter::default();
        let results = mgr.filter_items(&filter);
        assert_eq!(4, results.len());
    }

    #[test]
    fn test_install_item_success() {
        let mut mgr = MarketplaceManager::new();
        mgr.set_items(sample_items());

        let result = mgr.install_item("mcp-1");
        assert!(result.is_ok());
        assert!(mgr.is_installed("mcp-1"));
    }

    #[test]
    fn test_install_item_not_found() {
        let mut mgr = MarketplaceManager::new();
        mgr.set_items(sample_items());

        let result = mgr.install_item("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_install_item_already_installed() {
        let mut mgr = MarketplaceManager::new();
        mgr.set_items(sample_items());

        mgr.install_item("mcp-1").unwrap();
        let result = mgr.install_item("mcp-1");
        assert!(result.is_err());
    }

    #[test]
    fn test_install_creates_metadata() {
        let mut mgr = MarketplaceManager::new();
        mgr.set_items(sample_items());

        mgr.install_item("mcp-1").unwrap();
        let meta = mgr.get_installation_metadata("mcp-1").unwrap();
        assert_eq!("mcp-1", meta.item_id);
        assert_eq!(MarketplaceItemType::Mcp, meta.item_type);
        assert_eq!("marketplace", meta.source);
    }

    #[test]
    fn test_remove_item_success() {
        let mut mgr = MarketplaceManager::new();
        mgr.set_items(sample_items());

        mgr.install_item("mcp-1").unwrap();
        let removed = mgr.remove_item("mcp-1").unwrap();
        assert!(removed);
        assert!(!mgr.is_installed("mcp-1"));
    }

    #[test]
    fn test_remove_item_not_installed() {
        let mut mgr = MarketplaceManager::new();
        mgr.set_items(sample_items());

        let removed = mgr.remove_item("mcp-1").unwrap();
        assert!(!removed);
    }

    #[test]
    fn test_remove_item_not_found() {
        let mut mgr = MarketplaceManager::new();
        mgr.set_items(sample_items());

        let result = mgr.remove_item("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_is_installed_false_for_unknown() {
        let mgr = MarketplaceManager::new();
        assert!(!mgr.is_installed("unknown"));
    }

    #[test]
    fn test_get_metadata_none_for_unknown() {
        let mgr = MarketplaceManager::new();
        assert!(mgr.get_installation_metadata("unknown").is_none());
    }

    #[test]
    fn test_cleanup_clears_all() {
        let mut mgr = MarketplaceManager::new();
        mgr.set_items(sample_items());

        mgr.install_item("mcp-1").unwrap();
        mgr.install_item("mode-1").unwrap();
        mgr.cleanup();

        assert!(!mgr.is_installed("mcp-1"));
        assert!(!mgr.is_installed("mode-1"));
        assert!(mgr.get_installation_metadata("mcp-1").is_none());
        assert!(mgr.get_installation_metadata("mode-1").is_none());
        // Items still present but marked uninstalled
        assert_eq!(4, mgr.get_items().len());
    }

    #[test]
    fn test_filter_search_is_case_insensitive() {
        let mut mgr = MarketplaceManager::new();
        mgr.set_items(sample_items());

        let filter = MarketplaceFilter {
            search_query: Some("MCP".to_string()),
            ..Default::default()
        };
        let results = mgr.filter_items(&filter);
        assert!(results.len() >= 2);
    }

    #[test]
    fn test_filter_tags_case_insensitive() {
        let mut mgr = MarketplaceManager::new();
        mgr.set_items(sample_items());

        let filter = MarketplaceFilter {
            tags: Some(vec!["MCP".to_string()]),
            ..Default::default()
        };
        let results = mgr.filter_items(&filter);
        assert_eq!(2, results.len());
    }
}

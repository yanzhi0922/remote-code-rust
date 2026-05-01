use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Types of items available in the marketplace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarketplaceItemType {
    Mcp,
    Mode,
    Skill,
}

/// An item listed in the marketplace.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarketplaceItem {
    pub id: String,
    pub name: String,
    pub description: String,
    pub item_type: MarketplaceItemType,
    pub author: String,
    pub url: String,
    pub tags: Vec<String>,
    pub installed: bool,
    /// Extra fields such as `content`, `parameters`, etc.
    /// Stored as a JSON map to support flexible marketplace item data.
    #[serde(default, skip_serializing_if = "serde_json::Map::is_empty")]
    pub extra: serde_json::Map<String, Value>,
}

/// Response payload containing a list of marketplace items.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MarketplaceItemsResponse {
    pub items: Vec<MarketplaceItem>,
}

/// Filter criteria for querying marketplace items.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MarketplaceFilter {
    pub search_query: Option<String>,
    pub item_type: Option<MarketplaceItemType>,
    pub tags: Option<Vec<String>>,
}

/// Metadata recorded when an item is installed.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstallationMetadata {
    pub item_id: String,
    pub item_type: MarketplaceItemType,
    pub installed_at: u64,
    pub source: String,
}

/// Errors that can occur during marketplace operations.
#[derive(Clone, Debug, thiserror::Error)]
pub enum MarketplaceError {
    #[error("item not found: {0}")]
    ItemNotFound(String),

    #[error("item already installed: {0}")]
    AlreadyInstalled(String),

    #[error("serialization error: {0}")]
    SerializationError(String),
}

impl From<serde_json::Error> for MarketplaceError {
    fn from(err: serde_json::Error) -> Self {
        MarketplaceError::SerializationError(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_marketplace_item_type_serde_roundtrip() {
        let types = vec![
            MarketplaceItemType::Mcp,
            MarketplaceItemType::Mode,
            MarketplaceItemType::Skill,
        ];
        for t in types {
            let json = serde_json::to_string(&t).unwrap();
            let deserialized: MarketplaceItemType = serde_json::from_str(&json).unwrap();
            assert_eq!(t, deserialized);
        }
    }

    #[test]
    fn test_marketplace_item_type_json_values() {
        assert_eq!(
            "\"mcp\"",
            serde_json::to_string(&MarketplaceItemType::Mcp).unwrap()
        );
        assert_eq!(
            "\"mode\"",
            serde_json::to_string(&MarketplaceItemType::Mode).unwrap()
        );
        assert_eq!(
            "\"skill\"",
            serde_json::to_string(&MarketplaceItemType::Skill).unwrap()
        );
    }

    #[test]
    fn test_marketplace_item_serialization() {
        let item = MarketplaceItem {
            id: "test-1".to_string(),
            name: "Test Item".to_string(),
            description: "A test item".to_string(),
            item_type: MarketplaceItemType::Mcp,
            author: "test-author".to_string(),
            url: "https://example.com".to_string(),
            tags: vec!["test".to_string()],
            installed: false,
            extra: serde_json::Map::new(),
        };
        let json = serde_json::to_string(&item).unwrap();
        let deserialized: MarketplaceItem = serde_json::from_str(&json).unwrap();
        assert_eq!(item.id, deserialized.id);
        assert_eq!(item.name, deserialized.name);
        assert_eq!(item.item_type, deserialized.item_type);
    }

    #[test]
    fn test_marketplace_filter_default() {
        let filter = MarketplaceFilter::default();
        assert!(filter.search_query.is_none());
        assert!(filter.item_type.is_none());
        assert!(filter.tags.is_none());
    }

    #[test]
    fn test_installation_metadata_serialization() {
        let meta = InstallationMetadata {
            item_id: "item-1".to_string(),
            item_type: MarketplaceItemType::Skill,
            installed_at: 1700000000,
            source: "marketplace".to_string(),
        };
        let json = serde_json::to_string(&meta).unwrap();
        let deserialized: InstallationMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(meta.item_id, deserialized.item_id);
        assert_eq!(meta.installed_at, deserialized.installed_at);
    }

    #[test]
    fn test_marketplace_error_display() {
        let err = MarketplaceError::ItemNotFound("abc".to_string());
        assert_eq!(format!("{err}"), "item not found: abc");

        let err = MarketplaceError::AlreadyInstalled("abc".to_string());
        assert_eq!(format!("{err}"), "item already installed: abc");
    }

    #[test]
    fn test_marketplace_items_response_serialization() {
        let response = MarketplaceItemsResponse {
            items: vec![
                MarketplaceItem {
                    id: "1".to_string(),
                    name: "Item 1".to_string(),
                    description: "First".to_string(),
                    item_type: MarketplaceItemType::Mcp,
                    author: "author".to_string(),
                    url: "https://example.com/1".to_string(),
                    tags: vec![],
                    installed: false,
                    extra: serde_json::Map::new(),
                },
            ],
        };
        let json = serde_json::to_string(&response).unwrap();
        let deserialized: MarketplaceItemsResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(1, deserialized.items.len());
    }
}

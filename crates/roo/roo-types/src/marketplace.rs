//! Marketplace type definitions.
//!
//! Derived from `packages/types/src/marketplace.ts`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// McpParameter
// ---------------------------------------------------------------------------

/// MCP parameter definition.
///
/// Source: `packages/types/src/marketplace.ts` — `mcpParameterSchema`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpParameter {
    pub name: String,
    pub key: String,
    pub placeholder: Option<String>,
    #[serde(default)]
    pub optional: Option<bool>,
}

// ---------------------------------------------------------------------------
// McpInstallationMethod
// ---------------------------------------------------------------------------

/// MCP installation method.
///
/// Source: `packages/types/src/marketplace.ts` — `mcpInstallationMethodSchema`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpInstallationMethod {
    pub name: String,
    pub content: String,
    pub parameters: Option<Vec<McpParameter>>,
    pub prerequisites: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// MarketplaceItemType
// ---------------------------------------------------------------------------

/// Type of marketplace item.
///
/// Source: `packages/types/src/marketplace.ts` — `marketplaceItemTypeSchema`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MarketplaceItemType {
    #[serde(rename = "mode")]
    Mode,
    #[serde(rename = "mcp")]
    Mcp,
}

// ---------------------------------------------------------------------------
// ModeMarketplaceItem
// ---------------------------------------------------------------------------

/// Mode marketplace item.
///
/// Source: `packages/types/src/marketplace.ts` — `modeMarketplaceItemSchema`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeMarketplaceItem {
    #[serde(rename = "type")]
    pub item_type: MarketplaceItemType,
    pub id: String,
    pub name: String,
    pub description: String,
    pub content: String,
    pub author: Option<String>,
    pub author_url: Option<String>,
    pub tags: Option<Vec<String>>,
    pub prerequisites: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// McpMarketplaceItem
// ---------------------------------------------------------------------------

/// MCP marketplace item.
///
/// Source: `packages/types/src/marketplace.ts` — `mcpMarketplaceItemSchema`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpMarketplaceItem {
    #[serde(rename = "type")]
    pub item_type: MarketplaceItemType,
    pub id: String,
    pub name: String,
    pub description: String,
    pub url: String,
    pub content: McpContent,
    pub parameters: Option<Vec<McpParameter>>,
    pub author: Option<String>,
    pub author_url: Option<String>,
    pub tags: Option<Vec<String>>,
    pub prerequisites: Option<Vec<String>>,
}

/// MCP content can be a single config string or an array of installation methods.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum McpContent {
    Single(String),
    Multiple(Vec<McpInstallationMethod>),
}

// ---------------------------------------------------------------------------
// MarketplaceItem
// ---------------------------------------------------------------------------

/// Unified marketplace item.
///
/// Source: `packages/types/src/marketplace.ts` — `marketplaceItemSchema`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MarketplaceItem {
    #[serde(rename = "mode")]
    Mode(ModeMarketplaceItemBase),
    #[serde(rename = "mcp")]
    Mcp(McpMarketplaceItemBase),
}

/// Base fields for mode marketplace item (without type discriminator).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModeMarketplaceItemBase {
    pub id: String,
    pub name: String,
    pub description: String,
    pub content: String,
    pub author: Option<String>,
    pub author_url: Option<String>,
    pub tags: Option<Vec<String>>,
    pub prerequisites: Option<Vec<String>>,
}

/// Base fields for MCP marketplace item (without type discriminator).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpMarketplaceItemBase {
    pub id: String,
    pub name: String,
    pub description: String,
    pub url: String,
    pub content: McpContent,
    pub parameters: Option<Vec<McpParameter>>,
    pub author: Option<String>,
    pub author_url: Option<String>,
    pub tags: Option<Vec<String>>,
    pub prerequisites: Option<Vec<String>>,
}

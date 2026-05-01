//! Custom tool type definitions.
//!
//! Derived from `packages/types/src/custom-tool.ts`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// CustomToolDefinition (serialized form)
// ---------------------------------------------------------------------------

/// Serialized custom tool definition.
///
/// Source: `packages/types/src/custom-tool.ts` — `SerializedCustomToolDefinition`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedCustomToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// CustomToolConfig
// ---------------------------------------------------------------------------

/// Configuration for a custom tool loaded from a file.
///
/// Source: `packages/types/src/custom-tool.ts`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomToolConfig {
    pub name: String,
    pub description: String,
    pub file_path: String,
    pub source: CustomToolSource,
}

/// Where a custom tool was defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CustomToolSource {
    Global,
    Project,
}

//! `.roomodes` configuration schema type definitions.
//!
//! Derived from the `.roomodes` YAML schema used by Roo Code to define
//! custom modes at the project level.

use serde::{Deserialize, Serialize};

/// Top-level structure of a `.roomodes` file.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RoomodesConfig {
    /// List of custom mode definitions.
    #[serde(default)]
    pub custom_modes: Vec<CustomModeConfig>,
}

/// A single custom mode definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CustomModeConfig {
    /// URL-friendly identifier for the mode (e.g. "translate").
    pub slug: String,
    /// Human-readable display name (may include emoji).
    pub name: String,
    /// The system prompt / role definition for the mode.
    #[serde(default)]
    pub role_definition: String,
    /// Short description shown in mode selection UI.
    #[serde(default)]
    pub description: String,
    /// Hint for when to use this mode.
    #[serde(default, rename = "whenToUse")]
    pub when_to_use: Option<String>,
    /// Tool groups the mode has access to.
    #[serde(default)]
    pub groups: Vec<ToolGroup>,
    /// Where this mode definition originates from.
    #[serde(default)]
    pub source: ModeSource,
}

/// A tool group — either a simple string name or a group with a file-regex
/// constraint.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolGroup {
    /// Simple group name (e.g. "read", "edit", "command", "mcp").
    Simple(String),
    /// Group with an associated file-regex restriction.
    WithRegex {
        /// The tool group name (first element of the two-element array).
        #[serde(rename = "0")]
        group: String,
        /// Optional file-regex constraint.
        #[serde(skip_serializing_if = "Option::is_none")]
        file_regex: Option<FileRegexConstraint>,
    },
}

/// File-regex constraint applied to a tool group.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileRegexConstraint {
    /// Regex pattern that file paths must match.
    #[serde(rename = "fileRegex")]
    pub regex: String,
    /// Human-readable description of the constraint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Source of a custom mode definition.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModeSource {
    #[default]
    Project,
    Global,
}

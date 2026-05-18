//! `.roomodes` configuration schema type definitions.
//!
//! Derived from the `.roomodes` YAML schema used by Roo Code to define
//! custom modes at the project level.

use serde::{Deserialize, Serialize};

pub use crate::tool::{GroupEntry as ToolGroup, GroupOptions as FileRegexConstraint};

/// Top-level structure of a `.roomodes` file.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomodesConfig {
    /// List of custom mode definitions.
    #[serde(default)]
    pub custom_modes: Vec<CustomModeConfig>,
}

/// A single custom mode definition.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomModeConfig {
    /// URL-friendly identifier for the mode (e.g. "translate").
    pub slug: String,
    /// Human-readable display name (may include emoji).
    pub name: String,
    /// The system prompt / role definition for the mode.
    #[serde(default)]
    pub role_definition: String,
    /// Additional custom instructions appended to the mode prompt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_instructions: Option<String>,
    /// Short description shown in mode selection UI.
    #[serde(default)]
    pub description: String,
    /// Hint for when to use this mode.
    #[serde(default)]
    pub when_to_use: Option<String>,
    /// Tool groups the mode has access to.
    #[serde(default)]
    pub groups: Vec<ToolGroup>,
    /// Rule files bundled with imported/exported mode metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules_files: Option<Vec<String>>,
    /// Where this mode definition originates from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<ModeSource>,
}

/// Source of a custom mode definition.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModeSource {
    #[default]
    Project,
    Global,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tool::{GroupEntry, ToolGroup as BuiltinToolGroup};
    use serde_json::json;

    #[test]
    fn parses_upstream_roomodes_camel_case_fields_and_tuple_groups() {
        let config: RoomodesConfig = serde_json::from_value(json!({
            "customModes": [{
                "slug": "docs-review",
                "name": "Docs Review",
                "roleDefinition": "Review docs",
                "customInstructions": "Prefer concise edits",
                "description": "Documentation mode",
                "whenToUse": "When editing docs",
                "groups": [
                    "read",
                    ["edit", { "fileRegex": "\\\\.md$", "description": "Markdown only" }]
                ],
                "rulesFiles": ["rules-docs.md"],
                "source": "project"
            }]
        }))
        .expect("valid upstream .roomodes should deserialize");

        let mode = &config.custom_modes[0];
        assert_eq!(mode.slug, "docs-review");
        assert_eq!(mode.role_definition, "Review docs");
        assert_eq!(
            mode.custom_instructions.as_deref(),
            Some("Prefer concise edits")
        );
        assert_eq!(mode.when_to_use.as_deref(), Some("When editing docs"));
        assert_eq!(
            mode.rules_files.as_deref(),
            Some(&["rules-docs.md".to_string()][..])
        );
        assert_eq!(mode.source, Some(ModeSource::Project));
        assert!(matches!(
            mode.groups.first(),
            Some(GroupEntry::Plain(BuiltinToolGroup::Read))
        ));
        assert!(matches!(
            mode.groups.get(1),
            Some(GroupEntry::WithOptions(BuiltinToolGroup::Edit, options))
                if options.file_regex.as_deref() == Some("\\\\.md$")
                    && options.description.as_deref() == Some("Markdown only")
        ));
    }

    #[test]
    fn serializes_roomodes_with_roo_field_names() {
        let config = RoomodesConfig {
            custom_modes: vec![CustomModeConfig {
                slug: "reviewer".to_string(),
                name: "Reviewer".to_string(),
                role_definition: "Review code".to_string(),
                custom_instructions: Some("Be strict".to_string()),
                description: "Review".to_string(),
                when_to_use: Some("During review".to_string()),
                groups: vec![GroupEntry::Plain(BuiltinToolGroup::Read)],
                rules_files: Some(vec!["rules-review.md".to_string()]),
                source: Some(ModeSource::Global),
            }],
        };

        let value = serde_json::to_value(config).expect("roomodes should serialize");
        let mode = &value["customModes"][0];
        assert_eq!(mode["roleDefinition"], "Review code");
        assert_eq!(mode["customInstructions"], "Be strict");
        assert_eq!(mode["whenToUse"], "During review");
        assert_eq!(mode["rulesFiles"], json!(["rules-review.md"]));
        assert_eq!(mode["source"], "global");
    }
}

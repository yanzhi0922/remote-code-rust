//! Type definitions for skills.
//!
//! Derived from `src/shared/skills.ts` and `packages/types/src/skills.ts`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// SkillSource
// ---------------------------------------------------------------------------

/// Where a skill was defined: globally or in the current project.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillSource {
    Global,
    Project,
}

impl std::fmt::Display for SkillSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillSource::Global => write!(f, "global"),
            SkillSource::Project => write!(f, "project"),
        }
    }
}

// ---------------------------------------------------------------------------
// SkillMetadata
// ---------------------------------------------------------------------------

/// Metadata for a skill, parsed from a SKILL.md frontmatter.
///
/// Source: `src/shared/skills.ts` — `SkillMetadata`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillMetadata {
    /// Unique name of the skill (lowercase alphanumeric + hyphens).
    pub name: String,
    /// Short description of what the skill does.
    pub description: String,
    /// Filesystem path to the skill directory.
    pub path: String,
    /// Where this skill was defined.
    pub source: SkillSource,
    /// Deprecated: single mode slug. Prefer `mode_slugs`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    /// List of mode slugs this skill is available in.
    /// `None` or empty means available in all modes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mode_slugs: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// SkillContent
// ---------------------------------------------------------------------------

/// Full content of a skill, including its instructions.
///
/// Source: `src/shared/skills.ts` — `SkillContent`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillContent {
    /// The skill metadata.
    #[serde(flatten)]
    pub metadata: SkillMetadata,
    /// The instruction text (body of SKILL.md after frontmatter).
    pub instructions: String,
}

// ---------------------------------------------------------------------------
// SkillNameValidationError
// ---------------------------------------------------------------------------

/// Validation errors for skill names.
///
/// Source: `packages/types/src/skills.ts` — `SkillNameValidationError`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillNameValidationError {
    /// The name is empty.
    Empty,
    /// The name exceeds the maximum length.
    TooLong,
    /// The name contains invalid characters.
    InvalidFormat,
}

// ---------------------------------------------------------------------------
// SkillNameValidationResult
// ---------------------------------------------------------------------------

/// Result of validating a skill name.
///
/// Source: `packages/types/src/skills.ts` — `SkillNameValidationResult`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillNameValidationResult {
    /// Whether the name is valid.
    pub valid: bool,
    /// The validation error, if any.
    pub error: Option<SkillNameValidationError>,
}

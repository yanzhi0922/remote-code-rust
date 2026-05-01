//! Skills type definitions.
//!
//! Derived from `packages/types/src/skills.ts`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// SkillMetadata
// ---------------------------------------------------------------------------

/// Skill metadata for discovery (loaded at startup).
///
/// Source: `packages/types/src/skills.ts` — `SkillMetadata`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillMetadata {
    /// Skill identifier.
    pub name: String,
    /// When to use this skill.
    pub description: String,
    /// Absolute path to SKILL.md.
    pub path: String,
    /// Where the skill was discovered.
    pub source: SkillSource,
    /// Deprecated: use mode_slugs instead.
    #[serde(default)]
    pub mode: Option<String>,
    /// Mode slugs where this skill is available.
    /// None or empty array means available in all modes.
    #[serde(default)]
    pub mode_slugs: Option<Vec<String>>,
}

/// Where a skill was discovered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SkillSource {
    Global,
    Project,
}

// ---------------------------------------------------------------------------
// Skill name validation constants
// ---------------------------------------------------------------------------

/// Minimum skill name length.
pub const SKILL_NAME_MIN_LENGTH: usize = 1;

/// Maximum skill name length.
pub const SKILL_NAME_MAX_LENGTH: usize = 64;

// ---------------------------------------------------------------------------
// SkillNameValidationError
// ---------------------------------------------------------------------------

/// Error codes for skill name validation.
///
/// Source: `packages/types/src/skills.ts` — `SkillNameValidationError`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillNameValidationError {
    Empty,
    TooLong,
    InvalidFormat,
}

// ---------------------------------------------------------------------------
// SkillNameValidationResult
// ---------------------------------------------------------------------------

/// Result of skill name validation.
///
/// Source: `packages/types/src/skills.ts` — `SkillNameValidationResult`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillNameValidationResult {
    pub valid: bool,
    pub error: Option<SkillNameValidationError>,
}

/// Validates a skill name according to agentskills.io specification.
///
/// Source: `packages/types/src/skills.ts` — `validateSkillName`
pub fn validate_skill_name(name: &str) -> SkillNameValidationResult {
    if name.is_empty() || name.len() < SKILL_NAME_MIN_LENGTH {
        return SkillNameValidationResult {
            valid: false,
            error: Some(SkillNameValidationError::Empty),
        };
    }

    if name.len() > SKILL_NAME_MAX_LENGTH {
        return SkillNameValidationResult {
            valid: false,
            error: Some(SkillNameValidationError::TooLong),
        };
    }

    // Validate: lowercase letters, numbers, and hyphens only.
    // No leading/trailing hyphens, no consecutive hyphens.
    let bytes = name.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'-' {
            // No leading hyphen
            if i == 0 {
                return SkillNameValidationResult {
                    valid: false,
                    error: Some(SkillNameValidationError::InvalidFormat),
                };
            }
            // No trailing hyphen
            if i == bytes.len() - 1 {
                return SkillNameValidationResult {
                    valid: false,
                    error: Some(SkillNameValidationError::InvalidFormat),
                };
            }
            // No consecutive hyphens
            if bytes[i + 1] == b'-' {
                return SkillNameValidationResult {
                    valid: false,
                    error: Some(SkillNameValidationError::InvalidFormat),
                };
            }
        } else if !(c.is_ascii_lowercase() || c.is_ascii_digit()) {
            return SkillNameValidationResult {
                valid: false,
                error: Some(SkillNameValidationError::InvalidFormat),
            };
        }
        i += 1;
    }

    SkillNameValidationResult {
        valid: true,
        error: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_skill_names() {
        assert!(validate_skill_name("my-skill").valid);
        assert!(validate_skill_name("skill").valid);
        assert!(validate_skill_name("my-skill-123").valid);
        assert!(validate_skill_name("a").valid);
    }

    #[test]
    fn test_invalid_skill_names() {
        assert!(!validate_skill_name("").valid);
        assert!(!validate_skill_name("-skill").valid);
        assert!(!validate_skill_name("skill-").valid);
        assert!(!validate_skill_name("my--skill").valid);
        assert!(!validate_skill_name("MySkill").valid);
        assert!(!validate_skill_name("my_skill").valid);
    }
}

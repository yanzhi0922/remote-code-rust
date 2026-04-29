//! Skill name validation.
//!
//! Derived from `packages/types/src/skills.ts` — `validateSkillName`,
//! `getSkillNameErrorMessage`, and related constants.

use crate::types::{SkillNameValidationError, SkillNameValidationResult};

/// Minimum allowed length for a skill name.
pub const SKILL_NAME_MIN_LENGTH: usize = 1;

/// Maximum allowed length for a skill name.
pub const SKILL_NAME_MAX_LENGTH: usize = 64;

/// Regex pattern that valid skill names must match: `^[a-z0-9][a-z0-9-]*$`
pub const SKILL_NAME_REGEX_STR: &str = r"^[a-z0-9][a-z0-9-]*$";

/// Validate a skill name.
///
/// Returns a [`SkillNameValidationResult`] indicating whether the name is valid.
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

    let re = regex::Regex::new(SKILL_NAME_REGEX_STR).expect("invalid skill name regex");
    if !re.is_match(name) {
        return SkillNameValidationResult {
            valid: false,
            error: Some(SkillNameValidationError::InvalidFormat),
        };
    }

    SkillNameValidationResult {
        valid: true,
        error: None,
    }
}

/// Convert a validation error into a human-readable message.
///
/// Source: `packages/types/src/skills.ts` — `getSkillNameErrorMessage`
pub fn get_skill_name_error_message(name: &str, error: SkillNameValidationError) -> String {
    match error {
        SkillNameValidationError::Empty => "Skill name cannot be empty.".to_string(),
        SkillNameValidationError::TooLong => format!(
            "Skill name '{}' is too long (max {} characters).",
            name, SKILL_NAME_MAX_LENGTH
        ),
        SkillNameValidationError::InvalidFormat => format!(
            "Skill name '{}' must start with a lowercase letter or number and contain only lowercase letters, numbers, and hyphens.",
            name
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_simple_name() {
        let result = validate_skill_name("my-skill");
        assert!(result.valid);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_valid_alphanumeric_start() {
        let result = validate_skill_name("a1-skill");
        assert!(result.valid);
    }

    #[test]
    fn test_valid_single_char() {
        let result = validate_skill_name("a");
        assert!(result.valid);
    }

    #[test]
    fn test_valid_single_digit() {
        let result = validate_skill_name("1");
        assert!(result.valid);
    }

    #[test]
    fn test_empty_name() {
        let result = validate_skill_name("");
        assert!(!result.valid);
        assert_eq!(result.error, Some(SkillNameValidationError::Empty));
    }

    #[test]
    fn test_too_long_name() {
        let long_name = "a".repeat(65);
        let result = validate_skill_name(&long_name);
        assert!(!result.valid);
        assert_eq!(result.error, Some(SkillNameValidationError::TooLong));
    }

    #[test]
    fn test_max_length_name() {
        let max_name = "a".repeat(64);
        let result = validate_skill_name(&max_name);
        assert!(result.valid);
    }

    #[test]
    fn test_uppercase_invalid() {
        let result = validate_skill_name("My-Skill");
        assert!(!result.valid);
        assert_eq!(result.error, Some(SkillNameValidationError::InvalidFormat));
    }

    #[test]
    fn test_starts_with_hyphen() {
        let result = validate_skill_name("-skill");
        assert!(!result.valid);
        assert_eq!(result.error, Some(SkillNameValidationError::InvalidFormat));
    }

    #[test]
    fn test_underscore_invalid() {
        let result = validate_skill_name("my_skill");
        assert!(!result.valid);
        assert_eq!(result.error, Some(SkillNameValidationError::InvalidFormat));
    }

    #[test]
    fn test_space_invalid() {
        let result = validate_skill_name("my skill");
        assert!(!result.valid);
        assert_eq!(result.error, Some(SkillNameValidationError::InvalidFormat));
    }

    #[test]
    fn test_error_message_empty() {
        let msg = get_skill_name_error_message("", SkillNameValidationError::Empty);
        assert!(msg.contains("cannot be empty"));
    }

    #[test]
    fn test_error_message_too_long() {
        let msg = get_skill_name_error_message("abc", SkillNameValidationError::TooLong);
        assert!(msg.contains("too long"));
        assert!(msg.contains("64"));
    }

    #[test]
    fn test_error_message_invalid_format() {
        let msg = get_skill_name_error_message("ABC", SkillNameValidationError::InvalidFormat);
        assert!(msg.contains("lowercase"));
    }
}

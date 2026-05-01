//! Mode selection logic.
//!
//! Source: `src/shared/modes.ts` — `getModeSelection`, `ModeSelection`

use roo_types::mode::{default_modes, ModeConfig, PromptComponent};

// ---------------------------------------------------------------------------
// ModeSelection
// ---------------------------------------------------------------------------

/// The resolved prompt components for a given mode selection.
///
/// Source: `src/shared/modes.ts` — return type of `getModeSelection`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeSelection {
    /// The role definition for the selected mode.
    pub role_definition: String,
    /// Base instructions (custom instructions) for the selected mode.
    pub base_instructions: String,
    /// Short description of the selected mode.
    pub description: String,
}

// ---------------------------------------------------------------------------
// findModeBySlug (local helper)
// ---------------------------------------------------------------------------

/// Find a mode by slug in a given list.
///
/// Source: `src/shared/modes.ts` — `findModeBySlug`
fn find_mode_by_slug<'a>(slug: &str, modes: Option<&'a [ModeConfig]>) -> Option<&'a ModeConfig> {
    modes.and_then(|ms| ms.iter().find(|m| m.slug == slug))
}

// ---------------------------------------------------------------------------
// getModeSelection
// ---------------------------------------------------------------------------

/// Resolves the effective role definition, base instructions, and description
/// for a given mode slug, taking into account custom modes and prompt component
/// overrides.
///
/// Logic (matching TypeScript source):
/// 1. If the slug matches a custom mode → use custom mode's fields directly.
/// 2. Otherwise, look up the built-in mode (or fall back to the first default).
/// 3. Apply `prompt_component` overrides on top of the built-in mode.
///
/// Source: `src/shared/modes.ts` — `getModeSelection`
pub fn get_mode_selection(
    mode: &str,
    prompt_component: Option<&PromptComponent>,
    custom_modes: Option<&[ModeConfig]>,
) -> ModeSelection {
    // 1. Check custom modes first
    if let Some(custom) = find_mode_by_slug(mode, custom_modes) {
        return ModeSelection {
            role_definition: custom.role_definition.clone(),
            base_instructions: custom.custom_instructions.clone().unwrap_or_default(),
            description: custom.description.clone().unwrap_or_default(),
        };
    }

    // 2. Check built-in modes
    let defaults = default_modes();
    let base_mode = find_mode_by_slug(mode, Some(&defaults))
        .unwrap_or_else(|| defaults.first().expect("at least one default mode must exist"));

    // 3. Apply prompt_component overrides
    ModeSelection {
        role_definition: prompt_component
            .and_then(|pc| pc.role_definition.as_deref())
            .unwrap_or_else(|| base_mode.role_definition.as_str())
            .to_string(),
        base_instructions: prompt_component
            .and_then(|pc| pc.custom_instructions.as_deref())
            .unwrap_or_else(|| base_mode.custom_instructions.as_deref().unwrap_or(""))
            .to_string(),
        description: base_mode.description.clone().unwrap_or_default(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_mode_selection_builtin_code() {
        let sel = get_mode_selection("code", None, None);
        assert!(!sel.role_definition.is_empty());
        assert!(sel.description.contains("code"));
    }

    #[test]
    fn test_get_mode_selection_builtin_architect() {
        let sel = get_mode_selection("architect", None, None);
        assert!(sel.role_definition.contains("technical leader"));
        assert!(sel.base_instructions.contains("information gathering"));
    }

    #[test]
    fn test_get_mode_selection_custom_mode_takes_priority() {
        let custom = ModeConfig {
            slug: "code".into(),
            name: "Custom Code".into(),
            role_definition: "Custom role".into(),
            when_to_use: None,
            description: Some("Custom desc".into()),
            custom_instructions: Some("Custom instructions".into()),
            groups: vec![],
            source: None,
        };
        let sel = get_mode_selection("code", None, Some(&[custom]));
        assert_eq!(sel.role_definition, "Custom role");
        assert_eq!(sel.base_instructions, "Custom instructions");
        assert_eq!(sel.description, "Custom desc");
    }

    #[test]
    fn test_get_mode_selection_prompt_component_override() {
        let pc = PromptComponent {
            role_definition: Some("Overridden role".into()),
            custom_instructions: Some("Overridden instructions".into()),
            when_to_use: None,
            description: None,
        };
        let sel = get_mode_selection("code", Some(&pc), None);
        assert_eq!(sel.role_definition, "Overridden role");
        assert_eq!(sel.base_instructions, "Overridden instructions");
    }

    #[test]
    fn test_get_mode_selection_unknown_slug_falls_back_to_first() {
        let sel = get_mode_selection("nonexistent", None, None);
        // Should fall back to the first default mode (architect)
        let defaults = default_modes();
        let first = defaults.first().unwrap();
        assert_eq!(sel.role_definition, first.role_definition);
    }

    #[test]
    fn test_get_mode_selection_custom_mode_no_optional_fields() {
        let custom = ModeConfig {
            slug: "minimal".into(),
            name: "Minimal".into(),
            role_definition: "Min role".into(),
            when_to_use: None,
            description: None,
            custom_instructions: None,
            groups: vec![],
            source: None,
        };
        let sel = get_mode_selection("minimal", None, Some(&[custom]));
        assert_eq!(sel.role_definition, "Min role");
        assert_eq!(sel.base_instructions, "");
        assert_eq!(sel.description, "");
    }

    #[test]
    fn test_get_mode_selection_prompt_component_partial_override() {
        // Only role_definition is overridden; custom_instructions falls through
        let pc = PromptComponent {
            role_definition: Some("Partial override".into()),
            custom_instructions: None,
            when_to_use: None,
            description: None,
        };
        let sel = get_mode_selection("code", Some(&pc), None);
        assert_eq!(sel.role_definition, "Partial override");
        // code mode has no custom_instructions, so base_instructions should be ""
        assert_eq!(sel.base_instructions, "");
    }
}

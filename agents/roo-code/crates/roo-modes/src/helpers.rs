//! Helper functions for mode operations.
//!
//! Source: `src/shared/modes.ts` — `defaultModeSlug`, `isCustomMode`,
//! `findModeBySlug`, `getWhenToUse`, `getDescription`, `defaultPrompts`

use std::collections::HashMap;

use roo_types::mode::{default_modes, get_mode_by_slug, ModeConfig, PromptComponent};

// ---------------------------------------------------------------------------
// defaultModeSlug
// ---------------------------------------------------------------------------

/// Returns the slug of the first default mode ("architect").
///
/// Source: `src/shared/modes.ts` — `defaultModeSlug`
pub fn default_mode_slug() -> &'static str {
    "architect"
}

// ---------------------------------------------------------------------------
// isCustomMode
// ---------------------------------------------------------------------------

/// Returns `true` if the given slug matches any mode in the custom modes list.
///
/// Source: `src/shared/modes.ts` — `isCustomMode`
pub fn is_custom_mode(slug: &str, custom_modes: Option<&[ModeConfig]>) -> bool {
    custom_modes
        .map(|ms| ms.iter().any(|m| m.slug == slug))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// findModeBySlug
// ---------------------------------------------------------------------------

/// Find a mode by slug in a given list (no fallback to defaults).
///
/// Unlike [`roo_types::mode::get_mode_by_slug`], this function only searches
/// the provided list and does not merge with default modes.
///
/// Source: `src/shared/modes.ts` — `findModeBySlug`
pub fn find_mode_by_slug(slug: &str, modes: Option<&[ModeConfig]>) -> Option<ModeConfig> {
    modes.and_then(|ms| ms.iter().find(|m| m.slug == slug).cloned())
}

// ---------------------------------------------------------------------------
// getWhenToUse
// ---------------------------------------------------------------------------

/// Get the "when to use" description for a mode.
///
/// Looks up the mode by slug (custom modes take priority over defaults).
///
/// Source: `src/shared/modes.ts` — `getWhenToUse`
pub fn get_when_to_use(mode_slug: &str, custom_modes: Option<&[ModeConfig]>) -> String {
    get_mode_by_slug(mode_slug, custom_modes)
        .and_then(|m| m.when_to_use)
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// getDescription
// ---------------------------------------------------------------------------

/// Get the short description for a mode.
///
/// Looks up the mode by slug (custom modes take priority over defaults).
///
/// Source: `src/shared/modes.ts` — `getDescription`
pub fn get_description(mode_slug: &str, custom_modes: Option<&[ModeConfig]>) -> String {
    get_mode_by_slug(mode_slug, custom_modes)
        .and_then(|m| m.description)
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// defaultPrompts
// ---------------------------------------------------------------------------

/// Build the default prompts map from the built-in modes.
///
/// Returns a `HashMap<String, PromptComponent>` where each key is a mode slug
/// and each value contains the mode's `role_definition`, `when_to_use`,
/// `custom_instructions`, and `description`.
///
/// Source: `src/shared/modes.ts` — `defaultPrompts`
pub fn default_prompts() -> HashMap<String, PromptComponent> {
    default_modes()
        .into_iter()
        .map(|mode| {
            (
                mode.slug.clone(),
                PromptComponent {
                    role_definition: Some(mode.role_definition),
                    when_to_use: mode.when_to_use,
                    custom_instructions: mode.custom_instructions,
                    description: mode.description,
                },
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_mode_slug() {
        assert_eq!(default_mode_slug(), "architect");
    }

    #[test]
    fn test_is_custom_mode_true() {
        let custom = ModeConfig {
            slug: "my-mode".into(),
            name: "My Mode".into(),
            role_definition: "Custom".into(),
            when_to_use: None,
            description: None,
            custom_instructions: None,
            groups: vec![],
            source: None,
        };
        assert!(is_custom_mode("my-mode", Some(&[custom])));
    }

    #[test]
    fn test_is_custom_mode_false() {
        let custom = ModeConfig {
            slug: "my-mode".into(),
            name: "My Mode".into(),
            role_definition: "Custom".into(),
            when_to_use: None,
            description: None,
            custom_instructions: None,
            groups: vec![],
            source: None,
        };
        assert!(!is_custom_mode("code", Some(&[custom])));
    }

    #[test]
    fn test_is_custom_mode_none() {
        assert!(!is_custom_mode("code", None));
    }

    #[test]
    fn test_find_mode_by_slug_found() {
        let modes = default_modes();
        let found = find_mode_by_slug("code", Some(&modes));
        assert!(found.is_some());
        assert_eq!(found.unwrap().slug, "code");
    }

    #[test]
    fn test_find_mode_by_slug_not_found() {
        let found = find_mode_by_slug("nonexistent", None);
        assert!(found.is_none());
    }

    #[test]
    fn test_find_mode_by_slug_in_custom() {
        let custom = ModeConfig {
            slug: "custom-slug".into(),
            name: "Custom".into(),
            role_definition: "R".into(),
            when_to_use: None,
            description: None,
            custom_instructions: None,
            groups: vec![],
            source: None,
        };
        let found = find_mode_by_slug("custom-slug", Some(&[custom]));
        assert!(found.is_some());
        assert_eq!(found.unwrap().slug, "custom-slug");
    }

    #[test]
    fn test_get_when_to_use_builtin() {
        let wtu = get_when_to_use("code", None);
        assert!(wtu.contains("write, modify, or refactor code"));
    }

    #[test]
    fn test_get_when_to_use_unknown() {
        let wtu = get_when_to_use("nonexistent", None);
        assert_eq!(wtu, "");
    }

    #[test]
    fn test_get_description_builtin() {
        let desc = get_description("architect", None);
        assert!(desc.contains("Plan and design"));
    }

    #[test]
    fn test_get_description_unknown() {
        let desc = get_description("nonexistent", None);
        assert_eq!(desc, "");
    }

    #[test]
    fn test_default_prompts_contains_all_modes() {
        let prompts = default_prompts();
        assert!(prompts.contains_key("architect"));
        assert!(prompts.contains_key("code"));
        assert!(prompts.contains_key("ask"));
        assert!(prompts.contains_key("debug"));
        assert!(prompts.contains_key("orchestrator"));
        assert_eq!(prompts.len(), 5);
    }

    #[test]
    fn test_default_prompts_has_role_definition() {
        let prompts = default_prompts();
        let architect = prompts.get("architect").unwrap();
        assert!(architect.role_definition.is_some());
        assert!(architect.role_definition.as_ref().unwrap().contains("technical leader"));
    }

    #[test]
    fn test_default_prompts_has_custom_instructions() {
        let prompts = default_prompts();
        let code = prompts.get("code").unwrap();
        // code mode has no custom_instructions
        assert!(code.custom_instructions.is_none());

        let architect = prompts.get("architect").unwrap();
        assert!(architect.custom_instructions.is_some());
    }
}

//! Modes section.
//!
//! Source: `src/core/prompts/sections/modes.ts`

use roo_types::mode::ModeConfig;

/// Returns the modes section listing all available modes.
///
/// Source: `src/core/prompts/sections/modes.ts` — `getModesSection`
pub fn get_modes_section(modes: &[ModeConfig]) -> String {
    let modes_content = modes
        .iter()
        .map(|mode| {
            let description = if let Some(wtu) = &mode.when_to_use {
                if !wtu.trim().is_empty() {
                    // Use whenToUse as the primary description, indenting subsequent lines
                    wtu.replace('\n', "\n    ")
                } else {
                    // Fallback to the first sentence of roleDefinition
                    mode.role_definition.split('.').next().unwrap_or("").to_string()
                }
            } else {
                // Fallback to the first sentence of roleDefinition
                mode.role_definition.split('.').next().unwrap_or("").to_string()
            };
            format!("  * \"{}\" mode ({}) - {}", mode.name, mode.slug, description)
        })
        .collect::<Vec<_>>()
        .join("\n");

    format!(
        r#"====

MODES

- These are the currently available modes:
{modes_content}"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_mode(name: &str, slug: &str, role_definition: &str, when_to_use: Option<&str>) -> ModeConfig {
        ModeConfig {
            name: name.to_string(),
            slug: slug.to_string(),
            role_definition: role_definition.to_string(),
            when_to_use: when_to_use.map(|s| s.to_string()),
            groups: vec![],
            description: None,
            custom_instructions: None,
            source: None,
        }
    }

    #[test]
    fn test_get_modes_section_basic() {
        let modes = vec![
            make_mode("Code", "code", "You are a coding assistant.", Some("Use for coding tasks")),
            make_mode("Architect", "architect", "You are an architect.", Some("Use for planning")),
        ];
        let result = get_modes_section(&modes);
        assert!(result.starts_with("====\n\nMODES"));
        assert!(result.contains("These are the currently available modes"));
        assert!(result.contains(r#""Code" mode (code)"#));
        assert!(result.contains(r#""Architect" mode (architect)"#));
    }

    #[test]
    fn test_get_modes_section_uses_when_to_use() {
        let modes = vec![
            make_mode("Code", "code", "You are a coding assistant.", Some("Use for writing code")),
        ];
        let result = get_modes_section(&modes);
        assert!(result.contains("Use for writing code"));
    }

    #[test]
    fn test_get_modes_section_falls_back_to_role_definition() {
        let modes = vec![
            make_mode("Code", "code", "You are a coding assistant. You write code.", None),
        ];
        let result = get_modes_section(&modes);
        // Should use first sentence of roleDefinition
        assert!(result.contains("You are a coding assistant"));
        // Should NOT contain the second sentence
        assert!(!result.contains("You write code"));
    }

    #[test]
    fn test_get_modes_section_empty_when_to_use() {
        let modes = vec![
            make_mode("Code", "code", "You are a coding assistant.", Some("")),
        ];
        let result = get_modes_section(&modes);
        // Empty whenToUse should fall back to roleDefinition
        assert!(result.contains("You are a coding assistant"));
    }

    #[test]
    fn test_get_modes_section_multiline_when_to_use() {
        let modes = vec![
            make_mode("Code", "code", "A coder.", Some("Line one\nLine two")),
        ];
        let result = get_modes_section(&modes);
        // Multiline whenToUse should be indented
        assert!(result.contains("Line one\n    Line two"));
    }
}

//! System prompt builder.
//!
//! Source: `src/core/prompts/system.ts`

use roo_types::mode::{CustomModePrompts, ModeConfig, PromptComponent, get_mode_by_slug};

use crate::sections::*;
use crate::types::SystemPromptParams;

fn non_empty_string(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.to_string())
}

/// Helper function to get prompt component, filtering out empty objects.
///
/// Source: `src/core/prompts/system.ts` — `getPromptComponent`
pub fn get_prompt_component(
    custom_mode_prompts: Option<&CustomModePrompts>,
    mode: &str,
) -> Option<PromptComponent> {
    custom_mode_prompts
        .and_then(|cmp| cmp.get(mode))
        .and_then(|opt| opt.as_ref())
        .cloned()
        .map(|mut pc| {
            pc.role_definition = pc
                .role_definition
                .and_then(|value| non_empty_string(&value));
            pc.when_to_use = pc.when_to_use.and_then(|value| non_empty_string(&value));
            pc.description = pc.description.and_then(|value| non_empty_string(&value));
            pc.custom_instructions = pc
                .custom_instructions
                .and_then(|value| non_empty_string(&value));
            pc
        })
        .filter(|pc| {
            pc.role_definition.is_some()
                || pc.when_to_use.is_some()
                || pc.description.is_some()
                || pc.custom_instructions.is_some()
        })
}

fn find_custom_mode(mode_slug: &str, custom_modes: Option<&[ModeConfig]>) -> Option<ModeConfig> {
    custom_modes.and_then(|modes| modes.iter().find(|mode| mode.slug == mode_slug).cloned())
}

fn find_default_mode(mode_slug: &str) -> Option<ModeConfig> {
    roo_types::mode::default_modes()
        .into_iter()
        .find(|mode| mode.slug == mode_slug)
}

fn first_default_mode() -> ModeConfig {
    roo_types::mode::default_modes()
        .into_iter()
        .next()
        .expect("at least one default mode must exist")
}

fn apply_prompt_overrides_to_modes(
    mut modes: Vec<ModeConfig>,
    custom_mode_prompts: Option<&CustomModePrompts>,
) -> Vec<ModeConfig> {
    let Some(prompts) = custom_mode_prompts else {
        return modes;
    };

    for mode in &mut modes {
        if let Some(Some(prompt)) = prompts.get(&mode.slug) {
            if let Some(role_definition) = &prompt.role_definition
                && !role_definition.trim().is_empty()
            {
                mode.role_definition = role_definition.clone();
            }
            if let Some(when_to_use) = &prompt.when_to_use
                && !when_to_use.trim().is_empty()
            {
                mode.when_to_use = Some(when_to_use.clone());
            }
            if let Some(custom_instructions) = &prompt.custom_instructions
                && !custom_instructions.trim().is_empty()
            {
                mode.custom_instructions = Some(custom_instructions.clone());
            }
        }
    }

    modes
}

/// Gets the role definition for a mode, with optional prompt component override.
///
/// Source: `src/shared/modes.ts` — `getModeSelection`
fn get_role_definition(
    mode_slug: &str,
    custom_modes: Option<&[ModeConfig]>,
    prompt_component: Option<&PromptComponent>,
) -> String {
    if let Some(custom_mode) = find_custom_mode(mode_slug, custom_modes) {
        return custom_mode.role_definition;
    }

    let base_mode = find_default_mode(mode_slug).unwrap_or_else(first_default_mode);

    prompt_component
        .and_then(|pc| pc.role_definition.clone())
        .and_then(|value| non_empty_string(&value))
        .unwrap_or(base_mode.role_definition)
}

/// Gets the base instructions for a mode.
///
/// Source: `src/shared/modes.ts` — `getModeSelection`
fn get_base_instructions(
    mode_slug: &str,
    custom_modes: Option<&[ModeConfig]>,
    prompt_component: Option<&PromptComponent>,
) -> Option<String> {
    if let Some(custom_mode) = find_custom_mode(mode_slug, custom_modes) {
        return custom_mode.custom_instructions.filter(|s| !s.is_empty());
    }

    let base_mode = find_default_mode(mode_slug).unwrap_or_else(first_default_mode);

    prompt_component
        .and_then(|pc| pc.custom_instructions.clone())
        .and_then(|value| non_empty_string(&value))
        .or(base_mode.custom_instructions)
        .filter(|s| !s.is_empty())
}

/// Generate the system prompt.
///
/// Source: `src/core/prompts/system.ts` — `generatePrompt`
pub fn generate_system_prompt(params: SystemPromptParams) -> String {
    let role_definition = params.role_definition;
    let base_instructions = params.base_instructions;

    let modes_section = get_modes_section(&params.modes);
    let skills_section = get_skills_section(&params.skills, &params.mode);

    let tools_catalog = ""; // Tools catalog is not included in the system prompt

    let base_prompt = format!(
        "{role_definition}

{}

{}{}

\t{}

{}

{}
{}
{}

{}

{}

{}",
        markdown_formatting_section(),
        get_shared_tool_use_section(),
        tools_catalog,
        get_tool_use_guidelines_section(),
        get_capabilities_section(&params.cwd, params.has_mcp),
        modes_section,
        if skills_section.is_empty() {
            String::new()
        } else {
            format!("\n{}", skills_section)
        },
        get_rules_section(&params.cwd, &params.shell, params.settings.as_ref(),),
        get_system_info_section(
            &params.os_info,
            &params.shell,
            &params.home_dir,
            &params.cwd,
        ),
        get_objective_section(),
        add_custom_instructions(
            base_instructions.as_deref().unwrap_or(""),
            params.global_custom_instructions.as_deref().unwrap_or(""),
            &params.cwd,
            &params.mode,
            params.language.as_deref(),
            params.roo_ignore_instructions.as_deref(),
            params.settings.as_ref(),
        ),
    );

    base_prompt
}

/// High-level API to build the system prompt.
///
/// Source: `src/core/prompts/system.ts` — `SYSTEM_PROMPT`
#[allow(clippy::too_many_arguments)]
pub fn build_system_prompt(
    cwd: &str,
    mode: &str,
    custom_modes: Option<&[ModeConfig]>,
    custom_mode_prompts: Option<&CustomModePrompts>,
    has_mcp: bool,
    global_custom_instructions: Option<&str>,
    language: Option<&str>,
    roo_ignore_instructions: Option<&str>,
    settings: Option<&crate::types::SystemPromptSettings>,
    skills: &[crate::types::SkillInfo],
    os_info: &str,
    shell: &str,
    home_dir: &str,
) -> String {
    // Get the prompt component for this mode
    let prompt_component = get_prompt_component(custom_mode_prompts, mode);

    // Get the full mode config
    let current_mode = get_mode_by_slug(mode, custom_modes)
        .or_else(|| get_mode_by_slug(mode, None))
        .unwrap_or_else(|| {
            roo_types::mode::default_modes()
                .into_iter()
                .next()
                .expect("at least one default mode must exist")
        });

    let role_definition = get_role_definition(mode, custom_modes, prompt_component.as_ref());
    let base_instructions = get_base_instructions(mode, custom_modes, prompt_component.as_ref());

    // Get all modes for the modes section
    let all_modes = apply_prompt_overrides_to_modes(
        roo_types::mode::get_all_modes(custom_modes),
        custom_mode_prompts,
    );

    let params = SystemPromptParams {
        cwd: cwd.to_string(),
        mode: current_mode.slug.clone(),
        role_definition,
        base_instructions,
        global_custom_instructions: global_custom_instructions.map(|s| s.to_string()),
        has_mcp,
        language: language.map(|s| s.to_string()),
        roo_ignore_instructions: roo_ignore_instructions.map(|s| s.to_string()),
        settings: settings.cloned(),
        modes: all_modes,
        skills: skills.to_vec(),
        os_info: os_info.to_string(),
        shell: shell.to_string(),
        home_dir: home_dir.to_string(),
        custom_rules_content: String::new(),
    };

    generate_system_prompt(params)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_prompt_component_empty() {
        let result = get_prompt_component(None, "code");
        assert!(result.is_none());
    }

    #[test]
    fn test_get_prompt_component_with_value() {
        let mut prompts = std::collections::HashMap::new();
        let component = PromptComponent {
            role_definition: Some("Custom role".to_string()),
            when_to_use: None,
            description: None,
            custom_instructions: None,
        };
        prompts.insert("code".to_string(), Some(component));
        let result = get_prompt_component(Some(&prompts), "code");
        assert!(result.is_some());
        assert_eq!(
            result.unwrap().role_definition,
            Some("Custom role".to_string())
        );
    }

    #[test]
    fn test_build_system_prompt_basic() {
        let result = build_system_prompt(
            "/home/user/project",
            "code",
            None,
            None,
            false,
            None,
            None,
            None,
            None,
            &[],
            "Linux",
            "/bin/bash",
            "/home/user",
        );
        assert!(result.contains("TOOL USE"));
        assert!(result.contains("CAPABILITIES"));
        assert!(result.contains("RULES"));
        assert!(result.contains("OBJECTIVE"));
        assert!(result.contains("SYSTEM INFORMATION"));
        assert!(result.contains("/home/user/project"));
    }

    #[test]
    fn test_builtin_mode_includes_default_custom_instructions() {
        let result = build_system_prompt(
            "/home/user/project",
            "ask",
            None,
            None,
            false,
            None,
            None,
            None,
            None,
            &[],
            "Linux",
            "/bin/bash",
            "/home/user",
        );

        assert!(result.contains("Mode-specific Instructions:"));
        assert!(result.contains("do not switch to implementing code unless explicitly requested"));
    }

    #[test]
    fn test_builtin_prompt_component_overrides_default_custom_instructions() {
        let mut prompts = std::collections::HashMap::new();
        prompts.insert(
            "debug".to_string(),
            Some(PromptComponent {
                custom_instructions: Some("Use the project incident playbook.".to_string()),
                ..PromptComponent::default()
            }),
        );

        let result = build_system_prompt(
            "/home/user/project",
            "debug",
            None,
            Some(&prompts),
            false,
            None,
            None,
            None,
            None,
            &[],
            "Linux",
            "/bin/bash",
            "/home/user",
        );

        assert!(result.contains("Use the project incident playbook."));
        assert!(!result.contains("Reflect on 5-7 different possible sources"));
    }

    #[test]
    fn test_empty_prompt_component_fields_fall_back_to_builtin_mode() {
        let mut prompts = std::collections::HashMap::new();
        prompts.insert(
            "debug".to_string(),
            Some(PromptComponent {
                role_definition: Some("".to_string()),
                custom_instructions: Some("   ".to_string()),
                ..PromptComponent::default()
            }),
        );

        let result = build_system_prompt(
            "/home/user/project",
            "debug",
            None,
            Some(&prompts),
            false,
            None,
            None,
            None,
            None,
            &[],
            "Linux",
            "/bin/bash",
            "/home/user",
        );

        assert!(result.starts_with("You are Roo, an expert software debugger"));
        assert!(result.contains("Reflect on 5-7 different possible sources"));
    }

    #[test]
    fn test_custom_mode_ignores_prompt_component_overrides() {
        let custom_modes = vec![ModeConfig {
            slug: "research".to_string(),
            name: "Research".to_string(),
            role_definition: "Custom mode role".to_string(),
            when_to_use: None,
            description: None,
            custom_instructions: Some("Custom mode instructions".to_string()),
            groups: vec![],
            source: None,
        }];

        let mut prompts = std::collections::HashMap::new();
        prompts.insert(
            "research".to_string(),
            Some(PromptComponent {
                role_definition: Some("Prompt role override".to_string()),
                custom_instructions: Some("Prompt instruction override".to_string()),
                ..PromptComponent::default()
            }),
        );

        let result = build_system_prompt(
            "/home/user/project",
            "research",
            Some(&custom_modes),
            Some(&prompts),
            false,
            None,
            None,
            None,
            None,
            &[],
            "Linux",
            "/bin/bash",
            "/home/user",
        );

        assert!(result.starts_with("Custom mode role"));
        assert!(result.contains("Custom mode instructions"));
        assert!(!result.starts_with("Prompt role override"));
        assert!(!result.contains("Prompt instruction override"));
    }

    #[test]
    fn test_modes_section_applies_prompt_overrides() {
        let mut prompts = std::collections::HashMap::new();
        prompts.insert(
            "ask".to_string(),
            Some(PromptComponent {
                when_to_use: Some("Use for product support answers".to_string()),
                ..PromptComponent::default()
            }),
        );

        let result = build_system_prompt(
            "/home/user/project",
            "code",
            None,
            Some(&prompts),
            false,
            None,
            None,
            None,
            None,
            &[],
            "Linux",
            "/bin/bash",
            "/home/user",
        );

        assert!(result.contains(r#""❓ Ask" mode (ask) - Use for product support answers"#));
    }

    #[test]
    fn test_unknown_mode_falls_back_to_first_default_mode() {
        let result = build_system_prompt(
            "/home/user/project",
            "unknown-mode",
            None,
            None,
            false,
            None,
            None,
            None,
            None,
            &[],
            "Linux",
            "/bin/bash",
            "/home/user",
        );

        assert!(result.starts_with("You are Roo, an experienced technical leader"));
    }
}

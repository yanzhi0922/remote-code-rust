//! System prompt builder.
//!
//! Source: `src/core/prompts/system.ts`

use roo_types::mode::{get_mode_by_slug, CustomModePrompts, ModeConfig, PromptComponent};

use crate::sections::*;
use crate::types::SystemPromptParams;

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
        .filter(|pc| {
            pc.role_definition.is_some()
                || pc.when_to_use.is_some()
                || pc.description.is_some()
                || pc.custom_instructions.is_some()
        })
}

/// Gets the role definition for a mode, with optional prompt component override.
///
/// Source: `src/shared/modes.ts` — `getRoleDefinition`
fn get_role_definition(
    mode_slug: &str,
    custom_modes: Option<&[ModeConfig]>,
    prompt_component: Option<&PromptComponent>,
) -> String {
    let mode = get_mode_by_slug(mode_slug, custom_modes);
    let base = mode
        .as_ref()
        .map(|m| m.role_definition.as_str())
        .unwrap_or("");
    prompt_component
        .and_then(|pc| pc.role_definition.as_deref())
        .unwrap_or(base)
        .to_string()
}

/// Gets the base instructions for a mode.
///
/// Source: `src/shared/modes.ts` — `getModeSelection`
fn get_base_instructions(
    _mode_slug: &str,
    prompt_component: Option<&PromptComponent>,
) -> Option<String> {
    prompt_component
        .and_then(|pc| pc.custom_instructions.as_deref())
        .map(|s| s.to_string())
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
        get_rules_section(
            &params.cwd,
            &params.shell,
            params.settings.as_ref(),
        ),
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
                .find(|m| m.slug == "code")
                .expect("code mode must exist")
        });

    let role_definition = get_role_definition(mode, custom_modes, prompt_component.as_ref());
    let base_instructions = get_base_instructions(mode, prompt_component.as_ref());

    // Get all modes for the modes section
    let all_modes = roo_types::mode::get_all_modes(custom_modes);

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
        assert_eq!(result.unwrap().role_definition, Some("Custom role".to_string()));
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
}

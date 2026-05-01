//! skill tool implementation.
//!
//! Integrates with `roo_skills::SkillsManager` to load skill content
//! from SKILL.md files. Falls back to a placeholder message when no
//! manager is available.

use crate::helpers::*;
use crate::types::*;
use roo_skills::SkillsManager;
use roo_skills::frontmatter::parse_skill_md;
use roo_types::tool::SkillParams;

use std::path::Path;

/// Validate skill parameters.
pub fn validate_skill_params(params: &SkillParams) -> Result<(), MiscToolError> {
    validate_skill_name(&params.skill)
}

/// Process a skill request.
///
/// When a [`SkillsManager`] reference is provided, attempts to look up the
/// skill by name, read its SKILL.md file, and return the parsed instructions.
/// If no manager is supplied, or the skill is not found, returns a fallback
/// placeholder message.
pub fn process_skill(
    params: &SkillParams,
    skills_manager: Option<&SkillsManager>,
) -> Result<SkillResult, MiscToolError> {
    validate_skill_params(params)?;

    // Try to load skill content from the manager
    if let Some(manager) = skills_manager {
        // Find skill by name across all sources
        let all_skills = manager.get_all_skills();
        if let Some(skill_meta) = all_skills.into_iter().find(|s| s.name == params.skill) {
            // Try to read the SKILL.md file synchronously
            let skill_md_path = Path::new(&skill_meta.path).join("SKILL.md");
            if let Ok(file_content) = std::fs::read_to_string(&skill_md_path) {
                if let Some((_frontmatter, instructions)) = parse_skill_md(&file_content) {
                    let content = if let Some(args) = &params.args {
                        format!("{}\n\nContext: {}", instructions, args)
                    } else {
                        instructions
                    };
                    return Ok(SkillResult {
                        skill_name: params.skill.clone(),
                        args: params.args.clone(),
                        is_valid: true,
                        content: Some(content),
                    });
                }
            }
        }
    }

    // Fallback: return a placeholder message
    let content = if let Some(args) = &params.args {
        format!(
            "Load and follow the instructions for skill '{}'.\n\nContext: {}",
            params.skill, args
        )
    } else {
        format!(
            "Load and follow the instructions for skill '{}'.",
            params.skill
        )
    };
    Ok(SkillResult {
        skill_name: params.skill.clone(),
        args: params.args.clone(),
        is_valid: true,
        content: Some(content),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_empty_skill_name() {
        let params = SkillParams {
            skill: "".to_string(),
            args: None,
        };
        assert!(validate_skill_params(&params).is_err());
    }

    #[test]
    fn test_validate_valid_skill() {
        let params = SkillParams {
            skill: "my-skill".to_string(),
            args: None,
        };
        assert!(validate_skill_params(&params).is_ok());
    }

    #[test]
    fn test_process_skill_without_manager() {
        let params = SkillParams {
            skill: "react-dev".to_string(),
            args: Some("create component".to_string()),
        };
        let result = process_skill(&params, None).unwrap();
        assert_eq!(result.skill_name, "react-dev");
        assert_eq!(result.args, Some("create component".to_string()));
        assert!(result.is_valid);
        assert!(result.content.is_some());
        assert!(result.content.unwrap().contains("react-dev"));
    }

    #[test]
    fn test_process_skill_no_args() {
        let params = SkillParams {
            skill: "flutter-dev".to_string(),
            args: None,
        };
        let result = process_skill(&params, None).unwrap();
        assert!(result.args.is_none());
        assert!(result.content.is_some());
    }

    #[test]
    fn test_process_skill_with_empty_manager() {
        // An empty SkillsManager has no skills, so it falls back
        let manager = SkillsManager::new();
        let params = SkillParams {
            skill: "nonexistent".to_string(),
            args: None,
        };
        let result = process_skill(&params, Some(&manager)).unwrap();
        assert!(result.content.is_some());
        assert!(result.content.unwrap().contains("nonexistent"));
    }
}

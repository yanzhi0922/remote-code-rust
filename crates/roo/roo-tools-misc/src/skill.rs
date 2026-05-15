//! skill tool implementation.
//!
//! Integrates with `roo_skills::SkillsManager` to load skill content
//! from SKILL.md files. Returns a proper error when a skill is not
//! found, matching the TS `SkillTool.ts` behavior.

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
///
/// - If the manager is available but the skill is not found, returns an error
///   listing available skills so the AI knows what is available.
/// - If no manager is supplied, returns an error indicating the skills system
///   is not configured.
pub fn process_skill(
    params: &SkillParams,
    skills_manager: Option<&SkillsManager>,
) -> Result<SkillResult, MiscToolError> {
    validate_skill_params(params)?;

    // No manager available — skills system is not configured
    let manager = match skills_manager {
        Some(m) => m,
        None => {
            return Err(MiscToolError::InvalidSkill(format!(
                "Skills system is not configured. Cannot look up skill '{}'.",
                params.skill
            )));
        }
    };

    // Find skill by name across all sources
    let all_skills = manager.get_all_skills();
    if let Some(skill_meta) = all_skills.iter().find(|s| s.name == params.skill) {
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

    // Skill not found — list available skills for the AI
    let available_names: Vec<&str> = all_skills.iter().map(|s| s.name.as_str()).collect();
    let available_list = if available_names.is_empty() {
        "(none)".to_string()
    } else {
        available_names.join(", ")
    };

    Err(MiscToolError::InvalidSkill(format!(
        "Skill '{}' not found. Available skills: [{}]",
        params.skill, available_list
    )))
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
        // No manager means the skills system is not configured
        let params = SkillParams {
            skill: "react-dev".to_string(),
            args: Some("create component".to_string()),
        };
        let result = process_skill(&params, None);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("not configured"));
        assert!(err_msg.contains("react-dev"));
    }

    #[test]
    fn test_process_skill_no_args_without_manager() {
        let params = SkillParams {
            skill: "flutter-dev".to_string(),
            args: None,
        };
        let result = process_skill(&params, None);
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("not configured"));
    }

    #[test]
    fn test_process_skill_with_empty_manager() {
        // An empty SkillsManager has no skills — should return error listing available
        let manager = SkillsManager::new();
        let params = SkillParams {
            skill: "nonexistent".to_string(),
            args: None,
        };
        let result = process_skill(&params, Some(&manager));
        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(err_msg.contains("not found"));
        assert!(err_msg.contains("nonexistent"));
        assert!(err_msg.contains("Available skills"));
    }
}

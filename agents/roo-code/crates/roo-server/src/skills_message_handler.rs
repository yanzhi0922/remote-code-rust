//! Skills message handling for webview communication.
//!
//! Derived from `src/core/webview/skillsMessageHandler.ts`.
//!
//! Handles all skills-related messages from the webview, including
//! listing, creating, deleting, moving, and opening skills.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Skill source type.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SkillSource {
    Project,
    Global,
}

/// Skill metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    pub name: String,
    pub source: SkillSource,
    pub description: Option<String>,
    pub path: String,
    #[serde(default)]
    pub mode_slugs: Vec<String>,
}

/// A webview message for skills operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsWebviewMessage {
    #[serde(rename = "type")]
    pub msg_type: String,
    pub skill_name: Option<String>,
    pub source: Option<SkillSource>,
    pub skill_description: Option<String>,
    pub skill_mode: Option<String>,
    pub skill_mode_slugs: Option<Vec<String>>,
    pub new_skill_mode: Option<String>,
    pub new_skill_mode_slugs: Option<Vec<String>>,
}

/// Result of a skills operation.
#[derive(Debug, Clone)]
pub struct SkillsOperationResult {
    pub success: bool,
    pub skills: Vec<SkillMetadata>,
    pub error: Option<String>,
}

/// Trait for skills management operations.
pub trait SkillsManager: Send + Sync {
    /// Get all skills metadata.
    fn get_skills_metadata(&self) -> Vec<SkillMetadata>;

    /// Create a new skill.
    fn create_skill(
        &self,
        name: &str,
        source: &SkillSource,
        description: &str,
        mode_slugs: Option<&[String]>,
    ) -> Result<String, String>;

    /// Delete a skill.
    fn delete_skill(
        &self,
        name: &str,
        source: &SkillSource,
        mode: Option<&str>,
    ) -> Result<(), String>;

    /// Move a skill to a different mode.
    fn move_skill(
        &self,
        name: &str,
        source: &SkillSource,
        current_mode: Option<&str>,
        new_mode: &str,
    ) -> Result<(), String>;

    /// Update the mode associations for a skill.
    fn update_skill_modes(
        &self,
        name: &str,
        source: &SkillSource,
        new_mode_slugs: Option<&[String]>,
    ) -> Result<(), String>;

    /// Find a skill by name and source.
    fn find_skill_by_name_and_source(
        &self,
        name: &str,
        source: &SkillSource,
    ) -> Option<SkillMetadata>;
}

// ---------------------------------------------------------------------------
// Handler functions
// ---------------------------------------------------------------------------

/// Handles the requestSkills message - returns all skills metadata.
///
/// Source: `src/core/webview/skillsMessageHandler.ts` — `handleRequestSkills`
pub fn handle_request_skills(
    skills_manager: Option<&dyn SkillsManager>,
) -> Vec<SkillMetadata> {
    match skills_manager {
        Some(manager) => manager.get_skills_metadata(),
        None => vec![],
    }
}

/// Handles the createSkill message - creates a new skill.
///
/// Source: `src/core/webview/skillsMessageHandler.ts` — `handleCreateSkill`
pub fn handle_create_skill(
    skills_manager: Option<&dyn SkillsManager>,
    message: &SkillsWebviewMessage,
) -> SkillsOperationResult {
    let skill_name = match &message.skill_name {
        Some(n) if !n.is_empty() => n,
        _ => {
            return SkillsOperationResult {
                success: false,
                skills: vec![],
                error: Some("Missing skill name".to_string()),
            }
        }
    };

    let source = match &message.source {
        Some(s) => s,
        _ => {
            return SkillsOperationResult {
                success: false,
                skills: vec![],
                error: Some("Missing skill source".to_string()),
            }
        }
    };

    let description = match &message.skill_description {
        Some(d) if !d.is_empty() => d,
        _ => {
            return SkillsOperationResult {
                success: false,
                skills: vec![],
                error: Some("Missing skill description".to_string()),
            }
        }
    };

    let manager = match skills_manager {
        Some(m) => m,
        None => {
            return SkillsOperationResult {
                success: false,
                skills: vec![],
                error: Some("Skills manager unavailable".to_string()),
            }
        }
    };

    // Support new modeSlugs array or fall back to legacy skillMode
    let mode_slugs: Option<Vec<String>> = message
        .skill_mode_slugs
        .clone()
        .or_else(|| message.skill_mode.clone().map(|m| vec![m]));

    match manager.create_skill(skill_name, source, description, mode_slugs.as_deref()) {
        Ok(_path) => {
            let skills = manager.get_skills_metadata();
            SkillsOperationResult {
                success: true,
                skills,
                error: None,
            }
        }
        Err(e) => SkillsOperationResult {
            success: false,
            skills: vec![],
            error: Some(e),
        },
    }
}

/// Handles the deleteSkill message - deletes a skill.
///
/// Source: `src/core/webview/skillsMessageHandler.ts` — `handleDeleteSkill`
pub fn handle_delete_skill(
    skills_manager: Option<&dyn SkillsManager>,
    message: &SkillsWebviewMessage,
) -> SkillsOperationResult {
    let skill_name = match &message.skill_name {
        Some(n) if !n.is_empty() => n,
        _ => {
            return SkillsOperationResult {
                success: false,
                skills: vec![],
                error: Some("Missing skill name".to_string()),
            }
        }
    };

    let source = match &message.source {
        Some(s) => s,
        _ => {
            return SkillsOperationResult {
                success: false,
                skills: vec![],
                error: Some("Missing skill source".to_string()),
            }
        }
    };

    let manager = match skills_manager {
        Some(m) => m,
        None => {
            return SkillsOperationResult {
                success: false,
                skills: vec![],
                error: Some("Skills manager unavailable".to_string()),
            }
        }
    };

    let skill_mode = message
        .skill_mode_slugs
        .as_ref()
        .and_then(|s| s.first().cloned())
        .or_else(|| message.skill_mode.clone());

    match manager.delete_skill(skill_name, source, skill_mode.as_deref()) {
        Ok(()) => {
            let skills = manager.get_skills_metadata();
            SkillsOperationResult {
                success: true,
                skills,
                error: None,
            }
        }
        Err(e) => SkillsOperationResult {
            success: false,
            skills: vec![],
            error: Some(e),
        },
    }
}

/// Handles the moveSkill message - moves a skill to a different mode.
///
/// Source: `src/core/webview/skillsMessageHandler.ts` — `handleMoveSkill`
pub fn handle_move_skill(
    skills_manager: Option<&dyn SkillsManager>,
    message: &SkillsWebviewMessage,
) -> SkillsOperationResult {
    let skill_name = match &message.skill_name {
        Some(n) if !n.is_empty() => n,
        _ => {
            return SkillsOperationResult {
                success: false,
                skills: vec![],
                error: Some("Missing skill name".to_string()),
            }
        }
    };

    let source = match &message.source {
        Some(s) => s,
        _ => {
            return SkillsOperationResult {
                success: false,
                skills: vec![],
                error: Some("Missing skill source".to_string()),
            }
        }
    };

    let new_mode = match &message.new_skill_mode {
        Some(m) if !m.is_empty() => m,
        _ => {
            return SkillsOperationResult {
                success: false,
                skills: vec![],
                error: Some("Missing new skill mode".to_string()),
            }
        }
    };

    let manager = match skills_manager {
        Some(m) => m,
        None => {
            return SkillsOperationResult {
                success: false,
                skills: vec![],
                error: Some("Skills manager unavailable".to_string()),
            }
        }
    };

    match manager.move_skill(
        skill_name,
        source,
        message.skill_mode.as_deref(),
        new_mode,
    ) {
        Ok(()) => {
            let skills = manager.get_skills_metadata();
            SkillsOperationResult {
                success: true,
                skills,
                error: None,
            }
        }
        Err(e) => SkillsOperationResult {
            success: false,
            skills: vec![],
            error: Some(e),
        },
    }
}

/// Handles the updateSkillModes message.
///
/// Source: `src/core/webview/skillsMessageHandler.ts` — `handleUpdateSkillModes`
pub fn handle_update_skill_modes(
    skills_manager: Option<&dyn SkillsManager>,
    message: &SkillsWebviewMessage,
) -> SkillsOperationResult {
    let skill_name = match &message.skill_name {
        Some(n) if !n.is_empty() => n,
        _ => {
            return SkillsOperationResult {
                success: false,
                skills: vec![],
                error: Some("Missing skill name".to_string()),
            }
        }
    };

    let source = match &message.source {
        Some(s) => s,
        _ => {
            return SkillsOperationResult {
                success: false,
                skills: vec![],
                error: Some("Missing skill source".to_string()),
            }
        }
    };

    let manager = match skills_manager {
        Some(m) => m,
        None => {
            return SkillsOperationResult {
                success: false,
                skills: vec![],
                error: Some("Skills manager unavailable".to_string()),
            }
        }
    };

    match manager.update_skill_modes(
        skill_name,
        source,
        message.new_skill_mode_slugs.as_deref(),
    ) {
        Ok(()) => {
            let skills = manager.get_skills_metadata();
            SkillsOperationResult {
                success: true,
                skills,
                error: None,
            }
        }
        Err(e) => SkillsOperationResult {
            success: false,
            skills: vec![],
            error: Some(e),
        },
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_request_skills_no_manager() {
        let result = handle_request_skills(None);
        assert!(result.is_empty());
    }

    #[test]
    fn test_handle_create_skill_missing_name() {
        let message = SkillsWebviewMessage {
            msg_type: "createSkill".to_string(),
            skill_name: None,
            source: Some(SkillSource::Project),
            skill_description: Some("desc".to_string()),
            skill_mode: None,
            skill_mode_slugs: None,
            new_skill_mode: None,
            new_skill_mode_slugs: None,
        };
        let result = handle_create_skill(None, &message);
        assert!(!result.success);
        assert!(result.error.unwrap().contains("Missing skill name"));
    }

    #[test]
    fn test_handle_create_skill_missing_source() {
        let message = SkillsWebviewMessage {
            msg_type: "createSkill".to_string(),
            skill_name: Some("test-skill".to_string()),
            source: None,
            skill_description: Some("desc".to_string()),
            skill_mode: None,
            skill_mode_slugs: None,
            new_skill_mode: None,
            new_skill_mode_slugs: None,
        };
        let result = handle_create_skill(None, &message);
        assert!(!result.success);
        assert!(result.error.unwrap().contains("Missing skill source"));
    }

    #[test]
    fn test_handle_create_skill_missing_description() {
        let message = SkillsWebviewMessage {
            msg_type: "createSkill".to_string(),
            skill_name: Some("test-skill".to_string()),
            source: Some(SkillSource::Project),
            skill_description: None,
            skill_mode: None,
            skill_mode_slugs: None,
            new_skill_mode: None,
            new_skill_mode_slugs: None,
        };
        let result = handle_create_skill(None, &message);
        assert!(!result.success);
        assert!(result.error.unwrap().contains("Missing skill description"));
    }

    #[test]
    fn test_handle_delete_skill_missing_name() {
        let message = SkillsWebviewMessage {
            msg_type: "deleteSkill".to_string(),
            skill_name: None,
            source: Some(SkillSource::Project),
            skill_description: None,
            skill_mode: None,
            skill_mode_slugs: None,
            new_skill_mode: None,
            new_skill_mode_slugs: None,
        };
        let result = handle_delete_skill(None, &message);
        assert!(!result.success);
    }
}

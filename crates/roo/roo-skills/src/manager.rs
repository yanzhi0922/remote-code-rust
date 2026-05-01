//! Skills manager — discovery, querying, and management of skills.
//!
//! Derived from `src/services/skills/SkillsManager.ts`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tokio::fs;
use tracing::{debug, warn};

use crate::error::SkillsError;
use crate::frontmatter::{
    generate_new_skill_md, get_skill_dir_path, parse_skill_md, FrontMatter,
};
use crate::types::{SkillContent, SkillMetadata, SkillSource};
use crate::validate::{get_skill_name_error_message, validate_skill_name};

// ---------------------------------------------------------------------------
// SkillsManager
// ---------------------------------------------------------------------------

/// Manages the discovery, loading, and querying of skills.
///
/// Source: `src/services/skills/SkillsManager.ts`
pub struct SkillsManager {
    /// Discovered skills indexed by their unique key.
    skills: HashMap<String, SkillMetadata>,
}

/// Source priority for override resolution.
/// Project skills (2) override global skills (1).
fn source_priority(source: SkillSource) -> u8 {
    match source {
        SkillSource::Project => 2,
        SkillSource::Global => 1,
    }
}

impl SkillsManager {
    /// Create a new, empty `SkillsManager`.
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    // -----------------------------------------------------------------------
    // Key generation
    // -----------------------------------------------------------------------

    /// Generate a unique key for a skill.
    ///
    /// Format: `"{name}:{source}:{mode}"` where mode defaults to "".
    ///
    /// Source: `SkillsManager.getSkillKey()`
    pub fn get_skill_key(name: &str, source: SkillSource, mode: Option<&str>) -> String {
        format!("{}:{}:{}", source, mode.unwrap_or("generic"), name)
    }

    // -----------------------------------------------------------------------
    // Discovery
    // -----------------------------------------------------------------------

    /// Clear all cached skills and re-discover from the given directories.
    ///
    /// `skills_dirs` is a list of `(dir_path, source, mode)` tuples.
    ///
    /// Source: `SkillsManager.discoverSkills()`
    pub async fn discover_skills(
        &mut self,
        skills_dirs: &[(PathBuf, SkillSource, Option<String>)],
    ) -> Result<(), SkillsError> {
        self.skills.clear();

        for (dir_path, source, mode) in skills_dirs {
            if let Err(e) = self
                .scan_skills_directory(dir_path, *source, mode.as_deref())
                .await
            {
                debug!(
                    "Failed to scan skills directory {:?}: {}",
                    dir_path, e
                );
            }
        }

        debug!("Discovered {} skills", self.skills.len());
        Ok(())
    }

    /// Scan a single directory for skill subdirectories.
    ///
    /// Source: `SkillsManager.scanSkillsDirectory()`
    async fn scan_skills_directory(
        &mut self,
        dir_path: &Path,
        source: SkillSource,
        mode: Option<&str>,
    ) -> Result<(), SkillsError> {
        let dir_str = dir_path.to_string_lossy().to_string();

        if !dir_path.exists() {
            debug!("Skills directory does not exist: {}", dir_str);
            return Ok(());
        }

        let mut entries = fs::read_dir(dir_path).await.map_err(|e| {
            SkillsError::IoError(format!("Failed to read directory '{}': {}", dir_str, e))
        })?;

        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            SkillsError::IoError(format!("Failed to read entry in '{}': {}", dir_str, e))
        })? {
            let path = entry.path();

            // Handle symlinks — resolve them
            let resolved_path = if path.is_symlink() {
                match fs::read_link(&path).await {
                    Ok(target) => {
                        let resolved = if target.is_absolute() {
                            target
                        } else {
                            path.parent()
                                .unwrap_or(Path::new("."))
                                .join(target)
                        };
                        if !resolved.exists() {
                            debug!(
                                "Symlink target does not exist: {:?} -> {:?}",
                                path, resolved
                            );
                            continue;
                        }
                        resolved
                    }
                    Err(e) => {
                        warn!("Failed to read symlink {:?}: {}", path, e);
                        continue;
                    }
                }
            } else {
                path.clone()
            };

            // Only process directories
            if !resolved_path.is_dir() {
                continue;
            }

            let skill_name = match entry.file_name().to_str() {
                Some(name) => name.to_string(),
                None => continue,
            };

            if let Err(e) = self
                .load_skill_metadata(&resolved_path, source, mode, Some(&skill_name))
                .await
            {
                debug!(
                    "Failed to load skill metadata from {:?}: {}",
                    resolved_path, e
                );
            }
        }

        Ok(())
    }

    /// Load skill metadata from a SKILL.md file in the given directory.
    ///
    /// Source: `SkillsManager.loadSkillMetadata()`
    async fn load_skill_metadata(
        &mut self,
        skill_dir: &Path,
        source: SkillSource,
        mode: Option<&str>,
        fallback_name: Option<&str>,
    ) -> Result<(), SkillsError> {
        let skill_md_path = skill_dir.join("SKILL.md");
        let path_str = skill_md_path.to_string_lossy().to_string();

        if !skill_md_path.exists() {
            return Err(SkillsError::ParseError {
                path: path_str,
                reason: "SKILL.md not found".to_string(),
            });
        }

        let content = fs::read_to_string(&skill_md_path).await.map_err(|e| {
            SkillsError::IoError(format!("Failed to read '{}': {}", path_str, e))
        })?;

        let (frontmatter, _body) = parse_skill_md(&content).ok_or_else(|| {
            SkillsError::ParseError {
                path: path_str.clone(),
                reason: "No valid frontmatter found".to_string(),
            }
        })?;

        // Determine the skill name: frontmatter > fallback (directory name)
        let name = frontmatter
            .name
            .clone()
            .or_else(|| fallback_name.map(|n| n.to_string()))
            .ok_or_else(|| SkillsError::ParseError {
                path: path_str.clone(),
                reason: "Skill name not found in frontmatter or directory name".to_string(),
            })?;

        // Validate that frontmatter name matches the directory name
        // Corresponds to TS: "name field must match the parent directory name"
        if let Some(ref fm_name) = frontmatter.name {
            let effective_name = fallback_name.unwrap_or("");
            if fm_name != effective_name {
                warn!(
                    "Skill name \"{}\" doesn't match directory \"{}\"",
                    fm_name, effective_name
                );
                return Err(SkillsError::ParseError {
                    path: path_str.clone(),
                    reason: format!(
                        "Skill name \"{}\" doesn't match directory \"{}\"",
                        fm_name, effective_name
                    ),
                });
            }
        }

        // Validate the name
        let validation = validate_skill_name(&name);
        if !validation.valid {
            let error = validation.error.unwrap();
            return Err(SkillsError::InvalidName(get_skill_name_error_message(
                &name, error,
            )));
        }

        // Validate description is present (required field in TS)
        let description = frontmatter.description.ok_or_else(|| {
            SkillsError::ParseError {
                path: path_str.clone(),
                reason: "Skill is missing required 'description' field".to_string(),
            }
        })?;

        // Validate description length (1-1024 chars, matching TS)
        let trimmed_desc = description.trim();
        if trimmed_desc.is_empty() || trimmed_desc.len() > 1024 {
            warn!(
                "Skill \"{}\" has an invalid description length: must be 1-1024 characters (got {})",
                name,
                trimmed_desc.len()
            );
            return Err(SkillsError::InvalidDescription {
                length: trimmed_desc.len(),
            });
        }

        // Merge deprecated `mode` field into `mode_slugs`
        let mode_slugs = frontmatter.mode_slugs.or_else(|| {
            frontmatter.mode.map(|m| vec![m])
        });

        let metadata = SkillMetadata {
            name: name.clone(),
            description,
            path: skill_dir.to_string_lossy().to_string(),
            source,
            mode: mode.map(|m| m.to_string()),
            mode_slugs,
        };

        let key = Self::get_skill_key(&name, source, mode);

        // Check override rules
        if let Some(existing) = self.skills.get(&key) {
            if !Self::should_override_skill(existing, &metadata) {
                debug!(
                    "Skipping skill '{}' from {} — existing skill has higher priority",
                    name, source
                );
                return Ok(());
            }
        }

        debug!("Loaded skill '{}' from {}", name, source);
        self.skills.insert(key, metadata);
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Querying
    // -----------------------------------------------------------------------

    /// Get all discovered skills.
    ///
    /// Source: `SkillsManager.getAllSkills()`
    pub fn get_all_skills(&self) -> Vec<&SkillMetadata> {
        self.skills.values().collect()
    }

    /// Get skills available for a specific mode.
    ///
    /// Applies override rules: project > global, mode-specific > generic.
    ///
    /// Source: `SkillsManager.getSkillsForMode()`
    pub fn get_skills_for_mode(&self, current_mode: &str) -> Vec<&SkillMetadata> {
        let mut result: HashMap<String, &SkillMetadata> = HashMap::new();

        for skill in self.skills.values() {
            if !Self::is_skill_available_in_mode(skill, current_mode) {
                continue;
            }

            let key = skill.name.clone();

            if let Some(existing) = result.get(&key) {
                if Self::should_override_skill(existing, skill) {
                    result.insert(key, skill);
                }
            } else {
                result.insert(key, skill);
            }
        }

        result.into_values().collect()
    }

    /// Check if a skill is available in the given mode.
    ///
    /// A skill is available if:
    /// - `mode_slugs` is `None` or empty (available in all modes), OR
    /// - `mode_slugs` contains the current mode slug.
    ///
    /// Source: `SkillsManager.isSkillAvailableInMode()`
    pub fn is_skill_available_in_mode(skill: &SkillMetadata, current_mode: &str) -> bool {
        match &skill.mode_slugs {
            Some(slugs) if !slugs.is_empty() => slugs.iter().any(|s| s == current_mode),
            _ => true,
        }
    }

    /// Determine whether the new skill should override the existing one.
    ///
    /// Override rules (higher priority wins):
    /// 1. Source priority: Project (2) > Global (1)
    /// 2. Mode-specific skills > generic skills (skills with no mode set)
    ///
    /// Source: `SkillsManager.shouldOverrideSkill()`
    pub fn should_override_skill(existing: &SkillMetadata, new_skill: &SkillMetadata) -> bool {
        let existing_priority = source_priority(existing.source);
        let new_priority = source_priority(new_skill.source);

        if new_priority != existing_priority {
            return new_priority > existing_priority;
        }

        // Same source: mode-specific > generic
        // Corresponds to TS: checking modeSlugs array presence, not deprecated mode field
        let existing_has_modes = existing
            .mode_slugs
            .as_ref()
            .map_or(false, |s| !s.is_empty());
        let new_has_modes = new_skill
            .mode_slugs
            .as_ref()
            .map_or(false, |s| !s.is_empty());

        if new_has_modes != existing_has_modes {
            return new_has_modes;
        }

        // Same priority — don't override
        false
    }

    /// Find a skill by name and source.
    ///
    /// Source: `SkillsManager.findSkillByNameAndSource()`
    pub fn find_skill_by_name_and_source(
        &self,
        name: &str,
        source: SkillSource,
    ) -> Option<&SkillMetadata> {
        self.skills
            .values()
            .find(|s| s.name == name && s.source == source)
    }

    /// Get a skill by its key components.
    ///
    /// Source: `SkillsManager.getSkill()`
    pub fn get_skill(
        &self,
        name: &str,
        source: SkillSource,
        mode: Option<&str>,
    ) -> Option<&SkillMetadata> {
        let key = Self::get_skill_key(name, source, mode);
        self.skills.get(&key)
    }

    /// Read the full content of a skill's SKILL.md file.
    ///
    /// Source: `SkillsManager.getSkillContent()`
    pub async fn get_skill_content(
        &self,
        name: &str,
        current_mode: Option<&str>,
    ) -> Result<SkillContent, SkillsError> {
        // Find the best matching skill for the given mode
        let skill = if let Some(mode) = current_mode {
            let skills = self.get_skills_for_mode(mode);
            skills.into_iter().find(|s| s.name == name)
        } else {
            self.skills.values().find(|s| s.name == name)
        };

        let skill = skill.ok_or_else(|| SkillsError::NotFound {
            name: name.to_string(),
            skill_source: "any".to_string(),
            mode_info: current_mode.unwrap_or("any").to_string(),
        })?;

        let skill_md_path = Path::new(&skill.path).join("SKILL.md");
        let content = fs::read_to_string(&skill_md_path).await?;

        let (_frontmatter, instructions) =
            parse_skill_md(&content).unwrap_or((FrontMatter::default(), content.clone()));

        Ok(SkillContent {
            metadata: skill.clone(),
            instructions,
        })
    }

    // -----------------------------------------------------------------------
    // Management
    // -----------------------------------------------------------------------

    /// Create a new skill.
    ///
    /// Validates the name, creates the directory, and writes SKILL.md.
    ///
    /// Source: `SkillsManager.createSkill()`
    pub async fn create_skill(
        &mut self,
        name: &str,
        source: SkillSource,
        description: &str,
        mode_slugs: Option<&[String]>,
        base_dir: &str,
        mode: Option<&str>,
    ) -> Result<SkillMetadata, SkillsError> {
        // Validate name
        let validation = validate_skill_name(name);
        if !validation.valid {
            let error = validation.error.unwrap();
            return Err(SkillsError::InvalidName(get_skill_name_error_message(
                name, error,
            )));
        }

        // Validate description length (1-1024 chars, matching TS)
        let trimmed = description.trim();
        if trimmed.is_empty() || trimmed.len() > 1024 {
            return Err(SkillsError::InvalidDescription {
                length: description.len(),
            });
        }

        // Build directory path
        let skill_dir = get_skill_dir_path(base_dir, source, mode, name);
        let skill_dir_str = skill_dir.to_string_lossy().to_string();

        // Check if already exists
        if skill_dir.exists() {
            return Err(SkillsError::AlreadyExists {
                name: name.to_string(),
                path: skill_dir_str,
            });
        }

        // Create directory
        fs::create_dir_all(&skill_dir).await?;

        // Generate and write SKILL.md
        let content = generate_new_skill_md(name, description, mode_slugs);
        let skill_md_path = skill_dir.join("SKILL.md");
        fs::write(&skill_md_path, content).await?;

        let metadata = SkillMetadata {
            name: name.to_string(),
            description: description.to_string(),
            path: skill_dir_str,
            source,
            mode: mode.map(|m| m.to_string()),
            mode_slugs: mode_slugs.map(|s| s.to_vec()),
        };

        // Add to internal map
        let key = Self::get_skill_key(name, source, mode);
        self.skills.insert(key, metadata.clone());

        Ok(metadata)
    }

    /// Delete a skill by name, source, and optional mode.
    ///
    /// Source: `SkillsManager.deleteSkill()`
    pub async fn delete_skill(
        &mut self,
        name: &str,
        source: SkillSource,
        mode: Option<&str>,
    ) -> Result<(), SkillsError> {
        let key = Self::get_skill_key(name, source, mode);

        let skill = self.skills.get(&key).ok_or_else(|| SkillsError::NotFound {
            name: name.to_string(),
            skill_source: source.to_string(),
            mode_info: mode.unwrap_or("generic").to_string(),
        })?;

        let skill_path = Path::new(&skill.path);
        if skill_path.exists() {
            fs::remove_dir_all(skill_path).await?;
        }

        self.skills.remove(&key);
        Ok(())
    }

    /// Update the modeSlugs of an existing skill.
    ///
    /// Source: `SkillsManager.updateSkillModes()`
    pub async fn update_skill_modes(
        &mut self,
        name: &str,
        source: SkillSource,
        mode: Option<&str>,
        new_mode_slugs: Option<&[String]>,
    ) -> Result<SkillMetadata, SkillsError> {
        let key = Self::get_skill_key(name, source, mode);

        let skill = self
            .skills
            .get(&key)
            .cloned()
            .ok_or_else(|| SkillsError::NotFound {
                name: name.to_string(),
                skill_source: source.to_string(),
                mode_info: mode.unwrap_or("generic").to_string(),
            })?;

        let skill_path = Path::new(&skill.path);
        let skill_md_path = skill_path.join("SKILL.md");

        // Read existing content
        let content = fs::read_to_string(&skill_md_path).await?;

        // Parse to get instructions
        let (_frontmatter, instructions) = parse_skill_md(&content)
            .unwrap_or((FrontMatter::default(), String::new()));

        // Regenerate with new mode_slugs
        let new_content = crate::frontmatter::generate_skill_md(
            name,
            &skill.description,
            new_mode_slugs,
            &instructions,
        );
        fs::write(&skill_md_path, new_content).await?;

        // Update metadata
        let metadata = SkillMetadata {
            mode_slugs: new_mode_slugs.map(|s| s.to_vec()),
            ..skill
        };

        self.skills.insert(key, metadata.clone());

        Ok(metadata)
    }

    /// Move a skill to a different mode directory.
    ///
    /// Source: `SkillsManager.moveSkill()`
    pub async fn move_skill(
        &mut self,
        name: &str,
        source: SkillSource,
        current_mode: Option<&str>,
        new_mode: &str,
        base_dir: &str,
    ) -> Result<SkillMetadata, SkillsError> {
        let old_key = Self::get_skill_key(name, source, current_mode);

        // Clone early to avoid borrow issues
        let skill = self
            .skills
            .get(&old_key)
            .cloned()
            .ok_or_else(|| SkillsError::NotFound {
                name: name.to_string(),
                skill_source: source.to_string(),
                mode_info: current_mode.unwrap_or("generic").to_string(),
            })?;

        let old_path = Path::new(&skill.path);
        let new_dir = get_skill_dir_path(base_dir, source, Some(new_mode), name);

        if new_dir.exists() {
            return Err(SkillsError::AlreadyExists {
                name: name.to_string(),
                path: new_dir.to_string_lossy().to_string(),
            });
        }

        // Create new directory and move contents
        fs::create_dir_all(&new_dir).await?;

        // Copy all files from old to new directory
        if old_path.exists() {
            let mut entries = fs::read_dir(old_path).await?;
            while let Some(entry) = entries.next_entry().await? {
                let file_name = entry.file_name();
                let new_file_path = new_dir.join(&file_name);
                fs::rename(entry.path(), &new_file_path).await?;
            }
            // Remove old directory
            let _ = fs::remove_dir(old_path).await;
        }

        // Remove old key
        self.skills.remove(&old_key);

        // Create new metadata
        let new_metadata = SkillMetadata {
            path: new_dir.to_string_lossy().to_string(),
            mode: Some(new_mode.to_string()),
            ..skill
        };

        let new_key = Self::get_skill_key(name, source, Some(new_mode));
        self.skills.insert(new_key, new_metadata.clone());

        Ok(new_metadata)
    }
}

impl Default for SkillsManager {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SkillSource;

    // -----------------------------------------------------------------------
    // get_skill_key tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_get_skill_key_basic() {
        let key = SkillsManager::get_skill_key("my-skill", SkillSource::Global, None);
        assert_eq!(key, "global:generic:my-skill");
    }

    #[test]
    fn test_get_skill_key_with_mode() {
        let key = SkillsManager::get_skill_key("my-skill", SkillSource::Project, Some("code"));
        assert_eq!(key, "project:code:my-skill");
    }

    // -----------------------------------------------------------------------
    // is_skill_available_in_mode tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_available_no_mode_slugs() {
        let skill = SkillMetadata {
            name: "test".to_string(),
            description: "test".to_string(),
            path: "/test".to_string(),
            source: SkillSource::Global,
            mode: None,
            mode_slugs: None,
        };
        assert!(SkillsManager::is_skill_available_in_mode(&skill, "code"));
        assert!(SkillsManager::is_skill_available_in_mode(&skill, "architect"));
    }

    #[test]
    fn test_available_empty_mode_slugs() {
        let skill = SkillMetadata {
            name: "test".to_string(),
            description: "test".to_string(),
            path: "/test".to_string(),
            source: SkillSource::Global,
            mode: None,
            mode_slugs: Some(vec![]),
        };
        assert!(SkillsManager::is_skill_available_in_mode(&skill, "code"));
    }

    #[test]
    fn test_available_matching_mode() {
        let skill = SkillMetadata {
            name: "test".to_string(),
            description: "test".to_string(),
            path: "/test".to_string(),
            source: SkillSource::Global,
            mode: None,
            mode_slugs: Some(vec!["code".to_string(), "architect".to_string()]),
        };
        assert!(SkillsManager::is_skill_available_in_mode(&skill, "code"));
        assert!(SkillsManager::is_skill_available_in_mode(&skill, "architect"));
        assert!(!SkillsManager::is_skill_available_in_mode(&skill, "debug"));
    }

    // -----------------------------------------------------------------------
    // should_override_skill tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_override_project_over_global() {
        let global = SkillMetadata {
            name: "test".to_string(),
            description: "test".to_string(),
            path: "/global".to_string(),
            source: SkillSource::Global,
            mode: None,
            mode_slugs: None,
        };
        let project = SkillMetadata {
            name: "test".to_string(),
            description: "test".to_string(),
            path: "/project".to_string(),
            source: SkillSource::Project,
            mode: None,
            mode_slugs: None,
        };
        assert!(SkillsManager::should_override_skill(&global, &project));
        assert!(!SkillsManager::should_override_skill(&project, &global));
    }

    #[test]
    fn test_override_mode_specific_over_generic() {
        let generic = SkillMetadata {
            name: "test".to_string(),
            description: "test".to_string(),
            path: "/generic".to_string(),
            source: SkillSource::Global,
            mode: None,
            mode_slugs: None,
        };
        let specific = SkillMetadata {
            name: "test".to_string(),
            description: "test".to_string(),
            path: "/specific".to_string(),
            source: SkillSource::Global,
            mode: None,
            mode_slugs: Some(vec!["code".to_string()]),
        };
        assert!(SkillsManager::should_override_skill(&generic, &specific));
        assert!(!SkillsManager::should_override_skill(&specific, &generic));
    }

    #[test]
    fn test_no_override_same_priority() {
        let skill_a = SkillMetadata {
            name: "test".to_string(),
            description: "test".to_string(),
            path: "/a".to_string(),
            source: SkillSource::Global,
            mode: None,
            mode_slugs: None,
        };
        let skill_b = SkillMetadata {
            name: "test".to_string(),
            description: "test".to_string(),
            path: "/b".to_string(),
            source: SkillSource::Global,
            mode: None,
            mode_slugs: None,
        };
        assert!(!SkillsManager::should_override_skill(&skill_a, &skill_b));
    }

    // -----------------------------------------------------------------------
    // get_skills_for_mode tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_get_skills_for_mode_basic() {
        let mut mgr = SkillsManager::new();

        let skill = SkillMetadata {
            name: "generic-skill".to_string(),
            description: "A generic skill".to_string(),
            path: "/skills/generic-skill".to_string(),
            source: SkillSource::Global,
            mode: None,
            mode_slugs: None,
        };
        mgr.skills
            .insert("global:generic:generic-skill".to_string(), skill);

        let skills = mgr.get_skills_for_mode("code");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "generic-skill");
    }

    #[test]
    fn test_get_skills_for_mode_filters_by_mode() {
        let mut mgr = SkillsManager::new();

        let code_only = SkillMetadata {
            name: "code-skill".to_string(),
            description: "Code only".to_string(),
            path: "/skills/code-skill".to_string(),
            source: SkillSource::Global,
            mode: None,
            mode_slugs: Some(vec!["code".to_string()]),
        };
        mgr.skills
            .insert("global:generic:code-skill".to_string(), code_only);

        let skills = mgr.get_skills_for_mode("code");
        assert_eq!(skills.len(), 1);

        let skills = mgr.get_skills_for_mode("architect");
        assert_eq!(skills.len(), 0);
    }

    #[test]
    fn test_get_skills_for_mode_override() {
        let mut mgr = SkillsManager::new();

        let global_skill = SkillMetadata {
            name: "my-skill".to_string(),
            description: "Global version".to_string(),
            path: "/global/my-skill".to_string(),
            source: SkillSource::Global,
            mode: None,
            mode_slugs: None,
        };
        let project_skill = SkillMetadata {
            name: "my-skill".to_string(),
            description: "Project version".to_string(),
            path: "/project/my-skill".to_string(),
            source: SkillSource::Project,
            mode: None,
            mode_slugs: None,
        };

        mgr.skills
            .insert("global:generic:my-skill".to_string(), global_skill);
        mgr.skills
            .insert("project:generic:my-skill".to_string(), project_skill);

        let skills = mgr.get_skills_for_mode("code");
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].source, SkillSource::Project);
        assert_eq!(skills[0].description, "Project version");
    }

    // -----------------------------------------------------------------------
    // get_skill / find_skill tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_get_skill() {
        let mut mgr = SkillsManager::new();

        let skill = SkillMetadata {
            name: "test".to_string(),
            description: "Test skill".to_string(),
            path: "/test".to_string(),
            source: SkillSource::Global,
            mode: None,
            mode_slugs: None,
        };
        mgr.skills.insert("global:generic:test".to_string(), skill);

        let found = mgr.get_skill("test", SkillSource::Global, None);
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "test");

        let not_found = mgr.get_skill("nonexistent", SkillSource::Global, None);
        assert!(not_found.is_none());
    }

    #[test]
    fn test_find_skill_by_name_and_source() {
        let mut mgr = SkillsManager::new();

        let skill = SkillMetadata {
            name: "my-skill".to_string(),
            description: "Test".to_string(),
            path: "/test".to_string(),
            source: SkillSource::Project,
            mode: Some("code".to_string()),
            mode_slugs: None,
        };
        mgr.skills
            .insert("project:code:my-skill".to_string(), skill);

        let found = mgr.find_skill_by_name_and_source("my-skill", SkillSource::Project);
        assert!(found.is_some());

        let not_found = mgr.find_skill_by_name_and_source("my-skill", SkillSource::Global);
        assert!(not_found.is_none());
    }

    // -----------------------------------------------------------------------
    // source_priority tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_source_priority() {
        assert_eq!(source_priority(SkillSource::Global), 1);
        assert_eq!(source_priority(SkillSource::Project), 2);
    }

    // -----------------------------------------------------------------------
    // Default trait
    // -----------------------------------------------------------------------

    #[test]
    fn test_default() {
        let mgr = SkillsManager::default();
        assert_eq!(mgr.get_all_skills().len(), 0);
    }
}

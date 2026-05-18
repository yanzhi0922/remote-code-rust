//! Custom modes CRUD manager.
//!
//! Manages custom mode configurations stored in YAML files (`.roomodes` for
//! project-level, global settings file for global modes). Supports:
//! - Loading modes from project and global files
//! - Merging project and global modes (project takes precedence)
//! - CRUD operations (add, update, delete, list)
//! - Import/export with rules files
//! - File watching for live updates
//! - Write queue for serialized writes
//!
//! Source: `src/core/config/CustomModesManager.ts`

use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Serialize};

use roo_types::mode::{ModeConfig, ModeSource, PromptComponent};
use roo_types::tool::{GroupEntry, ToolGroup};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// The filename for project-level custom modes.
///
/// Source: `src/core/config/CustomModesManager.ts` — `ROOMODES_FILENAME`
pub const ROOMODES_FILENAME: &str = ".roomodes";

/// Cache TTL for custom modes (10 seconds).
///
/// Source: `src/core/config/CustomModesManager.ts` — `cacheTTL`
pub const CACHE_TTL_MS: u64 = 10_000;

// ---------------------------------------------------------------------------
// RuleFile / ExportResult / ImportResult
// ---------------------------------------------------------------------------

/// A rules file associated with a custom mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleFile {
    pub relative_path: String,
    #[serde(default)]
    pub content: String,
}

/// Extended mode config for export (includes rules files).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportedModeConfig {
    #[serde(flatten)]
    pub mode: ModeConfig,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rules_files: Option<Vec<RuleFile>>,
}

/// Result of exporting a mode.
#[derive(Debug, Clone)]
pub struct ExportResult {
    pub success: bool,
    pub yaml: Option<String>,
    pub error: Option<String>,
}

/// Result of importing a mode.
#[derive(Debug, Clone)]
pub struct ImportResult {
    pub success: bool,
    pub slug: Option<String>,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// CustomModesManager
// ---------------------------------------------------------------------------

/// Manager for custom mode configurations.
///
/// Handles loading, merging, and persisting custom modes from both project-level
/// (`.roomodes`) and global (settings file) sources. Project modes take precedence
/// over global modes when there are slug conflicts.
///
/// Source: `src/core/config/CustomModesManager.ts`
pub struct CustomModesManager {
    /// Path to the global custom modes settings file.
    global_settings_path: PathBuf,
    /// Path to the project `.roomodes` file (if any).
    project_roomodes_path: Option<PathBuf>,
    /// Cached modes (with TTL).
    cached_modes: Option<Vec<ModeConfig>>,
    /// Timestamp of last cache update.
    cached_at: Option<std::time::Instant>,
    /// Whether a write is in progress.
    #[allow(dead_code)]
    is_writing: bool,
    /// Queue of pending write operations.
    #[allow(dead_code)]
    write_queue:
        Vec<Box<dyn FnOnce() -> Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send>>,
}

// We need Pin for async boxed futures
use std::pin::Pin;

impl CustomModesManager {
    /// Create a new CustomModesManager.
    ///
    /// # Arguments
    /// * `global_settings_path` - Path to the global custom modes YAML file
    /// * `project_roomodes_path` - Optional path to the project `.roomodes` file
    pub fn new(global_settings_path: PathBuf, project_roomodes_path: Option<PathBuf>) -> Self {
        Self {
            global_settings_path,
            project_roomodes_path,
            cached_modes: None,
            cached_at: None,
            is_writing: false,
            write_queue: Vec::new(),
        }
    }

    /// Resolve Roo's global rules directory from the configured settings path.
    ///
    /// Roo-Code stores global rules under `~/.roo/rules-{slug}`. In tests and
    /// portable profiles the global settings path can point at a non-home
    /// `.roo` tree, so prefer that configured tree when it is recognizable.
    fn global_roo_dir(&self) -> PathBuf {
        if let Some(parent) = self.global_settings_path.parent() {
            if parent.file_name().and_then(|name| name.to_str()) == Some("settings")
                && let Some(grandparent) = parent.parent()
            {
                return grandparent.to_path_buf();
            }
            if parent.file_name().and_then(|name| name.to_str()) == Some(".roo") {
                return parent.to_path_buf();
            }
        }

        dirs::home_dir().unwrap_or_default().join(".roo")
    }

    fn rules_folder_for(
        &self,
        slug: &str,
        source: ModeSource,
        workspace_path: Option<&Path>,
    ) -> Result<PathBuf, String> {
        match source {
            ModeSource::Global => Ok(self.global_roo_dir().join(format!("rules-{slug}"))),
            ModeSource::Project => workspace_path
                .map(|wp| wp.join(".roo").join(format!("rules-{slug}")))
                .ok_or_else(|| "No workspace path".to_string()),
        }
    }

    fn delete_rules_folder_for(
        &self,
        slug: &str,
        source: ModeSource,
        workspace_path: Option<&Path>,
    ) -> Result<(), String> {
        let folder = self.rules_folder_for(slug, source, workspace_path)?;
        if folder.exists() {
            std::fs::remove_dir_all(&folder)
                .map_err(|e| format!("Failed to delete rules folder: {}", e))?;
        }
        Ok(())
    }

    // -------------------------------------------------------------------
    // Cache management
    // -------------------------------------------------------------------

    /// Clear the cached modes.
    fn clear_cache(&mut self) {
        self.cached_modes = None;
        self.cached_at = None;
    }

    /// Check if the cache is valid.
    fn is_cache_valid(&self) -> bool {
        match (self.cached_modes.as_ref(), self.cached_at) {
            (Some(_), Some(at)) => (at.elapsed().as_millis() as u64) < CACHE_TTL_MS,
            _ => false,
        }
    }

    // -------------------------------------------------------------------
    // YAML parsing with error handling
    // -------------------------------------------------------------------

    /// Clean invisible and problematic characters from YAML content.
    ///
    /// Mirrors the TS `cleanInvisibleCharacters()` method.
    pub fn clean_invisible_characters(content: &str) -> String {
        let mut result = String::with_capacity(content.len());
        for ch in content.chars() {
            match ch {
                '\u{00A0}' => result.push(' '), // Non-breaking space → regular space
                '\u{200B}' | '\u{200C}' | '\u{200D}' => {} // Zero-width → remove
                '\u{2018}' | '\u{2019}' => result.push('\''), // Smart single quotes
                '\u{201C}' | '\u{201D}' => result.push('"'), // Smart double quotes
                '\u{2010}'..='\u{2015}' | '\u{2212}' => result.push('-'), // Various dashes
                _ => result.push(ch),
            }
        }
        result
    }

    /// Parse YAML content safely, with JSON fallback for `.roomodes` files.
    ///
    /// Mirrors the TS `parseYamlSafely()` method.
    pub fn parse_yaml_safely(content: &str, file_path: &Path) -> serde_yaml::Value {
        // Strip BOM
        let content = content.strip_prefix('\u{FEFF}').unwrap_or(content);
        // Clean invisible characters
        let cleaned_content = Self::clean_invisible_characters(content);

        // Try YAML first
        match serde_yaml::from_str(&cleaned_content) {
            Ok(value) => value,
            Err(yaml_error) => {
                // For .roomodes files, try JSON as fallback
                if file_path
                    .file_name()
                    .map(|n| n == ROOMODES_FILENAME)
                    .unwrap_or(false)
                {
                    match serde_json::from_str::<serde_json::Value>(content) {
                        Ok(json_value) => {
                            // Convert JSON Value to YAML Value
                            serde_yaml::to_value(json_value).unwrap_or(serde_yaml::Value::Null)
                        }
                        Err(_) => {
                            tracing::error!(
                                "Failed to parse YAML from {}: {}",
                                file_path.display(),
                                yaml_error
                            );
                            serde_yaml::Value::Null
                        }
                    }
                } else {
                    tracing::error!(
                        "Failed to parse YAML from {}: {}",
                        file_path.display(),
                        yaml_error
                    );
                    serde_yaml::Value::Null
                }
            }
        }
    }

    fn is_valid_slug(slug: &str) -> bool {
        !slug.is_empty()
            && slug
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-')
    }

    fn group_name(group: &GroupEntry) -> ToolGroup {
        match group {
            GroupEntry::Plain(group) | GroupEntry::WithOptions(group, _) => *group,
        }
    }

    fn validate_mode_config(mode: &ModeConfig) -> Result<(), String> {
        if !Self::is_valid_slug(&mode.slug) {
            return Err("Slug must contain only letters numbers and dashes".to_string());
        }
        if mode.name.is_empty() {
            return Err("Name is required".to_string());
        }
        if mode.role_definition.is_empty() {
            return Err("Role definition is required".to_string());
        }

        let mut seen_groups = HashSet::new();
        for group in &mode.groups {
            let group_name = Self::group_name(group);
            if !seen_groups.insert(group_name) {
                return Err("Duplicate groups are not allowed".to_string());
            }
        }

        Ok(())
    }

    fn is_deprecated_group_entry(entry: &serde_yaml::Value) -> bool {
        match entry {
            serde_yaml::Value::String(group) => group == "browser",
            serde_yaml::Value::Sequence(items) => items
                .first()
                .and_then(|item| item.as_str())
                .is_some_and(|group| group == "browser"),
            _ => false,
        }
    }

    fn preprocess_mode_value(mode_value: serde_yaml::Value) -> Result<serde_yaml::Value, String> {
        let mut mapping = match mode_value {
            serde_yaml::Value::Mapping(mapping) => mapping,
            _ => return Err("Mode config must be an object".to_string()),
        };

        let groups_key = serde_yaml::Value::String("groups".to_string());
        if let Some(groups_value) = mapping.get_mut(&groups_key) {
            let groups = match groups_value {
                serde_yaml::Value::Sequence(groups) => groups,
                _ => return Err("groups must be an array".to_string()),
            };

            groups.retain(|entry| !Self::is_deprecated_group_entry(entry));
        }

        Ok(serde_yaml::Value::Mapping(mapping))
    }

    fn parse_mode_config_from_value(mode_value: serde_yaml::Value) -> Result<ModeConfig, String> {
        let mode_value = Self::preprocess_mode_value(mode_value)?;
        let mode: ModeConfig = serde_yaml::from_value(mode_value)
            .map_err(|e| format!("Invalid mode configuration: {e}"))?;
        Self::validate_mode_config(&mode)?;
        Ok(mode)
    }

    fn parse_custom_modes_array(modes_value: serde_yaml::Value) -> Result<Vec<ModeConfig>, String> {
        let modes_array = match modes_value {
            serde_yaml::Value::Sequence(items) => items,
            _ => return Err("customModes must be an array".to_string()),
        };

        let mut slugs = HashSet::new();
        let mut modes = Vec::new();

        for mode_value in modes_array {
            let mode = Self::parse_mode_config_from_value(mode_value)?;
            if !slugs.insert(mode.slug.clone()) {
                return Err("Duplicate mode slugs are not allowed".to_string());
            }
            modes.push(mode);
        }

        Ok(modes)
    }

    // -------------------------------------------------------------------
    // Loading modes from files
    // -------------------------------------------------------------------

    /// Load modes from a single file.
    ///
    /// Mirrors the TS `loadModesFromFile()` method.
    pub fn load_modes_from_file(&self, file_path: &Path) -> Vec<ModeConfig> {
        let content = match std::fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::error!("Failed to read modes from {}: {}", file_path.display(), e);
                return Vec::new();
            }
        };

        let parsed = Self::parse_yaml_safely(&content, file_path);

        // Extract customModes array
        let modes_value = match parsed.get("customModes") {
            Some(v) => v.clone(),
            None => return Vec::new(),
        };

        // Parse into ModeConfig Vec, matching Roo-Code's schema preprocessing
        // that strips deprecated tool groups before validation.
        match Self::parse_custom_modes_array(modes_value) {
            Ok(modes) => {
                let is_roomodes = file_path
                    .file_name()
                    .map(|n| n == ROOMODES_FILENAME)
                    .unwrap_or(false);
                let source = if is_roomodes {
                    ModeSource::Project
                } else {
                    ModeSource::Global
                };

                modes
                    .into_iter()
                    .map(|mut m| {
                        m.source = Some(source);
                        m
                    })
                    .collect()
            }
            Err(e) => {
                tracing::error!(
                    "Schema validation failed for {}: {}",
                    file_path.display(),
                    e
                );
                Vec::new()
            }
        }
    }

    /// Merge project and global modes.
    ///
    /// Project modes take precedence (by slug). Global modes with duplicate
    /// slugs are excluded.
    ///
    /// Mirrors the TS `mergeCustomModes()` method.
    pub fn merge_custom_modes(
        project_modes: &[ModeConfig],
        global_modes: &[ModeConfig],
    ) -> Vec<ModeConfig> {
        let mut seen_slugs = HashSet::new();
        let mut merged = Vec::new();

        // Add project modes (takes precedence)
        for mode in project_modes {
            if !seen_slugs.contains(&mode.slug) {
                seen_slugs.insert(mode.slug.clone());
                let mut m = mode.clone();
                m.source = Some(ModeSource::Project);
                merged.push(m);
            }
        }

        // Add non-duplicate global modes
        for mode in global_modes {
            if !seen_slugs.contains(&mode.slug) {
                seen_slugs.insert(mode.slug.clone());
                let mut m = mode.clone();
                m.source = Some(ModeSource::Global);
                merged.push(m);
            }
        }

        merged
    }

    // -------------------------------------------------------------------
    // Public API
    // -------------------------------------------------------------------

    /// Get all custom modes (merged from project and global).
    ///
    /// Uses a cache with 10-second TTL.
    ///
    /// Mirrors the TS `getCustomModes()` method.
    pub fn get_custom_modes(&mut self) -> Vec<ModeConfig> {
        if self.is_cache_valid() {
            return self.cached_modes.clone().unwrap_or_default();
        }

        // Load global modes
        let global_modes = if self.global_settings_path.exists() {
            self.load_modes_from_file(&self.global_settings_path)
        } else {
            Vec::new()
        };

        // Load project modes
        let project_modes = match &self.project_roomodes_path {
            Some(path) if path.exists() => self.load_modes_from_file(path),
            _ => Vec::new(),
        };

        // Merge
        let merged = Self::merge_custom_modes(&project_modes, &global_modes);

        self.cached_modes = Some(merged.clone());
        self.cached_at = Some(std::time::Instant::now());

        merged
    }

    /// Update or add a custom mode.
    ///
    /// Mirrors the TS `updateCustomMode()` method.
    pub fn update_custom_mode(&mut self, slug: &str, config: ModeConfig) -> Result<(), String> {
        // Validate the mode configuration
        Self::validate_mode_config(&config)?;

        let is_project_mode = config.source == Some(ModeSource::Project);

        let target_path = if is_project_mode {
            match &self.project_roomodes_path {
                Some(path) => path.clone(),
                None => return Err("No workspace folder found for project mode".to_string()),
            }
        } else {
            self.global_settings_path.clone()
        };

        // Update modes in the target file
        self.update_modes_in_file(&target_path, |modes| {
            let mut updated: Vec<ModeConfig> =
                modes.into_iter().filter(|m| m.slug != slug).collect();

            let mut mode_with_source = config.clone();
            mode_with_source.source = Some(if is_project_mode {
                ModeSource::Project
            } else {
                ModeSource::Global
            });
            updated.push(mode_with_source);
            updated
        })?;

        self.clear_cache();
        Ok(())
    }

    /// Delete a custom mode by slug.
    ///
    /// Mirrors the TS `deleteCustomMode()` method.
    pub fn delete_custom_mode(&mut self, slug: &str) -> Result<(), String> {
        let global_modes = if self.global_settings_path.exists() {
            self.load_modes_from_file(&self.global_settings_path)
        } else {
            Vec::new()
        };

        let project_modes = match &self.project_roomodes_path {
            Some(path) if path.exists() => self.load_modes_from_file(path),
            _ => Vec::new(),
        };

        let project_mode = project_modes.iter().find(|m| m.slug == slug);
        let global_mode = global_modes.iter().find(|m| m.slug == slug);

        if project_mode.is_none() && global_mode.is_none() {
            return Err(format!("Mode '{}' not found", slug));
        }

        // Delete from project file if present
        if project_mode.is_some()
            && let Some(roomodes_path) = &self.project_roomodes_path
        {
            self.update_modes_in_file(roomodes_path, |modes| {
                modes.into_iter().filter(|m| m.slug != slug).collect()
            })?;
        }

        // Delete from global file if present
        if global_mode.is_some() {
            self.update_modes_in_file(&self.global_settings_path, |modes| {
                modes.into_iter().filter(|m| m.slug != slug).collect()
            })?;
        }

        if let Some(mode) = project_mode {
            let workspace_path = self
                .project_roomodes_path
                .as_ref()
                .and_then(|path| path.parent());
            if let Err(e) = self.delete_rules_folder_for(
                slug,
                mode.source.unwrap_or(ModeSource::Project),
                workspace_path,
            ) {
                tracing::warn!("Failed to delete project rules folder for {slug}: {e}");
            }
        }
        if let Some(mode) = global_mode
            && let Err(e) =
                self.delete_rules_folder_for(slug, mode.source.unwrap_or(ModeSource::Global), None)
        {
            tracing::warn!("Failed to delete global rules folder for {slug}: {e}");
        }

        self.clear_cache();
        Ok(())
    }

    /// Reset all custom modes (clear the global settings file).
    ///
    /// Mirrors the TS `resetCustomModes()` method.
    pub fn reset_custom_modes(&mut self) -> Result<(), String> {
        let empty_data = serde_json::json!({"customModes": []});
        let empty_settings = serde_yaml::to_string(&empty_data)
            .map_err(|e| format!("Failed to serialize empty modes: {}", e))?;

        std::fs::write(&self.global_settings_path, empty_settings)
            .map_err(|e| format!("Failed to write modes file: {}", e))?;

        self.clear_cache();
        Ok(())
    }

    // -------------------------------------------------------------------
    // File operations
    // -------------------------------------------------------------------

    /// Update modes in a file using a transformation function.
    ///
    /// Mirrors the TS `updateModesInFile()` method.
    fn update_modes_in_file<F>(&self, file_path: &Path, operation: F) -> Result<(), String>
    where
        F: FnOnce(Vec<ModeConfig>) -> Vec<ModeConfig>,
    {
        // Read existing content
        let content = if file_path.exists() {
            std::fs::read_to_string(file_path).unwrap_or_else(|_| "customModes: []\n".to_string())
        } else {
            "customModes: []\n".to_string()
        };

        // Parse
        let mut parsed = Self::parse_yaml_safely(&content, file_path);

        // Ensure customModes exists
        if parsed.get("customModes").is_none() {
            parsed["customModes"] = serde_yaml::Value::Sequence(serde_yaml::Sequence::new());
        }

        // Extract and transform modes
        let modes_value = parsed
            .get("customModes")
            .cloned()
            .unwrap_or(serde_yaml::Value::Null);
        let existing_modes: Vec<ModeConfig> =
            Self::parse_custom_modes_array(modes_value).unwrap_or_default();

        let updated_modes = operation(existing_modes);

        // Write back
        let updated_value = serde_yaml::to_value(&updated_modes)
            .map_err(|e| format!("Failed to serialize modes: {}", e))?;

        let mut result = serde_yaml::Mapping::new();
        result.insert(
            serde_yaml::Value::String("customModes".to_string()),
            updated_value,
        );

        let yaml_content = serde_yaml::to_string(&serde_yaml::Value::Mapping(result))
            .map_err(|e| format!("Failed to serialize YAML: {}", e))?;

        // Ensure parent directory exists
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        std::fs::write(file_path, yaml_content)
            .map_err(|e| format!("Failed to write file: {}", e))?;

        Ok(())
    }

    /// Check if a mode has associated rules files with content.
    ///
    /// Mirrors the TS `checkRulesDirectoryHasContent()` method.
    pub fn check_rules_directory_has_content(
        &self,
        slug: &str,
        workspace_path: Option<&Path>,
    ) -> bool {
        let all_modes = self.cached_modes.clone().unwrap_or_default();
        let loaded_modes = if all_modes.is_empty() {
            let global_modes = if self.global_settings_path.exists() {
                self.load_modes_from_file(&self.global_settings_path)
            } else {
                Vec::new()
            };
            let project_modes = match &self.project_roomodes_path {
                Some(path) if path.exists() => self.load_modes_from_file(path),
                _ => Vec::new(),
            };
            Self::merge_custom_modes(&project_modes, &global_modes)
        } else {
            all_modes
        };
        let mode = loaded_modes.iter().find(|m| m.slug == slug);

        let rules_dir = match mode.and_then(|m| m.source) {
            Some(ModeSource::Global) => self.global_roo_dir().join(format!("rules-{slug}")),
            Some(ModeSource::Project) => match workspace_path {
                Some(wp) => wp.join(".roo").join(format!("rules-{slug}")),
                None => return false,
            },
            None => return false,
        };

        if !rules_dir.is_dir() {
            return false;
        }

        // Check for files with content
        match std::fs::read_dir(&rules_dir) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    if let Ok(file_type) = entry.file_type()
                        && file_type.is_file()
                        && let Ok(content) = std::fs::read_to_string(entry.path())
                        && !content.trim().is_empty()
                    {
                        return true;
                    }
                }
            }
            Err(_) => return false,
        }

        false
    }

    /// Export a mode with its associated rules files.
    ///
    /// Mirrors the TS `exportModeWithRules()` method.
    pub fn export_mode_with_rules(
        &mut self,
        slug: &str,
        custom_prompts: Option<PromptComponent>,
        workspace_path: Option<&Path>,
    ) -> ExportResult {
        let all_modes = self.get_custom_modes();
        let mode = match all_modes.iter().find(|m| m.slug == slug) {
            Some(m) => m.clone(),
            None => match roo_types::mode::default_modes()
                .into_iter()
                .find(|m| m.slug == slug)
            {
                Some(m) => m,
                None => {
                    return ExportResult {
                        success: false,
                        yaml: None,
                        error: Some("Mode not found".to_string()),
                    };
                }
            },
        };

        // Determine rules directory
        let rules_dir = match mode.source {
            Some(ModeSource::Global) => self.global_roo_dir().join(format!("rules-{slug}")),
            Some(ModeSource::Project) => match workspace_path {
                Some(wp) => wp.join(".roo").join(format!("rules-{slug}")),
                None => {
                    return ExportResult {
                        success: false,
                        yaml: None,
                        error: Some("No workspace found".to_string()),
                    };
                }
            },
            None => match workspace_path {
                Some(wp) => wp.join(".roo").join(format!("rules-{slug}")),
                None => {
                    return ExportResult {
                        success: false,
                        yaml: None,
                        error: Some("No workspace found".to_string()),
                    };
                }
            },
        };

        // Collect rules files
        let mut rules_files = Vec::new();
        if rules_dir.is_dir()
            && let Ok(entries) = std::fs::read_dir(&rules_dir)
        {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type()
                    && file_type.is_file()
                    && let Ok(content) = std::fs::read_to_string(entry.path())
                    && !content.trim().is_empty()
                {
                    let relative = entry
                        .path()
                        .strip_prefix(&rules_dir)
                        .unwrap_or(entry.path().as_path())
                        .to_string_lossy()
                        .replace('\\', "/");
                    rules_files.push(RuleFile {
                        relative_path: relative,
                        content: content.trim().to_string(),
                    });
                }
            }
        }

        // Build export config
        let mut export_mode = mode.clone();
        export_mode.source = Some(ModeSource::Project);

        // Merge custom prompts if provided
        if let Some(prompts) = custom_prompts {
            if let Some(rd) = prompts.role_definition {
                export_mode.role_definition = rd;
            }
            if let Some(d) = prompts.description {
                export_mode.description = Some(d);
            }
            if let Some(wtu) = prompts.when_to_use {
                export_mode.when_to_use = Some(wtu);
            }
            if let Some(ci) = prompts.custom_instructions {
                export_mode.custom_instructions = Some(ci);
            }
        }

        let export_config = ExportedModeConfig {
            mode: export_mode,
            rules_files: if rules_files.is_empty() {
                None
            } else {
                Some(rules_files)
            },
        };

        // Generate YAML
        let export_data = serde_json::json!({
            "customModes": [export_config]
        });

        match serde_yaml::to_string(&export_data) {
            Ok(yaml) => ExportResult {
                success: true,
                yaml: Some(yaml),
                error: None,
            },
            Err(e) => ExportResult {
                success: false,
                yaml: None,
                error: Some(format!("Failed to serialize: {}", e)),
            },
        }
    }

    /// Import modes from YAML content.
    ///
    /// Mirrors the TS `importModeWithRules()` method.
    pub fn import_mode_with_rules(
        &mut self,
        yaml_content: &str,
        source: ModeSource,
        workspace_path: Option<&Path>,
    ) -> ImportResult {
        // Parse YAML
        let parsed: serde_yaml::Value = match serde_yaml::from_str(yaml_content) {
            Ok(v) => v,
            Err(e) => {
                return ImportResult {
                    success: false,
                    slug: None,
                    error: Some(format!("Invalid YAML format: {}", e)),
                };
            }
        };

        // Extract customModes array
        let modes_array = match parsed.get("customModes") {
            Some(serde_yaml::Value::Sequence(arr)) => arr.clone(),
            _ => {
                return ImportResult {
                    success: false,
                    slug: None,
                    error: Some(
                        "Invalid import format: Expected 'customModes' array in YAML".to_string(),
                    ),
                };
            }
        };

        if modes_array.is_empty() {
            return ImportResult {
                success: false,
                slug: None,
                error: Some("No modes found in YAML".to_string()),
            };
        }

        // Check workspace for project import
        if source == ModeSource::Project && workspace_path.is_none() {
            return ImportResult {
                success: false,
                slug: None,
                error: Some("No workspace found".to_string()),
            };
        }

        // Process each mode
        let mut first_slug: Option<String> = None;
        for mode_value in &modes_array {
            let mode_config: ModeConfig =
                match Self::parse_mode_config_from_value(mode_value.clone()) {
                    Ok(m) => m,
                    Err(e) => {
                        return ImportResult {
                            success: false,
                            slug: None,
                            error: Some(e),
                        };
                    }
                };

            if first_slug.is_none() {
                first_slug = Some(mode_config.slug.clone());
            }

            // Update the mode
            let mut config = mode_config;
            config.source = Some(source);
            let slug = config.slug.clone();

            if let Err(e) = self.update_custom_mode(&slug, config) {
                return ImportResult {
                    success: false,
                    slug: None,
                    error: Some(e),
                };
            }

            // Import rules files if present
            if let Some(rules_files) = mode_value.get("rulesFiles")
                && let Ok(files) = serde_yaml::from_value::<Vec<RuleFile>>(rules_files.clone())
                && let Err(e) = self.import_rules_files(&slug, &files, source, workspace_path)
            {
                tracing::warn!("Failed to import rules files: {}", e);
            }
        }

        ImportResult {
            success: true,
            slug: first_slug,
            error: None,
        }
    }

    /// Import rules files for a mode.
    ///
    /// Mirrors the TS `importRulesFiles()` method.
    fn import_rules_files(
        &self,
        slug: &str,
        rules_files: &[RuleFile],
        source: ModeSource,
        workspace_path: Option<&Path>,
    ) -> Result<(), String> {
        let rules_folder = self.rules_folder_for(slug, source, workspace_path)?;

        // Remove existing rules folder
        if rules_folder.exists() {
            let _ = std::fs::remove_dir_all(&rules_folder);
        }

        // Import new rules files
        for rule_file in rules_files {
            if rule_file.relative_path.is_empty() || rule_file.content.is_empty() {
                continue;
            }

            let Some(normalized) = Self::normalize_import_rule_path(&rule_file.relative_path)
            else {
                tracing::warn!("Invalid file path detected: {}", rule_file.relative_path);
                continue;
            };

            let target_path = rules_folder.join(&normalized);

            // Ensure the resolved path stays within the rules folder
            let parent_dir = target_path.parent().unwrap_or_else(|| Path::new("."));
            std::fs::create_dir_all(parent_dir)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
            let canonical_rules = rules_folder
                .canonicalize()
                .unwrap_or_else(|_| rules_folder.clone());
            let canonical_parent = parent_dir
                .canonicalize()
                .unwrap_or_else(|_| parent_dir.to_path_buf());
            if !canonical_parent.starts_with(&canonical_rules) {
                tracing::warn!(
                    "Path traversal attempt detected: {}",
                    rule_file.relative_path
                );
                continue;
            }

            std::fs::write(&target_path, &rule_file.content)
                .map_err(|e| format!("Failed to write file: {}", e))?;
        }

        Ok(())
    }

    fn normalize_import_rule_path(relative_path: &str) -> Option<String> {
        fn has_unsafe_components(path: &Path) -> bool {
            path.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::Prefix(_) | Component::RootDir
                )
            })
        }

        let raw_path = Path::new(relative_path);
        if raw_path.is_absolute() || has_unsafe_components(raw_path) {
            return None;
        }

        let normalized = relative_path.replace('\\', "/");
        if normalized.starts_with('/')
            || normalized.is_empty()
            || normalized.contains(":/")
            || normalized
                .as_bytes()
                .get(1)
                .is_some_and(|byte| *byte == b':')
        {
            return None;
        }

        let cleaned = if normalized.starts_with("rules-") {
            normalized
                .split_once('/')
                .map(|(_, rest)| rest.to_string())
                .unwrap_or(normalized)
        } else {
            normalized
        };

        let cleaned_path = Path::new(&cleaned);
        if cleaned_path.is_absolute() || has_unsafe_components(cleaned_path) || cleaned.is_empty() {
            return None;
        }

        Some(cleaned)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn create_temp_modes_file(content: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("custom_modes.yaml");
        std::fs::write(&file_path, content).unwrap();
        dir
    }

    fn make_mode(slug: &str, name: &str, source: ModeSource) -> ModeConfig {
        ModeConfig {
            slug: slug.to_string(),
            name: name.to_string(),
            role_definition: "Test role".to_string(),
            when_to_use: None,
            description: None,
            custom_instructions: None,
            groups: vec![],
            source: Some(source),
        }
    }

    // ---- Test 1: Merge custom modes (project takes precedence) ----
    #[test]
    fn test_merge_custom_modes_precedence() {
        let project = vec![make_mode("test", "Test Mode", ModeSource::Project)];
        let global = vec![make_mode("test", "Test Mode Global", ModeSource::Global)];

        let merged = CustomModesManager::merge_custom_modes(&project, &global);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].source, Some(ModeSource::Project));
    }

    // ---- Test 2: Merge custom modes (no conflict) ----
    #[test]
    fn test_merge_custom_modes_no_conflict() {
        let project = vec![make_mode("project-mode", "Project", ModeSource::Project)];
        let global = vec![make_mode("global-mode", "Global", ModeSource::Global)];

        let merged = CustomModesManager::merge_custom_modes(&project, &global);
        assert_eq!(merged.len(), 2);
    }

    // ---- Test 3: Clean invisible characters ----
    #[test]
    fn test_clean_invisible_characters() {
        let input = "hello\u{00A0}world\u{201C}test\u{201D}";
        let result = CustomModesManager::clean_invisible_characters(input);
        assert_eq!(result, "hello world\"test\"");
    }

    // ---- Test 4: Clean zero-width characters ----
    #[test]
    fn test_clean_zero_width() {
        let input = "hello\u{200B}world";
        let result = CustomModesManager::clean_invisible_characters(input);
        assert_eq!(result, "helloworld");
    }

    // ---- Test 5: Load modes from file ----
    #[test]
    fn test_load_modes_from_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.yaml");
        let content = r#"
customModes:
  - slug: test-mode
    name: Test Mode
    roleDefinition: "Test role definition"
    groups: []
"#;
        std::fs::write(&file_path, content).unwrap();

        let manager = CustomModesManager::new(file_path.clone(), None);
        let modes = manager.load_modes_from_file(&file_path);
        assert_eq!(modes.len(), 1);
        assert_eq!(modes[0].slug, "test-mode");
    }

    #[test]
    fn load_modes_strips_deprecated_browser_groups_and_ignores_export_fields() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join(".roomodes");
        let content = r#"
customModes:
  - slug: browser-compat
    name: Browser Compat
    roleDefinition: "Test role definition"
    groups:
      - read
      - browser
      - [edit, { fileRegex: "\\.md$", description: "Markdown" }]
      - [browser, {}]
    rulesFiles:
      - relativePath: rule.md
"#;
        std::fs::write(&file_path, content).unwrap();

        let manager = CustomModesManager::new(file_path.clone(), Some(file_path.clone()));
        let modes = manager.load_modes_from_file(&file_path);

        assert_eq!(modes.len(), 1);
        assert_eq!(modes[0].source, Some(ModeSource::Project));
        assert_eq!(modes[0].groups.len(), 2);
        assert!(matches!(
            modes[0].groups[0],
            GroupEntry::Plain(ToolGroup::Read)
        ));
        assert!(matches!(
            modes[0].groups[1],
            GroupEntry::WithOptions(ToolGroup::Edit, _)
        ));
    }

    #[test]
    fn load_modes_rejects_invalid_slug_and_duplicate_groups() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("custom_modes.yaml");
        let content = r#"
customModes:
  - slug: "bad slug"
    name: Bad
    roleDefinition: "Test"
    groups: [read, read]
"#;
        std::fs::write(&file_path, content).unwrap();

        let manager = CustomModesManager::new(file_path.clone(), None);
        let modes = manager.load_modes_from_file(&file_path);

        assert!(modes.is_empty());
    }

    // ---- Test 6: Update custom mode ----
    #[test]
    fn test_update_custom_mode() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("modes.yaml");
        std::fs::write(&file_path, "customModes: []\n").unwrap();

        let mut manager = CustomModesManager::new(file_path.clone(), None);
        let mode = make_mode("new-mode", "New Mode", ModeSource::Global);
        manager.update_custom_mode("new-mode", mode).unwrap();

        let modes = manager.get_custom_modes();
        assert_eq!(modes.len(), 1);
        assert_eq!(modes[0].slug, "new-mode");
    }

    // ---- Test 7: Delete custom mode ----
    #[test]
    fn test_delete_custom_mode() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("modes.yaml");
        let content = r#"
customModes:
  - slug: to-delete
    name: To Delete
    roleDefinition: "Test"
    groups: []
"#;
        std::fs::write(&file_path, content).unwrap();

        let mut manager = CustomModesManager::new(file_path.clone(), None);
        manager.delete_custom_mode("to-delete").unwrap();

        let modes = manager.get_custom_modes();
        assert_eq!(modes.len(), 0);
    }

    #[test]
    fn delete_custom_mode_removes_global_rules_folder() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir
            .path()
            .join(".roo")
            .join("settings")
            .join("custom_modes.yaml");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        let content = r#"
customModes:
  - slug: to-delete
    name: To Delete
    roleDefinition: "Test"
    groups: []
"#;
        std::fs::write(&file_path, content).unwrap();
        let rules_dir = dir.path().join(".roo").join("rules-to-delete");
        std::fs::create_dir_all(&rules_dir).unwrap();
        std::fs::write(rules_dir.join("rule.md"), "rule").unwrap();

        let mut manager = CustomModesManager::new(file_path, None);
        manager.delete_custom_mode("to-delete").unwrap();

        assert!(!rules_dir.exists());
    }

    // ---- Test 8: Delete non-existent mode ----
    #[test]
    fn test_delete_nonexistent_mode() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("modes.yaml");
        std::fs::write(&file_path, "customModes: []\n").unwrap();

        let mut manager = CustomModesManager::new(file_path, None);
        let result = manager.delete_custom_mode("nonexistent");
        assert!(result.is_err());
    }

    // ---- Test 9: Reset custom modes ----
    #[test]
    fn test_reset_custom_modes() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("modes.yaml");
        let content = r#"
customModes:
  - slug: mode-1
    name: Mode 1
    roleDefinition: "Test"
    groups: []
"#;
        std::fs::write(&file_path, content).unwrap();

        let mut manager = CustomModesManager::new(file_path, None);
        manager.reset_custom_modes().unwrap();

        let modes = manager.get_custom_modes();
        assert!(modes.is_empty());
    }

    // ---- Test 10: Cache TTL ----
    #[test]
    fn test_cache_ttl() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("modes.yaml");
        std::fs::write(&file_path, "customModes: []\n").unwrap();

        let mut manager = CustomModesManager::new(file_path, None);

        // Initially no cache
        assert!(!manager.is_cache_valid());

        // After loading, cache is valid
        let _ = manager.get_custom_modes();
        assert!(manager.is_cache_valid());
    }

    // ---- Test 11: Export mode ----
    #[test]
    fn test_export_mode() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("modes.yaml");
        let content = r#"
customModes:
  - slug: export-test
    name: Export Test
    roleDefinition: "Test role"
    groups: []
"#;
        std::fs::write(&file_path, content).unwrap();

        let mut manager = CustomModesManager::new(file_path, None);
        let result = manager.export_mode_with_rules("export-test", None, None);
        assert!(result.success);
        assert!(result.yaml.is_some());
    }

    #[test]
    fn export_global_mode_reads_rules_from_configured_roo_dir() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir
            .path()
            .join(".roo")
            .join("settings")
            .join("custom_modes.yaml");
        std::fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        let content = r#"
customModes:
  - slug: export-test
    name: Export Test
    roleDefinition: "Test role"
    groups: []
"#;
        std::fs::write(&file_path, content).unwrap();
        let rules_dir = dir.path().join(".roo").join("rules-export-test");
        std::fs::create_dir_all(&rules_dir).unwrap();
        std::fs::write(rules_dir.join("rule.md"), "global rule").unwrap();

        let mut manager = CustomModesManager::new(file_path, None);
        let result = manager.export_mode_with_rules("export-test", None, None);

        assert!(result.success);
        let yaml = result.yaml.expect("export yaml");
        assert!(yaml.contains("rulesFiles:"));
        assert!(yaml.contains("global rule"));
    }

    #[test]
    fn import_multiple_modes_writes_rules_to_each_mode_folder() {
        let dir = tempfile::tempdir().unwrap();
        let global_path = dir
            .path()
            .join(".roo")
            .join("settings")
            .join("custom_modes.yaml");
        std::fs::create_dir_all(global_path.parent().unwrap()).unwrap();
        std::fs::write(&global_path, "customModes: []\n").unwrap();
        let project_roomodes = dir.path().join(".roomodes");
        let yaml = r#"
customModes:
  - slug: one
    name: One
    roleDefinition: "Role one"
    groups: []
    rulesFiles:
      - relativePath: rule.md
        content: one-rule
  - slug: two
    name: Two
    roleDefinition: "Role two"
    groups: []
    rulesFiles:
      - relativePath: rules-two/rule.md
        content: two-rule
"#;

        let mut manager = CustomModesManager::new(global_path, Some(project_roomodes));
        let result = manager.import_mode_with_rules(yaml, ModeSource::Project, Some(dir.path()));

        assert!(result.success, "{:?}", result.error);
        assert_eq!(
            std::fs::read_to_string(dir.path().join(".roo").join("rules-one").join("rule.md"))
                .unwrap(),
            "one-rule"
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join(".roo").join("rules-two").join("rule.md"))
                .unwrap(),
            "two-rule"
        );
    }

    #[test]
    fn import_rules_files_rejects_unsafe_paths_and_allows_missing_content() {
        let dir = tempfile::tempdir().unwrap();
        let global_path = dir
            .path()
            .join(".roo")
            .join("settings")
            .join("custom_modes.yaml");
        std::fs::create_dir_all(global_path.parent().unwrap()).unwrap();
        std::fs::write(&global_path, "customModes: []\n").unwrap();
        let project_roomodes = dir.path().join(".roomodes");
        let yaml = r#"
customModes:
  - slug: safe
    name: Safe
    roleDefinition: "Role"
    groups: []
    rulesFiles:
      - relativePath: rules-safe/nested/rule.md
        content: safe-rule
      - relativePath: ../outside.md
        content: unsafe-rule
      - relativePath: C:\outside.md
        content: absolute-rule
      - relativePath: empty.md
"#;

        let mut manager = CustomModesManager::new(global_path, Some(project_roomodes));
        let result = manager.import_mode_with_rules(yaml, ModeSource::Project, Some(dir.path()));

        assert!(result.success, "{:?}", result.error);
        assert_eq!(
            std::fs::read_to_string(
                dir.path()
                    .join(".roo")
                    .join("rules-safe")
                    .join("nested")
                    .join("rule.md")
            )
            .unwrap(),
            "safe-rule"
        );
        assert!(!dir.path().join(".roo").join("outside.md").exists());
        let entries = std::fs::read_dir(dir.path().join(".roo").join("rules-safe"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        assert!(!entries.iter().any(|entry| entry.starts_with("C")));
        assert!(
            !dir.path()
                .join(".roo")
                .join("rules-safe")
                .join("empty.md")
                .exists()
        );
    }

    #[test]
    fn normalize_import_rule_path_rejects_absolute_and_traversal_paths() {
        assert_eq!(
            CustomModesManager::normalize_import_rule_path("rules-safe/nested/rule.md"),
            Some("nested/rule.md".to_string())
        );
        assert!(CustomModesManager::normalize_import_rule_path("../rule.md").is_none());
        assert!(CustomModesManager::normalize_import_rule_path("/tmp/rule.md").is_none());
        assert!(CustomModesManager::normalize_import_rule_path(r"C:\tmp\rule.md").is_none());
        assert!(CustomModesManager::normalize_import_rule_path("C:/tmp/rule.md").is_none());
    }

    #[test]
    fn test_exported_rules_files_use_roo_camel_case_fields() {
        let mode = ModeConfig {
            slug: "export-test".to_string(),
            name: "Export Test".to_string(),
            role_definition: "Test role".to_string(),
            when_to_use: None,
            description: None,
            custom_instructions: None,
            groups: vec![],
            source: None,
        };
        let exported = ExportedModeConfig {
            mode,
            rules_files: Some(vec![RuleFile {
                relative_path: ".roo/rules-export-test/rule.md".to_string(),
                content: "rule".to_string(),
            }]),
        };
        let yaml = serde_yaml::to_string(&exported).unwrap();

        assert!(yaml.contains("rulesFiles:"));
        assert!(yaml.contains("relativePath:"));
        assert!(!yaml.contains("rules_files:"));
        assert!(!yaml.contains("relative_path:"));
    }

    // ---- Test 12: Export non-existent mode ----
    #[test]
    fn test_export_nonexistent_mode() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("modes.yaml");
        std::fs::write(&file_path, "customModes: []\n").unwrap();

        let mut manager = CustomModesManager::new(file_path, None);
        let result = manager.export_mode_with_rules("nonexistent", None, None);
        assert!(!result.success);
        assert!(result.error.is_some());
    }
}

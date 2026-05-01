//! Settings Migration
//!
//! Migrates old settings files to new file names and removes commands from old defaults.
//! Mirrors `migrateSettings.ts`.

use std::path::Path;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors during settings migration.
#[derive(Debug, thiserror::Error)]
pub enum MigrationError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

// ---------------------------------------------------------------------------
// File migration definitions
// ---------------------------------------------------------------------------

/// A file migration entry.
#[derive(Debug, Clone)]
pub struct FileMigration {
    pub old_name: String,
    pub new_name: String,
}

/// Default file migrations.
pub fn default_file_migrations() -> Vec<FileMigration> {
    vec![
        FileMigration {
            old_name: "cline_custom_modes.json".to_string(),
            new_name: "custom_modes.json".to_string(),
        },
        FileMigration {
            old_name: "cline_mcp_settings.json".to_string(),
            new_name: "mcp_settings.json".to_string(),
        },
    ]
}

/// Old default commands that should be removed for security.
const OLD_DEFAULT_COMMANDS: &[&str] = &["npm install", "npm test", "tsc"];

// ---------------------------------------------------------------------------
// Migration functions
// ---------------------------------------------------------------------------

/// Migrate settings files from old names to new names.
///
/// Source: `.research/Roo-Code/src/utils/migrateSettings.ts` — `migrateSettings`
pub async fn migrate_settings(settings_dir: &Path) -> Result<Vec<String>, MigrationError> {
    let mut log = Vec::new();

    if !settings_dir.exists() {
        log.push("No settings directory found, no migrations necessary".to_string());
        return Ok(log);
    }

    // Process each file migration
    let migrations = default_file_migrations();
    for migration in &migrations {
        let old_path = settings_dir.join(&migration.old_name);
        let new_path = settings_dir.join(&migration.new_name);

        let old_exists = old_path.exists();
        let new_exists = new_path.exists();

        if old_exists && !new_exists {
            tokio::fs::rename(&old_path, &new_path).await?;
            log.push(format!(
                "Renamed {} to {}",
                migration.old_name, migration.new_name
            ));
        } else {
            log.push(format!(
                "Skipping migration of {} to {}: {}",
                migration.old_name,
                migration.new_name,
                if old_exists {
                    "new file already exists"
                } else {
                    "old file not found"
                }
            ));
        }
    }

    // Special migration for custom_modes.json to custom_modes.yaml
    migrate_custom_modes_to_yaml(settings_dir, &mut log).await?;

    Ok(log)
}

/// Migrate custom_modes.json to YAML format.
///
/// Source: `migrateSettings.ts` — `migrateCustomModesToYaml`
pub async fn migrate_custom_modes_to_yaml(
    settings_dir: &Path,
    log: &mut Vec<String>,
) -> Result<(), MigrationError> {
    let old_json_path = settings_dir.join("custom_modes.json");
    let new_yaml_path = settings_dir.join("custom_modes.yaml");

    if !old_json_path.exists() {
        log.push("No custom_modes.json found, skipping YAML migration".to_string());
        return Ok(());
    }

    if new_yaml_path.exists() {
        log.push("custom_modes.yaml already exists, skipping migration".to_string());
        return Ok(());
    }

    match tokio::fs::read_to_string(&old_json_path).await {
        Ok(json_content) => {
            match serde_yaml::from_str::<serde_yaml::Value>(&json_content) {
                Ok(custom_modes_data) => {
                    let yaml_content = serde_yaml::to_string(&custom_modes_data)?;
                    tokio::fs::write(&new_yaml_path, &yaml_content).await?;
                    log.push(
                        "Successfully migrated custom_modes.json to YAML format (original JSON file preserved for rollback purposes)".to_string()
                    );
                }
                Err(e) => {
                    log.push(format!(
                        "Error parsing custom_modes.json: {}. File might be corrupted. Skipping migration.",
                        e
                    ));
                }
            }
        }
        Err(e) => {
            log.push(format!(
                "Error reading custom_modes.json: {}. Skipping migration.",
                e
            ));
        }
    }

    Ok(())
}

/// Remove old default commands from the allowed commands list.
///
/// Source: `migrateSettings.ts` — `migrateDefaultCommands`
pub fn migrate_default_commands(allowed_commands: &[String]) -> (Vec<String>, usize) {
    let original_length = allowed_commands.len();
    let filtered: Vec<String> = allowed_commands
        .iter()
        .filter(|cmd| {
            let cmd_lower = cmd.to_lowercase();
            !OLD_DEFAULT_COMMANDS
                .iter()
                .any(|old| cmd_lower == old.to_lowercase())
        })
        .cloned()
        .collect();

    let removed_count = original_length - filtered.len();
    (filtered, removed_count)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_migrate_settings_no_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let nonexistent = tmp.path().join("nonexistent");
        let log = migrate_settings(&nonexistent).await.unwrap();
        assert!(log.iter().any(|l| l.contains("No settings directory")));
    }

    #[tokio::test]
    async fn test_migrate_settings_renames_files() {
        let tmp = tempfile::tempdir().unwrap();
        let settings_dir = tmp.path();
        tokio::fs::create_dir_all(settings_dir).await.unwrap();

        // Create old file
        tokio::fs::write(settings_dir.join("cline_mcp_settings.json"), "{}")
            .await
            .unwrap();

        let log = migrate_settings(settings_dir).await.unwrap();
        assert!(settings_dir.join("mcp_settings.json").exists());
        assert!(!settings_dir.join("cline_mcp_settings.json").exists());
        assert!(log.iter().any(|l| l.contains("Renamed")));
    }

    #[test]
    fn test_migrate_default_commands_removes_old() {
        let commands = vec![
            "npm install".to_string(),
            "npm test".to_string(),
            "cargo build".to_string(),
            "tsc".to_string(),
        ];
        let (filtered, removed) = migrate_default_commands(&commands);
        assert_eq!(removed, 3);
        assert_eq!(filtered, vec!["cargo build"]);
    }

    #[test]
    fn test_migrate_default_commands_no_changes() {
        let commands = vec!["cargo build".to_string(), "cargo test".to_string()];
        let (filtered, removed) = migrate_default_commands(&commands);
        assert_eq!(removed, 0);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_migrate_default_commands_case_insensitive() {
        let commands = vec!["NPM INSTALL".to_string(), "Npm Test".to_string()];
        let (filtered, removed) = migrate_default_commands(&commands);
        assert_eq!(removed, 2);
        assert!(filtered.is_empty());
    }
}

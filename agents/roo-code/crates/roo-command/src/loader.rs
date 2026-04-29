//! High-level command loading API.
//!
//! Maps to TypeScript source: `src/services/command/commands.ts`
//! (getCommands, getCommand, getCommandNames, tryLoadCommand)

use std::collections::HashMap;
use std::path::Path;

use roo_config::{get_global_roo_directory, get_project_roo_directory_for_cwd};

use crate::frontmatter::parse_command_content;
use crate::scanner::{scan_command_directory, try_resolve_symlinked_command};
use crate::types::{Command, CommandSource};

/// Get all available commands from built-in, global, and project directories.
///
/// Priority order: `project` > `global` > `built-in` (later sources override earlier ones).
pub async fn get_commands(cwd: &Path) -> Vec<Command> {
    let mut commands: HashMap<String, Command> = HashMap::new();

    // 1. Built-in commands (lowest priority)
    let built_in = get_built_in_commands();
    for cmd in built_in {
        commands.insert(cmd.name.clone(), cmd);
    }

    // 2. Global commands (override built-in)
    let global_dir = get_global_roo_directory().join("commands");
    scan_command_directory(&global_dir, CommandSource::Global, &mut commands).await;

    // 3. Project commands (highest priority — override both global and built-in)
    let project_dir = get_project_roo_directory_for_cwd(cwd).join("commands");
    scan_command_directory(&project_dir, CommandSource::Project, &mut commands).await;

    commands.into_values().collect()
}

/// Get a specific command by name.
///
/// Checks sources in priority order: `project` → `global` → `built-in`.
/// Returns `None` if no command with the given name exists.
pub async fn get_command(cwd: &Path, name: &str) -> Option<Command> {
    let project_dir = get_project_roo_directory_for_cwd(cwd).join("commands");
    let global_dir = get_global_roo_directory().join("commands");

    // Project (highest priority)
    if let Some(cmd) = try_load_command(&project_dir, name, CommandSource::Project).await {
        return Some(cmd);
    }

    // Global
    if let Some(cmd) = try_load_command(&global_dir, name, CommandSource::Global).await {
        return Some(cmd);
    }

    // Built-in (lowest priority)
    get_built_in_command(name)
}

/// Get command names for autocomplete.
pub async fn get_command_names(cwd: &Path) -> Vec<String> {
    let commands = get_commands(cwd).await;
    commands.into_iter().map(|cmd| cmd.name).collect()
}

/// Try to load a specific command from a directory (supports symlinks).
pub async fn try_load_command(
    dir_path: &Path,
    name: &str,
    source: CommandSource,
) -> Option<Command> {
    // Check directory exists
    let metadata = tokio::fs::metadata(dir_path).await.ok()?;
    if !metadata.is_dir() {
        return None;
    }

    let command_file_name = format!("{name}.md");
    let file_path = dir_path.join(&command_file_name);

    // Try reading the file directly
    let (resolved_path, content) = match tokio::fs::read_to_string(&file_path).await {
        Ok(c) => (file_path.clone(), c),
        Err(_) => {
            // Try resolving as symlink
            let symlinked_path = try_resolve_symlinked_command(&file_path).await?;
            let content = tokio::fs::read_to_string(&symlinked_path).await.ok()?;
            (symlinked_path, content)
        }
    };

    let parsed = parse_command_content(&content);

    Some(Command {
        name: name.to_string(),
        content: parsed.body,
        source,
        file_path: resolved_path,
        description: parsed.frontmatter.description,
        argument_hint: parsed.frontmatter.argument_hint,
        mode: parsed.frontmatter.mode,
    })
}

// ---------------------------------------------------------------------------
// Built-in commands (placeholder)
// ---------------------------------------------------------------------------

/// Returns the list of built-in commands.
///
/// Currently returns an empty list. Built-in commands will be added in a
/// future iteration when the built-in command definitions are ported from
/// the TypeScript source.
pub fn get_built_in_commands() -> Vec<Command> {
    Vec::new()
}

/// Returns a built-in command by name.
///
/// Currently always returns `None`. See [`get_built_in_commands`].
pub fn get_built_in_command(_name: &str) -> Option<Command> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn create_md_file(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    #[tokio::test]
    async fn test_get_commands_empty() {
        let tmp = TempDir::new().unwrap();
        let cwd = tmp.path();

        // Create .roo/commands directory (empty)
        let project_commands = cwd.join(".roo").join("commands");
        fs::create_dir_all(&project_commands).unwrap();

        let commands = get_commands(cwd).await;
        // Built-in is empty, no global/project commands
        assert!(commands.is_empty());
    }

    #[tokio::test]
    async fn test_get_commands_from_project_dir() {
        let tmp = TempDir::new().unwrap();
        let cwd = tmp.path();

        let project_commands = cwd.join(".roo").join("commands");
        fs::create_dir_all(&project_commands).unwrap();
        create_md_file(&project_commands, "test-cmd.md", "---\ndescription: Test\n---\nTest body");

        let commands = get_commands(cwd).await;
        assert_eq!(commands.len(), 1);
        let cmd = &commands[0];
        assert_eq!(cmd.name, "test-cmd");
        assert_eq!(cmd.content, "Test body");
        assert_eq!(cmd.description.as_deref(), Some("Test"));
        assert_eq!(cmd.source, CommandSource::Project);
    }

    #[tokio::test]
    async fn test_get_command_by_name() {
        let tmp = TempDir::new().unwrap();
        let cwd = tmp.path();

        let project_commands = cwd.join(".roo").join("commands");
        fs::create_dir_all(&project_commands).unwrap();
        create_md_file(&project_commands, "find.md", "Find command body");

        let cmd = get_command(cwd, "find").await;
        assert!(cmd.is_some());
        assert_eq!(cmd.unwrap().content, "Find command body");
    }

    #[tokio::test]
    async fn test_get_command_not_found() {
        let tmp = TempDir::new().unwrap();
        let cwd = tmp.path();

        let project_commands = cwd.join(".roo").join("commands");
        fs::create_dir_all(&project_commands).unwrap();

        let cmd = get_command(cwd, "nonexistent").await;
        assert!(cmd.is_none());
    }

    #[tokio::test]
    async fn test_get_command_names() {
        let tmp = TempDir::new().unwrap();
        let cwd = tmp.path();

        let project_commands = cwd.join(".roo").join("commands");
        fs::create_dir_all(&project_commands).unwrap();
        create_md_file(&project_commands, "alpha.md", "Alpha");
        create_md_file(&project_commands, "beta.md", "Beta");

        let mut names = get_command_names(cwd).await;
        names.sort();
        assert_eq!(names, vec!["alpha", "beta"]);
    }

    #[tokio::test]
    async fn test_try_load_command_from_dir() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("commands");
        fs::create_dir_all(&dir).unwrap();
        create_md_file(&dir, "hello.md", "---\nmode: code\n---\nHello body");

        let cmd = try_load_command(&dir, "hello", CommandSource::Global).await;
        assert!(cmd.is_some());
        let cmd = cmd.unwrap();
        assert_eq!(cmd.name, "hello");
        assert_eq!(cmd.content, "Hello body");
        assert_eq!(cmd.mode.as_deref(), Some("code"));
        assert_eq!(cmd.source, CommandSource::Global);
    }

    #[tokio::test]
    async fn test_try_load_command_nonexistent() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("commands");
        fs::create_dir_all(&dir).unwrap();

        let cmd = try_load_command(&dir, "missing", CommandSource::Global).await;
        assert!(cmd.is_none());
    }

    #[tokio::test]
    async fn test_try_load_command_nonexistent_dir() {
        let cmd = try_load_command(
            Path::new("/nonexistent/dir"),
            "test",
            CommandSource::Global,
        )
        .await;
        assert!(cmd.is_none());
    }

    #[tokio::test]
    async fn test_built_in_commands_empty() {
        assert!(get_built_in_commands().is_empty());
        assert!(get_built_in_command("anything").is_none());
    }

    #[tokio::test]
    async fn test_project_overrides_global_in_get_commands() {
        let tmp = TempDir::new().unwrap();
        let cwd = tmp.path();

        // Set up global commands
        let _global_dir = roo_config::get_global_roo_directory().join("commands");

        // We don't want to write to the real global directory in tests,
        // so we just test with project commands overriding the concept.
        let project_commands = cwd.join(".roo").join("commands");
        fs::create_dir_all(&project_commands).unwrap();
        create_md_file(&project_commands, "override.md", "Project version");

        let commands = get_commands(cwd).await;
        let override_cmd = commands.iter().find(|c| c.name == "override");
        assert!(override_cmd.is_some());
        assert_eq!(override_cmd.unwrap().content, "Project version");
        assert_eq!(override_cmd.unwrap().source, CommandSource::Project);
    }

    #[tokio::test]
    async fn test_command_with_all_frontmatter_fields() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("commands");
        fs::create_dir_all(&dir).unwrap();
        create_md_file(
            &dir,
            "full.md",
            "---\ndescription: Full command\nargument-hint: <file>\nmode: architect\n---\nFull body",
        );

        let cmd = try_load_command(&dir, "full", CommandSource::Project)
            .await
            .unwrap();
        assert_eq!(cmd.description.as_deref(), Some("Full command"));
        assert_eq!(cmd.argument_hint.as_deref(), Some("<file>"));
        assert_eq!(cmd.mode.as_deref(), Some("architect"));
        assert_eq!(cmd.content, "Full body");
    }
}

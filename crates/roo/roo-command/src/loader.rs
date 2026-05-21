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
    let global_dir = get_global_roo_directory().join("commands");
    let project_dir = get_project_roo_directory_for_cwd(cwd).join("commands");

    get_commands_from_dirs(&global_dir, &project_dir).await
}

async fn get_commands_from_dirs(global_dir: &Path, project_dir: &Path) -> Vec<Command> {
    let mut commands: HashMap<String, Command> = HashMap::new();

    // 1. Built-in commands (lowest priority)
    let built_in = get_built_in_commands();
    for cmd in built_in {
        commands.insert(cmd.name.clone(), cmd);
    }

    // 2. Global commands (override built-in)
    scan_command_directory(global_dir, CommandSource::Global, &mut commands).await;

    // 3. Project commands (highest priority — override both global and built-in)
    scan_command_directory(project_dir, CommandSource::Project, &mut commands).await;

    commands.into_values().collect()
}

/// Get a specific command by name.
///
/// Checks sources in priority order: `project` → `global` → `built-in`.
/// Returns `None` if no command with the given name exists.
pub async fn get_command(cwd: &Path, name: &str) -> Option<Command> {
    let project_dir = get_project_roo_directory_for_cwd(cwd).join("commands");
    let global_dir = get_global_roo_directory().join("commands");

    get_command_from_dirs(&global_dir, &project_dir, name).await
}

async fn get_command_from_dirs(
    global_dir: &Path,
    project_dir: &Path,
    name: &str,
) -> Option<Command> {
    // Project (highest priority)
    if let Some(cmd) = try_load_command(project_dir, name, CommandSource::Project).await {
        return Some(cmd);
    }

    // Global
    if let Some(cmd) = try_load_command(global_dir, name, CommandSource::Global).await {
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
// Built-in commands
// Source: `src/services/command/built-in-commands.ts` — `BUILT_IN_COMMANDS`
// ---------------------------------------------------------------------------

/// The `/init` built-in command — analyzes codebase and creates AGENTS.md.
const BUILTIN_INIT_NAME: &str = "init";
const BUILTIN_INIT_DESCRIPTION: &str =
    "Analyze codebase and create concise AGENTS.md files for AI assistants";
const BUILTIN_INIT_CONTENT: &str = r#"<task>
Please analyze this codebase and create an AGENTS.md file containing:
1. Build/lint/test commands - especially for running a single test
2. Code style guidelines including imports, formatting, types, naming conventions, error handling, etc.
</task>

<initialization>
  <purpose>
    Create (or update) a concise AGENTS.md file that enables immediate productivity for AI assistants.
    Focus ONLY on project-specific, non-obvious information that you had to discover by reading files.
  </purpose>
</initialization>

<analysis_workflow>
  Follow the comprehensive analysis workflow to:
  1. Discovery Phase — check for existing AGENTS.md and other AI assistant rules
  2. Project Identification — identify language, stack, and build system
  3. Command Extraction — extract and verify essential commands
  4. Architecture Mapping — map core processes
  5. Component Analysis — document key components and interactions
  6. Pattern Analysis — identify project-specific patterns
  7. Code Style Extraction — extract formatting and naming conventions
  8. Testing Discovery — understand testing setup
</analysis_workflow>

<output_structure>
  Create or update AGENTS.md in the project root with ONLY non-obvious information.
  Include existing AI assistant rules from CLAUDE.md, .cursorrules, or .github/copilot-instructions.md.
  Keep it concise (~20 lines). Every line should prevent a potential mistake.
</output_structure>

<quality_criteria>
  - ONLY include non-obvious information discovered by reading files
  - Exclude anything derivable from standard practices
  - Focus on gotchas, hidden requirements, and counterintuitive patterns
  - Be extremely concise - if it's obvious, don't include it
</quality_criteria>
"#;

/// Returns the list of built-in commands.
///
/// Source: `src/services/command/built-in-commands.ts` — `getBuiltInCommands`
pub fn get_built_in_commands() -> Vec<Command> {
    vec![Command {
        name: BUILTIN_INIT_NAME.to_string(),
        content: BUILTIN_INIT_CONTENT.to_string(),
        source: CommandSource::BuiltIn,
        file_path: std::path::PathBuf::from(format!("<built-in:{BUILTIN_INIT_NAME}>")),
        description: Some(BUILTIN_INIT_DESCRIPTION.to_string()),
        argument_hint: None,
        mode: None,
    }]
}

/// Returns a built-in command by name.
///
/// Source: `src/services/command/built-in-commands.ts` — `getBuiltInCommand`
pub fn get_built_in_command(name: &str) -> Option<Command> {
    get_built_in_commands().into_iter().find(|c| c.name == name)
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

    fn command_dirs(tmp: &TempDir) -> (PathBuf, PathBuf) {
        let global = tmp.path().join("global").join("commands");
        let project = tmp.path().join("project").join(".roo").join("commands");
        fs::create_dir_all(&global).unwrap();
        fs::create_dir_all(&project).unwrap();
        (global, project)
    }

    fn command_by_name<'a>(commands: &'a [Command], name: &str) -> &'a Command {
        commands
            .iter()
            .find(|cmd| cmd.name == name)
            .unwrap_or_else(|| panic!("missing command {name}"))
    }

    #[tokio::test]
    async fn test_get_commands_empty() {
        let tmp = TempDir::new().unwrap();
        let (global_commands, project_commands) = command_dirs(&tmp);

        let commands = get_commands_from_dirs(&global_commands, &project_commands).await;
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].name, "init");
        assert_eq!(commands[0].source, CommandSource::BuiltIn);
    }

    #[tokio::test]
    async fn test_get_commands_from_project_dir() {
        let tmp = TempDir::new().unwrap();
        let (global_commands, project_commands) = command_dirs(&tmp);
        create_md_file(
            &project_commands,
            "test-cmd.md",
            "---\ndescription: Test\n---\nTest body",
        );

        let commands = get_commands_from_dirs(&global_commands, &project_commands).await;
        assert_eq!(commands.len(), 2);
        let cmd = command_by_name(&commands, "test-cmd");
        assert_eq!(cmd.name, "test-cmd");
        assert_eq!(cmd.content, "Test body");
        assert_eq!(cmd.description.as_deref(), Some("Test"));
        assert_eq!(cmd.source, CommandSource::Project);
    }

    #[tokio::test]
    async fn test_get_command_by_name() {
        let tmp = TempDir::new().unwrap();
        let (global_commands, project_commands) = command_dirs(&tmp);
        create_md_file(&project_commands, "find.md", "Find command body");

        let cmd = get_command_from_dirs(&global_commands, &project_commands, "find").await;
        assert!(cmd.is_some());
        assert_eq!(cmd.unwrap().content, "Find command body");
    }

    #[tokio::test]
    async fn test_get_command_not_found() {
        let tmp = TempDir::new().unwrap();
        let (global_commands, project_commands) = command_dirs(&tmp);

        let cmd = get_command_from_dirs(&global_commands, &project_commands, "nonexistent").await;
        assert!(cmd.is_none());
    }

    #[tokio::test]
    async fn test_get_command_names() {
        let tmp = TempDir::new().unwrap();
        let (global_commands, project_commands) = command_dirs(&tmp);
        create_md_file(&project_commands, "alpha.md", "Alpha");
        create_md_file(&project_commands, "beta.md", "Beta");

        let mut names = get_commands_from_dirs(&global_commands, &project_commands)
            .await
            .into_iter()
            .map(|cmd| cmd.name)
            .collect::<Vec<_>>();
        names.sort();
        assert_eq!(names, vec!["alpha", "beta", "init"]);
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
        let cmd =
            try_load_command(Path::new("/nonexistent/dir"), "test", CommandSource::Global).await;
        assert!(cmd.is_none());
    }

    #[tokio::test]
    async fn test_built_in_commands_empty() {
        let commands = get_built_in_commands();
        assert_eq!(commands.len(), 1);
        let init = &commands[0];
        assert_eq!(init.name, "init");
        assert_eq!(init.source, CommandSource::BuiltIn);
        assert_eq!(init.description.as_deref(), Some(BUILTIN_INIT_DESCRIPTION));
        assert!(init.content.contains("<analysis_workflow>"));
        assert!(get_built_in_command("anything").is_none());
        assert!(get_built_in_command("init").is_some());
    }

    #[tokio::test]
    async fn test_project_overrides_global_in_get_commands() {
        let tmp = TempDir::new().unwrap();
        let (global_commands, project_commands) = command_dirs(&tmp);
        create_md_file(&global_commands, "override.md", "Global version");
        create_md_file(&project_commands, "override.md", "Project version");

        let commands = get_commands_from_dirs(&global_commands, &project_commands).await;
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

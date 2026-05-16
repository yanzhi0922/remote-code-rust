//! run_slash_command tool implementation.
//!
//! Source: `src/core/tools/RunSlashCommandTool.ts`
//!
//! Provides standalone functions for resolving and executing slash commands.
//! Tries to resolve the command from the skills manager (checking if the
//! command name maps to a skill). The experiment gating from the TS version
//! is skipped — commands are always allowed in this Rust port.

use serde::{Deserialize, Serialize};

use roo_skills::SkillsManager;

/// Parameters for the `run_slash_command` tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSlashCommandParams {
    /// The slash command name (without the leading `/`).
    pub command: String,
    /// Optional arguments to pass to the command.
    #[serde(default)]
    pub args: Option<String>,
}

/// The result of executing a slash command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSlashCommandResult {
    /// The command name that was executed.
    pub command: String,
    /// Optional description from the command's frontmatter.
    pub description: Option<String>,
    /// Optional argument hint from the command's frontmatter.
    pub argument_hint: Option<String>,
    /// Optional mode that the command wants to switch to.
    pub mode: Option<String>,
    /// Arguments provided by the user.
    pub args: Option<String>,
    /// Where the command was loaded from (project, global, built-in).
    pub source: Option<String>,
    /// The resolved content of the command.
    pub content: String,
}

/// Validate `run_slash_command` parameters.
///
/// Returns an error message if the `command` field is missing or empty.
pub fn validate_run_slash_command_params(params: &RunSlashCommandParams) -> Result<(), String> {
    if params.command.trim().is_empty() {
        return Err("command must not be empty".to_string());
    }
    Ok(())
}

/// Strip a leading `/` from a command name, if present.
fn normalize_command_name(command: &str) -> &str {
    command.strip_prefix('/').unwrap_or(command)
}

/// Known built-in slash commands that are always recognized.
const BUILTIN_COMMANDS: &[&str] = &[
    "new",
    "mode",
    "help",
    "init",
    "review",
    "security-review",
    "commit",
    "clear",
];

/// Resolve a slash command and return a formatted result string.
///
/// Mirrors the behavior of the TS `RunSlashCommandTool.execute()`:
/// 1. Validates the command format.
/// 2. Strips a leading `/` from the command name.
/// 3. If a skills manager is provided, tries to resolve the command as a
///    skill and returns the skill content.
/// 4. If the command is a known built-in, returns a minimal result.
/// 5. Otherwise returns an error listing available commands and skills.
pub fn resolve_slash_command(
    params: &RunSlashCommandParams,
    skills_manager: Option<&SkillsManager>,
    current_mode: Option<&str>,
) -> Result<String, String> {
    validate_run_slash_command_params(params)?;

    let command_name = normalize_command_name(&params.command);

    // 1. Try to resolve from the skills manager
    if let Some(manager) = skills_manager {
        let all_skills = manager.get_all_skills();

        // Filter to skills available in the current mode if specified
        let relevant_skills: Vec<_> = if let Some(mode) = current_mode {
            all_skills
                .into_iter()
                .filter(|s| SkillsManager::is_skill_available_in_mode(s, mode))
                .collect()
        } else {
            all_skills
        };

        // Try to find a skill matching the command name
        if let Some(skill) = relevant_skills.iter().find(|s| s.name == command_name) {
            // Try to read the skill's SKILL.md synchronously
            let skill_md_path = std::path::Path::new(&skill.path).join("SKILL.md");
            if let Ok(file_content) = std::fs::read_to_string(&skill_md_path)
                && let Some((_frontmatter, instructions)) =
                    roo_skills::frontmatter::parse_skill_md(&file_content)
            {
                let content = if let Some(args) = &params.args {
                    format!("{}\n\nContext: {}", instructions, args)
                } else {
                    instructions
                };

                let result = RunSlashCommandResult {
                    command: command_name.to_string(),
                    description: Some(skill.description.clone()),
                    argument_hint: None,
                    mode: None,
                    args: params.args.clone(),
                    source: Some(format!("{:?}", skill.source).to_lowercase()),
                    content,
                };
                return Ok(format_slash_command_result(&result));
            }
        }

        // 2. Check built-in commands
        if BUILTIN_COMMANDS.contains(&command_name) {
            let result = RunSlashCommandResult {
                command: command_name.to_string(),
                description: Some(format!("Built-in command: /{}", command_name)),
                argument_hint: None,
                mode: None,
                args: params.args.clone(),
                source: Some("built-in".to_string()),
                content: format!("Execute built-in slash command /{}", command_name),
            };
            return Ok(format_slash_command_result(&result));
        }

        // 3. Not found — list available commands and skills
        let skill_names: Vec<&str> = relevant_skills.iter().map(|s| s.name.as_str()).collect();
        let mut available: Vec<String> =
            BUILTIN_COMMANDS.iter().map(|s| format!("/{}", s)).collect();
        available.extend(skill_names.iter().map(|s| format!("/{}", s)));
        let available_str = if available.is_empty() {
            "(none)".to_string()
        } else {
            available.join(", ")
        };

        return Err(format!(
            "Command '{}' not found. Available commands: {}",
            command_name, available_str
        ));
    }

    // No skills manager — check built-in commands
    if BUILTIN_COMMANDS.contains(&command_name) {
        let result = RunSlashCommandResult {
            command: command_name.to_string(),
            description: Some(format!("Built-in command: /{}", command_name)),
            argument_hint: None,
            mode: None,
            args: params.args.clone(),
            source: Some("built-in".to_string()),
            content: format!("Execute built-in slash command /{}", command_name),
        };
        return Ok(format_slash_command_result(&result));
    }

    // No manager and not a built-in — cannot resolve
    let builtins = BUILTIN_COMMANDS
        .iter()
        .map(|s| format!("/{}", s))
        .collect::<Vec<_>>()
        .join(", ");

    Err(format!(
        "Command '{}' not found. Known built-in commands: {}. Skills system is not available to check for skill-based commands.",
        command_name, builtins
    ))
}

/// Build the human-readable result string for a slash command.
///
/// Mirrors the format produced by the TS `RunSlashCommandTool.execute()`.
pub fn format_slash_command_result(result: &RunSlashCommandResult) -> String {
    let mut output = format!("Command: /{}", result.command);

    if let Some(desc) = &result.description {
        output.push_str(&format!("\nDescription: {}", desc));
    }

    if let Some(hint) = &result.argument_hint {
        output.push_str(&format!("\nArgument hint: {}", hint));
    }

    if let Some(mode) = &result.mode {
        output.push_str(&format!("\nMode: {}", mode));
    }

    if let Some(args) = &result.args {
        output.push_str(&format!("\nProvided arguments: {}", args));
    }

    if let Some(source) = &result.source {
        output.push_str(&format!("\nSource: {}", source));
    }

    output.push_str(&format!(
        "\n\n--- Command Content ---\n\n{}",
        result.content
    ));

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_valid_params() {
        let params = RunSlashCommandParams {
            command: "init".to_string(),
            args: None,
        };
        assert!(validate_run_slash_command_params(&params).is_ok());
    }

    #[test]
    fn test_validate_valid_params_with_args() {
        let params = RunSlashCommandParams {
            command: "test".to_string(),
            args: Some("my-module".to_string()),
        };
        assert!(validate_run_slash_command_params(&params).is_ok());
    }

    #[test]
    fn test_validate_empty_command() {
        let params = RunSlashCommandParams {
            command: "".to_string(),
            args: None,
        };
        assert!(validate_run_slash_command_params(&params).is_err());
    }

    #[test]
    fn test_validate_whitespace_command() {
        let params = RunSlashCommandParams {
            command: "   ".to_string(),
            args: None,
        };
        assert!(validate_run_slash_command_params(&params).is_err());
    }

    #[test]
    fn test_format_result_full() {
        let result = RunSlashCommandResult {
            command: "init".to_string(),
            description: Some("Initialize project".to_string()),
            argument_hint: Some("<template>".to_string()),
            mode: Some("code".to_string()),
            args: Some("react".to_string()),
            source: Some("project".to_string()),
            content: "Create a new project from template.".to_string(),
        };
        let output = format_slash_command_result(&result);
        assert!(output.contains("Command: /init"));
        assert!(output.contains("Description: Initialize project"));
        assert!(output.contains("Argument hint: <template>"));
        assert!(output.contains("Mode: code"));
        assert!(output.contains("Provided arguments: react"));
        assert!(output.contains("Source: project"));
        assert!(output.contains("--- Command Content ---"));
        assert!(output.contains("Create a new project from template."));
    }

    #[test]
    fn test_format_result_minimal() {
        let result = RunSlashCommandResult {
            command: "build".to_string(),
            description: None,
            argument_hint: None,
            mode: None,
            args: None,
            source: None,
            content: "cargo build".to_string(),
        };
        let output = format_slash_command_result(&result);
        assert!(output.contains("Command: /build"));
        assert!(!output.contains("Description:"));
        assert!(!output.contains("Argument hint:"));
        assert!(output.contains("--- Command Content ---"));
        assert!(output.contains("cargo build"));
    }

    #[test]
    fn test_params_serde_roundtrip() {
        let params = RunSlashCommandParams {
            command: "deploy".to_string(),
            args: Some("--production".to_string()),
        };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: RunSlashCommandParams = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.command, "deploy");
        assert_eq!(parsed.args, Some("--production".to_string()));
    }

    #[test]
    fn test_params_serde_missing_args() {
        let json = r#"{"command": "test"}"#;
        let parsed: RunSlashCommandParams = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.command, "test");
        assert_eq!(parsed.args, None);
    }

    // ---- resolve_slash_command tests ----

    #[test]
    fn test_resolve_builtin_new() {
        let params = RunSlashCommandParams {
            command: "new".to_string(),
            args: None,
        };
        let result = resolve_slash_command(&params, None, None);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("Command: /new"));
    }

    #[test]
    fn test_resolve_builtin_with_leading_slash() {
        let params = RunSlashCommandParams {
            command: "/help".to_string(),
            args: None,
        };
        let result = resolve_slash_command(&params, None, None);
        assert!(result.is_ok());
        assert!(result.unwrap().contains("Command: /help"));
    }

    #[test]
    fn test_resolve_unknown_command_no_manager() {
        let params = RunSlashCommandParams {
            command: "unknown-cmd".to_string(),
            args: None,
        };
        let result = resolve_slash_command(&params, None, None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("not found"));
        assert!(err.contains("/new"));
    }

    #[test]
    fn test_resolve_unknown_command_with_empty_manager() {
        let manager = SkillsManager::new();
        let params = RunSlashCommandParams {
            command: "nonexistent".to_string(),
            args: None,
        };
        let result = resolve_slash_command(&params, Some(&manager), None);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("not found"));
        assert!(err.contains("Available commands"));
    }

    #[test]
    fn test_resolve_builtin_with_manager() {
        let manager = SkillsManager::new();
        let params = RunSlashCommandParams {
            command: "init".to_string(),
            args: Some("react".to_string()),
        };
        let result = resolve_slash_command(&params, Some(&manager), None);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("Command: /init"));
        assert!(output.contains("Provided arguments: react"));
    }

    #[test]
    fn test_normalize_command_name() {
        assert_eq!(normalize_command_name("help"), "help");
        assert_eq!(normalize_command_name("/help"), "help");
        assert_eq!(normalize_command_name("//double"), "/double");
    }
}

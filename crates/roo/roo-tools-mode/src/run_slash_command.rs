//! run_slash_command tool implementation.
//!
//! Source: `src/core/tools/RunSlashCommandTool.ts`
//!
//! Provides a standalone function for executing slash commands.
//! The experiment gating from the TS version is skipped — commands
//! are always allowed in this Rust port.

use serde::{Deserialize, Serialize};

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
pub fn validate_run_slash_command_params(
    params: &RunSlashCommandParams,
) -> Result<(), String> {
    if params.command.trim().is_empty() {
        return Err("command must not be empty".to_string());
    }
    Ok(())
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

    output.push_str(&format!("\n\n--- Command Content ---\n\n{}", result.content));

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
}

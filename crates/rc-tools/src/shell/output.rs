use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::semantics::ShellCommandAnalysis;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellOutputSummary {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub artifact_path: Option<PathBuf>,
}

#[must_use]
pub fn truncate_output(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}\n...[truncated]")
    } else {
        truncated
    }
}

pub fn persist_shell_output(
    output_dir: Option<&Path>,
    file_stem: &str,
    contents: &str,
) -> Result<Option<PathBuf>> {
    let Some(output_dir) = output_dir else {
        return Ok(None);
    };
    fs::create_dir_all(output_dir)
        .with_context(|| format!("failed to create {}", output_dir.display()))?;
    let path = output_dir.join(format!("{file_stem}.log"));
    fs::write(&path, contents).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(Some(path))
}

#[must_use]
pub fn format_shell_result(
    command: &str,
    description: Option<&str>,
    cwd: &Path,
    analysis: &ShellCommandAnalysis,
    summary: &ShellOutputSummary,
) -> String {
    let mut sections = vec![
        format!("command: {command}"),
        format!("cwd: {}", cwd.display()),
        format!("semantic: {:?}", analysis.semantic).to_ascii_lowercase(),
        format!("read_only: {}", analysis.read_only),
    ];
    if let Some(description) = description.filter(|value| !value.trim().is_empty()) {
        sections.push(format!("description: {}", description.trim()));
    }
    if let Some(exit_code) = summary.exit_code {
        sections.push(format!("exit_code: {exit_code}"));
    }
    if summary.timed_out {
        sections.push("timed_out: true".to_owned());
    }
    if let Some(path) = &summary.artifact_path {
        sections.push(format!("artifact: {}", path.display()));
    }
    if !summary.stdout.trim().is_empty() {
        sections.push(format!("stdout:\n{}", summary.stdout.trim_end()));
    }
    if !summary.stderr.trim().is_empty() {
        sections.push(format!("stderr:\n{}", summary.stderr.trim_end()));
    }
    sections.join("\n\n")
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{ShellOutputSummary, format_shell_result, truncate_output};
    use crate::shell::semantics::{ShellCommandAnalysis, ShellCommandSemantic};

    #[test]
    fn truncate_output_marks_large_content() {
        let value = "hello world";
        assert!(truncate_output(value, 5).contains("[truncated]"));
    }

    #[test]
    fn format_shell_result_includes_core_fields() {
        let analysis = ShellCommandAnalysis {
            semantic: ShellCommandSemantic::ReadOnly,
            read_only: true,
            background: false,
            destructive_git: false,
            dangerous: false,
            changes_directory: false,
        };
        let summary = ShellOutputSummary {
            exit_code: Some(0),
            stdout: "ok".to_owned(),
            stderr: String::new(),
            timed_out: false,
            artifact_path: Some(PathBuf::from("artifact.log")),
        };
        let rendered = format_shell_result(
            "pwd",
            Some("show current directory"),
            PathBuf::from(".").as_path(),
            &analysis,
            &summary,
        );
        assert!(rendered.contains("command: pwd"));
        assert!(rendered.contains("description: show current directory"));
        assert!(rendered.contains("artifact: artifact.log"));
    }
}

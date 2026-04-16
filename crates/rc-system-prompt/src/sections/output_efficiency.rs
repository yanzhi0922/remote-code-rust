//! Output Efficiency section — guidance on concise communication.
//!
//! Matches `getOutputEfficiencySection()` in Claude Code's `prompts.ts`.

use anyhow::Result;

use crate::PromptContext;
use crate::sections::SystemPromptSection;

/// The output efficiency section.
pub struct OutputEfficiencySection;

impl SystemPromptSection for OutputEfficiencySection {
    fn name(&self) -> &str {
        "output_efficiency"
    }

    fn compute(&self, _ctx: &PromptContext) -> Result<Option<String>> {
        Ok(Some(
            "# Output efficiency\n\n\
            IMPORTANT: Go straight to the point. Try the simplest approach first without going in circles. Do not overdo it. Be extra concise.\n\n\
            Keep your text output brief and direct. Lead with the answer or action, not the reasoning. \
            Skip filler words, preamble, and unnecessary transitions. Do not restate what the user said \u{2014} just do it. \
            When explaining, include only what is necessary for the user to understand.\n\n\
            Focus text output on:\n\
            - Decisions that need the user's input\n\
            - High-level status updates at natural milestones\n\
            - Errors or blockers that change the plan\n\n\
            If you can say it in one sentence, don't use three. Prefer short, direct sentences over long explanations. \
            This does not apply to code or tool calls.".to_string()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::PathBuf;

    fn test_ctx() -> PromptContext {
        PromptContext {
            model: "test".to_string(),
            cwd: PathBuf::from("/tmp"),
            is_git: false,
            platform: "linux".to_string(),
            shell: "bash".to_string(),
            os_version: "Linux 6.6".to_string(),
            enabled_tools: HashSet::new(),
            language: None,
            output_style: None,
            mcp_clients: vec![],
            is_worktree: false,
            additional_dirs: vec![],
            is_non_interactive: false,
            is_fork_subagent_enabled: false,
            session_start_date: "2025-01-01".to_string(),
        }
    }

    #[test]
    fn output_efficiency_starts_with_header() {
        let section = OutputEfficiencySection;
        let result = section.compute(&test_ctx()).expect("compute ok");
        let content = result.expect("should be Some");
        assert!(content.starts_with("# Output efficiency"));
    }

    #[test]
    fn output_efficiency_mentions_concise() {
        let section = OutputEfficiencySection;
        let result = section.compute(&test_ctx()).expect("compute ok");
        let content = result.expect("should be Some");
        assert!(content.contains("Go straight to the point"));
        assert!(content.contains("extra concise"));
    }

    #[test]
    fn output_efficiency_mentions_focus_areas() {
        let section = OutputEfficiencySection;
        let result = section.compute(&test_ctx()).expect("compute ok");
        let content = result.expect("should be Some");
        assert!(content.contains("Decisions that need the user's input"));
        assert!(content.contains("Errors or blockers"));
    }
}

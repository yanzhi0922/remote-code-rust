//! Scratchpad Directory section — temporary file guidance.
//!
//! Matches `getScratchpadInstructions()` in Claude Code's `prompts.ts`.

use anyhow::Result;

use crate::PromptContext;
use crate::sections::SystemPromptSection;

/// The scratchpad directory section.
///
/// Returns instructions for using the scratchpad directory when enabled.
/// The scratchpad path is determined from the PromptContext.
pub struct ScratchpadSection;

impl SystemPromptSection for ScratchpadSection {
    fn name(&self) -> &str {
        "scratchpad"
    }

    fn compute(&self, _ctx: &PromptContext) -> Result<Option<String>> {
        // In the full implementation, this checks isScratchpadEnabled() and
        // getScratchpadDir(). For now, return None since the scratchpad
        // filesystem permissions module is not yet wired in.
        //
        // When integrated, the content would be:
        // # Scratchpad Directory
        // IMPORTANT: Always use this scratchpad directory for temporary files...
        Ok(None)
    }
}

/// Build the scratchpad instructions content for a given directory path.
/// Public so it can be used when the scratchpad path is known.
pub fn build_scratchpad_instructions(scratchpad_dir: &str) -> String {
    format!(
        "# Scratchpad Directory\n\n\
        IMPORTANT: Always use this scratchpad directory for temporary files instead of `/tmp` or other system temp directories:\n\
        `{scratchpad_dir}`\n\n\
        Use this directory for ALL temporary file needs:\n\
        - Storing intermediate results or data during multi-step tasks\n\
        - Writing temporary scripts or configuration files\n\
        - Saving outputs that don't belong in the user's project\n\
        - Creating working files during analysis or processing\n\
        - Any file that would otherwise go to `/tmp`\n\n\
        Only use `/tmp` if the user explicitly requests it.\n\n\
        The scratchpad directory is session-specific, isolated from the user's project, and can be used freely without permission prompts."
    )
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
    fn scratchpad_returns_none_by_default() {
        let section = ScratchpadSection;
        let result = section.compute(&test_ctx()).expect("compute ok");
        assert!(result.is_none());
    }

    #[test]
    fn build_scratchpad_instructions_format() {
        let content = build_scratchpad_instructions("/tmp/scratch-abc123");
        assert!(content.starts_with("# Scratchpad Directory"));
        assert!(content.contains("/tmp/scratch-abc123"));
        assert!(content.contains("session-specific"));
    }

    #[test]
    fn build_scratchpad_mentions_tmp_replacement() {
        let content = build_scratchpad_instructions("/some/dir");
        assert!(content.contains("instead of `/tmp`"));
    }
}

//! Memory section — MEMORY.md auto-load content.
//!
//! Matches `loadMemoryPrompt()` integration in Claude Code's `prompts.ts`.

use anyhow::Result;

use crate::PromptContext;
use crate::sections::SystemPromptSection;

/// The memory section.
///
/// In the full implementation, this would load content from MEMORY.md files.
/// For now, it returns `None` unless memory content is provided in the context.
pub struct MemorySection;

impl SystemPromptSection for MemorySection {
    fn name(&self) -> &str {
        "memory"
    }

    fn compute(&self, _ctx: &PromptContext) -> Result<Option<String>> {
        // In the full implementation, this would:
        // 1. Walk up from cwd looking for MEMORY.md / CLAUDE.md files
        // 2. Load and concatenate their contents
        // 3. Return the combined memory prompt
        //
        // For now, return None (no memory files loaded).
        // The actual file loading will be wired in when rc-session integration
        // provides the memory content via PromptContext.
        Ok(None)
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
    fn memory_section_returns_none_by_default() {
        let section = MemorySection;
        let result = section.compute(&test_ctx()).expect("compute ok");
        assert!(result.is_none());
    }

    #[test]
    fn memory_section_name() {
        let section = MemorySection;
        assert_eq!(section.name(), "memory");
    }
}

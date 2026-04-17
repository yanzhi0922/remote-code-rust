//! Hooks section — informs the model about user-configurable hooks.
//!
//! Matches `getHooksSection()` in Claude Code's `prompts.ts`.
//! Always included (non-conditional).

use anyhow::Result;

use crate::PromptContext;
use crate::sections::{BulletItem, SystemPromptSection, section_with_bullets};

/// The hooks section.
///
/// Informs the model that users may configure hooks that run before or
/// after tool use, and that any feedback from hooks should be incorporated
/// into the model's behavior.
pub struct HooksSection;

impl SystemPromptSection for HooksSection {
    fn name(&self) -> &str {
        "hooks"
    }

    fn compute(&self, _ctx: &PromptContext) -> Result<Option<String>> {
        let items = vec![
            BulletItem::Single(
                "Users may configure hooks that run before or after tool use. These hooks allow users to customize behavior around tool execution."
                    .to_string(),
            ),
            BulletItem::Single(
                "If a hook provides feedback or modifies a tool result, incorporate that feedback into your response and adjust your approach accordingly."
                    .to_string(),
            ),
            BulletItem::Single(
                "Hooks may block a tool call, transform its output, or provide additional context. Respect hook decisions and do not attempt to bypass them."
                    .to_string(),
            ),
        ];

        Ok(Some(section_with_bullets("Hooks", &items)))
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
    fn hooks_section_always_included() {
        let section = HooksSection;
        let result = section.compute(&test_ctx()).expect("compute ok");
        assert!(result.is_some(), "hooks section should always be Some");
    }

    #[test]
    fn hooks_section_mentions_hook_feedback() {
        let section = HooksSection;
        let result = section.compute(&test_ctx()).expect("compute ok");
        let content = result.expect("should be Some");
        assert!(content.starts_with("# Hooks"));
        assert!(content.contains("hooks that run before or after tool use"));
        assert!(content.contains("incorporate that feedback"));
    }

    #[test]
    fn hooks_section_name() {
        let section = HooksSection;
        assert_eq!(section.name(), "hooks");
    }
}

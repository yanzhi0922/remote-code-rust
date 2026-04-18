//! System Reminders section — explains `<system-reminder>` tags and auto-summarization.
//!
//! Matches `getSystemRemindersSection()` in Claude Code's `prompts.ts`.
//! Always included (non-conditional).

use anyhow::Result;

use crate::PromptContext;
use crate::sections::{BulletItem, SystemPromptSection, section_with_bullets};

/// The system reminders section.
///
/// Explains to the model that:
/// - `<system-reminder>` tags may appear in the conversation
/// - These are automatically inserted to provide context
/// - The system automatically summarizes old messages to maintain
///   unlimited context length
pub struct SystemRemindersSection;

impl SystemPromptSection for SystemRemindersSection {
    fn name(&self) -> &str {
        "system_reminders"
    }

    fn compute(&self, _ctx: &PromptContext) -> Result<Option<String>> {
        let items = vec![
            BulletItem::Single(
                "You may see <system-reminder> tags in your conversation. These are automatically inserted by the system to provide additional context and instructions."
                    .to_string(),
            ),
            BulletItem::Single(
                "Treat the content within <system-reminder> tags as important system-level guidance that should be followed."
                    .to_string(),
            ),
            BulletItem::Single(
                "The system automatically summarizes old messages to maintain unlimited context length. Summarized messages preserve key information while reducing token usage."
                    .to_string(),
            ),
        ];

        Ok(Some(section_with_bullets("System Reminders", &items)))
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
            mcp_instructions_delta_enabled: false,
            is_worktree: false,
            additional_dirs: vec![],
            is_non_interactive: false,
            is_fork_subagent_enabled: false,
            session_start_date: "2025-01-01".to_string(),
        }
    }

    #[test]
    fn system_reminders_always_included() {
        let section = SystemRemindersSection;
        let result = section.compute(&test_ctx()).expect("compute ok");
        assert!(
            result.is_some(),
            "system reminders section should always be Some"
        );
    }

    #[test]
    fn system_reminders_mentions_system_reminder_tags() {
        let section = SystemRemindersSection;
        let result = section.compute(&test_ctx()).expect("compute ok");
        let content = result.expect("should be Some");
        assert!(content.starts_with("# System Reminders"));
        assert!(content.contains("<system-reminder>"));
        assert!(content.contains("automatically inserted"));
        assert!(content.contains("automatically summarizes"));
    }

    #[test]
    fn system_reminders_section_name() {
        let section = SystemRemindersSection;
        assert_eq!(section.name(), "system_reminders");
    }
}

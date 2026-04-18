//! Token Budget section — guidance for managing token budgets.
//!
//! Matches the feature-gated token budget section in Claude Code's `prompts.ts`.
//! Only included when `is_non_interactive` is true (e.g. headless / CI mode).

use anyhow::Result;

use crate::PromptContext;
use crate::sections::{BulletItem, SystemPromptSection, section_with_bullets};

/// The token budget section.
///
/// Informs the model that it has a token budget and should prioritize
/// the most important information, be concise, and summarize when
/// approaching limits.
pub struct TokenBudgetSection;

impl SystemPromptSection for TokenBudgetSection {
    fn name(&self) -> &str {
        "token_budget"
    }

    fn compute(&self, ctx: &PromptContext) -> Result<Option<String>> {
        // Only include this section in non-interactive sessions
        // where token budgets are most relevant.
        if !ctx.is_non_interactive {
            return Ok(None);
        }

        let items = vec![
            BulletItem::Single(
                "You have a token budget for this task. Prioritize the most important information."
                    .to_string(),
            ),
            BulletItem::Single(
                "Be concise in tool outputs. Avoid unnecessary verbosity in responses."
                    .to_string(),
            ),
            BulletItem::Single(
                "Summarize when approaching limits. If you are running low on tokens, summarize key findings rather than producing full output."
                    .to_string(),
            ),
        ];

        Ok(Some(section_with_bullets("Token Budget", &items)))
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
    fn token_budget_omitted_in_interactive_mode() {
        let section = TokenBudgetSection;
        let result = section.compute(&test_ctx()).expect("compute ok");
        assert!(result.is_none(), "should be None in interactive mode");
    }

    #[test]
    fn token_budget_included_in_non_interactive_mode() {
        let mut ctx = test_ctx();
        ctx.is_non_interactive = true;
        let section = TokenBudgetSection;
        let result = section.compute(&ctx).expect("compute ok");
        let content = result.expect("should be Some in non-interactive mode");
        assert!(content.starts_with("# Token Budget"));
        assert!(content.contains("token budget"));
        assert!(content.contains("Prioritize"));
        assert!(content.contains("concise"));
        assert!(content.contains("Summarize"));
    }

    #[test]
    fn token_budget_section_name() {
        let section = TokenBudgetSection;
        assert_eq!(section.name(), "token_budget");
    }
}

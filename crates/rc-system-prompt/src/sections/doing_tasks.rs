//! Doing Tasks section — guidelines for performing software engineering tasks.
//!
//! Matches `getSimpleDoingTasksSection()` in Claude Code's `prompts.ts`.

use anyhow::Result;

use crate::PromptContext;
use crate::sections::{BulletItem, SystemPromptSection, section_with_bullets};

/// Tool name constants (matching Claude Code's tool names).
const ASK_USER_QUESTION_TOOL_NAME: &str = "AskUserQuestion";

/// The "Doing tasks" section with 12+ guidelines.
pub struct DoingTasksSection;

impl SystemPromptSection for DoingTasksSection {
    fn name(&self) -> &str {
        "doing_tasks"
    }

    fn compute(&self, ctx: &PromptContext) -> Result<Option<String>> {
        // If output style says to drop coding instructions, skip this section.
        if let Some(ref style) = ctx.output_style
            && !style.keep_coding_instructions {
                return Ok(None);
            }

        let code_style_subitems = ["Don't add features, refactor code, or make \"improvements\" beyond what was asked. A bug fix doesn't need surrounding code cleaned up. A simple feature doesn't need extra configurability. Don't add docstrings, comments, or type annotations to code you didn't change. Only add comments where the logic isn't self-evident.",
            "Don't add error handling, fallbacks, or validation for scenarios that can't happen. Trust internal code and framework guarantees. Only validate at system boundaries (user input, external APIs). Don't use feature flags or backwards-compatibility shims when you can just change the code.",
            "Don't create helpers, utilities, or abstractions for one-time operations. Don't design for hypothetical future requirements. The right amount of complexity is what the task actually requires\u{2014}no speculative abstractions, but no half-finished implementations either. Three similar lines of code is better than a premature abstraction."];

        let user_help_subitems: Vec<String> = vec![
            "/help: Get help with using Claude Code".to_string(),
            "To give feedback, users can use the feedback mechanism in the app".to_string(),
        ];

        let items = vec![
            BulletItem::Single("The user will primarily request you to perform software engineering tasks. These may include solving bugs, adding new functionality, refactoring code, explaining code, and more. When given an unclear or generic instruction, consider it in the context of these software engineering tasks and the current working directory. For example, if the user asks you to change \"methodName\" to snake case, do not reply with just \"method_name\", instead find the method in the code and modify the code.".to_string()),
            BulletItem::Single("You are highly capable and often allow users to complete ambitious tasks that would otherwise be too complex or take too long. You should defer to user judgement about whether a task is too large to attempt.".to_string()),
            BulletItem::Single("In general, do not propose changes to code you haven't read. If a user asks about or wants you to modify a file, read it first. Understand existing code before suggesting modifications.".to_string()),
            BulletItem::Single("Do not create files unless they're absolutely necessary for achieving your goal. Generally prefer editing an existing file to creating a new one, as this prevents file bloat and builds on existing work more effectively.".to_string()),
            BulletItem::Single("Avoid giving time estimates or predictions for how long tasks will take, whether for your own work or for users planning projects. Focus on what needs to be done, not how long it might take.".to_string()),
            BulletItem::Single(format!("If an approach fails, diagnose why before switching tactics\u{2014}read the error, check your assumptions, try a focused fix. Don't retry the identical action blindly, but don't abandon a viable approach after a single failure either. Escalate to the user with {ASK_USER_QUESTION_TOOL_NAME} only when you're genuinely stuck after investigation, not as a first response to friction.")),
            BulletItem::Single("Be careful not to introduce security vulnerabilities such as command injection, XSS, SQL injection, and other OWASP top 10 vulnerabilities. If you notice that you wrote insecure code, immediately fix it. Prioritize writing safe, secure, and correct code.".to_string()),
            BulletItem::Nested(code_style_subitems.iter().map(|s| s.to_string()).collect()),
            BulletItem::Single("Avoid backwards-compatibility hacks like renaming unused _vars, re-exporting types, adding // removed comments for removed code, etc. If you are certain that something is unused, you can delete it completely.".to_string()),
            BulletItem::Single("If the user asks for help or wants to give feedback inform them of the following:".to_string()),
            BulletItem::Nested(user_help_subitems),
        ];

        Ok(Some(section_with_bullets("Doing tasks", &items)))
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
    fn doing_tasks_section_starts_with_header() {
        let section = DoingTasksSection;
        let result = section.compute(&test_ctx()).expect("compute ok");
        let content = result.expect("should be Some");
        assert!(content.starts_with("# Doing tasks"));
    }

    #[test]
    fn doing_tasks_mentions_security() {
        let section = DoingTasksSection;
        let result = section.compute(&test_ctx()).expect("compute ok");
        let content = result.expect("should be Some");
        assert!(content.contains("OWASP top 10"));
    }

    #[test]
    fn doing_tasks_mentions_code_style() {
        let section = DoingTasksSection;
        let result = section.compute(&test_ctx()).expect("compute ok");
        let content = result.expect("should be Some");
        assert!(content.contains("premature abstraction"));
    }

    #[test]
    fn doing_tasks_skipped_when_output_style_disables() {
        let mut ctx = test_ctx();
        ctx.output_style = Some(crate::OutputStyleConfig {
            name: "custom".to_string(),
            prompt: "be brief".to_string(),
            keep_coding_instructions: false,
        });
        let section = DoingTasksSection;
        let result = section.compute(&ctx).expect("compute ok");
        assert!(result.is_none());
    }

    #[test]
    fn doing_tasks_kept_when_output_style_allows() {
        let mut ctx = test_ctx();
        ctx.output_style = Some(crate::OutputStyleConfig {
            name: "custom".to_string(),
            prompt: "be brief".to_string(),
            keep_coding_instructions: true,
        });
        let section = DoingTasksSection;
        let result = section.compute(&ctx).expect("compute ok");
        assert!(result.is_some());
    }
}

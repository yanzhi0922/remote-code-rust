//! Context-aware prompt suggestion scoring.
//!
//! Analyzes the current session state and conversation history to rank
//! suggestions for follow-up actions, commands, and queries.

use claude_core::ConversationEntry;
use serde::{Deserialize, Serialize};

/// Relative weight / importance of a suggestion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SuggestionWeight {
    /// Low confidence — shown only when no better suggestion exists.
    Low,
    /// Medium confidence — shown among top suggestions.
    Medium,
    /// High confidence — shown prominently.
    High,
}

/// A single prompt suggestion.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptSuggestion {
    /// Display text shown to the user.
    pub display: String,
    /// Full prompt text sent to the model.
    pub prompt: String,
    /// Category for grouping suggestions.
    pub category: SuggestionCategory,
    /// Confidence weight.
    pub weight: SuggestionWeight,
}

/// Category of suggestion for UI grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuggestionCategory {
    /// Follow-up based on conversation context.
    FollowUp,
    /// Common command suggestions.
    Command,
    /// File or workspace action.
    FileAction,
    /// Debug or error recovery.
    Debug,
    /// Code review or analysis.
    Review,
}

impl SuggestionCategory {
    fn priority(&self) -> usize {
        match self {
            Self::FollowUp => 0,
            Self::Command => 1,
            Self::Debug => 2,
            Self::FileAction => 3,
            Self::Review => 4,
        }
    }
}

/// Service that generates ranked prompt suggestions.
pub struct PromptSuggestionService;

impl PromptSuggestionService {
    /// Build suggestions based on conversation history and recent tool usage.
    pub fn suggest(
        conversation: &[ConversationEntry],
        recent_errors: usize,
        tool_calls_used: usize,
    ) -> Vec<PromptSuggestion> {
        let mut suggestions = Vec::new();

        // If there were recent errors, suggest debugging first.
        if recent_errors > 0 {
            suggestions.push(PromptSuggestion {
                display: "Fix recent errors".into(),
                prompt: "Review recent errors and suggest fixes.".into(),
                category: SuggestionCategory::Debug,
                weight: SuggestionWeight::High,
            });
        }

        // If many tool calls have been made, suggest summarizing progress.
        if tool_calls_used > 10 && tool_calls_used % 5 == 0 {
            suggestions.push(PromptSuggestion {
                display: "Summarize changes".into(),
                prompt: "Summarize all changes made so far in this session.".into(),
                category: SuggestionCategory::FollowUp,
                weight: SuggestionWeight::Medium,
            });
        }

        // Check the last few messages for common patterns.
        let last_msgs: Vec<&ConversationEntry> = conversation.iter().rev().take(6).collect();
        let last_text: String = last_msgs
            .iter()
            .filter_map(|e| {
                if e.text.is_empty() {
                    None
                } else {
                    Some(e.text.as_str())
                }
            })
            .collect::<Vec<_>>()
            .join(" ");

        if last_text.contains("error")
            || last_text.contains("failed")
            || last_text.contains("panic")
        {
            suggestions.push(PromptSuggestion {
                display: "Debug and fix".into(),
                prompt: "Debug and fix the error described above. Explain root cause.".into(),
                category: SuggestionCategory::Debug,
                weight: SuggestionWeight::High,
            });
        }

        if last_text.contains("test") || last_text.contains("assert") {
            suggestions.push(PromptSuggestion {
                display: "Run tests".into(),
                prompt: "Run tests for the current project.".into(),
                category: SuggestionCategory::Command,
                weight: SuggestionWeight::Medium,
            });
        }

        if last_text.contains("git") || last_text.contains("commit") || last_text.contains("push") {
            suggestions.push(PromptSuggestion {
                display: "Review changes before commit".into(),
                prompt: "Review all uncommitted changes and suggest a commit message.".into(),
                category: SuggestionCategory::Review,
                weight: SuggestionWeight::High,
            });
        }

        // Generic fallback suggestions when nothing specific matches.
        if suggestions.is_empty() {
            suggestions.push(PromptSuggestion {
                display: "Continue working".into(),
                prompt: "Continue with the current task.".into(),
                category: SuggestionCategory::FollowUp,
                weight: SuggestionWeight::Medium,
            });
        }

        // Sort by category priority, then weight.
        suggestions.sort_by(|a, b| {
            a.category
                .priority()
                .cmp(&b.category.priority())
                .then_with(|| {
                    let a_w = match a.weight {
                        SuggestionWeight::High => 2,
                        SuggestionWeight::Medium => 1,
                        SuggestionWeight::Low => 0,
                    };
                    let b_w = match b.weight {
                        SuggestionWeight::High => 2,
                        SuggestionWeight::Medium => 1,
                        SuggestionWeight::Low => 0,
                    };
                    b_w.cmp(&a_w)
                })
        });

        suggestions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(text: &str) -> ConversationEntry {
        ConversationEntry::user(text)
    }

    #[test]
    fn suggests_debug_on_error_keyword() {
        let conv = vec![make_entry("I got an error when running cargo build")];
        let suggestions = PromptSuggestionService::suggest(&conv, 0, 0);
        assert!(
            suggestions
                .iter()
                .any(|s| s.category == SuggestionCategory::Debug)
        );
    }

    #[test]
    fn suggests_test_on_test_keyword() {
        let conv = vec![make_entry("Run tests for the auth module")];
        let suggestions = PromptSuggestionService::suggest(&conv, 0, 0);
        assert!(
            suggestions
                .iter()
                .any(|s| s.category == SuggestionCategory::Command)
        );
    }

    #[test]
    fn suggests_commit_on_git_keyword() {
        let conv = vec![make_entry("git add and commit the changes")];
        let suggestions = PromptSuggestionService::suggest(&conv, 0, 0);
        assert!(
            suggestions
                .iter()
                .any(|s| s.category == SuggestionCategory::Review)
        );
    }

    #[test]
    fn high_priority_on_errors() {
        let conv = vec![];
        let suggestions = PromptSuggestionService::suggest(&conv, 3, 0);
        let top = suggestions.first().expect("should have suggestions");
        assert_eq!(top.category, SuggestionCategory::Debug);
        assert_eq!(top.weight, SuggestionWeight::High);
    }

    #[test]
    fn returns_continue_when_nothing_specific() {
        let conv = vec![make_entry("Hello, can you help me?")];
        let suggestions = PromptSuggestionService::suggest(&conv, 0, 0);
        assert!(!suggestions.is_empty());
    }

    #[test]
    fn empty_conversation_returns_fallback() {
        let suggestions = PromptSuggestionService::suggest(&[], 0, 0);
        assert!(!suggestions.is_empty());
    }
}

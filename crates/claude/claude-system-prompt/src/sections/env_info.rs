//! Environment Info section — platform, git status, model details.
//!
//! Matches `computeSimpleEnvInfo()` in Claude Code's `prompts.ts`.

use anyhow::Result;

use crate::PromptContext;
use crate::sections::{BulletItem, SystemPromptSection, prepend_bullets};

/// Knowledge cutoff dates per model family.
fn get_knowledge_cutoff(model_id: &str) -> Option<&'static str> {
    let lower = model_id.to_lowercase();
    if lower.contains("claude-sonnet-4-6") {
        Some("August 2025")
    } else if lower.contains("claude-opus-4-7") {
        Some("March 2025")
    } else if lower.contains("claude-opus-4-6") {
        Some("May 2025")
    } else if lower.contains("claude-opus-4-5") {
        Some("May 2025")
    } else if lower.contains("claude-haiku-4") {
        Some("February 2025")
    } else if lower.contains("claude-opus-4") || lower.contains("claude-sonnet-4") {
        Some("January 2025")
    } else {
        None
    }
}

fn get_marketing_name_for_model(model_id: &str) -> Option<String> {
    let lower = model_id.to_lowercase();
    let has_1m = lower.contains("[1m]");

    if lower.contains("claude-opus-4-7") {
        return Some(if has_1m {
            "Opus 4.7 (with 1M context)".to_string()
        } else {
            "Opus 4.7".to_string()
        });
    }
    if lower.contains("claude-opus-4-6") {
        return Some(if has_1m {
            "Opus 4.6 (with 1M context)".to_string()
        } else {
            "Opus 4.6".to_string()
        });
    }
    if lower.contains("claude-opus-4-5") {
        return Some("Opus 4.5".to_string());
    }
    if lower.contains("claude-opus-4") {
        return Some("Opus 4".to_string());
    }
    if lower.contains("claude-sonnet-4-6") {
        return Some(if has_1m {
            "Sonnet 4.6 (with 1M context)".to_string()
        } else {
            "Sonnet 4.6".to_string()
        });
    }
    if lower.contains("claude-sonnet-4-5") {
        return Some(if has_1m {
            "Sonnet 4.5 (with 1M context)".to_string()
        } else {
            "Sonnet 4.5".to_string()
        });
    }
    if lower.contains("claude-sonnet-4") {
        return Some(if has_1m {
            "Sonnet 4 (with 1M context)".to_string()
        } else {
            "Sonnet 4".to_string()
        });
    }
    if lower.contains("claude-3-7-sonnet") {
        return Some("Claude 3.7 Sonnet".to_string());
    }
    if lower.contains("claude-3-5-sonnet") {
        return Some("Claude 3.5 Sonnet".to_string());
    }
    if lower.contains("claude-haiku-4-5") {
        return Some("Haiku 4.5".to_string());
    }
    if lower.contains("claude-3-5-haiku") {
        return Some("Claude 3.5 Haiku".to_string());
    }

    None
}

/// Get shell info line with platform-specific guidance.
fn get_shell_info_line(shell: &str, platform: &str) -> String {
    let shell_name = if shell.contains("zsh") {
        "zsh"
    } else if shell.contains("bash") {
        "bash"
    } else {
        shell
    };

    if platform == "win32" {
        format!(
            "Shell: {shell_name} (use Unix shell syntax, not Windows \u{2014} e.g., /dev/null not NUL, forward slashes in paths)"
        )
    } else {
        format!("Shell: {shell_name}")
    }
}

/// The environment info section.
pub struct EnvInfoSection;

impl SystemPromptSection for EnvInfoSection {
    fn name(&self) -> &str {
        "env_info_simple"
    }

    fn compute(&self, ctx: &PromptContext) -> Result<Option<String>> {
        let model_description = if ctx.model.is_empty() {
            None
        } else if let Some(marketing_name) = get_marketing_name_for_model(&ctx.model) {
            Some(format!(
                "You are powered by the model named {marketing_name}. The exact model ID is {}.",
                ctx.model
            ))
        } else {
            Some(format!(
                "You are powered by the model {model}.",
                model = ctx.model
            ))
        };

        let cutoff = get_knowledge_cutoff(&ctx.model);
        let cutoff_msg = cutoff
            .map(|c| format!("Assistant knowledge cutoff is {c}."))
            .unwrap_or_default();

        let mut env_items: Vec<BulletItem> = vec![BulletItem::Single(format!(
            "Primary working directory: {}",
            ctx.cwd.display()
        ))];

        if ctx.is_worktree {
            env_items.push(BulletItem::Single(
                "This is a git worktree \u{2014} an isolated copy of the repository. Run all commands from this directory. Do NOT `cd` to the original repository root.".to_string(),
            ));
        }

        env_items.push(BulletItem::Nested(vec![format!(
            "Is a git repository: {}",
            if ctx.is_git { "true" } else { "false" }
        )]));

        if !ctx.additional_dirs.is_empty() {
            env_items.push(BulletItem::Single(
                "Additional working directories:".to_string(),
            ));
            env_items.push(BulletItem::Nested(
                ctx.additional_dirs
                    .iter()
                    .map(|d| format!("{}", d.display()))
                    .collect(),
            ));
        }

        env_items.push(BulletItem::Single(format!("Platform: {}", ctx.platform)));
        env_items.push(BulletItem::Single(get_shell_info_line(
            &ctx.shell,
            &ctx.platform,
        )));
        env_items.push(BulletItem::Single(format!(
            "OS Version: {}",
            ctx.os_version
        )));

        if let Some(model_description) = model_description {
            env_items.push(BulletItem::Single(model_description));
        }
        if !cutoff_msg.is_empty() {
            env_items.push(BulletItem::Single(cutoff_msg));
        }
        env_items.push(BulletItem::Single(
            "The most recent Claude model family is Claude 4.5/4.7. Model IDs \u{2014} Opus 4.7: 'claude-opus-4-7', Sonnet 4.6: 'claude-sonnet-4-6', Haiku 4.5: 'claude-haiku-4-5'. When building AI applications, default to the latest and most capable Claude models."
                .to_string(),
        ));
        env_items.push(BulletItem::Single(
            "Claude Code is available as a CLI in the terminal, desktop app (Mac/Windows), web app (claude.ai/code), and IDE extensions (VS Code, JetBrains)."
                .to_string(),
        ));
        env_items.push(BulletItem::Single(
            "Fast mode for Claude Code uses the same Claude Opus 4.7 model with faster output. It does NOT switch to a different model. It can be toggled with /fast."
                .to_string(),
        ));

        let mut lines = vec![
            "# Environment".to_string(),
            "You have been invoked in the following environment: ".to_string(),
        ];
        lines.extend(prepend_bullets(&env_items));

        Ok(Some(lines.join("\n")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::PathBuf;

    fn test_ctx() -> PromptContext {
        PromptContext {
            model: "claude-sonnet-4-6".to_string(),
            cwd: PathBuf::from("/home/user/project"),
            is_git: true,
            platform: "linux".to_string(),
            shell: "bash".to_string(),
            os_version: "Linux 6.6.4".to_string(),
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
            features: crate::PromptFeatures::default(),
        }
    }

    #[test]
    fn env_info_starts_with_header() {
        let section = EnvInfoSection;
        let result = section.compute(&test_ctx()).expect("compute ok");
        let content = result.expect("should be Some");
        assert!(content.starts_with("# Environment"));
    }

    #[test]
    fn env_info_shows_cwd() {
        let section = EnvInfoSection;
        let result = section.compute(&test_ctx()).expect("compute ok");
        let content = result.expect("should be Some");
        assert!(content.contains("/home/user/project"));
    }

    #[test]
    fn env_info_shows_git_status() {
        let section = EnvInfoSection;
        let result = section.compute(&test_ctx()).expect("compute ok");
        let content = result.expect("should be Some");
        assert!(content.contains("Is a git repository: true"));
    }

    #[test]
    fn env_info_shows_model() {
        let section = EnvInfoSection;
        let result = section.compute(&test_ctx()).expect("compute ok");
        let content = result.expect("should be Some");
        assert!(content.contains("Sonnet 4.6"));
        assert!(content.contains("claude-sonnet-4-6"));
    }

    #[test]
    fn env_info_knowledge_cutoff() {
        let section = EnvInfoSection;
        let result = section.compute(&test_ctx()).expect("compute ok");
        let content = result.expect("should be Some");
        assert!(content.contains("August 2025"));
    }

    #[test]
    fn env_info_worktree_notice() {
        let mut ctx = test_ctx();
        ctx.is_worktree = true;
        let section = EnvInfoSection;
        let result = section.compute(&ctx).expect("compute ok");
        let content = result.expect("should be Some");
        assert!(content.contains("git worktree"));
    }

    #[test]
    fn knowledge_cutoff_sonnet_4_6() {
        assert_eq!(
            get_knowledge_cutoff("claude-sonnet-4-6"),
            Some("August 2025")
        );
    }

    #[test]
    fn knowledge_cutoff_opus_4_5() {
        assert_eq!(get_knowledge_cutoff("claude-opus-4-5"), Some("May 2025"));
    }

    #[test]
    fn knowledge_cutoff_unknown() {
        assert_eq!(get_knowledge_cutoff("some-other-model"), None);
    }

    #[test]
    fn shell_info_windows() {
        let line = get_shell_info_line("cmd.exe", "win32");
        assert!(line.contains("Unix shell syntax"));
    }
}

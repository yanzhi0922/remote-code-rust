use rc_config::RuntimeConfig;
use rc_core::{ConversationEntry, ConversationRole, default_system_prompt};
use rc_permissions::PermissionBroker;
use rc_provider::context::ContextWindowManager;
use rc_provider::cost::CostTracker;
use rc_session::SessionStore;
use rc_tools::runtime_builtin_tool_specs;

use crate::theme::Theme;

pub mod help;
pub mod mcp;
pub mod memory;
pub mod model;
pub mod permissions;
pub mod plugins;
pub mod provider;
pub mod review;
pub mod session;
pub mod skills;
pub mod status;
pub mod tasks;
pub mod worktree;

/// Result of handling a slash command.
pub enum SlashCommandAction {
    /// Continue the input loop normally.
    Continue,
    /// Reset history scroll position (e.g. after /clear).
    ResetScroll,
    /// Exit the interactive session.
    Quit,
}

#[derive(Debug, Clone, Copy)]
pub struct SlashCommandSpec {
    pub name: &'static str,
    pub summary: &'static str,
    pub usage: &'static str,
}

pub const SLASH_COMMANDS: &[SlashCommandSpec] = &[
    SlashCommandSpec {
        name: "/help",
        summary: "Show the command reference",
        usage: "/help",
    },
    SlashCommandSpec {
        name: "/status",
        summary: "Show session, provider, and runtime status",
        usage: "/status",
    },
    SlashCommandSpec {
        name: "/provider",
        summary: "Show active provider and auth details",
        usage: "/provider",
    },
    SlashCommandSpec {
        name: "/model",
        summary: "Show model capabilities, effort, and fallback",
        usage: "/model",
    },
    SlashCommandSpec {
        name: "/permissions",
        summary: "Show or mutate permission mode, rules, and audits",
        usage: "/permissions [allow|ask|deny <tool-pattern>|reset]",
    },
    SlashCommandSpec {
        name: "/tasks",
        summary: "List tracked background tasks and outputs",
        usage: "/tasks [show <task-id>|output <task-id>]",
    },
    SlashCommandSpec {
        name: "/mcp",
        summary: "List or inspect discovered MCP servers",
        usage: "/mcp [list|show <server>|enable <server> [project]|disable <server> [project]|reset [project]]",
    },
    SlashCommandSpec {
        name: "/plugins",
        summary: "List or inspect installed plugins",
        usage: "/plugins [list|show <plugin>|validate [plugin]|enable <plugin>|disable <plugin>]",
    },
    SlashCommandSpec {
        name: "/skills",
        summary: "List, inspect, or show the skill lock file",
        usage: "/skills [list|show <slug>|lock|index]",
    },
    SlashCommandSpec {
        name: "/review",
        summary: "Show diff stat and PR-title suggestion",
        usage: "/review",
    },
    SlashCommandSpec {
        name: "/worktree",
        summary: "List or manage git worktrees",
        usage: "/worktree [list|add <branch> [path]|remove <branch> [path]]",
    },
    SlashCommandSpec {
        name: "/memory",
        summary: "Show global/project memory file status",
        usage: "/memory",
    },
    SlashCommandSpec {
        name: "/session",
        summary: "Show session metadata and resume state",
        usage: "/session",
    },
    SlashCommandSpec {
        name: "/compact",
        summary: "Show context compaction status",
        usage: "/compact",
    },
    SlashCommandSpec {
        name: "/compact!",
        summary: "Force context compaction immediately",
        usage: "/compact!",
    },
    SlashCommandSpec {
        name: "/tools",
        summary: "List currently exposed tools",
        usage: "/tools",
    },
    SlashCommandSpec {
        name: "/sessions",
        summary: "List recent sessions or session stats",
        usage: "/sessions [stats]",
    },
    SlashCommandSpec {
        name: "/cost",
        summary: "Show accumulated cost summary",
        usage: "/cost",
    },
    SlashCommandSpec {
        name: "/theme",
        summary: "Show or switch theme",
        usage: "/theme [dark|light|monokai|solarized]",
    },
    SlashCommandSpec {
        name: "/clear",
        summary: "Clear the in-memory conversation",
        usage: "/clear",
    },
    SlashCommandSpec {
        name: "/quit",
        summary: "Exit the interactive session",
        usage: "/quit",
    },
    SlashCommandSpec {
        name: "/exit",
        summary: "Exit the interactive session",
        usage: "/exit",
    },
];

pub struct SlashCommandContext<'a> {
    pub config: &'a RuntimeConfig,
    pub store: &'a SessionStore,
    pub conversation: &'a mut Vec<ConversationEntry>,
    pub context_manager: &'a ContextWindowManager,
    pub cost_tracker: &'a CostTracker,
    pub broker: &'a dyn PermissionBroker,
    pub theme: &'a mut Theme,
}

#[must_use]
pub fn command_names() -> Vec<String> {
    SLASH_COMMANDS
        .iter()
        .map(|spec| spec.name.to_owned())
        .collect()
}

pub fn dispatch(input: &str, context: SlashCommandContext<'_>) -> SlashCommandAction {
    let trimmed = input.trim();
    let mut parts = trimmed.split_whitespace();
    let command = parts.next().unwrap_or_default();

    match command {
        "/help" => help::render(),
        "/status" => status::render(
            context.config,
            context.conversation,
            context.context_manager,
            context.cost_tracker,
            context.broker,
        ),
        "/provider" => provider::render(context.config),
        "/model" => model::render(context.config),
        "/permissions" => permissions::dispatch(trimmed, context.config, context.broker),
        "/tasks" => tasks::dispatch(trimmed, context.config),
        "/mcp" => mcp::dispatch(trimmed, context.config),
        "/plugins" => plugins::dispatch(trimmed, context.config),
        "/skills" => skills::dispatch(trimmed, context.config),
        "/review" => review::render(context.config),
        "/worktree" => worktree::dispatch(trimmed, context.config),
        "/memory" => memory::render(context.config),
        "/session" => session::render(context.config, context.store),
        "/compact!" => {
            let before = context.conversation.len();
            let compacted = context.context_manager.compact(context.conversation);
            let removed = before.saturating_sub(compacted.len());
            *context.conversation = compacted;
            if removed > 0 {
                println!("Force-compacted: removed {removed} entries.");
            } else {
                println!(
                    "Conversation is too short to compact (needs more than 8 non-system entries)."
                );
            }
        }
        "/compact" => {
            let ratio = context.context_manager.usage_ratio(context.conversation);
            println!("Context usage: {:.1}%", ratio * 100.0);
            println!(
                "Available budget: {} tokens",
                context.context_manager.available_budget()
            );
            if context
                .context_manager
                .needs_compaction(context.conversation)
            {
                println!("Compaction will be applied on the next turn.");
            } else {
                println!("Context is within budget - no compaction needed.");
            }
        }
        "/tools" => {
            let specs = runtime_builtin_tool_specs();
            println!("Available tools ({}):", specs.len());
            for spec in &specs {
                let perm = if spec.requires_permission { "*" } else { " " };
                println!("  {perm} {} - {}", spec.name, spec.description);
            }
            println!("  (* = requires permission)");
        }
        "/sessions" => match parts.next() {
            Some("stats") => match context.store.list_sessions() {
                Ok(sessions) => {
                    if sessions.is_empty() {
                        println!("No sessions found.");
                    } else {
                        for session in sessions.iter().take(5) {
                            match context.store.load_session_bundle(session.session_id) {
                                Ok(bundle) => println!(
                                    "  {}  {}  in={} out={} tools={} err={} stop={}",
                                    bundle.summary.session_id,
                                    bundle.summary.title,
                                    bundle.stats.usage.input_tokens,
                                    bundle.stats.usage.output_tokens,
                                    bundle.stats.tool_call_count,
                                    bundle.stats.error_count,
                                    bundle.stats.last_stop_reason.as_deref().unwrap_or("(none)")
                                ),
                                Err(error) => {
                                    eprintln!(
                                        "Error loading session {}: {error}",
                                        session.session_id
                                    )
                                }
                            }
                        }
                    }
                }
                Err(error) => eprintln!("Error listing sessions: {error}"),
            },
            _ => match context.store.list_sessions() {
                Ok(sessions) => {
                    if sessions.is_empty() {
                        println!("No sessions found.");
                    } else {
                        for session in sessions.iter().take(10) {
                            println!(
                                "  {}  {}  {}",
                                session.session_id, session.updated_at, session.title
                            );
                        }
                    }
                }
                Err(error) => eprintln!("Error listing sessions: {error}"),
            },
        },
        "/cost" => {
            print!("{}", context.cost_tracker.summary());
        }
        "/clear" => {
            let system_prompt = context
                .conversation
                .iter()
                .find(|entry| matches!(entry.role, ConversationRole::System))
                .cloned()
                .unwrap_or_else(|| {
                    ConversationEntry::system(default_system_prompt(&context.config.cwd))
                });
            context.conversation.clear();
            context.conversation.push(system_prompt);
            println!("Conversation cleared (system prompt preserved).");
            println!("Note: transcript history is still saved in the session file.");
            return SlashCommandAction::ResetScroll;
        }
        "/theme" => {
            let theme_name = parts.next();
            match theme_name {
                Some(name) => {
                    if let Some(new_theme) = Theme::by_name(name) {
                        *context.theme = new_theme;
                        println!("Theme set to: {name}");
                    } else {
                        println!(
                            "Unknown theme '{name}'. Available: {}",
                            Theme::all_names().join(", ")
                        );
                    }
                }
                None => {
                    println!("Current theme: {}", context.theme.name);
                    println!("Available themes: {}", Theme::all_names().join(", "));
                    println!("Usage: /theme <name>");
                }
            }
        }
        "/quit" | "/exit" => return SlashCommandAction::Quit,
        _ => {
            println!("Unknown command `{trimmed}`. Type /help for a list of commands.");
        }
    }
    SlashCommandAction::Continue
}

#[cfg(test)]
mod tests {
    use super::*;
    use rc_config::{ProviderOverrides, RuntimeOverrides, load_runtime_config};
    use rc_core::{InputFormat, OutputFormat, PermissionMode};
    use rc_permissions::StaticPermissionBroker;
    use rc_provider::cost::CostTracker;
    use rc_session::SessionStore;
    use tempfile::tempdir;

    fn build_test_config() -> (RuntimeConfig, SessionStore) {
        let temp = tempdir().expect("tempdir should work");
        let root = temp.keep();
        let config = load_runtime_config(
            Some(root.clone()),
            Some(root.join(".remote-code-rust")),
            None,
            PermissionMode::Default,
            InputFormat::Text,
            OutputFormat::Text,
            false,
            false,
            false,
            false,
            8,
            ProviderOverrides {
                provider: Some("glm-coding".to_owned()),
                base_url: Some("https://open.bigmodel.cn/api/anthropic".to_owned()),
                api_key: Some("secret".to_owned()),
                model: Some("glm-5.1".to_owned()),
                protocol: Some(rc_core::ProviderProtocol::Anthropic),
            },
            RuntimeOverrides::default(),
        )
        .expect("config should load");
        let store = SessionStore::open(config.paths.clone()).expect("store should open");
        (config, store)
    }

    #[test]
    fn command_names_expose_management_surfaces() {
        let names = command_names();
        assert!(names.contains(&"/help".to_owned()));
        assert!(names.contains(&"/permissions".to_owned()));
        assert!(names.contains(&"/tasks".to_owned()));
        assert!(names.contains(&"/plugins".to_owned()));
        assert!(names.contains(&"/skills".to_owned()));
        assert!(names.contains(&"/quit".to_owned()));
    }

    #[test]
    fn clear_command_preserves_system_prompt_and_resets_scroll() {
        let (config, store) = build_test_config();
        let context_manager = ContextWindowManager::for_model("glm-5.1");
        let cost_tracker = CostTracker::new();
        let broker = StaticPermissionBroker::new(PermissionMode::Default);
        let mut theme = Theme::dark();
        let mut conversation = vec![
            ConversationEntry::system("system prompt"),
            ConversationEntry::user("hello"),
            ConversationEntry::assistant("world"),
        ];

        let action = dispatch(
            "/clear",
            SlashCommandContext {
                config: &config,
                store: &store,
                conversation: &mut conversation,
                context_manager: &context_manager,
                cost_tracker: &cost_tracker,
                broker: &broker,
                theme: &mut theme,
            },
        );

        assert!(matches!(action, SlashCommandAction::ResetScroll));
        assert_eq!(conversation.len(), 1);
        assert!(matches!(conversation[0].role, ConversationRole::System));
    }

    #[test]
    fn theme_command_switches_theme_and_quit_returns_quit_action() {
        let (config, store) = build_test_config();
        let context_manager = ContextWindowManager::for_model("glm-5.1");
        let cost_tracker = CostTracker::new();
        let broker = StaticPermissionBroker::new(PermissionMode::Default);
        let mut theme = Theme::dark();
        let mut conversation = vec![ConversationEntry::system("system prompt")];

        let action = dispatch(
            "/theme solarized",
            SlashCommandContext {
                config: &config,
                store: &store,
                conversation: &mut conversation,
                context_manager: &context_manager,
                cost_tracker: &cost_tracker,
                broker: &broker,
                theme: &mut theme,
            },
        );
        assert!(matches!(action, SlashCommandAction::Continue));
        assert_eq!(theme.name, "solarized");

        let quit_action = dispatch(
            "/quit",
            SlashCommandContext {
                config: &config,
                store: &store,
                conversation: &mut conversation,
                context_manager: &context_manager,
                cost_tracker: &cost_tracker,
                broker: &broker,
                theme: &mut theme,
            },
        );
        assert!(matches!(quit_action, SlashCommandAction::Quit));
    }
}

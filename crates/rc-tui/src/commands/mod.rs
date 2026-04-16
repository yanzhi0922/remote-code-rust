use rc_config::RuntimeConfig;
use rc_core::{ConversationEntry, ConversationRole, default_system_prompt};
use rc_permissions::PermissionBroker;
use rc_provider::context::ContextWindowManager;
use rc_provider::cost::CostTracker;
use rc_session::SessionStore;
use rc_tools::runtime_builtin_tool_specs;

use crate::theme::Theme;

pub mod agent_commands;
pub mod auth;
pub mod config;
pub mod git_commands;
pub mod help;
pub mod hooks_cmd;
pub mod keybindings;
pub mod mcp;
pub mod memory;
pub mod metrics;
pub mod misc_commands;
pub mod mode_commands;
pub mod model;
pub mod permissions;
pub mod plugins;
pub mod provider;
pub mod remote;
pub mod review;
pub mod security;
pub mod session;
pub mod session_mgmt;
pub mod skills;
pub mod status;
pub mod tasks;
pub mod utility;
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
    // --- New commands (Phase 10) ---
    SlashCommandSpec {
        name: "/config",
        summary: "Runtime configuration get/set/list",
        usage: "/config [list|get <key>|set <key> <value>]",
    },
    SlashCommandSpec {
        name: "/resume",
        summary: "List recent sessions for resumption",
        usage: "/resume",
    },
    SlashCommandSpec {
        name: "/rename",
        summary: "Rename the current session",
        usage: "/rename <new-name>",
    },
    SlashCommandSpec {
        name: "/rewind",
        summary: "Show rewind history for the current session",
        usage: "/rewind",
    },
    SlashCommandSpec {
        name: "/export",
        summary: "Export current session to JSON or Markdown",
        usage: "/export [json|markdown]",
    },
    SlashCommandSpec {
        name: "/share",
        summary: "Generate a share summary for the session",
        usage: "/share",
    },
    SlashCommandSpec {
        name: "/summary",
        summary: "Show current session summary",
        usage: "/summary",
    },
    SlashCommandSpec {
        name: "/tag",
        summary: "Manage session tags",
        usage: "/tag [list|add <tag>|remove <tag>]",
    },
    SlashCommandSpec {
        name: "/commit",
        summary: "One-click commit preview (git add + commit)",
        usage: "/commit",
    },
    SlashCommandSpec {
        name: "/diff",
        summary: "View code changes (git diff)",
        usage: "/diff",
    },
    SlashCommandSpec {
        name: "/pr_comments",
        summary: "View PR comments",
        usage: "/pr_comments",
    },
    SlashCommandSpec {
        name: "/branch",
        summary: "Branch management (list/create/switch)",
        usage: "/branch [list|create <name>|switch <name>]",
    },
    SlashCommandSpec {
        name: "/autofix-pr",
        summary: "Auto-fix PR issues",
        usage: "/autofix-pr",
    },
    SlashCommandSpec {
        name: "/usage",
        summary: "Show detailed token usage statistics",
        usage: "/usage",
    },
    SlashCommandSpec {
        name: "/extraUsage",
        summary: "Show extra usage details (cache tokens)",
        usage: "/extraUsage",
    },
    SlashCommandSpec {
        name: "/stats",
        summary: "Show session/tool/error statistics",
        usage: "/stats",
    },
    SlashCommandSpec {
        name: "/insights",
        summary: "Show session analysis report",
        usage: "/insights",
    },
    SlashCommandSpec {
        name: "/fork",
        summary: "Fork a sub-agent from the current session",
        usage: "/fork <description>",
    },
    SlashCommandSpec {
        name: "/peers",
        summary: "List peer agents in the current swarm",
        usage: "/peers",
    },
    SlashCommandSpec {
        name: "/plan",
        summary: "Enter or exit plan mode",
        usage: "/plan [on|off]",
    },
    SlashCommandSpec {
        name: "/effort",
        summary: "Adjust reasoning effort level",
        usage: "/effort [low|medium|high]",
    },
    SlashCommandSpec {
        name: "/fast",
        summary: "Toggle fast mode",
        usage: "/fast [on|off]",
    },
    SlashCommandSpec {
        name: "/outputStyle",
        summary: "Switch output style",
        usage: "/outputStyle [default|concise|verbose|technical]",
    },
    SlashCommandSpec {
        name: "/color",
        summary: "Switch color scheme",
        usage: "/color [auto|always|never]",
    },
    SlashCommandSpec {
        name: "/proactive",
        summary: "Toggle proactive mode",
        usage: "/proactive [on|off]",
    },
    SlashCommandSpec {
        name: "/brief",
        summary: "Toggle brief mode",
        usage: "/brief [on|off]",
    },
    SlashCommandSpec {
        name: "/files",
        summary: "List files in the working directory",
        usage: "/files [subdir]",
    },
    SlashCommandSpec {
        name: "/env",
        summary: "Show local environment variables",
        usage: "/env",
    },
    SlashCommandSpec {
        name: "/remoteEnv",
        summary: "Show remote environment variables",
        usage: "/remoteEnv",
    },
    SlashCommandSpec {
        name: "/context",
        summary: "Show context window usage",
        usage: "/context",
    },
    SlashCommandSpec {
        name: "/copy",
        summary: "Copy recent output to clipboard",
        usage: "/copy",
    },
    SlashCommandSpec {
        name: "/advisor",
        summary: "Show advisor suggestions",
        usage: "/advisor",
    },
    SlashCommandSpec {
        name: "/init",
        summary: "Initialize project configuration",
        usage: "/init",
    },
    SlashCommandSpec {
        name: "/add-dir",
        summary: "Add a working directory",
        usage: "/add-dir <path>",
    },
    SlashCommandSpec {
        name: "/feedback",
        summary: "Submit feedback",
        usage: "/feedback",
    },
    SlashCommandSpec {
        name: "/releaseNotes",
        summary: "Show release notes",
        usage: "/releaseNotes",
    },
    SlashCommandSpec {
        name: "/reloadPlugins",
        summary: "Reload plugins",
        usage: "/reloadPlugins",
    },
    SlashCommandSpec {
        name: "/securityReview",
        summary: "Perform a security review",
        usage: "/securityReview",
    },
    SlashCommandSpec {
        name: "/sandboxToggle",
        summary: "Toggle sandbox mode",
        usage: "/sandboxToggle [on|off|status]",
    },
    SlashCommandSpec {
        name: "/login",
        summary: "Show authentication status",
        usage: "/login",
    },
    SlashCommandSpec {
        name: "/logout",
        summary: "Log out from current session",
        usage: "/logout",
    },
    SlashCommandSpec {
        name: "/hooks",
        summary: "Manage hooks (list/run/test)",
        usage: "/hooks [list|run <event>|test]",
    },
    SlashCommandSpec {
        name: "/keybindings",
        summary: "Manage keybindings (list/set/reset)",
        usage: "/keybindings [list|set <key> <action>|reset]",
    },
    SlashCommandSpec {
        name: "/terminalSetup",
        summary: "Show terminal setup information",
        usage: "/terminalSetup",
    },
    SlashCommandSpec {
        name: "/remoteControlServer",
        summary: "Show remote control server status",
        usage: "/remoteControlServer",
    },
    SlashCommandSpec {
        name: "/remote-setup",
        summary: "Remote setup wizard",
        usage: "/remote-setup [1|2|3|4]",
    },
    SlashCommandSpec {
        name: "/ide",
        summary: "Show IDE integration status",
        usage: "/ide",
    },
    SlashCommandSpec {
        name: "/voice",
        summary: "Show voice mode status",
        usage: "/voice",
    },
    SlashCommandSpec {
        name: "/thinkback",
        summary: "Show thinking/reasoning playback",
        usage: "/thinkback",
    },
    SlashCommandSpec {
        name: "/debugToolCall",
        summary: "Debug tool call execution",
        usage: "/debugToolCall <tool-name>",
    },
    SlashCommandSpec {
        name: "/subscribe-pr",
        summary: "Subscribe to PR activity",
        usage: "/subscribe-pr <pr-url-or-number>",
    },
    SlashCommandSpec {
        name: "/upgrade",
        summary: "Check for updates",
        usage: "/upgrade",
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
        // --- New commands (Phase 10) ---
        "/config" => config::dispatch(trimmed, context.config),
        "/resume" => session_mgmt::render_resume(context.config, context.store),
        "/rename" => session_mgmt::dispatch_rename(trimmed, context.config),
        "/rewind" => session_mgmt::render_rewind(context.config, context.store),
        "/export" => session_mgmt::dispatch_export(trimmed, context.config, context.store),
        "/share" => session_mgmt::render_share(context.config, context.store),
        "/summary" => session_mgmt::render_summary(context.config, context.store),
        "/tag" => session_mgmt::dispatch_tag(trimmed, context.config),
        "/commit" => git_commands::render_commit(context.config),
        "/diff" => git_commands::render_diff(context.config),
        "/pr_comments" => git_commands::render_pr_comments(context.config),
        "/branch" => git_commands::dispatch_branch(trimmed, context.config),
        "/autofix-pr" => git_commands::render_autofix_pr(context.config),
        "/usage" => metrics::render_usage(context.config, context.store),
        "/extraUsage" => metrics::render_extra_usage(context.config, context.store),
        "/stats" => metrics::render_stats(context.config, context.store, context.cost_tracker),
        "/insights" => metrics::render_insights(context.config, context.store),
        "/fork" => agent_commands::dispatch_fork(trimmed, context.config),
        "/peers" => agent_commands::render_peers(context.config),
        "/plan" => mode_commands::dispatch_plan(trimmed, context.config),
        "/effort" => mode_commands::dispatch_effort(trimmed, context.config),
        "/fast" => mode_commands::dispatch_fast(trimmed, context.config),
        "/outputStyle" => mode_commands::dispatch_output_style(trimmed, context.config),
        "/color" => mode_commands::dispatch_color(trimmed, context.config),
        "/proactive" => mode_commands::dispatch_proactive(trimmed, context.config),
        "/brief" => mode_commands::dispatch_brief(trimmed, context.config),
        "/files" => utility::dispatch_files(trimmed, context.config),
        "/env" => utility::render_env(),
        "/remoteEnv" => utility::render_remote_env(context.config),
        "/context" => utility::render_context(
            context.config,
            context.context_manager,
            context.conversation,
        ),
        "/copy" => utility::render_copy(),
        "/advisor" => utility::render_advisor(context.config),
        "/init" => utility::render_init(context.config),
        "/add-dir" => utility::dispatch_add_dir(trimmed, context.config),
        "/feedback" => utility::render_feedback(),
        "/releaseNotes" => utility::render_release_notes(),
        "/reloadPlugins" => utility::render_reload_plugins(context.config),
        "/securityReview" => security::render_security_review(context.config),
        "/sandboxToggle" => security::dispatch_sandbox_toggle(trimmed, context.config),
        "/login" => auth::render_login(context.config),
        "/logout" => auth::render_logout(context.config),
        "/hooks" => hooks_cmd::dispatch(trimmed, context.config),
        "/keybindings" => keybindings::dispatch(trimmed, context.config),
        "/terminalSetup" => keybindings::render_terminal_setup(context.config),
        "/remoteControlServer" => remote::render_remote_control_server(context.config),
        "/remote-setup" => remote::dispatch_remote_setup(trimmed, context.config),
        "/ide" => misc_commands::render_ide(context.config),
        "/voice" => misc_commands::render_voice(context.config),
        "/thinkback" => misc_commands::render_thinkback(context.config),
        "/debugToolCall" => misc_commands::dispatch_debug_tool_call(trimmed, context.config),
        "/subscribe-pr" => misc_commands::dispatch_subscribe_pr(trimmed, context.config),
        "/upgrade" => misc_commands::render_upgrade(),
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

    fn build_context<'a>(
        config: &'a RuntimeConfig,
        store: &'a SessionStore,
        conversation: &'a mut Vec<ConversationEntry>,
        context_manager: &'a ContextWindowManager,
        cost_tracker: &'a CostTracker,
        broker: &'a StaticPermissionBroker,
        theme: &'a mut Theme,
    ) -> SlashCommandContext<'a> {
        SlashCommandContext {
            config,
            store,
            conversation,
            context_manager,
            cost_tracker,
            broker,
            theme,
        }
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
    fn command_names_include_new_phase10_commands() {
        let names = command_names();
        assert!(names.contains(&"/config".to_owned()));
        assert!(names.contains(&"/resume".to_owned()));
        assert!(names.contains(&"/rename".to_owned()));
        assert!(names.contains(&"/export".to_owned()));
        assert!(names.contains(&"/commit".to_owned()));
        assert!(names.contains(&"/diff".to_owned()));
        assert!(names.contains(&"/branch".to_owned()));
        assert!(names.contains(&"/usage".to_owned()));
        assert!(names.contains(&"/stats".to_owned()));
        assert!(names.contains(&"/insights".to_owned()));
        assert!(names.contains(&"/fork".to_owned()));
        assert!(names.contains(&"/peers".to_owned()));
        assert!(names.contains(&"/plan".to_owned()));
        assert!(names.contains(&"/effort".to_owned()));
        assert!(names.contains(&"/fast".to_owned()));
        assert!(names.contains(&"/outputStyle".to_owned()));
        assert!(names.contains(&"/color".to_owned()));
        assert!(names.contains(&"/files".to_owned()));
        assert!(names.contains(&"/env".to_owned()));
        assert!(names.contains(&"/context".to_owned()));
        assert!(names.contains(&"/login".to_owned()));
        assert!(names.contains(&"/logout".to_owned()));
        assert!(names.contains(&"/hooks".to_owned()));
        assert!(names.contains(&"/keybindings".to_owned()));
        assert!(names.contains(&"/upgrade".to_owned()));
    }

    #[test]
    fn clear_command_preserves_system_prompt_and_resets_scroll() {
        let (config, store) = build_test_config();
        let context_manager = ContextWindowManager::for_model("glm-5.1");
        let cost_tracker = CostTracker::new();
        let broker = StaticPermissionBroker::new(false);
        let mut theme = Theme::dark();
        let mut conversation = vec![
            ConversationEntry::system("system prompt"),
            ConversationEntry::user("hello"),
            ConversationEntry::assistant("world"),
        ];

        let action = dispatch(
            "/clear",
            build_context(
                &config,
                &store,
                &mut conversation,
                &context_manager,
                &cost_tracker,
                &broker,
                &mut theme,
            ),
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
        let broker = StaticPermissionBroker::new(false);
        let mut theme = Theme::dark();
        let mut conversation = vec![ConversationEntry::system("system prompt")];

        let action = dispatch(
            "/theme solarized",
            build_context(
                &config,
                &store,
                &mut conversation,
                &context_manager,
                &cost_tracker,
                &broker,
                &mut theme,
            ),
        );
        assert!(matches!(action, SlashCommandAction::Continue));
        assert_eq!(theme.name, "solarized");

        let quit_action = dispatch(
            "/quit",
            build_context(
                &config,
                &store,
                &mut conversation,
                &context_manager,
                &cost_tracker,
                &broker,
                &mut theme,
            ),
        );
        assert!(matches!(quit_action, SlashCommandAction::Quit));
    }

    #[test]
    fn unknown_command_returns_continue() {
        let (config, store) = build_test_config();
        let context_manager = ContextWindowManager::for_model("glm-5.1");
        let cost_tracker = CostTracker::new();
        let broker = StaticPermissionBroker::new(false);
        let mut theme = Theme::dark();
        let mut conversation = vec![ConversationEntry::system("system prompt")];

        let action = dispatch(
            "/unknown_command_xyz",
            build_context(
                &config,
                &store,
                &mut conversation,
                &context_manager,
                &cost_tracker,
                &broker,
                &mut theme,
            ),
        );
        assert!(matches!(action, SlashCommandAction::Continue));
    }

    #[test]
    fn dispatch_config_command() {
        let (config, store) = build_test_config();
        let context_manager = ContextWindowManager::for_model("glm-5.1");
        let cost_tracker = CostTracker::new();
        let broker = StaticPermissionBroker::new(false);
        let mut theme = Theme::dark();
        let mut conversation = vec![ConversationEntry::system("system prompt")];

        let action = dispatch(
            "/config list",
            build_context(
                &config,
                &store,
                &mut conversation,
                &context_manager,
                &cost_tracker,
                &broker,
                &mut theme,
            ),
        );
        assert!(matches!(action, SlashCommandAction::Continue));
    }

    #[test]
    fn dispatch_resume_command() {
        let (config, store) = build_test_config();
        let context_manager = ContextWindowManager::for_model("glm-5.1");
        let cost_tracker = CostTracker::new();
        let broker = StaticPermissionBroker::new(false);
        let mut theme = Theme::dark();
        let mut conversation = vec![ConversationEntry::system("system prompt")];

        let action = dispatch(
            "/resume",
            build_context(
                &config,
                &store,
                &mut conversation,
                &context_manager,
                &cost_tracker,
                &broker,
                &mut theme,
            ),
        );
        assert!(matches!(action, SlashCommandAction::Continue));
    }

    #[test]
    fn dispatch_commit_command() {
        let (config, store) = build_test_config();
        let context_manager = ContextWindowManager::for_model("glm-5.1");
        let cost_tracker = CostTracker::new();
        let broker = StaticPermissionBroker::new(false);
        let mut theme = Theme::dark();
        let mut conversation = vec![ConversationEntry::system("system prompt")];

        let action = dispatch(
            "/commit",
            build_context(
                &config,
                &store,
                &mut conversation,
                &context_manager,
                &cost_tracker,
                &broker,
                &mut theme,
            ),
        );
        assert!(matches!(action, SlashCommandAction::Continue));
    }

    #[test]
    fn dispatch_usage_command() {
        let (config, store) = build_test_config();
        let context_manager = ContextWindowManager::for_model("glm-5.1");
        let cost_tracker = CostTracker::new();
        let broker = StaticPermissionBroker::new(false);
        let mut theme = Theme::dark();
        let mut conversation = vec![ConversationEntry::system("system prompt")];

        let action = dispatch(
            "/usage",
            build_context(
                &config,
                &store,
                &mut conversation,
                &context_manager,
                &cost_tracker,
                &broker,
                &mut theme,
            ),
        );
        assert!(matches!(action, SlashCommandAction::Continue));
    }

    #[test]
    fn dispatch_fork_command() {
        let (config, store) = build_test_config();
        let context_manager = ContextWindowManager::for_model("glm-5.1");
        let cost_tracker = CostTracker::new();
        let broker = StaticPermissionBroker::new(false);
        let mut theme = Theme::dark();
        let mut conversation = vec![ConversationEntry::system("system prompt")];

        let action = dispatch(
            "/fork test task",
            build_context(
                &config,
                &store,
                &mut conversation,
                &context_manager,
                &cost_tracker,
                &broker,
                &mut theme,
            ),
        );
        assert!(matches!(action, SlashCommandAction::Continue));
    }

    #[test]
    fn dispatch_plan_command() {
        let (config, store) = build_test_config();
        let context_manager = ContextWindowManager::for_model("glm-5.1");
        let cost_tracker = CostTracker::new();
        let broker = StaticPermissionBroker::new(false);
        let mut theme = Theme::dark();
        let mut conversation = vec![ConversationEntry::system("system prompt")];

        let action = dispatch(
            "/plan on",
            build_context(
                &config,
                &store,
                &mut conversation,
                &context_manager,
                &cost_tracker,
                &broker,
                &mut theme,
            ),
        );
        assert!(matches!(action, SlashCommandAction::Continue));
    }

    #[test]
    fn dispatch_login_command() {
        let (config, store) = build_test_config();
        let context_manager = ContextWindowManager::for_model("glm-5.1");
        let cost_tracker = CostTracker::new();
        let broker = StaticPermissionBroker::new(false);
        let mut theme = Theme::dark();
        let mut conversation = vec![ConversationEntry::system("system prompt")];

        let action = dispatch(
            "/login",
            build_context(
                &config,
                &store,
                &mut conversation,
                &context_manager,
                &cost_tracker,
                &broker,
                &mut theme,
            ),
        );
        assert!(matches!(action, SlashCommandAction::Continue));
    }

    #[test]
    fn dispatch_upgrade_command() {
        let (config, store) = build_test_config();
        let context_manager = ContextWindowManager::for_model("glm-5.1");
        let cost_tracker = CostTracker::new();
        let broker = StaticPermissionBroker::new(false);
        let mut theme = Theme::dark();
        let mut conversation = vec![ConversationEntry::system("system prompt")];

        let action = dispatch(
            "/upgrade",
            build_context(
                &config,
                &store,
                &mut conversation,
                &context_manager,
                &cost_tracker,
                &broker,
                &mut theme,
            ),
        );
        assert!(matches!(action, SlashCommandAction::Continue));
    }

    #[test]
    fn dispatch_env_command() {
        let (config, store) = build_test_config();
        let context_manager = ContextWindowManager::for_model("glm-5.1");
        let cost_tracker = CostTracker::new();
        let broker = StaticPermissionBroker::new(false);
        let mut theme = Theme::dark();
        let mut conversation = vec![ConversationEntry::system("system prompt")];

        let action = dispatch(
            "/env",
            build_context(
                &config,
                &store,
                &mut conversation,
                &context_manager,
                &cost_tracker,
                &broker,
                &mut theme,
            ),
        );
        assert!(matches!(action, SlashCommandAction::Continue));
    }

    #[test]
    fn dispatch_hooks_command() {
        let (config, store) = build_test_config();
        let context_manager = ContextWindowManager::for_model("glm-5.1");
        let cost_tracker = CostTracker::new();
        let broker = StaticPermissionBroker::new(false);
        let mut theme = Theme::dark();
        let mut conversation = vec![ConversationEntry::system("system prompt")];

        let action = dispatch(
            "/hooks list",
            build_context(
                &config,
                &store,
                &mut conversation,
                &context_manager,
                &cost_tracker,
                &broker,
                &mut theme,
            ),
        );
        assert!(matches!(action, SlashCommandAction::Continue));
    }

    #[test]
    fn dispatch_security_review_command() {
        let (config, store) = build_test_config();
        let context_manager = ContextWindowManager::for_model("glm-5.1");
        let cost_tracker = CostTracker::new();
        let broker = StaticPermissionBroker::new(false);
        let mut theme = Theme::dark();
        let mut conversation = vec![ConversationEntry::system("system prompt")];

        let action = dispatch(
            "/securityReview",
            build_context(
                &config,
                &store,
                &mut conversation,
                &context_manager,
                &cost_tracker,
                &broker,
                &mut theme,
            ),
        );
        assert!(matches!(action, SlashCommandAction::Continue));
    }

    #[test]
    fn dispatch_keybindings_command() {
        let (config, store) = build_test_config();
        let context_manager = ContextWindowManager::for_model("glm-5.1");
        let cost_tracker = CostTracker::new();
        let broker = StaticPermissionBroker::new(false);
        let mut theme = Theme::dark();
        let mut conversation = vec![ConversationEntry::system("system prompt")];

        let action = dispatch(
            "/keybindings",
            build_context(
                &config,
                &store,
                &mut conversation,
                &context_manager,
                &cost_tracker,
                &broker,
                &mut theme,
            ),
        );
        assert!(matches!(action, SlashCommandAction::Continue));
    }

    #[test]
    fn dispatch_remote_setup_command() {
        let (config, store) = build_test_config();
        let context_manager = ContextWindowManager::for_model("glm-5.1");
        let cost_tracker = CostTracker::new();
        let broker = StaticPermissionBroker::new(false);
        let mut theme = Theme::dark();
        let mut conversation = vec![ConversationEntry::system("system prompt")];

        let action = dispatch(
            "/remote-setup",
            build_context(
                &config,
                &store,
                &mut conversation,
                &context_manager,
                &cost_tracker,
                &broker,
                &mut theme,
            ),
        );
        assert!(matches!(action, SlashCommandAction::Continue));
    }

    #[test]
    fn dispatch_ide_command() {
        let (config, store) = build_test_config();
        let context_manager = ContextWindowManager::for_model("glm-5.1");
        let cost_tracker = CostTracker::new();
        let broker = StaticPermissionBroker::new(false);
        let mut theme = Theme::dark();
        let mut conversation = vec![ConversationEntry::system("system prompt")];

        let action = dispatch(
            "/ide",
            build_context(
                &config,
                &store,
                &mut conversation,
                &context_manager,
                &cost_tracker,
                &broker,
                &mut theme,
            ),
        );
        assert!(matches!(action, SlashCommandAction::Continue));
    }

    #[test]
    fn total_command_count_includes_new_commands() {
        // We should have significantly more than the original ~22 commands
        let names = command_names();
        assert!(names.len() > 60, "Expected 60+ commands, got {}", names.len());
    }
}

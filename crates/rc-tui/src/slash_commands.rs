//! Slash command handler for the interactive TUI.

use rc_config::RuntimeConfig;
use rc_core::{ConversationEntry, ConversationRole, default_system_prompt};
use rc_provider::context::ContextWindowManager;
use rc_provider::cost::CostTracker;
use rc_session::SessionStore;
use rc_tools::builtin_tool_specs;

use crate::theme::Theme;

/// Result of handling a slash command.
pub enum SlashCommandAction {
    /// Continue the input loop normally.
    Continue,
    /// Reset history scroll position (e.g. after /clear).
    ResetScroll,
    /// Exit the interactive session.
    Quit,
}

/// Handle slash commands.
///
/// Modifies `conversation` in-place for /clear and /compact! commands.
/// Returns a [`SlashCommandAction`] indicating what the caller should do next.
#[allow(clippy::too_many_lines)]
pub fn handle_slash_command(
    input: &str,
    config: &RuntimeConfig,
    store: &SessionStore,
    conversation: &mut Vec<ConversationEntry>,
    context_manager: &ContextWindowManager,
    cost_tracker: &CostTracker,
    theme: &mut Theme,
) -> SlashCommandAction {
    let trimmed = input.trim();
    let mut parts = trimmed.split_whitespace();
    let command = parts.next().unwrap_or_default();

    match command {
        "/help" => {
            println!("Available commands:");
            println!("  /help      Show this help");
            println!("  /status    Show session and provider details");
            println!("  /compact   Show context compaction info");
            println!("  /compact!  Force context compaction now");
            println!("  /tools     List available tools");
            println!("  /sessions  List recent sessions");
            println!("  /cost      Show accumulated cost summary");
            println!("  /theme     Show or switch color theme (dark/light/monokai/solarized)");
            println!("  /clear     Clear conversation (keeps system prompt)");
            println!("  /quit      Exit the interactive session");
        }
        "/status" => {
            println!("Session:  {}", config.session_id);
            println!("CWD:      {}", config.cwd.display());
            println!(
                "Provider: {} ({})",
                config.provider.name,
                config.provider.protocol.as_str()
            );
            println!(
                "Model:    {}",
                config.provider.model.as_deref().unwrap_or("(missing)")
            );
            println!(
                "Base URL: {}",
                config.provider.base_url.as_deref().unwrap_or("(missing)")
            );
            println!(
                "API key:  {}",
                if config.provider.api_key.is_some() {
                    "present"
                } else {
                    "missing"
                }
            );
            println!("Permission mode: {}", config.permission_mode.as_legacy_str());
            println!("Conversation entries: {}", conversation.len());
            let usage_ratio = context_manager.usage_ratio(conversation);
            println!("Context usage: {:.1}%", usage_ratio * 100.0);
            let total_cost = cost_tracker.total_cost_usd();
            if total_cost > 0.0 {
                println!("Estimated cost: ${total_cost:.6} USD");
            }
        }
        "/compact!" => {
            // Force compaction regardless of current threshold
            let before = conversation.len();
            let compacted = context_manager.compact(conversation);
            let removed = before.saturating_sub(compacted.len());
            *conversation = compacted;
            if removed > 0 {
                println!("Force-compacted: removed {removed} entries.");
            } else {
                println!("Conversation is too short to compact (needs more than 8 non-system entries).");
            }
        }
        "/compact" => {
            let ratio = context_manager.usage_ratio(conversation);
            println!("Context usage: {:.1}%", ratio * 100.0);
            println!("Available budget: {} tokens", context_manager.available_budget());
            if context_manager.needs_compaction(conversation) {
                println!("Compaction will be applied on the next turn.");
            } else {
                println!("Context is within budget — no compaction needed.");
            }
        }
        "/tools" => {
            let specs = builtin_tool_specs();
            println!("Available tools ({}):", specs.len());
            for spec in &specs {
                let perm = if spec.requires_permission { "*" } else { " " };
                println!("  {perm} {} — {}", spec.name, spec.description);
            }
            println!("  (* = requires permission)");
        }
        "/sessions" => {
            match store.list_sessions() {
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
            }
        }
        "/cost" => {
            print!("{}", cost_tracker.summary());
        }
        "/clear" => {
            // Preserve the system prompt, clear everything else.
            // Note: the transcript file (JSONL) is append-only, so old entries
            // remain on disk. This only clears the in-memory conversation that
            // gets sent to the provider.
            let system_prompt = conversation
                .iter()
                .find(|e| matches!(e.role, ConversationRole::System))
                .cloned()
                .unwrap_or_else(|| ConversationEntry::system(default_system_prompt(&config.cwd)));
            conversation.clear();
            conversation.push(system_prompt);
            println!("Conversation cleared (system prompt preserved).");
            println!("Note: transcript history is still saved in the session file.");
            return SlashCommandAction::ResetScroll;
        }
        "/theme" => {
            let theme_name = parts.next();
            match theme_name {
                Some(name) => {
                    if let Some(new_theme) = Theme::by_name(name) {
                        *theme = new_theme;
                        println!("Theme set to: {name}");
                    } else {
                        println!("Unknown theme '{name}'. Available: {}", Theme::all_names().join(", "));
                    }
                }
                None => {
                    println!("Current theme: {}", theme.name);
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

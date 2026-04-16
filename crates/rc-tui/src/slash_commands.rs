//! Slash command handler for the interactive TUI.

use rc_config::RuntimeConfig;
use rc_core::ConversationEntry;
use rc_permissions::PermissionBroker;
use rc_provider::context::ContextWindowManager;
use rc_provider::cost::CostTracker;
use rc_session::SessionStore;

use crate::commands;
use crate::theme::Theme;

pub use crate::commands::SlashCommandAction;
// Re-export for convenience.

/// Handle slash commands via the modular command registry.
#[allow(clippy::too_many_arguments)]
pub fn handle_slash_command(
    input: &str,
    config: &RuntimeConfig,
    store: &SessionStore,
    conversation: &mut Vec<ConversationEntry>,
    context_manager: &ContextWindowManager,
    cost_tracker: &CostTracker,
    broker: &dyn PermissionBroker,
    theme: &mut Theme,
) -> SlashCommandAction {
    commands::dispatch(
        input,
        commands::SlashCommandContext {
            config,
            store,
            conversation,
            context_manager,
            cost_tracker,
            broker,
            theme,
        },
    )
}

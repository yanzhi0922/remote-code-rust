use anyhow::Result;
use rc_config::RuntimeConfig;
use rc_session::SessionStore;

/// Run the interactive shell by delegating to the rc-tui crate.
///
/// The TUI app provides a simple async input loop with:
/// - Multi-turn conversation with the provider
/// - Automatic tool execution with permission checks
/// - Context window compaction
/// - Slash commands for session management
pub(crate) async fn run_interactive_shell(
    config: RuntimeConfig,
    store: &SessionStore,
) -> Result<()> {
    rc_tui::run_tui_app(config, store).await
}

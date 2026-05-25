use claude_config::{RuntimeConfig, SettingSource};

/// Returns `true` when the given setting source is included in the runtime's
/// `allowed_setting_sources` list.
///
/// This check is used across hooks, plugins, MCP, and skills to gate access
/// to user-scoped, project-scoped, and local-scoped configuration.
pub(crate) fn setting_source_enabled(config: &RuntimeConfig, source: SettingSource) -> bool {
    config.allowed_setting_sources.contains(&source)
}

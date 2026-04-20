use anyhow::Result;
use rc_config::RuntimeConfig;
use rc_core::ConversationEntry;
use rc_provider::DiscoveredToolScope;
use rc_runtime_prompt::RuntimePromptSettings;

pub use rc_runtime_prompt::{
    PromptRuntimeOverrides, conversation_with_runtime_user_context_with_settings,
};

#[must_use]
pub fn runtime_prompt_settings(config: &RuntimeConfig) -> RuntimePromptSettings {
    RuntimePromptSettings::from_config(config)
}

pub async fn refresh_runtime_system_prompt(
    config: &RuntimeConfig,
    conversation: &mut Vec<ConversationEntry>,
    overrides: &PromptRuntimeOverrides,
    discovered_tool_scope: &DiscoveredToolScope,
) -> Result<()> {
    let settings = runtime_prompt_settings(config);
    rc_runtime_prompt::refresh_runtime_system_prompt(
        config,
        conversation,
        overrides,
        &settings,
        discovered_tool_scope,
    )
    .await
}

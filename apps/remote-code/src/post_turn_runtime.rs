use std::sync::Arc;

use rc_config::RuntimeConfig;
use rc_core::ConversationEntry;
use rc_provider::{ConversationBackend, DiscoveredToolScope};
use rc_query_engine::QuerySource;
use rc_session::SessionStore;

use crate::conversation::PromptEventSink;
use crate::extract_memories::spawn_extract_memories_after_turn;

#[must_use]
pub(crate) fn should_run_background_extract_memories(
    run_background_extract_memories: bool,
    query_source: QuerySource,
) -> bool {
    run_background_extract_memories && query_source == QuerySource::User
}

pub(crate) fn handle_query_finished_runtime_tasks(
    config: &RuntimeConfig,
    store: &SessionStore,
    backend: Arc<dyn ConversationBackend>,
    discovered_tool_scope: DiscoveredToolScope,
    conversation: &[ConversationEntry],
    event_sink: Option<PromptEventSink>,
    run_background_extract_memories: bool,
    query_source: QuerySource,
) {
    if should_run_background_extract_memories(run_background_extract_memories, query_source) {
        spawn_extract_memories_after_turn(
            config,
            store,
            backend,
            discovered_tool_scope,
            conversation,
            event_sink,
        );
    }
}

#[cfg(test)]
mod tests {
    use rc_query_engine::QuerySource;

    use super::should_run_background_extract_memories;

    #[test]
    fn background_extract_memories_only_runs_for_user_queries() {
        assert!(should_run_background_extract_memories(true, QuerySource::User));
        assert!(!should_run_background_extract_memories(
            false,
            QuerySource::User
        ));
        assert!(!should_run_background_extract_memories(
            true,
            QuerySource::Agent
        ));
        assert!(!should_run_background_extract_memories(
            true,
            QuerySource::SessionMemory
        ));
    }
}

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use anyhow::Result;
use rc_config::RuntimeConfig;
use rc_core::{ConversationEntry, ConversationRole, PermissionMode};
use rc_permissions::{PermissionBroker, PermissionDecision, PermissionRequest};
use rc_provider::{ConversationBackend, DiscoveredToolScope};
use rc_provider::context::TokenEstimator;
use rc_query_engine::QuerySource;
use rc_runtime_prompt::{runtime_env_defined_falsy, runtime_env_truthy};
use rc_session::SessionStore;
use rc_session::session_memory::{
    SessionMemoryConfig, SessionMemoryState, build_session_memory_update_prompt,
    ensure_session_memory_file, load_session_memory_content,
};
use rc_telemetry::growthbook::{FeatureGate, FeatureValue, GrowthBookClient};
use serde_json::{Value, json};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::query_engine_compat::{
    CompatRunOverrides, ForkCacheSafeParams, run_no_persist_forked_query,
};

static SESSION_MEMORY_STATES: OnceLock<
    Mutex<HashMap<Uuid, Arc<std::sync::Mutex<SessionMemoryRuntimeState>>>>,
> = OnceLock::new();
static SESSION_MEMORY_GROWTHBOOK: OnceLock<GrowthBookClient> = OnceLock::new();

#[derive(Debug, Default)]
pub(crate) struct SessionMemoryRuntimeState {
    pub(crate) shared: SessionMemoryState,
    pub(crate) last_memory_message_id: Option<String>,
}

fn session_memory_states()
-> &'static Mutex<HashMap<Uuid, Arc<std::sync::Mutex<SessionMemoryRuntimeState>>>> {
    SESSION_MEMORY_STATES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn growthbook_client() -> &'static GrowthBookClient {
    SESSION_MEMORY_GROWTHBOOK.get_or_init(|| GrowthBookClient::with_defaults())
}

pub(crate) async fn session_memory_state_for_session(
    session_id: Uuid,
) -> Arc<std::sync::Mutex<SessionMemoryRuntimeState>> {
    let mut states = session_memory_states().lock().await;
    states
        .entry(session_id)
        .or_insert_with(|| Arc::new(std::sync::Mutex::new(SessionMemoryRuntimeState::default())))
        .clone()
}

#[derive(Clone, Debug)]
struct SessionMemoryPermissionBroker {
    summary_path: PathBuf,
}

impl SessionMemoryPermissionBroker {
    fn new(summary_path: PathBuf) -> Self {
        Self { summary_path }
    }

    fn is_exact_summary_path(&self, candidate: &Path) -> bool {
        let candidate = candidate
            .canonicalize()
            .unwrap_or_else(|_| candidate.to_path_buf());
        let summary_path = self
            .summary_path
            .canonicalize()
            .unwrap_or_else(|_| self.summary_path.clone());
        candidate == summary_path
    }
}

#[async_trait::async_trait]
impl PermissionBroker for SessionMemoryPermissionBroker {
    fn mode(&self) -> Option<PermissionMode> {
        Some(PermissionMode::DontAsk)
    }

    async fn decide(&self, request: PermissionRequest) -> PermissionDecision {
        match request.tool_name.as_str() {
            "edit_file" => {
                let candidate = request
                    .tool_input
                    .get("path")
                    .or_else(|| request.tool_input.get("file_path"))
                    .and_then(Value::as_str)
                    .map(PathBuf::from);
                if candidate
                    .as_deref()
                    .is_some_and(|path| self.is_exact_summary_path(path))
                {
                    PermissionDecision::allow()
                } else {
                    PermissionDecision::deny(format!(
                        "only Edit on {} is allowed",
                        self.summary_path.display()
                    ))
                }
            }
            _ => PermissionDecision::deny(format!(
                "only Edit on {} is allowed",
                self.summary_path.display()
            )),
        }
    }
}

fn is_auto_compact_enabled(config: &RuntimeConfig) -> Result<bool> {
    if runtime_env_truthy("DISABLE_COMPACT")
        || runtime_env_truthy("CLAUDE_CODE_DISABLE_COMPACT")
        || runtime_env_truthy("REMOTE_CODE_DISABLE_COMPACT")
        || runtime_env_truthy("DISABLE_AUTO_COMPACT")
        || runtime_env_truthy("CLAUDE_CODE_DISABLE_AUTO_COMPACT")
        || runtime_env_truthy("REMOTE_CODE_DISABLE_AUTO_COMPACT")
    {
        return Ok(false);
    }

    let settings = rc_config::settings_layers::load_runtime_settings(&config.settings_files)?;
    Ok(settings.auto_compact_enabled.unwrap_or(true))
}

fn session_memory_gate_enabled(config: &RuntimeConfig) -> Result<bool> {
    if runtime_env_truthy("CLAUDE_CODE_FEATURE_TENGU_SESSION_MEMORY")
        || runtime_env_truthy("REMOTE_CODE_FEATURE_TENGU_SESSION_MEMORY")
        || runtime_env_truthy("TENGU_SESSION_MEMORY")
    {
        return Ok(true);
    }
    if runtime_env_defined_falsy("CLAUDE_CODE_FEATURE_TENGU_SESSION_MEMORY")
        || runtime_env_defined_falsy("REMOTE_CODE_FEATURE_TENGU_SESSION_MEMORY")
        || runtime_env_defined_falsy("TENGU_SESSION_MEMORY")
    {
        return Ok(false);
    }
    if !is_auto_compact_enabled(config)? {
        return Ok(false);
    }
    if runtime_env_truthy("CLAUDE_CODE_REMOTE") || runtime_env_truthy("REMOTE_CODE_REMOTE") {
        return Ok(false);
    }
    Ok(growthbook_client().is_gate_enabled(FeatureGate::SessionMemory))
}

fn session_memory_dynamic_config() -> SessionMemoryConfig {
    let default = SessionMemoryConfig::default();
    let feature_value = growthbook_client()
        .get_all_features()
        .get("tengu_sm_config")
        .cloned();
    let Some(FeatureValue::Json(Value::Object(object))) = feature_value else {
        return default;
    };

    fn positive_u64(value: Option<&Value>) -> Option<u64> {
        value
            .and_then(Value::as_u64)
            .filter(|candidate| *candidate > 0)
    }

    SessionMemoryConfig {
        minimum_message_tokens_to_init: positive_u64(object.get("minimumMessageTokensToInit"))
            .unwrap_or(default.minimum_message_tokens_to_init),
        minimum_tokens_between_update: positive_u64(object.get("minimumTokensBetweenUpdate"))
            .unwrap_or(default.minimum_tokens_between_update),
        tool_calls_between_updates: positive_u64(object.get("toolCallsBetweenUpdates"))
            .unwrap_or(default.tool_calls_between_updates),
    }
}

fn init_session_memory_config_if_needed(state: &mut SessionMemoryRuntimeState) {
    if state.shared.initialized {
        return;
    }
    state.shared.config = session_memory_dynamic_config();
}

fn has_tool_calls_in_last_assistant_turn(conversation: &[ConversationEntry]) -> bool {
    conversation
        .iter()
        .rev()
        .find(|entry| entry.role == ConversationRole::Assistant)
        .is_some_and(|entry| !entry.tool_calls.is_empty())
}

fn count_tool_calls_since(conversation: &[ConversationEntry], since_uuid: Option<Uuid>) -> u64 {
    let mut count = 0u64;
    let mut found_start = since_uuid.is_none();

    for entry in conversation {
        if !found_start {
            if Some(entry.uuid) == since_uuid {
                found_start = true;
            }
            continue;
        }

        if entry.role == ConversationRole::Assistant {
            count += u64::try_from(entry.tool_calls.len()).unwrap_or(u64::MAX);
        }
    }

    count
}

fn update_last_memory_message_id(
    conversation: &[ConversationEntry],
    state: &mut SessionMemoryRuntimeState,
) {
    if let Some(last_message) = conversation.last() {
        state.last_memory_message_id = Some(last_message.uuid.to_string());
    }
}

fn count_conversation_tokens(conversation: &[ConversationEntry]) -> u64 {
    let estimator = TokenEstimator::new();
    conversation
        .iter()
        .map(|entry| {
            let text_tokens = estimator.estimate(&entry.text);
            let tool_tokens = entry
                .tool_calls
                .iter()
                .map(|tool_call| {
                    estimator
                        .estimate(&tool_call.name)
                        .saturating_add(estimator.estimate(&tool_call.input.to_string()))
                })
                .sum::<u64>();
            text_tokens.saturating_add(tool_tokens)
        })
        .sum()
}

fn should_extract_memory(
    conversation: &[ConversationEntry],
    state: &mut SessionMemoryRuntimeState,
) -> bool {
    let current_token_count = count_conversation_tokens(conversation);

    if !state.shared.initialized {
        if !state.shared.has_met_initialization_threshold(current_token_count) {
            return false;
        }
        state.shared.mark_initialized();
    }

    let has_met_token_threshold = state.shared.has_met_update_threshold(current_token_count);
    let last_memory_uuid = state
        .last_memory_message_id
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok());
    let has_met_tool_call_threshold = count_tool_calls_since(conversation, last_memory_uuid)
        >= state.shared.config.tool_calls_between_updates;
    let has_tool_calls_in_last_turn = has_tool_calls_in_last_assistant_turn(conversation);

    let should_extract = (has_met_token_threshold && has_met_tool_call_threshold)
        || (has_met_token_threshold && !has_tool_calls_in_last_turn);

    if should_extract {
        update_last_memory_message_id(conversation, state);
    }

    should_extract
}

fn update_last_summarized_message_id_if_safe(
    conversation: &[ConversationEntry],
    state: &mut SessionMemoryRuntimeState,
) {
    if has_tool_calls_in_last_assistant_turn(conversation) {
        return;
    }
    if let Some(last_message) = conversation.last() {
        state.shared
            .set_last_summarized_message_id(Some(last_message.uuid.to_string()));
    }
}

pub(crate) async fn maybe_spawn_session_memory_update(
    config: &RuntimeConfig,
    _store: &SessionStore,
    backend: Arc<dyn ConversationBackend>,
    discovered_tool_scope: DiscoveredToolScope,
    conversation: &[ConversationEntry],
    fork_snapshot: Option<ForkCacheSafeParams>,
) {
    let gate_enabled = match session_memory_gate_enabled(config) {
        Ok(enabled) => enabled,
        Err(_) => false,
    };
    if !gate_enabled {
        return;
    }

    let state = session_memory_state_for_session(config.session_id).await;
    let mut guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    init_session_memory_config_if_needed(&mut guard);
    if !should_extract_memory(conversation, &mut guard) {
        return;
    }
    guard.shared.mark_extraction_started();
    drop(guard);

    let child_config = config.clone();
    let conversation = conversation.to_vec();
    let discovered_tool_scope = discovered_tool_scope.clone();
    let paths = config.paths.clone();
    let state = state.clone();
    let backend = backend.clone();
    let fork_snapshot =
        fork_snapshot.unwrap_or_else(|| ForkCacheSafeParams::from_conversation(&conversation));
    tokio::spawn(async move {
        let store = match SessionStore::open(paths) {
            Ok(store) => store,
            Err(_) => {
                let mut state = state
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                state.shared.mark_extraction_completed();
                return;
            }
        };
        let _ = run_session_memory_update(
            &child_config,
            &store,
            backend,
            discovered_tool_scope,
            &conversation,
            fork_snapshot,
            state,
        )
        .await;
    });
}

struct SessionMemoryFileSetup {
    summary_path: PathBuf,
    current_memory: String,
}

fn fresh_read_session_memory_content(config: &RuntimeConfig) -> Result<String> {
    Ok(load_session_memory_content(config)?.unwrap_or_default())
}

fn setup_session_memory_file(
    config: &RuntimeConfig,
    store: &SessionStore,
) -> Result<SessionMemoryFileSetup> {
    let summary_path = ensure_session_memory_file(config)?;
    let current_memory = fresh_read_session_memory_content(config)?;
    store.append_named_event(
        config.session_id,
        "tengu_session_memory_file_read",
        json!({
            "content_length": current_memory.len(),
        }),
    )?;
    Ok(SessionMemoryFileSetup {
        summary_path,
        current_memory,
    })
}

fn session_memory_tool_results_dir(config: &RuntimeConfig) -> PathBuf {
    std::env::var_os("CLAUDE_CODE_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("remote-code-session-memory")
        .join(config.session_id.to_string())
}

fn build_session_memory_extraction_prompt(
    config: &RuntimeConfig,
    current_memory: &str,
    summary_path: &Path,
) -> String {
    build_session_memory_update_prompt(config, current_memory, summary_path)
}

async fn run_session_memory_update(
    config: &RuntimeConfig,
    store: &SessionStore,
    backend: Arc<dyn ConversationBackend>,
    discovered_tool_scope: DiscoveredToolScope,
    conversation: &[ConversationEntry],
    fork_snapshot: ForkCacheSafeParams,
    state: Arc<std::sync::Mutex<SessionMemoryRuntimeState>>,
) -> Result<()> {
    let file_setup = setup_session_memory_file(config, store)?;
    let prompt = build_session_memory_extraction_prompt(
        config,
        &file_setup.current_memory,
        &file_setup.summary_path,
    );
    let broker: Arc<dyn PermissionBroker> =
        Arc::new(SessionMemoryPermissionBroker::new(file_setup.summary_path.clone()));
    let discovery = crate::hooks::discover_runtime_hooks(config, &[]);
    let mut hook_state = crate::hooks::HookRunState::load(store, config.session_id)?;

    let run_result = run_no_persist_forked_query(
        config,
        store,
        backend,
        discovered_tool_scope,
        broker,
        &discovery,
        &mut hook_state,
        fork_snapshot,
        &prompt,
        CompatRunOverrides {
            allowed_tools: Some(vec!["Edit".to_owned()]),
            ..CompatRunOverrides::default()
        },
        QuerySource::SessionMemory,
        None,
        session_memory_tool_results_dir(config),
    )
    .await;

    let outcome = match run_result {
        Ok(outcome) => outcome,
        Err(error) => {
            let mut state = state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state.shared.mark_extraction_completed();
            return Err(error);
        }
    };

    let guard = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    store.append_named_event(
        config.session_id,
        "tengu_session_memory_extraction",
        json!({
            "input_tokens": outcome.usage.input_tokens,
            "output_tokens": outcome.usage.output_tokens,
            "cache_read_input_tokens": outcome.cache_read_input_tokens,
            "cache_creation_input_tokens": outcome.cache_creation_input_tokens,
            "config_min_message_tokens_to_init": guard.shared.config.minimum_message_tokens_to_init,
            "config_min_tokens_between_update": guard.shared.config.minimum_tokens_between_update,
            "config_tool_calls_between_updates": guard.shared.config.tool_calls_between_updates,
        }),
    )?;
    drop(guard);

    let mut state = state
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    state
        .shared
        .record_extraction_token_count(count_conversation_tokens(conversation));
    update_last_summarized_message_id_if_safe(conversation, &mut state);
    state.shared.mark_extraction_completed();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::SessionMemoryPermissionBroker;
    use rc_permissions::{PermissionBroker, PermissionRequest};
    use serde_json::json;
    use tempfile::tempdir;

    #[tokio::test]
    async fn session_memory_broker_allows_exact_edit_file_path_only() {
        let tempdir = tempdir().expect("tempdir");
        let summary_path = tempdir.path().join("summary.md");
        std::fs::write(&summary_path, "# Session Title\n").expect("write summary");
        let broker = SessionMemoryPermissionBroker::new(summary_path.clone());

        let allow = broker
            .decide(PermissionRequest {
                tool_name: "edit_file".to_owned(),
                permission_class: None,
                tool_input: json!({
                    "path": summary_path,
                    "edits": [{"search": "# Session Title", "replace": "# Session Title"}]
                }),
                working_directory: None,
                tool_use_id: None,
                title: None,
                description: None,
                blocked_path: None,
                permission_suggestions: Vec::new(),
            })
            .await;
        assert!(allow.allowed);

        let allow_file_path = broker
            .decide(PermissionRequest {
                tool_name: "edit_file".to_owned(),
                permission_class: None,
                tool_input: json!({
                    "file_path": summary_path,
                    "edits": [{"search": "# Session Title", "replace": "# Session Title"}]
                }),
                working_directory: None,
                tool_use_id: None,
                title: None,
                description: None,
                blocked_path: None,
                permission_suggestions: Vec::new(),
            })
            .await;
        assert!(allow_file_path.allowed);
    }

    #[tokio::test]
    async fn session_memory_broker_denies_replace_in_file_and_wrong_path() {
        let tempdir = tempdir().expect("tempdir");
        let summary_path = tempdir.path().join("summary.md");
        let other_path = tempdir.path().join("other.md");
        std::fs::write(&summary_path, "# Session Title\n").expect("write summary");
        std::fs::write(&other_path, "# Other\n").expect("write other");
        let broker = SessionMemoryPermissionBroker::new(summary_path.clone());

        let deny_replace = broker
            .decide(PermissionRequest {
                tool_name: "replace_in_file".to_owned(),
                permission_class: None,
                tool_input: json!({
                    "path": summary_path,
                    "search": "a",
                    "replace": "b"
                }),
                working_directory: None,
                tool_use_id: None,
                title: None,
                description: None,
                blocked_path: None,
                permission_suggestions: Vec::new(),
            })
            .await;
        assert!(!deny_replace.allowed);

        let deny_other_path = broker
            .decide(PermissionRequest {
                tool_name: "edit_file".to_owned(),
                permission_class: None,
                tool_input: json!({
                    "path": other_path,
                    "edits": [{"search": "# Other", "replace": "# Other"}]
                }),
                working_directory: None,
                tool_use_id: None,
                title: None,
                description: None,
                blocked_path: None,
                permission_suggestions: Vec::new(),
            })
            .await;
        assert!(!deny_other_path.allowed);
    }
}

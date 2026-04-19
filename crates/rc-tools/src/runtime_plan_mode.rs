use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, RwLock};

use anyhow::{Result, anyhow};
use async_trait::async_trait;
use base64::Engine;
use chrono::Utc;
use rc_config::{AppPaths, RuntimeConfig};
use rc_core::{
    Attachment, AttachmentMediaType, ConversationEntry, ConversationRole, PermissionMode,
};
use rc_permissions::{
    LayeredPermissionBroker, PermissionBroker, PermissionClass, PermissionDecision,
    PermissionRequest, auto_allows, load_layered_rules,
};
use rc_session::{SessionStore, plan_state::PlanModeState};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::plan_mode::{self, PlanModeRuntime, PlanModeRuntimeSnapshot};

const PLAN_MODE_MARKER: &str = "## Plan Mode Active";
const PLAN_MODE_REENTRY_MARKER: &str = "## Re-entering Plan Mode";
const PLAN_MODE_EXIT_MARKER: &str = "## Exited Plan Mode";
const APPROVED_PLAN_MARKER: &str = "Approved plan:\n";
const TRUNCATED_TOOL_OUTPUT_MARKER: &str = "... [truncated:";
const FILE_SNAPSHOT_EVENT: &str = "file_snapshot";
const PLAN_CONTENT_EVENT: &str = "plan_content";
const PLAN_FILE_SNAPSHOT_KEY: &str = "plan";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileSnapshotEntry {
    key: String,
    path: PathBuf,
    content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileSnapshotEvent {
    files: Vec<FileSnapshotEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PlanContentEvent {
    path: PathBuf,
    content: String,
}

pub struct RuntimePlanModeController {
    store: Arc<SessionStore>,
    session_id: Uuid,
    cwd: PathBuf,
    plans_dir: PathBuf,
    state: RwLock<PlanModeState>,
}

impl std::fmt::Debug for RuntimePlanModeController {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimePlanModeController")
            .field("session_id", &self.session_id)
            .field("cwd", &self.cwd)
            .field("plans_dir", &self.plans_dir)
            .field("current_mode", &self.current_mode())
            .finish_non_exhaustive()
    }
}

impl RuntimePlanModeController {
    pub fn load(config: &RuntimeConfig, store: &SessionStore) -> Result<Arc<Self>> {
        let owned_store = Arc::new(SessionStore::open(store.paths().clone())?);
        Self::from_owned_store(config, owned_store)
    }

    fn from_owned_store(config: &RuntimeConfig, store: Arc<SessionStore>) -> Result<Arc<Self>> {
        let state = restore_plan_mode_state_for_resume(&store, &config.paths, config.session_id)?
            .unwrap_or_else(|| PlanModeState {
                current_permission_mode: config.permission_mode,
                ..PlanModeState::default()
            });
        let controller = Arc::new(Self {
            store,
            session_id: config.session_id,
            cwd: config.cwd.clone(),
            plans_dir: config.paths.profile_dir.join("plans"),
            state: RwLock::new(state),
        });
        if controller
            .state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .plan_slug
            .is_some()
        {
            controller.ensure_plan_paths_if_needed()?;
        }
        Ok(controller)
    }

    pub fn current_mode(&self) -> PermissionMode {
        self.state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .current_permission_mode
    }

    pub fn plan_file_matches_request(&self, request: &PermissionRequest) -> bool {
        let Some(raw_path) = request
            .tool_input
            .get("path")
            .or_else(|| request.tool_input.get("file_path"))
            .and_then(serde_json::Value::as_str)
        else {
            return false;
        };
        self.plan_file_matches_path(raw_path)
    }

    pub fn plan_file_matches_path(&self, raw_path: &str) -> bool {
        let state = self
            .state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let Some(plan_file_path) = state.plan_file_path.as_ref() else {
            return false;
        };
        normalize_joined_path(raw_path, &self.cwd) == normalize_path(plan_file_path)
    }

    pub fn snapshot_state(&self) -> PlanModeState {
        self.state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn activate_for_slash_command(&self, objective: Option<&str>) -> Result<()> {
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.ensure_plan_descriptor_locked(&mut state)?;
        if !state.is_plan_mode() {
            state.pre_plan_permission_mode = Some(state.current_permission_mode);
        }
        state.current_permission_mode = PermissionMode::Plan;
        if let Some(objective) = objective.map(str::trim).filter(|value| !value.is_empty()) {
            state.plan_objective = Some(objective.to_owned());
        }
        state.needs_plan_mode_exit_attachment = false;
        self.persist_locked(&mut state)
    }

    fn persist_plan_snapshot_for_path(&self, plan_file_path: &Path) -> Result<()> {
        if !plan_file_path.exists() {
            return Ok(());
        }
        let plan_content = fs::read_to_string(plan_file_path)?;
        if plan_content.trim().is_empty() {
            return Ok(());
        }

        self.store.append_named_event(
            self.session_id,
            FILE_SNAPSHOT_EVENT,
            serde_json::to_value(FileSnapshotEvent {
                files: vec![FileSnapshotEntry {
                    key: PLAN_FILE_SNAPSHOT_KEY.to_owned(),
                    path: plan_file_path.to_path_buf(),
                    content: plan_content.clone(),
                }],
            })?,
        )?;
        self.store.append_named_event(
            self.session_id,
            PLAN_CONTENT_EVENT,
            serde_json::to_value(PlanContentEvent {
                path: plan_file_path.to_path_buf(),
                content: plan_content,
            })?,
        )
    }

    fn persist_plan_snapshot_inner(&self) -> Result<()> {
        let plan_file_path = self
            .state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .plan_file_path
            .clone();
        let Some(plan_file_path) = plan_file_path else {
            return Ok(());
        };
        self.persist_plan_snapshot_for_path(&plan_file_path)
    }

    fn ensure_plan_paths_if_needed(&self) -> Result<()> {
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let changed = self.ensure_plan_descriptor_locked(&mut state)?;
        if changed {
            self.persist_locked(&mut state)?;
        }
        Ok(())
    }

    fn ensure_plan_descriptor_locked(&self, state: &mut PlanModeState) -> Result<bool> {
        let mut changed = false;
        if state.plan_slug.is_none() {
            let slug = format!("plan-{}", &Uuid::new_v4().simple().to_string()[..12]);
            state.plan_slug = Some(slug);
            changed = true;
        }
        if state.plan_id.is_none() {
            state.plan_id = Some(format!("plan-{}", Uuid::new_v4().simple()));
            changed = true;
        }

        let Some(plan_slug) = state.plan_slug.as_deref() else {
            return Err(anyhow!("plan slug missing after initialization"));
        };

        let plan_file_path = self.plans_dir.join(format!("{plan_slug}.md"));
        if state.plan_file_path.as_ref() != Some(&plan_file_path) {
            state.plan_file_path = Some(plan_file_path);
            changed = true;
        }
        std::fs::create_dir_all(&self.plans_dir)?;
        Ok(changed)
    }

    fn persist_locked(&self, state: &mut PlanModeState) -> Result<()> {
        state.updated_at = Utc::now();
        self.store.save_plan_mode_state(self.session_id, state)
    }

    fn enter_plan_mode_message(&self, state: &PlanModeState, objective: &str) -> String {
        let plan_file_path = state
            .plan_file_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "(missing)".to_owned());

        format!(
            "Entered plan mode.\n\nObjective: {objective}\nPlan file: {plan_file_path}\n\n{PLAN_MODE_MARKER}\n\nPlan mode is active. You MUST NOT make code edits, run non-read-only tools, or otherwise change the system. The only file you may edit is the plan file above.\n\nUse read-only tools to inspect the codebase, capture findings in the plan file as you go, ask clarifying questions when needed, and call `exit_plan_mode` when the plan is ready."
        )
    }

    fn exit_plan_mode_message(&self, state: &PlanModeState) -> String {
        let plan_file_path = state
            .plan_file_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "(missing)".to_owned());
        let plan = state
            .plan_file_path
            .as_ref()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .filter(|content| !content.trim().is_empty());

        match plan {
            Some(plan) => format!(
                "Exited plan mode. You can now proceed with implementation.\n\nPlan file: {plan_file_path}\n\nApproved plan:\n{plan}"
            ),
            None => format!(
                "Exited plan mode. You can now proceed with implementation.\n\nPlan file: {plan_file_path}"
            ),
        }
    }
}

impl PlanModeRuntime for RuntimePlanModeController {
    fn enter_plan_mode(&self, objective: &str) -> Result<String> {
        let objective = objective.trim();
        if objective.is_empty() {
            return Err(anyhow!("objective cannot be empty"));
        }

        let mut state = self
            .state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.ensure_plan_descriptor_locked(&mut state)?;
        if !state.is_plan_mode() {
            state.pre_plan_permission_mode = Some(state.current_permission_mode);
        }
        state.current_permission_mode = PermissionMode::Plan;
        state.plan_objective = Some(objective.to_owned());
        state.needs_plan_mode_exit_attachment = false;
        self.persist_locked(&mut state)?;
        Ok(self.enter_plan_mode_message(&state, objective))
    }

    fn exit_plan_mode(
        &self,
        _plan_summary: Option<&str>,
        _steps_planned: &[String],
    ) -> Result<String> {
        let mut state = self
            .state
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !state.is_plan_mode() {
            return Ok(
                "You are not currently in plan mode. Continue implementing or enter plan mode first."
                    .to_owned(),
            );
        }

        let restored_mode = state
            .pre_plan_permission_mode
            .take()
            .unwrap_or(PermissionMode::Default);
        state.current_permission_mode = restored_mode;
        state.has_exited_plan_mode = true;
        state.needs_plan_mode_exit_attachment = true;
        if let Some(plan_file_path) = state.plan_file_path.clone() {
            let _ = self.persist_plan_snapshot_for_path(&plan_file_path);
        }
        self.persist_locked(&mut state)?;
        Ok(self.exit_plan_mode_message(&state))
    }

    fn snapshot(&self) -> PlanModeRuntimeSnapshot {
        let state = self
            .state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        PlanModeRuntimeSnapshot {
            permission_mode: state.current_permission_mode,
            plan_file_path: state.plan_file_path.clone(),
        }
    }

    fn persist_plan_snapshot(&self) -> Result<()> {
        self.persist_plan_snapshot_inner()
    }
}

#[derive(Debug)]
struct DynamicPermissionFallbackBroker {
    controller: Arc<RuntimePlanModeController>,
}

#[async_trait]
impl PermissionBroker for DynamicPermissionFallbackBroker {
    async fn decide(&self, request: PermissionRequest) -> PermissionDecision {
        let mode = self.controller.current_mode();
        if auto_allows(mode, request.resolved_permission_class()) {
            return PermissionDecision::allow();
        }
        PermissionDecision::deny("Permission denied by runtime broker")
    }

    fn mode(&self) -> Option<PermissionMode> {
        Some(self.controller.current_mode())
    }
}

pub struct RuntimePermissionBroker {
    controller: Arc<RuntimePlanModeController>,
    inner: LayeredPermissionBroker<DynamicPermissionFallbackBroker>,
}

impl RuntimePermissionBroker {
    fn new(config: &RuntimeConfig, controller: Arc<RuntimePlanModeController>) -> Self {
        let rules = load_layered_rules(
            &config.cwd,
            &config.paths.profile_dir,
            &config.settings_files,
            &config.cli_settings_files,
        );
        let inner = LayeredPermissionBroker::new(
            DynamicPermissionFallbackBroker {
                controller: controller.clone(),
            },
            rules,
        );
        Self { controller, inner }
    }

    fn decide_plan_mode(&self, request: PermissionRequest) -> PermissionDecision {
        match request.resolved_permission_class() {
            PermissionClass::Read => PermissionDecision::allow(),
            PermissionClass::Edit if self.controller.plan_file_matches_request(&request) => {
                PermissionDecision::allow()
            }
            PermissionClass::Edit => PermissionDecision::deny(
                "Plan mode is active. Only the current plan file may be edited.",
            ),
            _ => PermissionDecision::deny(
                "Plan mode is active. Only read-only tools and plan-file edits are allowed.",
            ),
        }
    }
}

#[async_trait]
impl PermissionBroker for RuntimePermissionBroker {
    async fn decide(&self, request: PermissionRequest) -> PermissionDecision {
        if self.controller.current_mode() == PermissionMode::Plan {
            return self.decide_plan_mode(request);
        }
        self.inner.decide(request).await
    }

    fn add_session_rule(
        &self,
        action: rc_permissions::RuleAction,
        tool_pattern: String,
    ) -> Result<()> {
        self.inner.add_session_rule(action, tool_pattern)
    }

    fn clear_session_rules(&self) -> Result<usize> {
        self.inner.clear_session_rules()
    }

    fn mode(&self) -> Option<PermissionMode> {
        Some(self.controller.current_mode())
    }

    fn audit_records(&self) -> Vec<rc_permissions::PermissionAuditRecord> {
        self.inner.audit_records()
    }

    fn layered_rules(&self) -> Vec<rc_permissions::SourceAwarePermissionRule> {
        self.inner.layered_rules()
    }
}

pub struct ActivePlanModeRuntime;

impl Drop for ActivePlanModeRuntime {
    fn drop(&mut self) {
        let _ = plan_mode::configure_plan_mode_runtime(None);
    }
}

pub fn install_plan_mode_runtime(
    controller: Arc<RuntimePlanModeController>,
) -> Result<ActivePlanModeRuntime> {
    plan_mode::configure_plan_mode_runtime(Some(controller))?;
    Ok(ActivePlanModeRuntime)
}

pub fn build_runtime_plan_mode(
    config: &RuntimeConfig,
    store: &SessionStore,
) -> Result<(Arc<RuntimePlanModeController>, Arc<dyn PermissionBroker>)> {
    let controller = RuntimePlanModeController::load(config, store)?;
    let broker: Arc<dyn PermissionBroker> =
        Arc::new(RuntimePermissionBroker::new(config, controller.clone()));
    Ok((controller, broker))
}

pub fn restore_plan_mode_state_for_resume(
    store: &SessionStore,
    paths: &AppPaths,
    session_id: Uuid,
) -> Result<Option<PlanModeState>> {
    let Some(mut state) = store.load_plan_mode_state(session_id)? else {
        return Ok(None);
    };

    let snapshot = latest_plan_file_snapshot(store, session_id)?;
    let target_plan_path = resolve_resume_plan_file_path(paths, &state, snapshot.as_ref());
    let Some(target_plan_path) = target_plan_path else {
        return Ok(Some(state));
    };

    let mut changed = false;
    if state.plan_file_path.as_ref() != Some(&target_plan_path) {
        state.plan_file_path = Some(target_plan_path.clone());
        changed = true;
    }
    if state.plan_slug.is_none()
        && let Some(slug) = target_plan_path.file_stem().and_then(|stem| stem.to_str())
    {
        state.plan_slug = Some(slug.to_owned());
        changed = true;
    }

    if !target_plan_path.exists() {
        let recovered = snapshot
            .as_ref()
            .map(|entry| entry.content.clone())
            .or_else(|| latest_plan_content_event(store, session_id).ok().flatten())
            .or_else(|| {
                recover_plan_content_from_conversation(store, session_id)
                    .ok()
                    .flatten()
            });
        if let Some(plan_content) = recovered.filter(|content| !content.trim().is_empty()) {
            if let Some(parent) = target_plan_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&target_plan_path, plan_content)?;
        }
    }

    if changed {
        state.updated_at = Utc::now();
        store.save_plan_mode_state(session_id, &state)?;
    }
    Ok(Some(state))
}

fn resolve_resume_plan_file_path(
    paths: &AppPaths,
    state: &PlanModeState,
    snapshot: Option<&FileSnapshotEntry>,
) -> Option<PathBuf> {
    state
        .plan_file_path
        .clone()
        .or_else(|| snapshot.map(|entry| entry.path.clone()))
        .or_else(|| {
            state
                .plan_slug
                .as_deref()
                .map(|slug| paths.profile_dir.join("plans").join(format!("{slug}.md")))
        })
}

fn latest_plan_file_snapshot(
    store: &SessionStore,
    session_id: Uuid,
) -> Result<Option<FileSnapshotEntry>> {
    let transcript = store.load_transcript(session_id)?;
    Ok(transcript
        .latest_named_event_as::<FileSnapshotEvent>(FILE_SNAPSHOT_EVENT)?
        .and_then(|event| {
            event.files.into_iter().find(|entry| {
                entry.key == PLAN_FILE_SNAPSHOT_KEY && !entry.content.trim().is_empty()
            })
        }))
}

fn latest_plan_content_event(store: &SessionStore, session_id: Uuid) -> Result<Option<String>> {
    let transcript = store.load_transcript(session_id)?;
    Ok(transcript
        .latest_named_event_as::<PlanContentEvent>(PLAN_CONTENT_EVENT)?
        .map(|event| event.content)
        .filter(|content| !content.trim().is_empty()))
}

fn recover_plan_content_from_conversation(
    store: &SessionStore,
    session_id: Uuid,
) -> Result<Option<String>> {
    let conversation = store.load_conversation(session_id)?;
    Ok(conversation.iter().rev().find_map(|entry| {
        if entry.role == ConversationRole::User {
            let attachment_plan = entry
                .attachments
                .iter()
                .find_map(recover_plan_content_from_attachment);
            if attachment_plan.is_some() {
                return attachment_plan;
            }
        }
        if entry.role == ConversationRole::Assistant {
            let assistant_plan = entry
                .tool_calls
                .iter()
                .rev()
                .find(|tool_call| tool_call.name == "exit_plan_mode")
                .and_then(|tool_call| tool_call.input.get("plan"))
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|plan| !plan.is_empty())
                .map(str::to_owned);
            if assistant_plan.is_some() {
                return assistant_plan;
            }
        }
        if entry.role != ConversationRole::Tool
            || entry.name.as_deref() != Some("exit_plan_mode")
            || entry.text.contains(TRUNCATED_TOOL_OUTPUT_MARKER)
        {
            return None;
        }
        entry
            .text
            .split_once(APPROVED_PLAN_MARKER)
            .map(|(_, plan)| plan.trim_end().to_owned())
            .filter(|plan| !plan.is_empty())
    }))
}

fn recover_plan_content_from_attachment(attachment: &Attachment) -> Option<String> {
    if attachment.media_type != AttachmentMediaType::ApplicationPdf {
        return None;
    }

    let filename = attachment.filename.as_deref().unwrap_or_default();
    let lower = filename.to_ascii_lowercase();
    if !lower.ends_with(".md") && !lower.contains("plan") {
        return None;
    }

    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&attachment.data)
        .ok()?;
    let decoded = String::from_utf8(bytes).ok()?;
    let trimmed = decoded.trim();
    (!trimmed.is_empty()).then_some(trimmed.to_owned())
}

pub fn copy_plan_mode_state_for_fork(
    store: &SessionStore,
    paths: &AppPaths,
    source_session_id: Uuid,
    target_session_id: Uuid,
) -> Result<Option<PlanModeState>> {
    let Some(source_state) = restore_plan_mode_state_for_resume(store, paths, source_session_id)?
    else {
        return Ok(None);
    };

    let plans_dir = paths.profile_dir.join("plans");
    fs::create_dir_all(&plans_dir)?;

    let mut target_state = source_state.clone();
    let new_slug = format!("plan-{}", &Uuid::new_v4().simple().to_string()[..12]);
    let target_plan_path = plans_dir.join(format!("{new_slug}.md"));
    if let Some(source_plan_path) = source_state
        .plan_file_path
        .clone()
        .or_else(|| {
            source_state
                .plan_slug
                .as_deref()
                .map(|slug| plans_dir.join(format!("{slug}.md")))
        })
        .filter(|path| path.exists())
    {
        let _copied = fs::copy(source_plan_path, &target_plan_path)?;
    }

    target_state.updated_at = Utc::now();
    target_state.parent_session_id = Some(source_session_id);
    target_state.plan_id = Some(format!("plan-{}", Uuid::new_v4().simple()));
    target_state.plan_slug = Some(new_slug);
    target_state.plan_file_path = Some(target_plan_path);
    store.save_plan_mode_state(target_session_id, &target_state)?;
    Ok(Some(target_state))
}

pub fn inject_plan_mode_runtime_messages(
    store: &SessionStore,
    session_id: Uuid,
    conversation: &mut Vec<ConversationEntry>,
) -> Result<()> {
    let Some(mut state) = store.load_plan_mode_state(session_id)? else {
        return Ok(());
    };

    if state.current_permission_mode == PermissionMode::Plan {
        let plan_file_path = state
            .plan_file_path
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "(missing)".to_owned());
        let reminder = if state.has_exited_plan_mode
            && state
                .plan_file_path
                .as_ref()
                .is_some_and(|path| path.exists())
        {
            state.has_exited_plan_mode = false;
            format!(
                "{PLAN_MODE_REENTRY_MARKER}\n\nYou are returning to plan mode after having previously exited it. A plan file exists at {plan_file_path}.\n\nRead the existing plan, decide whether this is the same task or a fresh plan, update the plan file accordingly, and only then continue planning."
            )
        } else if conversation.iter().any(|entry| {
            entry.role == ConversationRole::User
                && (entry.text.contains(PLAN_MODE_MARKER)
                    || entry.text.contains(PLAN_MODE_REENTRY_MARKER))
        }) {
            String::new()
        } else {
            format!(
                "{PLAN_MODE_MARKER}\n\nPlan mode is active. You must remain read-only except for the current plan file: {plan_file_path}.\n\nUse read-only tools to inspect the project, update the plan file as you learn, ask clarifying questions when needed, and use `exit_plan_mode` when the plan is ready."
            )
        };

        if !reminder.is_empty() {
            append_reminder_message(store, session_id, conversation, reminder)?;
        }
    } else if state.needs_plan_mode_exit_attachment {
        state.needs_plan_mode_exit_attachment = false;
        let plan_reference = state
            .plan_file_path
            .as_ref()
            .map(|path| format!("\n\nPlan file: {}", path.display()))
            .unwrap_or_default();
        append_reminder_message(
            store,
            session_id,
            conversation,
            format!(
                "{PLAN_MODE_EXIT_MARKER}\n\nYou have exited plan mode. You can now make edits, run tools, and continue implementation.{plan_reference}"
            ),
        )?;
    }

    state.updated_at = Utc::now();
    store.save_plan_mode_state(session_id, &state)?;
    Ok(())
}

fn append_reminder_message(
    store: &SessionStore,
    session_id: Uuid,
    conversation: &mut Vec<ConversationEntry>,
    text: String,
) -> Result<()> {
    if conversation
        .iter()
        .rev()
        .take(4)
        .any(|entry| entry.role == ConversationRole::User && entry.text == text)
    {
        return Ok(());
    }
    let entry = ConversationEntry::user(text);
    store.append_conversation_entry(session_id, &entry)?;
    conversation.push(entry);
    Ok(())
}

fn normalize_joined_path(path: &str, cwd: &Path) -> PathBuf {
    let candidate = PathBuf::from(path);
    if candidate.is_absolute() {
        normalize_path(candidate)
    } else {
        normalize_path(cwd.join(candidate))
    }
}

fn normalize_path(path: impl Into<PathBuf>) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.into().components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use std::fs;

    use rc_permissions::{PermissionBroker, PermissionClass, PermissionRequest};
    use serde_json::json;

    use super::{
        PLAN_CONTENT_EVENT, RuntimePermissionBroker, copy_plan_mode_state_for_fork,
        restore_plan_mode_state_for_resume,
    };
    use crate::plan_mode::PlanModeRuntime;
    use rc_config::{ProviderOverrides, RuntimeOverrides, load_runtime_config};
    use rc_core::{
        Attachment, AttachmentMediaType, ConversationEntry, InputFormat, OutputFormat,
        PermissionMode,
    };
    use rc_session::SessionStore;
    use tempfile::tempdir;
    use uuid::Uuid;

    use super::RuntimePlanModeController;

    fn test_config_and_store() -> (rc_config::RuntimeConfig, SessionStore) {
        let tempdir = tempdir().expect("tempdir");
        let config = load_runtime_config(
            Some(tempdir.path().join("workspace")),
            Some(tempdir.path().join(".remote-code-rust")),
            None,
            PermissionMode::Default,
            InputFormat::Text,
            OutputFormat::Text,
            false,
            false,
            false,
            false,
            4,
            ProviderOverrides {
                provider: Some("mock".to_owned()),
                base_url: Some("mock://provider".to_owned()),
                api_key: Some("mock".to_owned()),
                model: Some("mock-model".to_owned()),
                protocol: Some(rc_core::ProviderProtocol::Anthropic),
            },
            RuntimeOverrides::default(),
        )
        .expect("config");
        let store = SessionStore::open(config.paths.clone()).expect("store");
        (config, store)
    }

    fn permission_request(tool_name: &str, class: PermissionClass) -> PermissionRequest {
        PermissionRequest {
            tool_name: tool_name.to_owned(),
            permission_class: Some(class),
            tool_input: json!({}),
            working_directory: None,
            tool_use_id: Some(format!("tool-{tool_name}")),
            title: None,
            description: None,
            blocked_path: None,
        }
    }

    #[tokio::test]
    async fn runtime_permission_broker_dont_ask_denies_agent_and_edit() {
        let (mut config, store) = test_config_and_store();
        config.permission_mode = PermissionMode::DontAsk;
        let controller = RuntimePlanModeController::load(&config, &store).expect("controller");
        let broker = RuntimePermissionBroker::new(&config, controller);

        let read = broker
            .decide(permission_request("read_file", PermissionClass::Read))
            .await;
        assert!(read.allowed, "dont-ask should still allow reads");

        let agent = broker
            .decide(permission_request("agent", PermissionClass::Agent))
            .await;
        assert!(!agent.allowed, "dont-ask should deny agent execution");
        assert_eq!(
            agent.message.as_deref(),
            Some("Permission denied by runtime broker")
        );

        let edit = broker
            .decide(permission_request("edit_file", PermissionClass::Edit))
            .await;
        assert!(!edit.allowed, "dont-ask should deny edits");
        assert_eq!(
            edit.message.as_deref(),
            Some("Permission denied by runtime broker")
        );
    }

    #[tokio::test]
    async fn runtime_permission_broker_accept_edits_and_bypass_match_mode_semantics() {
        let (mut config, store) = test_config_and_store();
        config.permission_mode = PermissionMode::AcceptEdits;
        let controller = RuntimePlanModeController::load(&config, &store).expect("controller");
        let accept_edits = RuntimePermissionBroker::new(&config, controller);

        let edit = accept_edits
            .decide(permission_request("edit_file", PermissionClass::Edit))
            .await;
        assert!(edit.allowed, "accept-edits should allow file edits");

        let agent = accept_edits
            .decide(permission_request("agent", PermissionClass::Agent))
            .await;
        assert!(
            !agent.allowed,
            "accept-edits should still deny agent execution"
        );

        config.permission_mode = PermissionMode::BypassPermissions;
        let controller = RuntimePlanModeController::load(&config, &store).expect("controller");
        let bypass = RuntimePermissionBroker::new(&config, controller);

        let agent = bypass
            .decide(permission_request("agent", PermissionClass::Agent))
            .await;
        assert!(
            agent.allowed,
            "bypass-permissions should allow agent execution"
        );

        let bash = bypass
            .decide(permission_request("bash_command", PermissionClass::Bash))
            .await;
        assert!(
            bash.allowed,
            "bypass-permissions should allow bash execution"
        );
    }

    #[test]
    fn fork_plan_mode_state_gets_new_slug_and_copies_plan_file() {
        let (config, store) = test_config_and_store();
        let controller = RuntimePlanModeController::load(&config, &store).expect("controller");
        controller
            .enter_plan_mode("audit runtime")
            .expect("enter plan mode");
        let source_state = controller.snapshot_state();
        let source_plan_path = source_state
            .plan_file_path
            .clone()
            .expect("source plan path");
        fs::write(&source_plan_path, "# Plan\n- inspect\n").expect("write plan");

        let target_session_id = Uuid::new_v4();
        store
            .ensure_session_with_parent(
                target_session_id,
                &config.cwd,
                &config.provider.name,
                config.provider.model.as_deref(),
                Some("fork child"),
                Some(config.session_id),
            )
            .expect("child session");

        let target_state = copy_plan_mode_state_for_fork(
            &store,
            &config.paths,
            config.session_id,
            target_session_id,
        )
        .expect("copy fork plan")
        .expect("fork plan state");

        assert_eq!(target_state.parent_session_id, Some(config.session_id));
        assert_ne!(target_state.plan_slug, source_state.plan_slug);
        let target_plan_path = target_state.plan_file_path.expect("target plan path");
        assert!(target_plan_path.exists());
        assert_eq!(
            fs::read_to_string(target_plan_path).expect("read target plan"),
            "# Plan\n- inspect\n"
        );
    }

    #[test]
    fn resume_restores_missing_plan_file_from_snapshot() {
        let (config, store) = test_config_and_store();
        let controller = RuntimePlanModeController::load(&config, &store).expect("controller");
        controller
            .enter_plan_mode("audit runtime")
            .expect("enter plan mode");
        let state = controller.snapshot_state();
        let plan_path = state.plan_file_path.clone().expect("plan path");
        fs::write(&plan_path, "# Plan\n- snapshot restore\n").expect("write plan");

        controller
            .persist_plan_snapshot()
            .expect("persist plan snapshot");
        fs::remove_file(&plan_path).expect("remove plan file");

        let restored = restore_plan_mode_state_for_resume(&store, &config.paths, config.session_id)
            .expect("restore state")
            .expect("state should exist");
        let restored_path = restored.plan_file_path.expect("restored plan path");

        assert!(restored_path.exists());
        assert_eq!(
            fs::read_to_string(restored_path).expect("read restored plan"),
            "# Plan\n- snapshot restore\n"
        );
    }

    #[test]
    fn resume_falls_back_to_exit_plan_mode_tool_result_when_snapshot_missing() {
        let (config, store) = test_config_and_store();
        let controller = RuntimePlanModeController::load(&config, &store).expect("controller");
        controller
            .enter_plan_mode("audit runtime")
            .expect("enter plan mode");
        let state = controller.snapshot_state();
        let plan_path = state.plan_file_path.clone().expect("plan path");
        fs::write(&plan_path, "# Plan\n- exit fallback\n").expect("write plan");

        let exit_message = controller
            .exit_plan_mode(None, &[])
            .expect("exit plan mode");
        store
            .append_conversation_entry(
                config.session_id,
                &ConversationEntry::tool("tool-1", "exit_plan_mode", exit_message, false),
            )
            .expect("append tool result");
        fs::remove_file(&plan_path).expect("remove plan file");

        let restored = restore_plan_mode_state_for_resume(&store, &config.paths, config.session_id)
            .expect("restore state")
            .expect("state should exist");
        let restored_path = restored.plan_file_path.expect("restored plan path");

        assert!(restored_path.exists());
        assert_eq!(
            fs::read_to_string(restored_path).expect("read restored plan"),
            "# Plan\n- exit fallback\n"
        );
    }

    #[test]
    fn resume_restores_missing_plan_file_from_named_event_carrier() {
        let (config, store) = test_config_and_store();
        let controller = RuntimePlanModeController::load(&config, &store).expect("controller");
        controller
            .enter_plan_mode("audit runtime")
            .expect("enter plan mode");
        let state = controller.snapshot_state();
        let plan_path = state.plan_file_path.clone().expect("plan path");
        store
            .append_named_event(
                config.session_id,
                PLAN_CONTENT_EVENT,
                serde_json::json!({
                    "path": plan_path,
                    "content": "# Plan\n- named event carrier\n",
                }),
            )
            .expect("append named event");

        let plan_path = state.plan_file_path.expect("plan path");
        if plan_path.exists() {
            fs::remove_file(&plan_path).expect("remove plan file");
        }

        let restored = restore_plan_mode_state_for_resume(&store, &config.paths, config.session_id)
            .expect("restore state")
            .expect("state should exist");
        let restored_path = restored.plan_file_path.expect("restored plan path");

        assert!(restored_path.exists());
        assert_eq!(
            fs::read_to_string(restored_path).expect("read restored plan"),
            "# Plan\n- named event carrier\n"
        );
    }

    #[test]
    fn resume_recovers_plan_from_assistant_exit_plan_mode_input() {
        let (config, store) = test_config_and_store();
        let controller = RuntimePlanModeController::load(&config, &store).expect("controller");
        controller
            .enter_plan_mode("audit runtime")
            .expect("enter plan mode");
        let state = controller.snapshot_state();
        let plan_path = state.plan_file_path.clone().expect("plan path");
        fs::write(&plan_path, "# Plan\n- assistant carrier\n").expect("write plan");
        fs::remove_file(&plan_path).expect("remove plan file");

        let mut assistant = ConversationEntry::assistant("");
        assistant.tool_calls.push(rc_core::ToolCall {
            id: "tool-1".to_owned(),
            name: "exit_plan_mode".to_owned(),
            input: serde_json::json!({
                "plan": "# Plan\n- assistant carrier\n",
                "plan_file_path": plan_path,
            }),
        });
        store
            .append_conversation_entry(config.session_id, &assistant)
            .expect("append assistant tool call");

        let restored = restore_plan_mode_state_for_resume(&store, &config.paths, config.session_id)
            .expect("restore state")
            .expect("state should exist");
        let restored_path = restored.plan_file_path.expect("restored plan path");

        assert!(restored_path.exists());
        assert_eq!(
            fs::read_to_string(restored_path).expect("read restored plan"),
            "# Plan\n- assistant carrier"
        );
    }

    #[test]
    fn resume_recovers_plan_from_attachment_reference_message() {
        let (config, store) = test_config_and_store();
        let controller = RuntimePlanModeController::load(&config, &store).expect("controller");
        controller
            .enter_plan_mode("audit runtime")
            .expect("enter plan mode");
        let state = controller.snapshot_state();
        let plan_path = state.plan_file_path.clone().expect("plan path");
        if plan_path.exists() {
            fs::remove_file(&plan_path).expect("remove existing plan file");
        }

        store
            .append_conversation_entry(
                config.session_id,
                &ConversationEntry::user_with_attachments(
                    format!("Plan file reference: {}", plan_path.display()),
                    vec![Attachment::from_bytes(
                        AttachmentMediaType::ApplicationPdf,
                        b"# Plan\n- attachment carrier\n",
                        Some(plan_path.display().to_string()),
                    )],
                ),
            )
            .expect("append plan attachment");

        let restored = restore_plan_mode_state_for_resume(&store, &config.paths, config.session_id)
            .expect("restore state")
            .expect("state should exist");
        let restored_path = restored.plan_file_path.expect("restored plan path");

        assert!(restored_path.exists());
        assert_eq!(
            fs::read_to_string(restored_path).expect("read restored plan"),
            "# Plan\n- attachment carrier"
        );
    }
}

//! Background task management system.
//!
//! Provides an in-memory store for background tasks that can be created,
//! queried, updated, and stopped by the agent during a session.

use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::{Result, anyhow};
use once_cell::sync::Lazy;
use rc_ui_bridge::{UiTaskKind, UiTaskNode, UiTaskStatus};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::task_output;

static TASK_STORE: Lazy<Mutex<HashMap<String, BackgroundTask>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static TASK_OUTPUT_DIR: Lazy<Mutex<Option<PathBuf>>> = Lazy::new(|| Mutex::new(None));

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundTask {
    pub id: String,
    #[serde(default)]
    pub parent_task_id: Option<String>,
    #[serde(default)]
    pub depth: u32,
    #[serde(default)]
    pub kind: TaskKind,
    pub title: String,
    pub status: TaskStatus,
    #[serde(default)]
    pub summary: String,
    pub output: String,
    pub output_path: Option<String>,
    #[serde(default)]
    pub turns_used: Option<u32>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedTaskRecord {
    id: String,
    #[serde(default)]
    parent_task_id: Option<String>,
    #[serde(default)]
    depth: u32,
    #[serde(default)]
    kind: TaskKind,
    title: String,
    status: TaskStatus,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    turns_used: Option<u32>,
    created_at: String,
    updated_at: String,
    #[serde(default)]
    output_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    #[default]
    Background,
    Delegation,
    Batch,
}

impl TaskKind {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Delegation => "delegation",
            Self::Batch => "batch",
        }
    }

    #[must_use]
    pub fn to_ui_kind(&self) -> UiTaskKind {
        match self {
            Self::Background => UiTaskKind::Background,
            Self::Delegation => UiTaskKind::Delegation,
            Self::Batch => UiTaskKind::Batch,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Stopped,
}

impl TaskStatus {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Stopped => "stopped",
        }
    }

    #[must_use]
    pub fn to_ui_status(&self) -> UiTaskStatus {
        match self {
            Self::Pending => UiTaskStatus::Pending,
            Self::Running => UiTaskStatus::Running,
            Self::Completed => UiTaskStatus::Completed,
            Self::Failed => UiTaskStatus::Failed,
            Self::Stopped => UiTaskStatus::Stopped,
        }
    }
}

fn generate_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("task_{timestamp}_{count}")
}

fn now_timestamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

#[must_use]
pub fn allocate_task_id() -> String {
    generate_id()
}

pub fn configure_task_output_dir(path: Option<PathBuf>) -> Result<()> {
    let mut output_dir = TASK_OUTPUT_DIR
        .lock()
        .map_err(|_| anyhow!("task output dir lock poisoned"))?;
    *output_dir = path;
    Ok(())
}

#[must_use]
pub fn task_snapshots() -> Vec<BackgroundTask> {
    let store = TASK_STORE
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut tasks = store.values().cloned().collect::<Vec<_>>();
    tasks.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    tasks
}

pub fn load_persisted_tasks(base_dir: &Path) -> Result<Vec<BackgroundTask>> {
    if !base_dir.exists() {
        return Ok(Vec::new());
    }

    let mut tasks = Vec::new();
    for entry in fs::read_dir(base_dir)
        .with_context(|| format!("failed to read {}", base_dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let contents =
            fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))?;
        let record: PersistedTaskRecord = serde_json::from_str(&contents)
            .with_context(|| format!("failed to parse {}", path.display()))?;
        let output = match &record.output_path {
            Some(output_path) => fs::read_to_string(output_path).unwrap_or_default(),
            None => String::new(),
        };
        tasks.push(BackgroundTask {
            id: record.id,
            parent_task_id: record.parent_task_id,
            depth: record.depth,
            kind: record.kind,
            title: record.title,
            status: record.status,
            summary: record.summary,
            output,
            output_path: record.output_path,
            turns_used: record.turns_used,
            created_at: record.created_at,
            updated_at: record.updated_at,
        });
    }

    tasks.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
    Ok(tasks)
}

pub fn load_persisted_task(base_dir: &Path, task_id: &str) -> Result<Option<BackgroundTask>> {
    Ok(load_persisted_tasks(base_dir)?
        .into_iter()
        .find(|task| task.id == task_id))
}

#[must_use]
pub fn ui_task_snapshots() -> Vec<UiTaskNode> {
    task_snapshots()
        .into_iter()
        .map(|task| UiTaskNode {
            id: task.id,
            parent_task_id: task.parent_task_id,
            title: task.title,
            status: task.status.to_ui_status(),
            kind: task.kind.to_ui_kind(),
            depth: task.depth,
            summary: task.summary,
            turns_used: task.turns_used,
            output_path: task.output_path,
            created_at: task.created_at,
            updated_at: task.updated_at,
        })
        .collect()
}

pub fn create_background_task(title: &str) -> Result<BackgroundTask> {
    let task = BackgroundTask {
        id: generate_id(),
        parent_task_id: None,
        depth: 0,
        kind: TaskKind::Background,
        title: title.to_owned(),
        status: TaskStatus::Pending,
        summary: String::new(),
        output: String::new(),
        output_path: None,
        turns_used: None,
        created_at: now_timestamp(),
        updated_at: now_timestamp(),
    };
    let mut store = TASK_STORE
        .lock()
        .map_err(|_| anyhow!("task store lock poisoned"))?;
    store.insert(task.id.clone(), task.clone());
    drop(store);
    persist_task_if_configured(&task.id)?;
    Ok(task)
}

pub fn start_tracked_task(
    task_id: String,
    title: &str,
    parent_task_id: Option<String>,
    depth: u32,
    kind: TaskKind,
    summary: Option<&str>,
) -> Result<BackgroundTask> {
    let now = now_timestamp();
    let mut store = TASK_STORE
        .lock()
        .map_err(|_| anyhow!("task store lock poisoned"))?;
    let task = store.entry(task_id.clone()).or_insert_with(|| BackgroundTask {
        id: task_id.clone(),
        parent_task_id: parent_task_id.clone(),
        depth,
        kind: kind.clone(),
        title: title.to_owned(),
        status: TaskStatus::Running,
        summary: summary.unwrap_or_default().to_owned(),
        output: String::new(),
        output_path: None,
        turns_used: None,
        created_at: now.clone(),
        updated_at: now.clone(),
    });
    task.parent_task_id = parent_task_id;
    task.depth = depth;
    task.kind = kind;
    task.title = title.to_owned();
    task.status = TaskStatus::Running;
    if let Some(summary) = summary {
        task.summary = summary.to_owned();
    }
    task.updated_at = now_timestamp();
    persist_existing_task(task)?;
    Ok(task.clone())
}

pub fn update_task_progress(task_id: &str, summary: &str) -> Result<()> {
    let mut store = TASK_STORE
        .lock()
        .map_err(|_| anyhow!("task store lock poisoned"))?;
    let task = store
        .get_mut(task_id)
        .ok_or_else(|| anyhow!("task '{task_id}' not found"))?;
    task.status = TaskStatus::Running;
    task.summary = summary.to_owned();
    task.updated_at = now_timestamp();
    persist_existing_task(task)
}

pub fn mark_task_running(task_id: &str, output: Option<&str>) -> Result<()> {
    let mut store = TASK_STORE
        .lock()
        .map_err(|_| anyhow!("task store lock poisoned"))?;
    let task = store
        .get_mut(task_id)
        .ok_or_else(|| anyhow!("task '{task_id}' not found"))?;
    task.status = TaskStatus::Running;
    if let Some(output) = output {
        task.output = output.to_owned();
    }
    task.updated_at = now_timestamp();
    persist_existing_task(task)
}

pub fn finish_background_task(task_id: &str, status: TaskStatus, output: &str) -> Result<()> {
    finish_tracked_task(task_id, status, None, output, None)
}

pub fn finish_tracked_task(
    task_id: &str,
    status: TaskStatus,
    summary: Option<&str>,
    output: &str,
    turns_used: Option<u32>,
) -> Result<()> {
    let mut store = TASK_STORE
        .lock()
        .map_err(|_| anyhow!("task store lock poisoned"))?;
    let task = store
        .get_mut(task_id)
        .ok_or_else(|| anyhow!("task '{task_id}' not found"))?;
    task.status = status;
    if let Some(summary) = summary {
        task.summary = summary.to_owned();
    }
    task.output = output.to_owned();
    task.turns_used = turns_used;
    task.updated_at = now_timestamp();
    persist_existing_task(task)
}

pub fn task_create(input: &Value) -> Result<String> {
    let title = input["title"]
        .as_str()
        .ok_or_else(|| anyhow!("title is required"))?;

    let task = BackgroundTask {
        id: generate_id(),
        parent_task_id: None,
        depth: 0,
        kind: TaskKind::Background,
        title: title.to_owned(),
        status: TaskStatus::Pending,
        summary: String::new(),
        output: String::new(),
        output_path: None,
        turns_used: None,
        created_at: now_timestamp(),
        updated_at: now_timestamp(),
    };

    let id = task.id.clone();
    let mut store = TASK_STORE
        .lock()
        .map_err(|_| anyhow!("task store lock poisoned"))?;
    store.insert(id.clone(), task);
    drop(store);
    persist_task_if_configured(&id)?;

    Ok(json!({
        "id": id,
        "status": "pending",
        "message": format!("Task '{title}' created.")
    })
    .to_string())
}

pub fn task_get(input: &Value) -> Result<String> {
    let id = input["id"]
        .as_str()
        .ok_or_else(|| anyhow!("id is required"))?;

    let store = TASK_STORE
        .lock()
        .map_err(|_| anyhow!("task store lock poisoned"))?;
    let task = store
        .get(id)
        .ok_or_else(|| anyhow!("task '{id}' not found"))?;

    Ok(serde_json::to_string_pretty(task)?)
}

pub fn task_list(_input: &Value) -> Result<String> {
    let tasks = task_snapshots();

    if tasks.is_empty() {
        return Ok("No tasks found.".to_owned());
    }

    Ok(serde_json::to_string_pretty(&tasks)?)
}

pub fn task_stop(input: &Value) -> Result<String> {
    let id = input["id"]
        .as_str()
        .ok_or_else(|| anyhow!("id is required"))?;

    let mut store = TASK_STORE
        .lock()
        .map_err(|_| anyhow!("task store lock poisoned"))?;
    let task = store
        .get_mut(id)
        .ok_or_else(|| anyhow!("task '{id}' not found"))?;

    task.status = TaskStatus::Stopped;
    task.updated_at = now_timestamp();
    persist_existing_task(task)?;

    Ok(json!({
        "id": id,
        "status": "stopped",
        "message": format!("Task '{id}' stopped.")
    })
    .to_string())
}

pub fn task_update(input: &Value) -> Result<String> {
    let id = input["id"]
        .as_str()
        .ok_or_else(|| anyhow!("id is required"))?;

    let mut store = TASK_STORE
        .lock()
        .map_err(|_| anyhow!("task store lock poisoned"))?;
    let task = store
        .get_mut(id)
        .ok_or_else(|| anyhow!("task '{id}' not found"))?;

    if let Some(status_str) = input["status"].as_str() {
        task.status = match status_str {
            "pending" => TaskStatus::Pending,
            "running" => TaskStatus::Running,
            "completed" => TaskStatus::Completed,
            "failed" => TaskStatus::Failed,
            "stopped" => TaskStatus::Stopped,
            other => return Err(anyhow!("invalid status '{other}'")),
        };
    }
    if let Some(parent_task_id) = input.get("parent_task_id").and_then(Value::as_str) {
        task.parent_task_id = Some(parent_task_id.to_owned());
    }
    if let Some(depth) = input.get("depth").and_then(Value::as_u64) {
        task.depth = u32::try_from(depth).unwrap_or(u32::MAX);
    }
    if let Some(kind) = input.get("kind").and_then(Value::as_str) {
        task.kind = match kind {
            "background" => TaskKind::Background,
            "delegation" => TaskKind::Delegation,
            "batch" => TaskKind::Batch,
            other => return Err(anyhow!("invalid kind '{other}'")),
        };
    }
    if let Some(summary) = input.get("summary").and_then(Value::as_str) {
        task.summary = summary.to_owned();
    }
    if let Some(output) = input["output"].as_str() {
        task.output = output.to_owned();
    }
    if let Some(turns_used) = input.get("turns_used").and_then(Value::as_u64) {
        task.turns_used = Some(u32::try_from(turns_used).unwrap_or(u32::MAX));
    }

    task.updated_at = now_timestamp();
    persist_existing_task(task)?;

    let status_str = task.status.as_str();
    Ok(json!({
        "id": id,
        "status": status_str,
        "message": format!("Task '{id}' updated.")
    })
    .to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn task_create_and_get_work() {
        let create_result = task_create(&json!({"title": "Test task"}));
        assert!(create_result.is_ok(), "create failed: {:?}", create_result);

        let create_str = create_result.expect("create should work");
        let create_json: Value = serde_json::from_str(&create_str).expect("should be valid JSON");
        let task_id = create_json["id"].as_str().expect("should have id");

        let get_result = task_get(&json!({"id": task_id}));
        assert!(get_result.is_ok(), "get failed: {:?}", get_result);

        let get_str = get_result.expect("get should work");
        let task: BackgroundTask = serde_json::from_str(&get_str).expect("should parse task");
        assert_eq!(task.title, "Test task");
        assert_eq!(task.status.as_str(), "pending");
        assert!(task.output_path.is_none());
        assert_eq!(task.kind.as_str(), "background");
    }

    #[test]
    fn task_update_changes_status() {
        let create_str = task_create(&json!({"title": "Update test"})).expect("create should work");
        let create_json: Value = serde_json::from_str(&create_str).expect("should be valid JSON");
        let task_id = create_json["id"].as_str().expect("should have id");

        let update_result = task_update(&json!({
            "id": task_id,
            "status": "running",
            "output": "in progress"
        }));
        assert!(update_result.is_ok(), "update failed: {:?}", update_result);

        let get_str = task_get(&json!({"id": task_id})).expect("get should work");
        let task: BackgroundTask = serde_json::from_str(&get_str).expect("should parse task");
        assert_eq!(task.status.as_str(), "running");
        assert_eq!(task.output, "in progress");
    }

    #[test]
    fn tracked_task_records_tree_metadata() {
        let task = start_tracked_task(
            "delegation-root".to_owned(),
            "Fix delegation",
            Some("parent-1".to_owned()),
            2,
            TaskKind::Delegation,
            Some("started"),
        )
        .expect("tracked task");

        assert_eq!(task.parent_task_id.as_deref(), Some("parent-1"));
        assert_eq!(task.depth, 2);
        assert_eq!(task.kind.as_str(), "delegation");
        assert_eq!(task.summary, "started");
    }

    #[test]
    fn task_stop_marks_stopped() {
        let create_str = task_create(&json!({"title": "Stop test"})).expect("create should work");
        let create_json: Value = serde_json::from_str(&create_str).expect("should be valid JSON");
        let task_id = create_json["id"].as_str().expect("should have id");

        let stop_result = task_stop(&json!({"id": task_id}));
        assert!(stop_result.is_ok(), "stop failed: {:?}", stop_result);

        let get_str = task_get(&json!({"id": task_id})).expect("get should work");
        let task: BackgroundTask = serde_json::from_str(&get_str).expect("should parse task");
        assert_eq!(task.status.as_str(), "stopped");
    }

    #[test]
    fn task_list_returns_tasks() {
        let _ = task_create(&json!({"title": "List test"}));

        let list_result = task_list(&json!({}));
        assert!(list_result.is_ok(), "list failed: {:?}", list_result);

        let list_str = list_result.expect("list should work");
        assert!(
            !list_str.contains("No tasks found"),
            "should have at least one task"
        );
    }

    #[test]
    fn task_get_missing_returns_error() {
        let result = task_get(&json!({"id": "nonexistent"}));
        assert!(result.is_err());
    }

    #[test]
    fn ui_task_snapshot_exports_task_tree_fields() {
        let task_id = allocate_task_id();
        start_tracked_task(
            task_id.clone(),
            "Snapshot task",
            Some("parent-x".to_owned()),
            1,
            TaskKind::Delegation,
            Some("working"),
        )
        .expect("tracked task");

        let tasks = ui_task_snapshots();
        let task = tasks
            .into_iter()
            .find(|task| task.id == task_id)
            .expect("snapshot should contain task");
        assert_eq!(task.parent_task_id.as_deref(), Some("parent-x"));
        assert_eq!(task.depth, 1);
    }

    #[test]
    fn configure_output_dir_persists_task_output() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        configure_task_output_dir(Some(tempdir.path().to_path_buf())).expect("configure output");

        let create_str =
            task_create(&json!({"title": "Persist output test"})).expect("create should work");
        let create_json: Value = serde_json::from_str(&create_str).expect("valid JSON");
        let task_id = create_json["id"].as_str().expect("task id");
        task_update(&json!({
            "id": task_id,
            "status": "completed",
            "output": "done"
        }))
        .expect("update should work");

        let get_str = task_get(&json!({"id": task_id})).expect("get should work");
        let task: BackgroundTask = serde_json::from_str(&get_str).expect("parse task");
        assert!(task.output_path.is_some());
        assert!(tempdir.path().join(format!("{task_id}.json")).exists());
    }
}

fn persist_task_if_configured(task_id: &str) -> Result<()> {
    let output_dir = TASK_OUTPUT_DIR
        .lock()
        .map_err(|_| anyhow!("task output dir lock poisoned"))?
        .clone();
    let Some(output_dir) = output_dir else {
        return Ok(());
    };

    let mut store = TASK_STORE
        .lock()
        .map_err(|_| anyhow!("task store lock poisoned"))?;
    let task = store
        .get_mut(task_id)
        .ok_or_else(|| anyhow!("task '{task_id}' not found"))?;
    let persisted_path = task_output::persist_task(&output_dir, task)?;
    task.output_path = persisted_path.map(|path| path.display().to_string());
    Ok(())
}

fn persist_existing_task(task: &mut BackgroundTask) -> Result<()> {
    let output_dir = TASK_OUTPUT_DIR
        .lock()
        .map_err(|_| anyhow!("task output dir lock poisoned"))?
        .clone();
    let Some(output_dir) = output_dir else {
        return Ok(());
    };

    let persisted_path = task_output::persist_task(&output_dir, task)?;
    task.output_path = persisted_path.map(|path| path.display().to_string());
    Ok(())
}

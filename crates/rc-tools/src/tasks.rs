//! Background task management system.
//!
//! Provides an in-memory store for background tasks that can be created,
//! queried, updated, and stopped by the agent during a session.

use std::collections::HashMap;
use std::sync::Mutex;

use anyhow::{Result, anyhow};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

static TASK_STORE: Lazy<Mutex<HashMap<String, BackgroundTask>>> = Lazy::new(|| {
    Mutex::new(HashMap::new())
});

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackgroundTask {
    pub id: String,
    pub title: String,
    pub status: TaskStatus,
    pub output: String,
    pub created_at: String,
    pub updated_at: String,
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

pub fn task_create(input: &Value) -> Result<String> {
    let title = input["title"]
        .as_str()
        .ok_or_else(|| anyhow!("title is required"))?;

    let task = BackgroundTask {
        id: generate_id(),
        title: title.to_owned(),
        status: TaskStatus::Pending,
        output: String::new(),
        created_at: now_timestamp(),
        updated_at: now_timestamp(),
    };

    let id = task.id.clone();
    let mut store = TASK_STORE
        .lock()
        .map_err(|_| anyhow!("task store lock poisoned"))?;
    store.insert(id.clone(), task);

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
    let store = TASK_STORE
        .lock()
        .map_err(|_| anyhow!("task store lock poisoned"))?;
    let tasks: Vec<&BackgroundTask> = store.values().collect();

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

    if let Some(output) = input["output"].as_str() {
        task.output = output.to_owned();
    }

    task.updated_at = now_timestamp();

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
        let create_json: Value =
            serde_json::from_str(&create_str).expect("should be valid JSON");
        let task_id = create_json["id"].as_str().expect("should have id");

        let get_result = task_get(&json!({"id": task_id}));
        assert!(get_result.is_ok(), "get failed: {:?}", get_result);

        let get_str = get_result.expect("get should work");
        let task: BackgroundTask =
            serde_json::from_str(&get_str).expect("should parse task");
        assert_eq!(task.title, "Test task");
        assert_eq!(task.status.as_str(), "pending");
    }

    #[test]
    fn task_update_changes_status() {
        let create_str = task_create(&json!({"title": "Update test"}))
            .expect("create should work");
        let create_json: Value =
            serde_json::from_str(&create_str).expect("should be valid JSON");
        let task_id = create_json["id"].as_str().expect("should have id");

        let update_result = task_update(&json!({
            "id": task_id,
            "status": "running",
            "output": "in progress"
        }));
        assert!(update_result.is_ok(), "update failed: {:?}", update_result);

        let get_str = task_get(&json!({"id": task_id})).expect("get should work");
        let task: BackgroundTask =
            serde_json::from_str(&get_str).expect("should parse task");
        assert_eq!(task.status.as_str(), "running");
        assert_eq!(task.output, "in progress");
    }

    #[test]
    fn task_stop_marks_stopped() {
        let create_str = task_create(&json!({"title": "Stop test"}))
            .expect("create should work");
        let create_json: Value =
            serde_json::from_str(&create_str).expect("should be valid JSON");
        let task_id = create_json["id"].as_str().expect("should have id");

        let stop_result = task_stop(&json!({"id": task_id}));
        assert!(stop_result.is_ok(), "stop failed: {:?}", stop_result);

        let get_str = task_get(&json!({"id": task_id})).expect("get should work");
        let task: BackgroundTask =
            serde_json::from_str(&get_str).expect("should parse task");
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
}

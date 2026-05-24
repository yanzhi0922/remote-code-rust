//! Bridge from [`UnifiedAgentEvent`] → [`RuntimeEventDetail`].
//!
//! This allows any Agent adapter (Roo, Codex, or future ones) to feed events
//! into the same control-plane timeline that Claude sessions already use.

use std::sync::Arc;

use rc_engine_events::{MessageRole, RuntimeEventDetail};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::events::UnifiedAgentEvent;

/// Convert a [`UnifiedAgentEvent`] into a [`RuntimeEventDetail`] suitable for
/// posting to the control plane timeline. Returns `None` for lifecycle events
/// that have no timeline representation (Started, Ready, Stopped, Completed).
pub fn unified_event_to_runtime_detail(event: &UnifiedAgentEvent) -> Option<RuntimeEventDetail> {
    match event {
        UnifiedAgentEvent::MessageDelta { delta, .. } => Some(RuntimeEventDetail::MessageDelta {
            role: MessageRole::Assistant,
            delta: delta.clone(),
            message_id: None,
        }),

        UnifiedAgentEvent::ToolCallStarted {
            session_id,
            tool_name,
            tool_input,
            ..
        } => Some(RuntimeEventDetail::ToolStarted {
            tool_call_id: derive_tool_call_id(session_id, tool_name, tool_input),
            tool_name: Arc::from(tool_name.as_str()),
        }),

        UnifiedAgentEvent::ToolCallProgress {
            tool_name,
            progress,
            ..
        } => {
            let (tool_call_id, delta) = parse_progress_tool_call_id(progress);
            let name = non_empty_arc(tool_name);
            Some(RuntimeEventDetail::ToolProgress {
                tool_call_id: tool_call_id
                    .map(Arc::from)
                    .or_else(|| name.as_ref().map(Arc::clone)),
                tool_name: name,
                delta: Some(delta),
                elapsed_time_seconds: None,
            })
        }

        UnifiedAgentEvent::ToolCallCompleted {
            session_id,
            tool_name,
            result,
            ..
        } => Some(RuntimeEventDetail::ToolFinished {
            tool_call_id: derive_tool_call_id(session_id, tool_name, result),
            tool_name: Arc::from(tool_name.as_str()),
            is_error: result_is_error(result),
            summary: Some(result.to_string()),
        }),

        UnifiedAgentEvent::PermissionRequest { tool_name, .. } => {
            let name: Arc<str> = Arc::from(tool_name.as_str());
            Some(RuntimeEventDetail::ToolStarted {
                tool_call_id: format!("approval-{name}").into(),
                tool_name: format!("approval:{name}").into(),
            })
        }

        UnifiedAgentEvent::SubtaskStarted {
            task_id,
            description,
            ..
        } => Some(RuntimeEventDetail::SubtaskStarted {
            task_id: Arc::from(task_id.as_str()),
            parent_task_id: None,
            description: description.clone(),
            depth: 0,
        }),

        UnifiedAgentEvent::SubtaskProgress {
            task_id, progress, ..
        } => Some(RuntimeEventDetail::SubtaskProgress {
            task_id: Arc::from(task_id.as_str()),
            status: "running".to_owned(),
            summary: progress.clone(),
        }),

        UnifiedAgentEvent::SubtaskCompleted {
            task_id, result, ..
        } => Some(RuntimeEventDetail::SubtaskCompleted {
            task_id: Arc::from(task_id.as_str()),
            status: "completed".to_owned(),
            summary: result.to_string(),
            turns_used: None,
        }),

        UnifiedAgentEvent::ContextUsage { used, total, .. } => {
            let ratio = if *total > 0 {
                *used as f64 / *total as f64
            } else {
                0.0
            };
            Some(RuntimeEventDetail::ContextUsage {
                estimated_tokens: *used as u64,
                max_input_tokens: *total as u64,
                threshold_tokens: (*total as f64 * 0.8) as u64,
                ratio,
            })
        }

        UnifiedAgentEvent::ContextOverflow { used, total, .. } => {
            let ratio = if *total > 0 {
                *used as f64 / *total as f64
            } else {
                1.0
            };
            Some(RuntimeEventDetail::ContextOverflow {
                estimated_tokens: *used as u64,
                max_input_tokens: *total as u64,
                threshold_tokens: (*total as f64 * 0.8) as u64,
                ratio,
            })
        }

        UnifiedAgentEvent::ContextCompacted {
            entries_removed,
            usage_ratio,
            ..
        } => Some(RuntimeEventDetail::ContextCompacted {
            entries_removed: *entries_removed as u32,
            usage_ratio: *usage_ratio,
        }),

        UnifiedAgentEvent::Error { message, .. } => Some(RuntimeEventDetail::RuntimeError {
            message: message.clone(),
        }),

        // Lifecycle events with no timeline representation
        UnifiedAgentEvent::Started(_)
        | UnifiedAgentEvent::Ready
        | UnifiedAgentEvent::Stopped
        | UnifiedAgentEvent::Completed { .. } => None,

        // Codex-specific notifications are handled directly by the GUI
        // (desktop.rs emits a dedicated Tauri event). Mapping them to
        // RuntimeError here was semantically wrong — they are informational,
        // not errors — and produced spurious red error cards in remote timelines.
        UnifiedAgentEvent::CodexAppServerNotification { .. } => None,
    }
}

fn non_empty_arc(value: &str) -> Option<Arc<str>> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| Arc::from(trimmed))
}

fn parse_progress_tool_call_id(progress: &str) -> (Option<String>, String) {
    let Some(rest) = progress.strip_prefix('[') else {
        return (None, progress.to_owned());
    };
    let Some(end) = rest.find(']') else {
        return (None, progress.to_owned());
    };
    let id = rest[..end].trim();
    if id.is_empty() {
        return (None, progress.to_owned());
    }
    let delta = rest[end + 1..].trim_start().to_owned();
    (Some(id.to_owned()), delta)
}

fn result_is_error(result: &Value) -> bool {
    if let Some(is_error) = result.get("is_error").and_then(Value::as_bool) {
        return is_error;
    }
    result
        .get("success")
        .and_then(Value::as_bool)
        .is_some_and(|success| !success)
}

fn derive_tool_call_id(session_id: &str, tool_name: &str, payload: &Value) -> Arc<str> {
    extract_tool_call_id(payload)
        .unwrap_or_else(|| stable_tool_call_id(session_id, tool_name, payload))
        .into()
}

fn extract_tool_call_id(value: &Value) -> Option<String> {
    const ID_KEYS: &[&str] = &["tool_call_id", "tool_use_id", "tool_id", "item_id", "id"];

    match value {
        Value::Object(map) => {
            for key in ID_KEYS {
                if let Some(id) = map
                    .get(*key)
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|id| !id.is_empty())
                {
                    return Some(id.to_owned());
                }
            }
            map.values().find_map(extract_tool_call_id)
        }
        Value::Array(values) => values.iter().find_map(extract_tool_call_id),
        _ => None,
    }
}

fn stable_tool_call_id(session_id: &str, tool_name: &str, payload: &Value) -> String {
    let mut hasher = Sha256::new();
    hasher.update(session_id.as_bytes());
    hasher.update([0]);
    hasher.update(tool_name.as_bytes());
    hasher.update([0]);
    hasher.update(payload.to_string().as_bytes());
    let digest = hasher.finalize();
    let mut suffix = String::with_capacity(16);
    for byte in &digest[..8] {
        #[allow(clippy::format_push_string)]
        suffix.push_str(&format!("{byte:02x}"));
    }
    let prefix = if tool_name.trim().is_empty() {
        "tool"
    } else {
        tool_name.trim()
    };
    format!("{prefix}-{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_delta_maps_correctly() {
        let event = UnifiedAgentEvent::MessageDelta {
            session_id: "s1".into(),
            delta: "hello".into(),
        };
        let detail = unified_event_to_runtime_detail(&event).unwrap();
        assert!(matches!(
            detail,
            RuntimeEventDetail::MessageDelta { role: MessageRole::Assistant, delta, .. }
            if delta == "hello"
        ));
    }

    #[test]
    fn tool_lifecycle_maps_correctly() {
        let started = UnifiedAgentEvent::ToolCallStarted {
            session_id: "s1".into(),
            tool_name: "read_file".into(),
            tool_input: serde_json::json!({"path": "/tmp/a.rs"}),
        };
        let d = unified_event_to_runtime_detail(&started).unwrap();
        if let RuntimeEventDetail::ToolStarted {
            tool_call_id,
            tool_name,
        } = d
        {
            assert_eq!(&*tool_name, "read_file");
            assert!(
                tool_call_id.starts_with("read_file-"),
                "fallback tool_call_id should start with tool name, got: {tool_call_id}"
            );
        } else {
            panic!("expected ToolStarted");
        }

        let completed = UnifiedAgentEvent::ToolCallCompleted {
            session_id: "s1".into(),
            tool_name: "read_file".into(),
            result: serde_json::json!({"success": true, "content": "ok"}),
        };
        let d = unified_event_to_runtime_detail(&completed).unwrap();
        assert!(matches!(
            d,
            RuntimeEventDetail::ToolFinished {
                is_error: false,
                ..
            }
        ));
    }

    #[test]
    fn tool_call_ids_are_stable_for_the_same_event() {
        let event = UnifiedAgentEvent::ToolCallStarted {
            session_id: "s1".into(),
            tool_name: "read_file".into(),
            tool_input: serde_json::json!({"path": "/tmp/a.rs"}),
        };
        let d1 = unified_event_to_runtime_detail(&event).unwrap();
        let d2 = unified_event_to_runtime_detail(&event).unwrap();

        let id1 = match d1 {
            RuntimeEventDetail::ToolStarted { tool_call_id, .. } => tool_call_id,
            _ => panic!("expected ToolStarted"),
        };
        let id2 = match d2 {
            RuntimeEventDetail::ToolStarted { tool_call_id, .. } => tool_call_id,
            _ => panic!("expected ToolStarted"),
        };
        assert_eq!(id1, id2, "same tool event should keep the same ID");
    }

    #[test]
    fn explicit_tool_call_id_links_lifecycle_events() {
        let started = UnifiedAgentEvent::ToolCallStarted {
            session_id: "s1".into(),
            tool_name: "read_file".into(),
            tool_input: serde_json::json!({"tool_call_id": "toolu_123", "path": "/tmp/a.rs"}),
        };
        let completed = UnifiedAgentEvent::ToolCallCompleted {
            session_id: "s1".into(),
            tool_name: "read_file".into(),
            result: serde_json::json!({"tool_call_id": "toolu_123", "is_error": false}),
        };
        let progress = UnifiedAgentEvent::ToolCallProgress {
            session_id: "s1".into(),
            tool_name: String::new(),
            progress: "[toolu_123] streamed output".into(),
        };

        let started_id = match unified_event_to_runtime_detail(&started).unwrap() {
            RuntimeEventDetail::ToolStarted { tool_call_id, .. } => tool_call_id,
            _ => panic!("expected ToolStarted"),
        };
        let completed_id = match unified_event_to_runtime_detail(&completed).unwrap() {
            RuntimeEventDetail::ToolFinished {
                tool_call_id,
                is_error,
                ..
            } => {
                assert!(!is_error);
                tool_call_id
            }
            _ => panic!("expected ToolFinished"),
        };
        let (progress_id, delta) = match unified_event_to_runtime_detail(&progress).unwrap() {
            RuntimeEventDetail::ToolProgress {
                tool_call_id,
                delta,
                ..
            } => (tool_call_id, delta),
            _ => panic!("expected ToolProgress"),
        };

        assert_eq!(&*started_id, "toolu_123");
        assert_eq!(started_id, completed_id);
        assert_eq!(progress_id.as_deref(), Some("toolu_123"));
        assert_eq!(delta.as_deref(), Some("streamed output"));
    }

    #[test]
    fn lifecycle_events_map_to_none() {
        assert!(unified_event_to_runtime_detail(&UnifiedAgentEvent::Ready).is_none());
        assert!(unified_event_to_runtime_detail(&UnifiedAgentEvent::Stopped).is_none());
        assert!(
            unified_event_to_runtime_detail(&UnifiedAgentEvent::Started(crate::types::AgentInfo {
                name: "test".into(),
                version: "0.1".into(),
                capabilities: Default::default(),
                status: crate::types::AgentStatus::Ready,
            }))
            .is_none()
        );
    }

    #[test]
    fn context_usage_maps_with_ratio() {
        let event = UnifiedAgentEvent::ContextUsage {
            session_id: "s1".into(),
            used: 80_000,
            total: 200_000,
        };
        let detail = unified_event_to_runtime_detail(&event).unwrap();
        if let RuntimeEventDetail::ContextUsage { ratio, .. } = detail {
            assert!((ratio - 0.4).abs() < 0.01);
        } else {
            panic!("expected ContextUsage");
        }
    }

    #[test]
    fn subtask_lifecycle_maps() {
        let started = UnifiedAgentEvent::SubtaskStarted {
            session_id: "s1".into(),
            task_id: "t1".into(),
            description: "explore code".into(),
        };
        let d = unified_event_to_runtime_detail(&started).unwrap();
        assert!(
            matches!(d, RuntimeEventDetail::SubtaskStarted { task_id, description, .. }
            if &*task_id == "t1" && description == "explore code")
        );

        let completed = UnifiedAgentEvent::SubtaskCompleted {
            session_id: "s1".into(),
            task_id: "t1".into(),
            result: serde_json::json!("done"),
        };
        let d = unified_event_to_runtime_detail(&completed).unwrap();
        assert!(
            matches!(d, RuntimeEventDetail::SubtaskCompleted { status, .. } if status == "completed")
        );
    }

    #[test]
    fn codex_app_server_notification_maps_to_none() {
        let event = UnifiedAgentEvent::CodexAppServerNotification {
            session_id: "s1".into(),
            method: "model/verification".into(),
            params: serde_json::json!({"status": "ok"}),
        };
        assert!(
            unified_event_to_runtime_detail(&event).is_none(),
            "CodexAppServerNotification should not produce a RuntimeEventDetail"
        );
    }
}

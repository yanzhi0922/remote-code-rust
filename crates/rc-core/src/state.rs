use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{AgentId, Message, PermissionMode, SessionId};

/// Mutable permission context propagated alongside tool execution.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolPermissionContext {
    #[serde(default)]
    pub allowlisted_tools: BTreeSet<String>,
    #[serde(default)]
    pub denylisted_tools: BTreeSet<String>,
    #[serde(default)]
    pub working_directory: Option<PathBuf>,
    #[serde(default)]
    pub extra: Value,
}

/// File history snapshot used by compaction/recovery flows.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FileHistoryState {
    #[serde(default)]
    pub touched_paths: BTreeSet<PathBuf>,
    #[serde(default)]
    pub checkpoints: Vec<String>,
}

impl FileHistoryState {
    /// Record that a path was touched in the current run.
    pub fn note_path(&mut self, path: impl Into<PathBuf>) {
        self.touched_paths.insert(path.into());
    }

    /// Record a named checkpoint.
    pub fn note_checkpoint(&mut self, checkpoint: impl Into<String>) {
        self.checkpoints.push(checkpoint.into());
    }
}

/// Shared application state snapshot for TUI/GUI/remote surfaces.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppState {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<SessionId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_agent_id: Option<AgentId>,
    #[serde(default)]
    pub permission_mode: PermissionMode,
    #[serde(default)]
    pub messages: Vec<Message>,
    #[serde(default)]
    pub discovered_skills: BTreeSet<String>,
    #[serde(default)]
    pub active_tools: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default)]
    pub queued_task_count: usize,
}

impl AppState {
    /// Push a new message into the state snapshot.
    pub fn push_message(&mut self, message: Message) {
        self.messages.push(message);
    }

    /// Record a newly discovered skill slug.
    pub fn note_skill(&mut self, skill: impl Into<String>) {
        self.discovered_skills.insert(skill.into());
    }

    /// Record an active tool.
    pub fn note_tool(&mut self, tool_name: impl Into<String>) {
        self.active_tools.insert(tool_name.into());
    }
}

#[cfg(test)]
mod tests {
    use super::AppState;
    use crate::{ConversationEntry, Message};

    #[test]
    fn app_state_tracks_messages_skills_and_tools() {
        let mut state = AppState::default();
        state.push_message(Message::from(ConversationEntry::user("hello")));
        state.note_skill("openai-docs");
        state.note_tool("read_file");

        assert_eq!(state.messages.len(), 1);
        assert!(state.discovered_skills.contains("openai-docs"));
        assert!(state.active_tools.contains("read_file"));
    }

    #[test]
    fn file_history_tracks_paths_and_checkpoints() {
        let mut history = super::FileHistoryState::default();
        history.note_path("src/main.rs");
        history.note_checkpoint("before_compact");

        assert!(
            history
                .touched_paths
                .iter()
                .any(|path| path.ends_with("src/main.rs"))
        );
        assert_eq!(history.checkpoints, vec!["before_compact"]);
    }
}

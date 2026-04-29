//! Task message persistence.
//!
//! Provides read/write operations for task messages stored as JSON files.
//! Uses a filesystem trait abstraction for testability.

use std::path::Path;

use roo_types::message::ClineMessage;

use crate::storage::TaskFileSystem;
use crate::TaskPersistenceError;

// ---------------------------------------------------------------------------
// read_task_messages
// ---------------------------------------------------------------------------

/// Read task messages from a JSON file.
///
/// Returns an empty vector if the file does not exist or contains invalid JSON.
pub fn read_task_messages(
    fs: &dyn TaskFileSystem,
    path: &Path,
) -> Result<Vec<ClineMessage>, TaskPersistenceError> {
    if !fs.file_exists(path)? {
        return Ok(Vec::new());
    }

    let content = fs.read_file(path)?;

    if content.trim().is_empty() {
        return Ok(Vec::new());
    }

    match serde_json::from_str::<Vec<ClineMessage>>(&content) {
        Ok(messages) => Ok(messages),
        Err(e) => {
            // Match TS behavior: return empty on parse errors rather than
            // propagating the error. The TS source catches parse errors and
            // returns `[]`.
            eprintln!(
                "[readTaskMessages] Failed to parse {}: {}",
                path.display(),
                e
            );
            Ok(Vec::new())
        }
    }
}

// ---------------------------------------------------------------------------
// save_task_messages
// ---------------------------------------------------------------------------

/// Save task messages to a JSON file.
///
/// Creates parent directories if they don't exist. Overwrites any existing file.
pub fn save_task_messages(
    fs: &dyn TaskFileSystem,
    path: &Path,
    messages: &[ClineMessage],
) -> Result<(), TaskPersistenceError> {
    if let Some(parent) = path.parent() {
        fs.create_dir_all(parent)?;
    }

    let content = serde_json::to_string_pretty(messages)?;
    fs.write_file(path, &content)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::OsFileSystem;

    use std::path::PathBuf;

    #[test]
    fn test_read_task_messages_nonexistent_file() {
        let fs = OsFileSystem;
        let path = PathBuf::from("/nonexistent/path/messages.json");
        let result = read_task_messages(&fs, &path).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_read_task_messages_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("messages.json");
        std::fs::write(&path, "").unwrap();

        let fs = OsFileSystem;
        let result = read_task_messages(&fs, &path).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_read_task_messages_whitespace_only_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("messages.json");
        std::fs::write(&path, "   \n\t  ").unwrap();

        let fs = OsFileSystem;
        let result = read_task_messages(&fs, &path).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_read_task_messages_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("messages.json");
        std::fs::write(&path, "not valid json").unwrap();

        let fs = OsFileSystem;
        // Matches TS behavior: returns empty vector on parse errors
        let result = read_task_messages(&fs, &path).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_read_task_messages_valid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("messages.json");
        let json = r#"[{"ts":1700000000.0,"type":"say","say":"text","text":"Hello"}]"#;
        std::fs::write(&path, json).unwrap();

        let fs = OsFileSystem;
        let result = read_task_messages(&fs, &path).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text.as_deref(), Some("Hello"));
    }

    #[test]
    fn test_read_task_messages_empty_array() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("messages.json");
        std::fs::write(&path, "[]").unwrap();

        let fs = OsFileSystem;
        let result = read_task_messages(&fs, &path).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_save_task_messages_creates_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("dir").join("messages.json");

        let fs = OsFileSystem;
        let messages: Vec<ClineMessage> = Vec::new();
        save_task_messages(&fs, &path, &messages).unwrap();

        assert!(path.exists());
    }

    #[test]
    fn test_save_task_messages_writes_valid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("messages.json");

        let fs = OsFileSystem;
        let messages: Vec<ClineMessage> = Vec::new();
        save_task_messages(&fs, &path, &messages).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&content).unwrap();
        assert!(parsed.is_empty());
    }

    #[test]
    fn test_save_task_messages_overwrites_existing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("messages.json");

        let fs = OsFileSystem;

        // First write
        let msg1 = ClineMessage {
            ts: 1700000000.0,
            r#type: roo_types::message::MessageType::Say,
            ask: None,
            say: Some(roo_types::message::ClineSay::Text),
            text: Some("First".to_string()),
            images: None,
            partial: None,
            reasoning: None,
            conversation_history_index: None,
            checkpoint: None,
            progress_status: None,
            context_condense: None,
            context_truncation: None,
            is_protected: None,
            api_protocol: None,
            is_answered: None,
        };
        save_task_messages(&fs, &path, &[msg1]).unwrap();

        // Second write (overwrite)
        let msg2 = ClineMessage {
            ts: 1700000001.0,
            r#type: roo_types::message::MessageType::Say,
            ask: None,
            say: Some(roo_types::message::ClineSay::Text),
            text: Some("Second".to_string()),
            images: None,
            partial: None,
            reasoning: None,
            conversation_history_index: None,
            checkpoint: None,
            progress_status: None,
            context_condense: None,
            context_truncation: None,
            is_protected: None,
            api_protocol: None,
            is_answered: None,
        };
        save_task_messages(&fs, &path, &[msg2]).unwrap();

        let result = read_task_messages(&fs, &path).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].text.as_deref(), Some("Second"));
    }

    #[test]
    fn test_save_and_read_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("messages.json");

        let fs = OsFileSystem;

        let msg = ClineMessage {
            ts: 1700000000.0,
            r#type: roo_types::message::MessageType::Ask,
            ask: Some(roo_types::message::ClineAsk::Followup),
            say: None,
            text: Some("What do you think?".to_string()),
            images: Some(vec!["base64img".to_string()]),
            partial: Some(false),
            reasoning: Some("Let me think...".to_string()),
            conversation_history_index: Some(0),
            checkpoint: None,
            progress_status: None,
            context_condense: None,
            context_truncation: None,
            is_protected: Some(false),
            api_protocol: None,
            is_answered: None,
        };
        save_task_messages(&fs, &path, &[msg.clone()]).unwrap();

        let result = read_task_messages(&fs, &path).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].ts, 1700000000.0);
        assert_eq!(result[0].text.as_deref(), Some("What do you think?"));
        assert_eq!(result[0].images.as_ref().unwrap().len(), 1);
    }
}

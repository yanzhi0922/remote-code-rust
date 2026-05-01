//! API conversation history persistence.
//!
//! Provides read/write operations for API messages (the raw conversation history
//! sent to/from the LLM API) stored as JSON files.
//!
//! Source: `src/core/task-persistence/apiMessages.ts` — `readApiMessages`, `saveApiMessages`

use std::path::Path;

use roo_types::api::ApiMessage;

use crate::storage::TaskFileSystem;
use crate::TaskPersistenceError;

// ---------------------------------------------------------------------------
// read_api_messages
// ---------------------------------------------------------------------------

/// Read API conversation history messages from a JSON file.
///
/// Returns an empty vector if the file does not exist, is empty, or contains
/// invalid JSON. This matches the TypeScript behavior of returning `[]` on
/// any read error rather than propagating the error.
///
/// Source: `src/core/task-persistence/apiMessages.ts` — `readApiMessages`
pub fn read_api_messages(
    fs: &dyn TaskFileSystem,
    path: &Path,
) -> Result<Vec<ApiMessage>, TaskPersistenceError> {
    if !fs.file_exists(path)? {
        return Ok(Vec::new());
    }

    let content = fs.read_file(path)?;

    if content.trim().is_empty() {
        return Ok(Vec::new());
    }

    match serde_json::from_str::<Vec<ApiMessage>>(&content) {
        Ok(messages) => {
            if messages.is_empty() {
                eprintln!(
                    "[readApiMessages] API conversation history file exists but is empty. Path: {}",
                    path.display()
                );
            }
            Ok(messages)
        }
        Err(e) => {
            eprintln!(
                "[readApiMessages] Error parsing API conversation history file, returning empty. Path: {}, Error: {}",
                path.display(),
                e
            );
            Ok(Vec::new())
        }
    }
}

// ---------------------------------------------------------------------------
// save_api_messages
// ---------------------------------------------------------------------------

/// Save API conversation history messages to a JSON file.
///
/// Creates parent directories if they don't exist. Overwrites any existing file.
///
/// Source: `src/core/task-persistence/apiMessages.ts` — `saveApiMessages`
pub fn save_api_messages(
    fs: &dyn TaskFileSystem,
    path: &Path,
    messages: &[ApiMessage],
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

    use roo_types::api::{ContentBlock, MessageRole};
    use std::path::PathBuf;

    fn make_api_message(role: MessageRole, text: &str) -> ApiMessage {
        ApiMessage {
            role,
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
            reasoning: None,
            ts: Some(1700000000.0),
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        }
    }

    #[test]
    fn test_read_api_messages_nonexistent_file() {
        let fs = OsFileSystem;
        let path = PathBuf::from("/nonexistent/path/api_messages.json");
        let result = read_api_messages(&fs, &path).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_read_api_messages_empty_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("api_conversation_history.json");
        std::fs::write(&path, "").unwrap();

        let fs = OsFileSystem;
        let result = read_api_messages(&fs, &path).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_read_api_messages_whitespace_only_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("api_conversation_history.json");
        std::fs::write(&path, "   \n\t  ").unwrap();

        let fs = OsFileSystem;
        let result = read_api_messages(&fs, &path).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_read_api_messages_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("api_conversation_history.json");
        std::fs::write(&path, "not valid json").unwrap();

        let fs = OsFileSystem;
        // Should return empty vector, not error (matches TS behavior)
        let result = read_api_messages(&fs, &path).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_read_api_messages_valid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("api_conversation_history.json");

        let msg = make_api_message(MessageRole::User, "Hello");
        let json = serde_json::to_string_pretty(&vec![msg]).unwrap();
        std::fs::write(&path, json).unwrap();

        let fs = OsFileSystem;
        let result = read_api_messages(&fs, &path).unwrap();
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_read_api_messages_empty_array() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("api_conversation_history.json");
        std::fs::write(&path, "[]").unwrap();

        let fs = OsFileSystem;
        let result = read_api_messages(&fs, &path).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_save_api_messages_creates_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("dir").join("api_messages.json");

        let fs = OsFileSystem;
        let messages: Vec<ApiMessage> = Vec::new();
        save_api_messages(&fs, &path, &messages).unwrap();

        assert!(path.exists());
    }

    #[test]
    fn test_save_api_messages_writes_valid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("api_messages.json");

        let fs = OsFileSystem;
        let messages: Vec<ApiMessage> = Vec::new();
        save_api_messages(&fs, &path, &messages).unwrap();

        let content = std::fs::read_to_string(&path).unwrap();
        let parsed: Vec<serde_json::Value> = serde_json::from_str(&content).unwrap();
        assert!(parsed.is_empty());
    }

    #[test]
    fn test_save_and_read_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("api_messages.json");

        let fs = OsFileSystem;

        let messages = vec![
            make_api_message(MessageRole::User, "Hello"),
            make_api_message(MessageRole::Assistant, "Hi there!"),
            make_api_message(MessageRole::User, "How are you?"),
        ];

        save_api_messages(&fs, &path, &messages).unwrap();
        let loaded = read_api_messages(&fs, &path).unwrap();

        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].role, MessageRole::User);
        assert_eq!(loaded[1].role, MessageRole::Assistant);
        assert_eq!(loaded[2].role, MessageRole::User);
    }

    #[test]
    fn test_save_api_messages_overwrites_existing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("api_messages.json");

        let fs = OsFileSystem;

        let first = vec![make_api_message(MessageRole::User, "First")];
        save_api_messages(&fs, &path, &first).unwrap();

        let second = vec![
            make_api_message(MessageRole::User, "Second1"),
            make_api_message(MessageRole::Assistant, "Second2"),
        ];
        save_api_messages(&fs, &path, &second).unwrap();

        let loaded = read_api_messages(&fs, &path).unwrap();
        assert_eq!(loaded.len(), 2);
    }
}

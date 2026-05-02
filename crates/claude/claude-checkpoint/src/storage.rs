//! SQLite-based storage for checkpoints.
//!
//! Checkpoints are persisted in a SQLite table alongside session data,
//! enabling fast queries by session, message index, or time range.

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::sync::Mutex;

use crate::types::*;

/// SQLite-backed checkpoint store.
pub struct CheckpointStore {
    conn: Mutex<Connection>,
}

impl CheckpointStore {
    /// Open (or create) the checkpoint database at the given path.
    pub fn open(db_path: &Path) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_schema()?;
        Ok(store)
    }

    /// Open an in-memory database (for testing).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_schema()?;
        Ok(store)
    }

    /// Initialize the database schema.
    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("lock error: {e}"))?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS checkpoints (
                id              TEXT PRIMARY KEY,
                session_id      TEXT NOT NULL,
                message_id      TEXT NOT NULL,
                message_index   INTEGER NOT NULL,
                created_at      TEXT NOT NULL,
                summary         TEXT NOT NULL DEFAULT '',
                stats_json      TEXT NOT NULL DEFAULT '{}'
            );

            CREATE INDEX IF NOT EXISTS idx_checkpoints_session
                ON checkpoints(session_id, message_index);

            CREATE TABLE IF NOT EXISTS file_changes (
                checkpoint_id   TEXT NOT NULL REFERENCES checkpoints(id),
                path            TEXT NOT NULL,
                operation       TEXT NOT NULL,
                hash_before     TEXT,
                hash_after      TEXT,
                lines_added     INTEGER NOT NULL DEFAULT 0,
                lines_removed   INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (checkpoint_id, path)
            );

            CREATE INDEX IF NOT EXISTS idx_file_changes_checkpoint
                ON file_changes(checkpoint_id);

            CREATE TABLE IF NOT EXISTS file_content_cache (
                hash            TEXT PRIMARY KEY,
                content         BLOB NOT NULL
            );",
        )?;
        Ok(())
    }

    /// Save a complete checkpoint with its file changes.
    pub fn save_checkpoint(&self, checkpoint: &Checkpoint) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("lock error: {e}"))?;

        let stats_json = serde_json::to_string(&checkpoint.stats)?;

        conn.execute(
            "INSERT INTO checkpoints (id, session_id, message_id, message_index, created_at, summary, stats_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                checkpoint.id.as_str(),
                checkpoint.session_id,
                checkpoint.message_id,
                checkpoint.message_index,
                checkpoint.created_at.to_rfc3339(),
                checkpoint.summary,
                stats_json,
            ],
        )?;

        for change in &checkpoint.file_changes {
            // Reject paths that attempt directory traversal
            if change.path.starts_with('/') || change.path.starts_with('\\') || change.path.contains("..") {
                anyhow::bail!("unsafe path in checkpoint file change: {}", change.path);
            }
            conn.execute(
                "INSERT INTO file_changes (checkpoint_id, path, operation, hash_before, hash_after, lines_added, lines_removed)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    checkpoint.id.as_str(),
                    change.path,
                    change.operation.to_string(),
                    change.hash_before,
                    change.hash_after,
                    change.lines_added,
                    change.lines_removed,
                ],
            )?;
        }

        Ok(())
    }

    /// Store file content in the content cache (keyed by hash).
    pub fn cache_file_content(&self, hash: &str, content: &[u8]) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("lock error: {e}"))?;
        conn.execute(
            "INSERT OR IGNORE INTO file_content_cache (hash, content) VALUES (?1, ?2)",
            params![hash, content],
        )?;
        Ok(())
    }

    /// Retrieve cached file content by hash.
    pub fn get_cached_content(&self, hash: &str) -> Result<Option<Vec<u8>>> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("lock error: {e}"))?;
        let result: Option<Vec<u8>> = conn
            .query_row(
                "SELECT content FROM file_content_cache WHERE hash = ?1",
                params![hash],
                |row| row.get(0),
            )
            .optional()?;
        Ok(result)
    }

    /// Get a specific checkpoint by ID.
    pub fn get_checkpoint(&self, id: &CheckpointId) -> Result<Option<Checkpoint>> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("lock error: {e}"))?;

        let checkpoint_row: Option<(String, String, String, i64, String, String, String)> = conn
            .query_row(
                "SELECT id, session_id, message_id, message_index, created_at, summary, stats_json
                 FROM checkpoints WHERE id = ?1",
                params![id.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?, row.get(6)?)),
            )
            .optional()?;

        let Some((cid, session_id, message_id, msg_idx, created_at_str, summary, stats_json)) =
            checkpoint_row
        else {
            return Ok(None);
        };

        let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|e| anyhow::anyhow!("invalid timestamp: {e}"))?
            .to_utc();
        let stats: CheckpointStats = serde_json::from_str(&stats_json)?;
        let file_changes = self.load_file_changes(&conn, id)?;

        Ok(Some(Checkpoint {
            id: CheckpointId(cid),
            session_id,
            message_id,
            message_index: msg_idx as usize,
            created_at,
            file_changes,
            summary,
            stats,
        }))
    }

    /// List all checkpoints for a session, ordered by message index.
    pub fn list_checkpoints(&self, session_id: &str) -> Result<Vec<CheckpointSummary>> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("lock error: {e}"))?;

        let mut stmt = conn.prepare(
            "SELECT id, session_id, message_id, message_index, created_at, summary, stats_json
             FROM checkpoints WHERE session_id = ?1 ORDER BY message_index ASC",
        )?;

        let rows = stmt.query_map(params![session_id], |row| {
            Ok(CheckpointSummary {
                id: CheckpointId(row.get(0)?),
                session_id: row.get(1)?,
                message_id: row.get(2)?,
                message_index: row.get::<_, i64>(3)? as usize,
                created_at: chrono::DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)
                    .map(|dt| dt.to_utc())
                    .unwrap_or_default(),
                summary: row.get(5)?,
                stats: serde_json::from_str(&row.get::<_, String>(6)?).unwrap_or_default(),
            })
        })?;

        let mut summaries = Vec::new();
        for row in rows {
            summaries.push(row?);
        }
        Ok(summaries)
    }

    /// Get the latest checkpoint for a session.
    pub fn get_latest_checkpoint(&self, session_id: &str) -> Result<Option<Checkpoint>> {
        let summaries = self.list_checkpoints(session_id)?;
        let latest = summaries.last();
        match latest {
            Some(summary) => self.get_checkpoint(&summary.id),
            None => Ok(None),
        }
    }

    /// Delete a checkpoint and its associated file changes.
    pub fn delete_checkpoint(&self, id: &CheckpointId) -> Result<()> {
        let conn = self.conn.lock().map_err(|e| anyhow::anyhow!("lock error: {e}"))?;
        conn.execute("DELETE FROM file_changes WHERE checkpoint_id = ?1", params![id.as_str()])?;
        conn.execute("DELETE FROM checkpoints WHERE id = ?1", params![id.as_str()])?;
        Ok(())
    }

    /// Load file changes for a checkpoint.
    fn load_file_changes(&self, conn: &Connection, id: &CheckpointId) -> Result<Vec<FileChange>> {
        let mut stmt = conn.prepare(
            "SELECT path, operation, hash_before, hash_after, lines_added, lines_removed
             FROM file_changes WHERE checkpoint_id = ?1",
        )?;

        let rows = stmt.query_map(params![id.as_str()], |row| {
            let op_str: String = row.get(1)?;
            let operation = match op_str.as_str() {
                "created" => FileOperation::Created,
                "modified" => FileOperation::Modified,
                "deleted" => FileOperation::Deleted,
                _ => FileOperation::Modified,
            };
            Ok(FileChange {
                path: row.get(0)?,
                operation,
                hash_before: row.get(2)?,
                hash_after: row.get(3)?,
                lines_added: row.get::<_, i64>(4)? as usize,
                lines_removed: row.get::<_, i64>(5)? as usize,
            })
        })?;

        let mut changes = Vec::new();
        for row in rows {
            changes.push(row?);
        }
        Ok(changes)
    }
}

impl Default for CheckpointStats {
    fn default() -> Self {
        Self {
            files_added: 0,
            files_modified: 0,
            files_deleted: 0,
            lines_added: 0,
            lines_removed: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_test_checkpoint(session_id: &str, msg_idx: usize) -> Checkpoint {
        Checkpoint {
            id: CheckpointId::new(),
            session_id: session_id.to_string(),
            message_id: format!("msg-{msg_idx}"),
            message_index: msg_idx,
            created_at: Utc::now(),
            file_changes: vec![FileChange {
                path: format!("file_{msg_idx}.rs"),
                operation: FileOperation::Created,
                hash_before: None,
                hash_after: Some("abc123".to_string()),
                lines_added: 10,
                lines_removed: 0,
            }],
            summary: format!("Checkpoint {msg_idx}"),
            stats: CheckpointStats {
                files_added: 1,
                files_modified: 0,
                files_deleted: 0,
                lines_added: 10,
                lines_removed: 0,
            },
        }
    }

    #[test]
    fn test_save_and_get_checkpoint() {
        let store = CheckpointStore::open_in_memory().unwrap();
        let cp = make_test_checkpoint("session-1", 0);
        let id = cp.id.clone();

        store.save_checkpoint(&cp).unwrap();

        let loaded = store.get_checkpoint(&id).unwrap().unwrap();
        assert_eq!(loaded.session_id, "session-1");
        assert_eq!(loaded.message_index, 0);
        assert_eq!(loaded.file_changes.len(), 1);
        assert_eq!(loaded.file_changes[0].path, "file_0.rs");
    }

    #[test]
    fn test_list_checkpoints() {
        let store = CheckpointStore::open_in_memory().unwrap();
        for i in 0..3 {
            let cp = make_test_checkpoint("session-1", i);
            store.save_checkpoint(&cp).unwrap();
        }

        let summaries = store.list_checkpoints("session-1").unwrap();
        assert_eq!(summaries.len(), 3);
        assert_eq!(summaries[0].message_index, 0);
        assert_eq!(summaries[2].message_index, 2);
    }

    #[test]
    fn test_get_latest_checkpoint() {
        let store = CheckpointStore::open_in_memory().unwrap();
        for i in 0..3 {
            let cp = make_test_checkpoint("session-1", i);
            store.save_checkpoint(&cp).unwrap();
        }

        let latest = store.get_latest_checkpoint("session-1").unwrap().unwrap();
        assert_eq!(latest.message_index, 2);
    }

    #[test]
    fn test_delete_checkpoint() {
        let store = CheckpointStore::open_in_memory().unwrap();
        let cp = make_test_checkpoint("session-1", 0);
        let id = cp.id.clone();

        store.save_checkpoint(&cp).unwrap();
        store.delete_checkpoint(&id).unwrap();

        let loaded = store.get_checkpoint(&id).unwrap();
        assert!(loaded.is_none());
    }

    #[test]
    fn test_cache_file_content() {
        let store = CheckpointStore::open_in_memory().unwrap();
        let content = b"fn main() {}";
        let hash = "test_hash_123";

        store.cache_file_content(hash, content).unwrap();

        let cached = store.get_cached_content(hash).unwrap().unwrap();
        assert_eq!(cached, content);
    }

    #[test]
    fn test_get_nonexistent_checkpoint() {
        let store = CheckpointStore::open_in_memory().unwrap();
        let result = store.get_checkpoint(&CheckpointId("nonexistent".into())).unwrap();
        assert!(result.is_none());
    }
}

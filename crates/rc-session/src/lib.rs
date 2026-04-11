//! Session persistence with SQLite metadata and NDJSON transcripts.
//!
//! [`SessionStore`] manages session lifecycle: creation, conversation append,
//! event storage, and export. Each session is backed by a SQLite row for
//! metadata and an NDJSON file for the full event transcript.

pub mod memory;
pub mod replay;

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use rc_config::AppPaths;
use rc_core::{ConversationEntry, StoredEvent};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Summary metadata for a single session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    /// Unique session identifier.
    pub session_id: Uuid,
    /// Human-readable session title.
    pub title: String,
    /// Working directory at session creation time.
    pub cwd: PathBuf,
    /// Provider name used for this session.
    pub provider_name: String,
    /// Model identifier, if known.
    pub model: Option<String>,
    /// When the session was created.
    pub created_at: DateTime<Utc>,
    /// When the session was last updated.
    pub updated_at: DateTime<Utc>,
    /// Path to the NDJSON transcript file.
    pub transcript_path: PathBuf,
}

/// Token usage aggregated across a session.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionUsageSummary {
    /// Total input tokens consumed.
    pub input_tokens: u64,
    /// Total output tokens consumed.
    pub output_tokens: u64,
}

/// Statistical summary of a session's contents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStats {
    /// Total number of stored events.
    pub total_events: usize,
    /// Number of conversation entries.
    pub conversation_entries: usize,
    /// Message count broken down by role.
    pub messages_by_role: BTreeMap<String, usize>,
    /// Number of tool calls made.
    pub tool_call_count: usize,
    /// Number of error events.
    pub error_count: usize,
    /// Stop reason from the last provider response.
    pub last_stop_reason: Option<String>,
    /// Token usage summary.
    pub usage: SessionUsageSummary,
}

/// A complete bundle of session data for export or inspection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionBundle {
    /// Session metadata.
    pub summary: SessionSummary,
    /// Session statistics.
    pub stats: SessionStats,
    /// Full conversation history.
    pub conversation: Vec<ConversationEntry>,
    /// All stored events.
    pub events: Vec<StoredEvent>,
}

/// Persistent session store backed by SQLite and NDJSON files.
pub struct SessionStore {
    paths: AppPaths,
}

impl SessionStore {
    /// Open (or create) the session store at the given application paths.
    ///
    /// # Errors
    /// Returns an error if the database cannot be opened or the schema cannot be initialised.
    pub fn open(paths: AppPaths) -> Result<Self> {
        paths.ensure_exists()?;
        let store = Self { paths };
        store.init_schema()?;
        Ok(store)
    }

    /// Return a reference to the application paths.
    #[must_use]
    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    /// Ensure a session exists with the given metadata, creating it if needed.
    ///
    /// # Errors
    /// Returns an error if the transcript file or database row cannot be created.
    pub fn ensure_session(
        &self,
        session_id: Uuid,
        cwd: &Path,
        provider_name: &str,
        model: Option<&str>,
        title_hint: Option<&str>,
    ) -> Result<PathBuf> {
        let transcript_path = self.session_transcript_path(session_id);
        if let Some(parent) = transcript_path.parent() {
            fs::create_dir_all(parent)?;
        }
        if !transcript_path.exists() {
            File::create(&transcript_path)?;
        }
        let now = Utc::now();
        let existing = self.try_get_session_summary(session_id)?;
        let title = match existing.as_ref() {
            Some(summary) if !is_default_title(&summary.title, session_id) => summary.title.clone(),
            Some(summary) => {
                normalize_title_hint(title_hint).unwrap_or_else(|| summary.title.clone())
            }
            None => {
                normalize_title_hint(title_hint).unwrap_or_else(|| format!("session-{session_id}"))
            }
        };
        let created_at = existing.as_ref().map_or(now, |summary| summary.created_at);
        self.connection()?.execute(
            "INSERT INTO sessions (
                session_id, title, cwd, provider_name, model, created_at, updated_at, transcript_path
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(session_id) DO UPDATE SET
                title = excluded.title,
                cwd = excluded.cwd,
                provider_name = excluded.provider_name,
                model = excluded.model,
                updated_at = excluded.updated_at,
                transcript_path = excluded.transcript_path",
            params![
                session_id.to_string(),
                title,
                cwd.display().to_string(),
                provider_name,
                model,
                created_at.to_rfc3339(),
                now.to_rfc3339(),
                transcript_path.display().to_string(),
            ],
        )?;
        Ok(transcript_path)
    }

    /// Append a conversation entry to the session transcript.
    ///
    /// # Errors
    /// Returns an error if the event cannot be written to the transcript file.
    pub fn append_conversation_entry(
        &self,
        session_id: Uuid,
        conversation: &ConversationEntry,
    ) -> Result<()> {
        let event = StoredEvent {
            timestamp: Utc::now(),
            session_id,
            event_type: "conversation".to_owned(),
            conversation: Some(conversation.clone()),
            payload: None,
        };
        self.append_event(&event)?;
        self.touch(session_id)?;
        Ok(())
    }

    /// Append a named event with a JSON payload to the session transcript.
    ///
    /// # Errors
    /// Returns an error if the event cannot be written to the transcript file.
    pub fn append_named_event(
        &self,
        session_id: Uuid,
        event_type: impl Into<String>,
        payload: Value,
    ) -> Result<()> {
        let event = StoredEvent {
            timestamp: Utc::now(),
            session_id,
            event_type: event_type.into(),
            conversation: None,
            payload: Some(payload),
        };
        self.append_event(&event)?;
        self.touch(session_id)?;
        Ok(())
    }

    /// List all sessions ordered by last-updated time (newest first).
    ///
    /// # Errors
    /// Returns an error if the database query fails.
    pub fn list_sessions(&self) -> Result<Vec<SessionSummary>> {
        let conn = self.connection()?;
        let mut statement = conn.prepare(
            "SELECT session_id, title, cwd, provider_name, model, created_at, updated_at, transcript_path
             FROM sessions ORDER BY updated_at DESC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
            ))
        })?;
        let raw_rows = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        raw_rows.into_iter().map(raw_row_to_summary).collect()
    }

    /// Get the summary for a specific session.
    ///
    /// # Errors
    /// Returns an error if the session does not exist.
    pub fn get_session_summary(&self, session_id: Uuid) -> Result<SessionSummary> {
        self.try_get_session_summary(session_id)?
            .ok_or_else(|| anyhow!("session {session_id} does not exist"))
    }

    /// Load all stored events for a session from its NDJSON transcript.
    ///
    /// # Errors
    /// Returns an error if the transcript file cannot be read or parsed.
    pub fn load_events(&self, session_id: Uuid) -> Result<Vec<StoredEvent>> {
        let transcript_path = self.session_transcript_path(session_id);
        let file = File::open(&transcript_path)
            .with_context(|| format!("failed to open {}", transcript_path.display()))?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            events.push(serde_json::from_str(&line)?);
        }
        Ok(events)
    }

    /// Load only the conversation entries for a session.
    ///
    /// # Errors
    /// Returns an error if the transcript file cannot be read or parsed.
    pub fn load_conversation(&self, session_id: Uuid) -> Result<Vec<ConversationEntry>> {
        Ok(self
            .load_events(session_id)?
            .into_iter()
            .filter_map(|event| event.conversation)
            .collect())
    }

    /// Load a complete session bundle (summary, stats, conversation, events).
    ///
    /// # Errors
    /// Returns an error if the session does not exist or data cannot be read.
    pub fn load_session_bundle(&self, session_id: Uuid) -> Result<SessionBundle> {
        let summary = self.get_session_summary(session_id)?;
        let events = self.load_events(session_id)?;
        let conversation = events
            .iter()
            .filter_map(|event| event.conversation.clone())
            .collect::<Vec<_>>();
        let stats = build_session_stats(&events, &conversation);
        Ok(SessionBundle {
            summary,
            stats,
            conversation,
            events,
        })
    }

    /// Export the session transcript to an NDJSON file.
    ///
    /// # Errors
    /// Returns an error if the session does not exist or the file copy fails.
    pub fn export_session(
        &self,
        session_id: Uuid,
        output_path: Option<PathBuf>,
    ) -> Result<PathBuf> {
        let source = self.session_transcript_path(session_id);
        if !source.exists() {
            return Err(anyhow!("session {session_id} does not exist"));
        }
        let destination = output_path.unwrap_or_else(|| {
            self.paths
                .artifacts_dir
                .join(format!("session-{session_id}.ndjson"))
        });
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(&source, &destination)?;
        Ok(destination)
    }

    /// Export the session bundle as a single JSON file.
    ///
    /// # Errors
    /// Returns an error if the session cannot be loaded or the file cannot be written.
    pub fn export_session_bundle_json(
        &self,
        session_id: Uuid,
        output_path: Option<PathBuf>,
    ) -> Result<PathBuf> {
        let bundle = self.load_session_bundle(session_id)?;
        let destination = output_path.unwrap_or_else(|| {
            self.paths
                .artifacts_dir
                .join(format!("session-{session_id}.json"))
        });
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_vec_pretty(&bundle)?;
        fs::write(&destination, contents)?;
        Ok(destination)
    }

    #[must_use]
    pub fn session_transcript_path(&self, session_id: Uuid) -> PathBuf {
        self.paths.sessions_dir.join(format!("{session_id}.ndjson"))
    }

    fn append_event(&self, event: &StoredEvent) -> Result<()> {
        let transcript_path = self.session_transcript_path(event.session_id);
        if let Some(parent) = transcript_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&transcript_path)?;
        serde_json::to_writer(&mut file, event)?;
        file.write_all(b"\n")?;
        Ok(())
    }

    fn touch(&self, session_id: Uuid) -> Result<()> {
        self.connection()?.execute(
            "UPDATE sessions SET updated_at = ?2 WHERE session_id = ?1",
            params![session_id.to_string(), Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.connection()?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sessions (
                session_id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                cwd TEXT NOT NULL,
                provider_name TEXT NOT NULL,
                model TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                transcript_path TEXT NOT NULL
            );",
        )?;
        Ok(())
    }

    fn connection(&self) -> Result<Connection> {
        Connection::open(&self.paths.state_db_path)
            .with_context(|| format!("failed to open {}", self.paths.state_db_path.display()))
    }

    fn try_get_session_summary(&self, session_id: Uuid) -> Result<Option<SessionSummary>> {
        let conn = self.connection()?;
        let mut statement = conn.prepare(
            "SELECT session_id, title, cwd, provider_name, model, created_at, updated_at, transcript_path
             FROM sessions WHERE session_id = ?1 LIMIT 1",
        )?;
        let row = statement.query_row([session_id.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, String>(7)?,
            ))
        });

        match row {
            Ok(raw) => raw_row_to_summary(raw).map(Some),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

fn parse_timestamp(value: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(value)?.with_timezone(&Utc))
}

fn normalize_title_hint(title_hint: Option<&str>) -> Option<String> {
    title_hint
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(80).collect::<String>())
}

fn is_default_title(title: &str, session_id: Uuid) -> bool {
    title == format!("session-{session_id}")
}

fn raw_row_to_summary(
    raw: (
        String,
        String,
        String,
        String,
        Option<String>,
        String,
        String,
        String,
    ),
) -> Result<SessionSummary> {
    let (session_id, title, cwd, provider_name, model, created_at, updated_at, transcript_path) =
        raw;
    Ok(SessionSummary {
        session_id: Uuid::parse_str(&session_id)?,
        title,
        cwd: PathBuf::from(cwd),
        provider_name,
        model,
        created_at: parse_timestamp(&created_at)?,
        updated_at: parse_timestamp(&updated_at)?,
        transcript_path: PathBuf::from(transcript_path),
    })
}

fn build_session_stats(events: &[StoredEvent], conversation: &[ConversationEntry]) -> SessionStats {
    let mut messages_by_role = BTreeMap::new();
    let mut tool_call_count = 0usize;
    let mut error_count = 0usize;
    for entry in conversation {
        let role = match entry.role {
            rc_core::ConversationRole::System => "system",
            rc_core::ConversationRole::User => "user",
            rc_core::ConversationRole::Assistant => "assistant",
            rc_core::ConversationRole::Tool => "tool",
        };
        *messages_by_role.entry(role.to_owned()).or_insert(0) += 1;
        tool_call_count += entry.tool_calls.len();
        if entry.is_error {
            error_count += 1;
        }
    }

    let mut usage = SessionUsageSummary::default();
    let mut last_stop_reason = None;
    for event in events {
        if let Some(payload) = &event.payload {
            if payload
                .get("is_error")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                error_count += 1;
            }
            if let Some(stop_reason) = payload.get("stop_reason").and_then(Value::as_str) {
                last_stop_reason = Some(stop_reason.to_owned());
            }
            if let Some(event_usage) = payload.get("usage") {
                usage.input_tokens += event_usage
                    .get("input_tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or_default();
                usage.output_tokens += event_usage
                    .get("output_tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or_default();
            }
        }
    }

    SessionStats {
        total_events: events.len(),
        conversation_entries: conversation.len(),
        messages_by_role,
        tool_call_count,
        error_count,
        last_stop_reason,
        usage,
    }
}

#[cfg(test)]
mod tests {
    use super::SessionStore;
    use rc_config::AppPaths;
    use rc_core::ConversationEntry;
    use serde_json::json;
    use tempfile::tempdir;
    use uuid::Uuid;

    #[test]
    fn store_can_round_trip_sessions() {
        let tempdir = match tempdir() {
            Ok(dir) => dir,
            Err(error) => panic!("failed to create tempdir: {error}"),
        };
        let paths = AppPaths::discover(Some(tempdir.path().join(".remote-code-rust")));
        assert!(paths.is_ok());
        let store = SessionStore::open(paths.unwrap_or_else(|error| panic!("{error}")));
        assert!(store.is_ok());
        let store = store.unwrap_or_else(|error| panic!("{error}"));

        let session_id = Uuid::new_v4();
        let ensured = store.ensure_session(
            session_id,
            tempdir.path(),
            "mock",
            Some("mock-model"),
            Some("hello world"),
        );
        assert!(ensured.is_ok());
        let appended =
            store.append_conversation_entry(session_id, &ConversationEntry::user("ship it"));
        assert!(appended.is_ok());
        let list = store.list_sessions();
        assert!(list.is_ok());
        let list = list.unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(list.len(), 1);

        let loaded = store.load_conversation(session_id);
        assert!(loaded.is_ok());
        let loaded = loaded.unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(loaded.len(), 1);

        let appended = store.append_named_event(
            session_id,
            "result",
            json!({
                "is_error": false,
                "stop_reason": "end_turn",
                "usage": {"input_tokens": 2, "output_tokens": 3}
            }),
        );
        assert!(appended.is_ok());

        let bundle = store.load_session_bundle(session_id);
        assert!(bundle.is_ok());
        let bundle = bundle.unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(bundle.stats.total_events, 2);
        assert_eq!(bundle.stats.usage.output_tokens, 3);

        let export = store.export_session_bundle_json(session_id, None);
        assert!(export.is_ok());
        let export = export.unwrap_or_else(|error| panic!("{error}"));
        assert!(export.exists());
    }
}

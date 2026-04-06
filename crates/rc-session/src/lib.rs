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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSummary {
    pub session_id: Uuid,
    pub title: String,
    pub cwd: PathBuf,
    pub provider_name: String,
    pub model: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub transcript_path: PathBuf,
}

pub struct SessionStore {
    paths: AppPaths,
}

impl SessionStore {
    pub fn open(paths: AppPaths) -> Result<Self> {
        paths.ensure_exists()?;
        let store = Self { paths };
        store.init_schema()?;
        Ok(store)
    }

    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

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
        let title = title_hint
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| value.chars().take(80).collect::<String>())
            .unwrap_or_else(|| format!("session-{session_id}"));
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
                now.to_rfc3339(),
                now.to_rfc3339(),
                transcript_path.display().to_string(),
            ],
        )?;
        Ok(transcript_path)
    }

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
        raw_rows
            .into_iter()
            .map(
                |(
                    session_id,
                    title,
                    cwd,
                    provider_name,
                    model,
                    created_at,
                    updated_at,
                    transcript_path,
                )| {
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
                },
            )
            .collect()
    }

    pub fn load_conversation(&self, session_id: Uuid) -> Result<Vec<ConversationEntry>> {
        let transcript_path = self.session_transcript_path(session_id);
        let file = File::open(&transcript_path)
            .with_context(|| format!("failed to open {}", transcript_path.display()))?;
        let reader = BufReader::new(file);
        let mut entries = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let event: StoredEvent = serde_json::from_str(&line)?;
            if let Some(conversation) = event.conversation {
                entries.push(conversation);
            }
        }
        Ok(entries)
    }

    pub fn export_session(
        &self,
        session_id: Uuid,
        output_path: Option<PathBuf>,
    ) -> Result<PathBuf> {
        let source = self.session_transcript_path(session_id);
        if !source.exists() {
            return Err(anyhow!("session {} does not exist", session_id));
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
}

fn parse_timestamp(value: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(value)?.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::SessionStore;
    use rc_config::AppPaths;
    use rc_core::ConversationEntry;
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
    }
}

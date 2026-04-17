//! Session management integration tests.
//!
//! Tests session creation, conversation append, transcript round-trip,
//! session recovery, and compaction.

use std::path::Path;

fn make_paths(dir: &tempfile::TempDir) -> rc_config::AppPaths {
    rc_config::AppPaths {
        profile_dir: dir.path().to_path_buf(),
        state_db_path: dir.path().join("state.db"),
        sessions_dir: dir.path().join("sessions"),
        artifacts_dir: dir.path().join("artifacts"),
        logs_dir: dir.path().join("logs"),
        profiles_dir: dir.path().join("profiles"),
        skills_dir: dir.path().join("skills"),
        plugins_dir: dir.path().join("plugins"),
    }
}

// ─── Session types round-trip ───────────────────────────────────────────────

#[test]
fn session_summary_round_trips() {
    let summary = rc_session::SessionSummary {
        session_id: uuid::Uuid::new_v4(),
        title: "Test Session".to_owned(),
        cwd: std::path::PathBuf::from("/tmp/test"),
        provider_name: "anthropic".to_owned(),
        model: Some("claude-sonnet-4-20250514".to_owned()),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        transcript_path: std::path::PathBuf::from("/tmp/test.jsonl"),
        archived: false,
    };
    let json = serde_json::to_string(&summary).expect("serialize summary");
    let decoded: rc_session::SessionSummary =
        serde_json::from_str(&json).expect("deserialize summary");
    assert_eq!(decoded.session_id, summary.session_id);
    assert_eq!(decoded.title, "Test Session");
    assert!(!decoded.archived);
}

#[test]
fn session_stats_round_trips() {
    let stats = rc_session::SessionStats {
        total_events: 100,
        conversation_entries: 50,
        messages_by_role: {
            let mut map = std::collections::BTreeMap::new();
            map.insert("user".to_owned(), 20);
            map.insert("assistant".to_owned(), 20);
            map.insert("tool".to_owned(), 10);
            map
        },
        tool_call_count: 10,
        error_count: 2,
        last_stop_reason: Some("end_turn".to_owned()),
        usage: rc_session::SessionUsageSummary {
            input_tokens: 5000,
            output_tokens: 2000,
        },
    };
    let json = serde_json::to_string(&stats).expect("serialize stats");
    let decoded: rc_session::SessionStats = serde_json::from_str(&json).expect("deserialize stats");
    assert_eq!(decoded.total_events, 100);
    assert_eq!(decoded.conversation_entries, 50);
    assert_eq!(decoded.tool_call_count, 10);
}

#[test]
fn session_bundle_round_trips() {
    let bundle = rc_session::SessionBundle {
        summary: rc_session::SessionSummary {
            session_id: uuid::Uuid::new_v4(),
            title: "Bundle Test".to_owned(),
            cwd: std::path::PathBuf::from("/tmp"),
            provider_name: "openai".to_owned(),
            model: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            transcript_path: std::path::PathBuf::from("/tmp/test.jsonl"),
            archived: false,
        },
        stats: rc_session::SessionStats {
            total_events: 10,
            conversation_entries: 5,
            messages_by_role: std::collections::BTreeMap::new(),
            tool_call_count: 0,
            error_count: 0,
            last_stop_reason: None,
            usage: rc_session::SessionUsageSummary::default(),
        },
        conversation: vec![
            rc_core::ConversationEntry::user("hello"),
            rc_core::ConversationEntry::assistant("hi there"),
        ],
        events: vec![],
    };
    let json = serde_json::to_string(&bundle).expect("serialize bundle");
    let decoded: rc_session::SessionBundle =
        serde_json::from_str(&json).expect("deserialize bundle");
    assert_eq!(decoded.conversation.len(), 2);
    assert_eq!(decoded.summary.title, "Bundle Test");
}

// ─── Session store with tempdir ─────────────────────────────────────────────

#[test]
fn session_store_creates_and_lists_sessions() {
    let dir = tempfile::tempdir().expect("tempdir should be created");
    let paths = make_paths(&dir);

    let store = rc_session::SessionStore::open(paths).expect("store should open");
    let session_id = uuid::Uuid::new_v4();

    store
        .ensure_session(
            session_id,
            Path::new("/tmp"),
            "anthropic",
            Some("claude-sonnet-4-20250514"),
            Some("Test Session"),
        )
        .expect("session should be created");

    let sessions = store.list_sessions().expect("should list sessions");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id, session_id);
    assert_eq!(sessions[0].provider_name, "anthropic");
}

#[test]
fn session_store_append_conversation_entries() {
    let dir = tempfile::tempdir().expect("tempdir should be created");
    let paths = make_paths(&dir);

    let store = rc_session::SessionStore::open(paths).expect("store should open");
    let session_id = uuid::Uuid::new_v4();

    store
        .ensure_session(session_id, Path::new("/tmp"), "anthropic", None, None)
        .expect("session should be created");

    // Append conversation entries
    store
        .append_conversation_entry(session_id, &rc_core::ConversationEntry::user("hello"))
        .expect("should append user entry");
    store
        .append_conversation_entry(
            session_id,
            &rc_core::ConversationEntry::assistant("hi there"),
        )
        .expect("should append assistant entry");
    store
        .append_conversation_entry(
            session_id,
            &rc_core::ConversationEntry::tool("tc-1", "read_file", "file contents", false),
        )
        .expect("should append tool entry");

    // Verify via load_conversation
    let conversation = store
        .load_conversation(session_id)
        .expect("should load conversation");
    assert_eq!(conversation.len(), 3);
}

#[test]
fn session_store_append_named_events() {
    let dir = tempfile::tempdir().expect("tempdir should be created");
    let paths = make_paths(&dir);

    let store = rc_session::SessionStore::open(paths).expect("store should open");
    let session_id = uuid::Uuid::new_v4();

    store
        .ensure_session(session_id, Path::new("/tmp"), "anthropic", None, None)
        .expect("session should be created");

    store
        .append_named_event(
            session_id,
            "tool_result",
            serde_json::json!({"tool": "read_file"}),
        )
        .expect("should append named event");

    let events = store.load_events(session_id).expect("should load events");
    assert_eq!(events.len(), 1); // only the named_event (ensure_session writes metadata, not events)
}

#[test]
fn session_store_archive_and_restore() {
    let dir = tempfile::tempdir().expect("tempdir should be created");
    let paths = make_paths(&dir);

    let store = rc_session::SessionStore::open(paths).expect("store should open");
    let session_id = uuid::Uuid::new_v4();

    store
        .ensure_session(
            session_id,
            Path::new("/tmp"),
            "anthropic",
            None,
            Some("Archive Test"),
        )
        .expect("session should be created");

    // Archive
    store
        .set_archived(session_id, true)
        .expect("should archive");

    let active = store.list_active_sessions().expect("should list active");
    assert!(active.is_empty());

    let archived = store
        .list_archived_sessions()
        .expect("should list archived");
    assert_eq!(archived.len(), 1);

    // Restore
    store
        .set_archived(session_id, false)
        .expect("should restore");
    let active = store.list_active_sessions().expect("should list active");
    assert_eq!(active.len(), 1);
}

// ─── Transcript round-trip ──────────────────────────────────────────────────

#[tokio::test]
async fn transcript_storage_write_and_read() {
    let dir = tempfile::tempdir().expect("tempdir should be created");
    let path = dir.path().join("test.jsonl");

    let storage = rc_transcript::TranscriptStorage::new(&path);
    let session_id = uuid::Uuid::new_v4();

    // Write entries
    storage
        .append(&rc_transcript::TranscriptEntry::conversation_now(
            session_id,
            rc_core::ConversationEntry::user("test message"),
        ))
        .await
        .expect("should write conversation entry");

    storage
        .append(&rc_transcript::TranscriptEntry::named_event_now(
            session_id,
            "tool_call",
            Some(serde_json::json!({"name": "read_file"})),
        ))
        .await
        .expect("should write named event");

    // Read back
    let entries = storage.read_all().await.expect("should read entries");
    assert_eq!(entries.len(), 2);
    assert_eq!(
        entries[0].kind(),
        rc_transcript::TranscriptEntryKind::Conversation
    );
    assert_eq!(
        entries[1].kind(),
        rc_transcript::TranscriptEntryKind::NamedEvent
    );
}

#[tokio::test]
async fn transcript_storage_empty_file() {
    let dir = tempfile::tempdir().expect("tempdir should be created");
    let path = dir.path().join("empty.jsonl");

    let storage = rc_transcript::TranscriptStorage::new(&path);
    let entries = storage.read_all().await.expect("should read empty");
    assert!(entries.is_empty());
}

// ─── Resume state ───────────────────────────────────────────────────────────

#[test]
fn resume_state_round_trips() {
    let state = rc_session::resume_state::ResumeState::from_pending_calls(vec![
        rc_session::resume_state::PendingToolCall {
            id: "tc-1".to_owned(),
            name: "read_file".to_owned(),
            input: serde_json::json!({"path": "/tmp/test.rs"}),
        },
    ]);
    let json = serde_json::to_string(&state).expect("serialize resume state");
    let decoded: rc_session::resume_state::ResumeState =
        serde_json::from_str(&json).expect("deserialize resume state");
    assert_eq!(decoded.pending_tool_calls.len(), 1);
    assert_eq!(decoded.pending_tool_calls[0].name, "read_file");
}

#[test]
fn resume_state_empty() {
    let state = rc_session::resume_state::ResumeState::empty();
    assert!(state.pending_tool_calls.is_empty());
}

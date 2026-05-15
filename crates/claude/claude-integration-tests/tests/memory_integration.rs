//! Memory system integration tests.
//!
//! Validates the memory pipeline across rc-utils (MemoryStore, MemoryEntry,
//! MemoryType) and cross-crate integration with rc-tools (memory_tools
//! BM25 search, memory save/load/delete tools).

use std::path::PathBuf;

// ─── Helpers ──────────────────────────────────────────────────────────────

fn ok<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("unexpected error: {error}"),
    }
}

// ─── MemoryType ───────────────────────────────────────────────────────────

#[test]
fn memory_type_dir_names() {
    assert_eq!(
        claude_utils::memory_types::MemoryType::Project.dir_name(),
        "project"
    );
    assert_eq!(
        claude_utils::memory_types::MemoryType::User.dir_name(),
        "user"
    );
    assert_eq!(
        claude_utils::memory_types::MemoryType::Agent.dir_name(),
        "agent"
    );
}

#[test]
fn memory_type_display() {
    assert_eq!(
        claude_utils::memory_types::MemoryType::Project.to_string(),
        "project"
    );
    assert_eq!(
        claude_utils::memory_types::MemoryType::User.to_string(),
        "user"
    );
    assert_eq!(
        claude_utils::memory_types::MemoryType::Agent.to_string(),
        "agent"
    );
}

#[test]
fn memory_type_from_str_opt() {
    assert_eq!(
        claude_utils::memory_types::MemoryType::from_str_opt("project"),
        Some(claude_utils::memory_types::MemoryType::Project)
    );
    assert_eq!(
        claude_utils::memory_types::MemoryType::from_str_opt("user"),
        Some(claude_utils::memory_types::MemoryType::User)
    );
    assert_eq!(
        claude_utils::memory_types::MemoryType::from_str_opt("agent"),
        Some(claude_utils::memory_types::MemoryType::Agent)
    );
    assert_eq!(
        claude_utils::memory_types::MemoryType::from_str_opt("unknown"),
        None
    );
}

#[test]
fn memory_type_serialization_round_trip() {
    for mt in claude_utils::memory_types::MemoryType::all_values() {
        let json = serde_json::to_string(mt).expect("serialize");
        let decoded: claude_utils::memory_types::MemoryType =
            serde_json::from_str(&json).expect("deserialize");
        assert_eq!(*mt, decoded);
    }
}

#[test]
fn memory_type_values_constant() {
    assert_eq!(
        claude_utils::memory_types::MEMORY_TYPE_VALUES,
        &["project", "user", "agent"]
    );
}

// ─── MemoryEntry ──────────────────────────────────────────────────────────

#[test]
fn memory_entry_new_has_timestamp() {
    let entry = claude_utils::memory_types::MemoryEntry::new(
        "test content".to_owned(),
        claude_utils::memory_types::MemoryType::Project,
    );
    assert_eq!(entry.content, "test content");
    assert_eq!(entry.scope, claude_utils::memory_types::MemoryType::Project);
    assert!(!entry.timestamp.is_empty());
    assert!(entry.tags.is_empty());
}

#[test]
fn memory_entry_with_tags() {
    let entry = claude_utils::memory_types::MemoryEntry::new(
        "tagged content".to_owned(),
        claude_utils::memory_types::MemoryType::User,
    )
    .with_tags(vec!["important".to_owned(), "notes".to_owned()]);

    assert!(entry.has_tag("important"));
    assert!(entry.has_tag("notes"));
    assert!(!entry.has_tag("missing"));
}

#[test]
fn memory_entry_serialization_round_trip() {
    let entry = claude_utils::memory_types::MemoryEntry::new(
        "persist me".to_owned(),
        claude_utils::memory_types::MemoryType::Agent,
    )
    .with_tags(vec!["test".to_owned()]);

    let json = serde_json::to_string(&entry).expect("serialize");
    let decoded: claude_utils::memory_types::MemoryEntry =
        serde_json::from_str(&json).expect("deserialize");
    assert_eq!(entry, decoded);
}

// ─── Memory directory layout ──────────────────────────────────────────────

#[test]
fn memory_dir_layout() {
    let base = PathBuf::from("/tmp/project");
    let dir = claude_utils::memory_types::memory_dir(
        &base,
        claude_utils::memory_types::MemoryType::Project,
    );
    assert_eq!(
        dir,
        PathBuf::from("/tmp/project/.remote-code/memory/project")
    );
}

#[test]
fn memory_file_path_layout() {
    let base = PathBuf::from("/tmp/project");
    let path = claude_utils::memory_types::memory_file_path(
        &base,
        claude_utils::memory_types::MemoryType::User,
        "notes.json",
    );
    assert_eq!(
        path,
        PathBuf::from("/tmp/project/.remote-code/memory/user/notes.json")
    );
}

// ─── MemoryStore save/load/delete ─────────────────────────────────────────

#[test]
fn memory_store_save_and_load() {
    let temp = ok(tempfile::tempdir());
    let store = claude_utils::memory_store::MemoryStore::new(temp.path());

    let stored = ok(store.save_memory(
        "test-key",
        "Hello, memory!",
        claude_utils::memory_types::MemoryType::Project,
        vec!["test".to_owned()],
    ));
    assert_eq!(stored.key, "test-key");
    assert_eq!(stored.entry.content, "Hello, memory!");
    assert!(stored.entry.has_tag("test"));

    let loaded = ok(store.load_memory("test-key", claude_utils::memory_types::MemoryType::Project));
    assert_eq!(loaded.key, "test-key");
    assert_eq!(loaded.entry.content, "Hello, memory!");
}

#[test]
fn memory_store_creates_directory_on_save() {
    let temp = ok(tempfile::tempdir());
    let store = claude_utils::memory_store::MemoryStore::new(temp.path());
    assert!(
        !store
            .scope_dir(claude_utils::memory_types::MemoryType::User)
            .exists()
    );

    ok(store.save_memory(
        "x",
        "content",
        claude_utils::memory_types::MemoryType::User,
        vec![],
    ));
    assert!(
        store
            .scope_dir(claude_utils::memory_types::MemoryType::User)
            .exists()
    );
}

#[test]
fn memory_store_delete_removes_entry() {
    let temp = ok(tempfile::tempdir());
    let store = claude_utils::memory_store::MemoryStore::new(temp.path());

    ok(store.save_memory(
        "del-me",
        "bye",
        claude_utils::memory_types::MemoryType::Project,
        vec![],
    ));
    assert!(store.exists("del-me", claude_utils::memory_types::MemoryType::Project));

    ok(store.delete_memory("del-me", claude_utils::memory_types::MemoryType::Project));
    assert!(!store.exists("del-me", claude_utils::memory_types::MemoryType::Project));
}

#[test]
fn memory_store_delete_nonexistent_fails() {
    let temp = ok(tempfile::tempdir());
    let store = claude_utils::memory_store::MemoryStore::new(temp.path());
    let err = store
        .delete_memory("ghost", claude_utils::memory_types::MemoryType::Project)
        .expect_err("deleting a missing memory should fail");
    assert!(err.to_string().contains("ghost"));
}

#[test]
fn memory_store_load_nonexistent_fails() {
    let temp = ok(tempfile::tempdir());
    let store = claude_utils::memory_store::MemoryStore::new(temp.path());
    let err = store
        .load_memory("nope", claude_utils::memory_types::MemoryType::Project)
        .expect_err("loading a missing memory should fail");
    assert!(err.to_string().contains("nope"));
}

// ─── MemoryStore list ─────────────────────────────────────────────────────

#[test]
fn memory_store_list_empty_scope() {
    let temp = ok(tempfile::tempdir());
    let store = claude_utils::memory_store::MemoryStore::new(temp.path());
    let list = ok(store.list_memories(claude_utils::memory_types::MemoryType::Project));
    assert!(list.is_empty());
}

#[test]
fn memory_store_list_sorted_by_key() {
    let temp = ok(tempfile::tempdir());
    let store = claude_utils::memory_store::MemoryStore::new(temp.path());
    ok(store.save_memory(
        "gamma",
        "C",
        claude_utils::memory_types::MemoryType::Project,
        vec![],
    ));
    ok(store.save_memory(
        "alpha",
        "A",
        claude_utils::memory_types::MemoryType::Project,
        vec![],
    ));
    ok(store.save_memory(
        "beta",
        "B",
        claude_utils::memory_types::MemoryType::Project,
        vec![],
    ));

    let list = ok(store.list_memories(claude_utils::memory_types::MemoryType::Project));
    assert_eq!(list.len(), 3);
    assert_eq!(list[0].key, "alpha");
    assert_eq!(list[1].key, "beta");
    assert_eq!(list[2].key, "gamma");
}

#[test]
fn memory_store_list_isolated_by_scope() {
    let temp = ok(tempfile::tempdir());
    let store = claude_utils::memory_store::MemoryStore::new(temp.path());
    ok(store.save_memory(
        "shared",
        "project content",
        claude_utils::memory_types::MemoryType::Project,
        vec![],
    ));
    ok(store.save_memory(
        "shared",
        "user content",
        claude_utils::memory_types::MemoryType::User,
        vec![],
    ));

    let project = ok(store.list_memories(claude_utils::memory_types::MemoryType::Project));
    let user = ok(store.list_memories(claude_utils::memory_types::MemoryType::User));
    assert_eq!(project.len(), 1);
    assert_eq!(user.len(), 1);
    assert_eq!(project[0].entry.content, "project content");
    assert_eq!(user[0].entry.content, "user content");
}

#[test]
fn memory_store_list_all_memories() {
    let temp = ok(tempfile::tempdir());
    let store = claude_utils::memory_store::MemoryStore::new(temp.path());
    ok(store.save_memory(
        "p1",
        "P1",
        claude_utils::memory_types::MemoryType::Project,
        vec![],
    ));
    ok(store.save_memory(
        "u1",
        "U1",
        claude_utils::memory_types::MemoryType::User,
        vec![],
    ));

    let all = ok(store.list_all_memories());
    assert_eq!(
        all[&claude_utils::memory_types::MemoryType::Project].len(),
        1
    );
    assert_eq!(all[&claude_utils::memory_types::MemoryType::User].len(), 1);
    assert_eq!(all[&claude_utils::memory_types::MemoryType::Agent].len(), 0);
}

// ─── MemoryStore search ───────────────────────────────────────────────────

#[test]
fn memory_store_search_by_content() {
    let temp = ok(tempfile::tempdir());
    let store = claude_utils::memory_store::MemoryStore::new(temp.path());
    ok(store.save_memory(
        "note1",
        "Rust programming tips",
        claude_utils::memory_types::MemoryType::Project,
        vec![],
    ));
    ok(store.save_memory(
        "note2",
        "Python data science",
        claude_utils::memory_types::MemoryType::Project,
        vec![],
    ));

    let results = ok(store.search_memory("rust"));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "note1");
}

#[test]
fn memory_store_search_by_tag() {
    let temp = ok(tempfile::tempdir());
    let store = claude_utils::memory_store::MemoryStore::new(temp.path());
    ok(store.save_memory(
        "tagged",
        "content",
        claude_utils::memory_types::MemoryType::Project,
        vec!["important".to_owned()],
    ));
    ok(store.save_memory(
        "untagged",
        "content",
        claude_utils::memory_types::MemoryType::Project,
        vec![],
    ));

    let results = ok(store.search_memory("important"));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "tagged");
}

#[test]
fn memory_store_search_by_key() {
    let temp = ok(tempfile::tempdir());
    let store = claude_utils::memory_store::MemoryStore::new(temp.path());
    ok(store.save_memory(
        "deploy-notes",
        "deploy content",
        claude_utils::memory_types::MemoryType::Project,
        vec![],
    ));
    ok(store.save_memory(
        "test-notes",
        "test content",
        claude_utils::memory_types::MemoryType::Project,
        vec![],
    ));

    let results = ok(store.search_memory("deploy"));
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "deploy-notes");
}

#[test]
fn memory_store_search_case_insensitive() {
    let temp = ok(tempfile::tempdir());
    let store = claude_utils::memory_store::MemoryStore::new(temp.path());
    ok(store.save_memory(
        "note",
        "Rust Programming",
        claude_utils::memory_types::MemoryType::Project,
        vec![],
    ));

    let results_lower = ok(store.search_memory("rust"));
    let results_upper = ok(store.search_memory("RUST"));
    assert_eq!(results_lower.len(), 1);
    assert_eq!(results_upper.len(), 1);
}

// ─── MemoryStore update and tags ──────────────────────────────────────────

#[test]
fn memory_store_update_existing() {
    let temp = ok(tempfile::tempdir());
    let store = claude_utils::memory_store::MemoryStore::new(temp.path());
    ok(store.save_memory(
        "updatable",
        "original",
        claude_utils::memory_types::MemoryType::Project,
        vec!["v1".to_owned()],
    ));

    ok(store.update_memory(
        "updatable",
        "updated content",
        claude_utils::memory_types::MemoryType::Project,
        vec!["v2".to_owned()],
    ));

    let loaded =
        ok(store.load_memory("updatable", claude_utils::memory_types::MemoryType::Project));
    assert_eq!(loaded.entry.content, "updated content");
    assert!(loaded.entry.has_tag("v2"));
    assert!(!loaded.entry.has_tag("v1"));
}

#[test]
fn memory_store_update_nonexistent_fails() {
    let temp = ok(tempfile::tempdir());
    let store = claude_utils::memory_store::MemoryStore::new(temp.path());
    let err = store
        .update_memory(
            "ghost",
            "content",
            claude_utils::memory_types::MemoryType::Project,
            vec![],
        )
        .expect_err("updating a missing memory should fail");
    assert!(err.to_string().contains("ghost"));
}

#[test]
fn memory_store_all_tags() {
    let temp = ok(tempfile::tempdir());
    let store = claude_utils::memory_store::MemoryStore::new(temp.path());
    ok(store.save_memory(
        "a",
        "content a",
        claude_utils::memory_types::MemoryType::Project,
        vec!["alpha".to_owned()],
    ));
    ok(store.save_memory(
        "b",
        "content b",
        claude_utils::memory_types::MemoryType::User,
        vec!["beta".to_owned()],
    ));

    let tags = ok(store.all_tags());
    assert!(tags.contains(&"alpha".to_owned()));
    assert!(tags.contains(&"beta".to_owned()));
}

#[test]
fn memory_store_count() {
    let temp = ok(tempfile::tempdir());
    let store = claude_utils::memory_store::MemoryStore::new(temp.path());
    assert_eq!(
        ok(store.count(claude_utils::memory_types::MemoryType::Project)),
        0
    );

    ok(store.save_memory(
        "a",
        "A",
        claude_utils::memory_types::MemoryType::Project,
        vec![],
    ));
    ok(store.save_memory(
        "b",
        "B",
        claude_utils::memory_types::MemoryType::Project,
        vec![],
    ));
    assert_eq!(
        ok(store.count(claude_utils::memory_types::MemoryType::Project)),
        2
    );
}

// ─── Cross-crate: rc-utils MemoryStore → rc-tools BM25 search ────────────

#[test]
fn claude_tools_bm25_search_with_memory_data() {
    let mut engine = claude_tools::discover_skills::Bm25SkillSearchEngine::new();
    engine.add_skill(claude_tools::discover_skills::SkillMetadata {
        name: "memory-manager".to_owned(),
        description: "Manage persistent memory entries".to_owned(),
        triggers: vec!["memory".to_owned(), "save".to_owned(), "load".to_owned()],
        path: PathBuf::from("/skills/memory-manager"),
    });

    let results = engine.search("memory", 5);
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].skill.name, "memory-manager");
}

// ─── StoredMemory serialization ───────────────────────────────────────────

#[test]
fn stored_memory_serialization_round_trip() {
    let temp = ok(tempfile::tempdir());
    let store = claude_utils::memory_store::MemoryStore::new(temp.path());
    let stored = ok(store.save_memory(
        "serialize-test",
        "serializable content",
        claude_utils::memory_types::MemoryType::Project,
        vec!["serde".to_owned()],
    ));

    // The entry itself should round-trip through JSON
    let json = serde_json::to_string_pretty(&stored.entry).expect("serialize entry");
    let decoded: claude_utils::memory_types::MemoryEntry =
        serde_json::from_str(&json).expect("deserialize entry");
    assert_eq!(decoded.content, "serializable content");
    assert!(decoded.has_tag("serde"));
}

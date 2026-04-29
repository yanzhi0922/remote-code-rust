use std::collections::HashSet;

use crate::store::{MetadataStore, Result};
use crate::types::{FileMetadataEntry, RecordSource, RecordState, TaskMetadata};

/// Core file context tracker.
///
/// Tracks file operations that may result in stale context. If a user modifies
/// a file outside of Roo, the context may become stale and need to be updated.
/// This tracker informs Roo that a change has occurred and tells Roo to reload
/// the file before making changes to it.
///
/// Corresponds to `FileContextTracker` in `FileContextTracker.ts`.
pub struct FileContextTracker<S: MetadataStore> {
    task_id: String,
    store: S,
    recently_modified_files: HashSet<String>,
    recently_edited_by_roo: HashSet<String>,
    checkpoint_possible_files: HashSet<String>,
}

impl<S: MetadataStore> FileContextTracker<S> {
    /// Create a new tracker for the given task ID, backed by the given store.
    pub fn new(task_id: impl Into<String>, store: S) -> Self {
        Self {
            task_id: task_id.into(),
            store,
            recently_modified_files: HashSet::new(),
            recently_edited_by_roo: HashSet::new(),
            checkpoint_possible_files: HashSet::new(),
        }
    }

    /// Main entry point: track a file context operation (immutable).
    ///
    /// Persists the operation to the metadata store but does not update
    /// in-memory tracking sets. For the mutable variant that also updates
    /// in-memory sets, use [`track_file_context_mut`](Self::track_file_context_mut).
    pub fn track_file_context(&self, file_path: &str, source: RecordSource) -> Result<()> {
        self.add_file_to_context(file_path, source)
    }

    /// Mutable version of `track_file_context` that also updates in-memory sets.
    pub fn track_file_context_mut(&mut self, file_path: &str, source: RecordSource) -> Result<()> {
        self.add_file_to_context_mut(file_path, source)
    }

    /// Core business logic: add a file entry to the context tracker (immutable).
    ///
    /// 1. Mark existing active entries for the same path as stale.
    /// 2. Preserve the latest date values from prior entries.
    /// 3. Set new date fields based on the source type.
    /// 4. Persist the updated metadata.
    pub fn add_file_to_context(&self, file_path: &str, source: RecordSource) -> Result<()> {
        let mut metadata = self.store.load(&self.task_id)?;
        let now = chrono::Utc::now().timestamp_millis();

        // Mark existing active entries for this file as stale
        for entry in metadata.files_in_context.iter_mut() {
            if entry.path == file_path && entry.record_state == RecordState::Active {
                entry.record_state = RecordState::Stale;
            }
        }

        // Get the latest date values from prior entries for this file
        let roo_read_date = get_latest_date_for_field(&metadata, file_path, |e| e.roo_read_date);
        let roo_edit_date = get_latest_date_for_field(&metadata, file_path, |e| e.roo_edit_date);
        let user_edit_date = get_latest_date_for_field(&metadata, file_path, |e| e.user_edit_date);

        let mut new_entry = FileMetadataEntry {
            path: file_path.to_string(),
            record_state: RecordState::Active,
            record_source: source,
            roo_read_date,
            roo_edit_date,
            user_edit_date,
        };

        match source {
            RecordSource::UserEdited => {
                new_entry.user_edit_date = Some(now);
            }
            RecordSource::RooEdited => {
                new_entry.roo_read_date = Some(now);
                new_entry.roo_edit_date = Some(now);
            }
            RecordSource::ReadTool | RecordSource::FileMentioned => {
                new_entry.roo_read_date = Some(now);
            }
        }

        metadata.files_in_context.push(new_entry);
        self.store.save(&self.task_id, &metadata)?;
        Ok(())
    }

    /// Mutable version of `add_file_to_context` that also updates the
    /// in-memory tracking sets (recently modified, checkpoint possible, etc.).
    pub fn add_file_to_context_mut(&mut self, file_path: &str, source: RecordSource) -> Result<()> {
        let now = chrono::Utc::now().timestamp_millis();

        let mut metadata = self.store.load(&self.task_id)?;

        // Mark existing active entries for this file as stale
        for entry in metadata.files_in_context.iter_mut() {
            if entry.path == file_path && entry.record_state == RecordState::Active {
                entry.record_state = RecordState::Stale;
            }
        }

        let roo_read_date = get_latest_date_for_field(&metadata, file_path, |e| e.roo_read_date);
        let roo_edit_date = get_latest_date_for_field(&metadata, file_path, |e| e.roo_edit_date);
        let user_edit_date = get_latest_date_for_field(&metadata, file_path, |e| e.user_edit_date);

        let mut new_entry = FileMetadataEntry {
            path: file_path.to_string(),
            record_state: RecordState::Active,
            record_source: source,
            roo_read_date,
            roo_edit_date,
            user_edit_date,
        };

        match source {
            RecordSource::UserEdited => {
                new_entry.user_edit_date = Some(now);
                self.recently_modified_files.insert(file_path.to_string());
            }
            RecordSource::RooEdited => {
                new_entry.roo_read_date = Some(now);
                new_entry.roo_edit_date = Some(now);
                self.checkpoint_possible_files.insert(file_path.to_string());
                self.recently_edited_by_roo.insert(file_path.to_string());
            }
            RecordSource::ReadTool | RecordSource::FileMentioned => {
                new_entry.roo_read_date = Some(now);
            }
        }

        metadata.files_in_context.push(new_entry);
        self.store.save(&self.task_id, &metadata)?;
        Ok(())
    }

    /// Returns (and clears) the set of recently modified files.
    pub fn get_and_clear_recently_modified_files(&mut self) -> Vec<String> {
        self.recently_modified_files.drain().collect()
    }

    /// Returns (and clears) the set of checkpoint-possible files.
    pub fn get_and_clear_checkpoint_possible_files(&mut self) -> Vec<String> {
        self.checkpoint_possible_files.drain().collect()
    }

    /// Mark a file as edited by Roo to prevent false positives.
    pub fn mark_file_as_edited_by_roo(&mut self, file_path: &str) {
        self.recently_edited_by_roo.insert(file_path.to_string());
    }

    /// Check if a file was recently edited by Roo.
    pub fn is_edited_by_roo(&self, file_path: &str) -> bool {
        self.recently_edited_by_roo.contains(file_path)
    }

    /// Convenience method: track a file edit operation by Roo.
    ///
    /// Records that Roo has edited the given file at the specified timestamp.
    /// This is equivalent to calling `add_file_to_context_mut` with
    /// `RecordSource::RooEdited`, but uses an explicit timestamp instead of
    /// the current time.
    pub fn track_file_edit(&mut self, path: &str, timestamp: i64) -> Result<()> {
        let mut metadata = self.store.load(&self.task_id)?;

        // Mark existing active entries for this file as stale
        for entry in metadata.files_in_context.iter_mut() {
            if entry.path == path && entry.record_state == RecordState::Active {
                entry.record_state = RecordState::Stale;
            }
        }

        let _roo_read_date = get_latest_date_for_field(&metadata, path, |e| e.roo_read_date);
        let _roo_edit_date = get_latest_date_for_field(&metadata, path, |e| e.roo_edit_date);
        let user_edit_date = get_latest_date_for_field(&metadata, path, |e| e.user_edit_date);

        let new_entry = FileMetadataEntry {
            path: path.to_string(),
            record_state: RecordState::Active,
            record_source: RecordSource::RooEdited,
            roo_read_date: Some(timestamp),
            roo_edit_date: Some(timestamp),
            user_edit_date,
        };

        metadata.files_in_context.push(new_entry);
        self.store.save(&self.task_id, &metadata)?;

        self.checkpoint_possible_files.insert(path.to_string());
        self.recently_edited_by_roo.insert(path.to_string());

        Ok(())
    }

    /// Convenience method: track a file read operation by Roo.
    ///
    /// Records that Roo has read the given file at the specified timestamp.
    /// This is equivalent to calling `add_file_to_context_mut` with
    /// `RecordSource::ReadTool`, but uses an explicit timestamp instead of
    /// the current time.
    pub fn track_file_read(&mut self, path: &str, timestamp: i64) -> Result<()> {
        let mut metadata = self.store.load(&self.task_id)?;

        // Mark existing active entries for this file as stale
        for entry in metadata.files_in_context.iter_mut() {
            if entry.path == path && entry.record_state == RecordState::Active {
                entry.record_state = RecordState::Stale;
            }
        }

        let _roo_read_date = get_latest_date_for_field(&metadata, path, |e| e.roo_read_date);
        let roo_edit_date = get_latest_date_for_field(&metadata, path, |e| e.roo_edit_date);
        let user_edit_date = get_latest_date_for_field(&metadata, path, |e| e.user_edit_date);

        let new_entry = FileMetadataEntry {
            path: path.to_string(),
            record_state: RecordState::Active,
            record_source: RecordSource::ReadTool,
            roo_read_date: Some(timestamp),
            roo_edit_date,
            user_edit_date,
        };

        metadata.files_in_context.push(new_entry);
        self.store.save(&self.task_id, &metadata)?;

        Ok(())
    }

    /// Get a list of unique file paths that Roo has read during this task.
    ///
    /// Files are sorted by most recently read first. If `since_timestamp` is
    /// provided, only files read after that time are included.
    pub fn get_files_read_by_roo(&self, since_timestamp: Option<i64>) -> Result<Vec<String>> {
        let metadata = self.store.load(&self.task_id)?;

        let mut read_entries: Vec<&FileMetadataEntry> = metadata
            .files_in_context
            .iter()
            .filter(|entry| {
                // Only include files that were read by Roo (not user edits)
                let is_read_by_roo = matches!(
                    entry.record_source,
                    RecordSource::ReadTool | RecordSource::FileMentioned
                );
                if !is_read_by_roo {
                    return false;
                }

                // If since_timestamp is provided, only include files read after that time
                if let (Some(ts), Some(read_date)) = (since_timestamp, entry.roo_read_date) {
                    return read_date >= ts;
                }

                true
            })
            .collect();

        // Sort by roo_read_date descending (most recent first)
        // Entries without a date go to the end
        read_entries.sort_by(|a, b| {
            let date_a = a.roo_read_date.unwrap_or(0);
            let date_b = b.roo_read_date.unwrap_or(0);
            date_b.cmp(&date_a)
        });

        // Deduplicate while preserving order (first occurrence = most recent read)
        let mut seen = HashSet::new();
        let mut unique_paths = Vec::new();
        for entry in &read_entries {
            if !seen.contains(&entry.path) {
                seen.insert(entry.path.clone());
                unique_paths.push(entry.path.clone());
            }
        }

        Ok(unique_paths)
    }

    /// Proxy to the underlying store: load task metadata.
    pub fn get_task_metadata(&self, task_id: &str) -> Result<TaskMetadata> {
        self.store.load(task_id)
    }

    /// Proxy to the underlying store: save task metadata.
    pub fn save_task_metadata(&self, task_id: &str, metadata: &TaskMetadata) -> Result<()> {
        self.store.save(task_id, metadata)
    }

    /// Get the task ID this tracker is associated with.
    pub fn task_id(&self) -> &str {
        &self.task_id
    }
}

/// Helper: get the latest (maximum) date value for a given field across all
/// entries matching the specified path.
///
/// Corresponds to `getLatestDateForField` in `FileContextTracker.ts`.
fn get_latest_date_for_field(
    metadata: &TaskMetadata,
    path: &str,
    field_accessor: impl Fn(&FileMetadataEntry) -> Option<i64>,
) -> Option<i64> {
    metadata
        .files_in_context
        .iter()
        .filter(|entry| entry.path == path)
        .filter_map(|entry| field_accessor(entry))
        .max()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::InMemoryMetadataStore;
    use crate::types::{RecordSource, RecordState};

    /// Helper: create a tracker backed by an in-memory store.
    fn make_tracker() -> FileContextTracker<InMemoryMetadataStore> {
        FileContextTracker::new("test-task", InMemoryMetadataStore::new())
    }

    // --- Basic tracking tests ---

    #[test]
    fn test_track_read_tool() {
        let mut tracker = make_tracker();
        tracker
            .add_file_to_context_mut("src/main.rs", RecordSource::ReadTool)
            .unwrap();

        let metadata = tracker.get_task_metadata("test-task").unwrap();
        assert_eq!(metadata.files_in_context.len(), 1);

        let entry = &metadata.files_in_context[0];
        assert_eq!(entry.path, "src/main.rs");
        assert_eq!(entry.record_state, RecordState::Active);
        assert_eq!(entry.record_source, RecordSource::ReadTool);
        assert!(entry.roo_read_date.is_some());
        assert!(entry.roo_edit_date.is_none());
        assert!(entry.user_edit_date.is_none());
    }

    #[test]
    fn test_track_roo_edited() {
        let mut tracker = make_tracker();
        tracker
            .add_file_to_context_mut("src/lib.rs", RecordSource::RooEdited)
            .unwrap();

        let metadata = tracker.get_task_metadata("test-task").unwrap();
        assert_eq!(metadata.files_in_context.len(), 1);

        let entry = &metadata.files_in_context[0];
        assert_eq!(entry.path, "src/lib.rs");
        assert_eq!(entry.record_state, RecordState::Active);
        assert_eq!(entry.record_source, RecordSource::RooEdited);
        assert!(entry.roo_read_date.is_some());
        assert!(entry.roo_edit_date.is_some());
        assert!(entry.user_edit_date.is_none());

        // roo_read_date and roo_edit_date should both be set and equal
        assert_eq!(entry.roo_read_date, entry.roo_edit_date);
    }

    #[test]
    fn test_track_user_edited() {
        let mut tracker = make_tracker();
        tracker
            .add_file_to_context_mut("config.toml", RecordSource::UserEdited)
            .unwrap();

        let metadata = tracker.get_task_metadata("test-task").unwrap();
        assert_eq!(metadata.files_in_context.len(), 1);

        let entry = &metadata.files_in_context[0];
        assert_eq!(entry.path, "config.toml");
        assert_eq!(entry.record_state, RecordState::Active);
        assert_eq!(entry.record_source, RecordSource::UserEdited);
        assert!(entry.roo_read_date.is_none());
        assert!(entry.roo_edit_date.is_none());
        assert!(entry.user_edit_date.is_some());
    }

    #[test]
    fn test_track_file_mentioned() {
        let mut tracker = make_tracker();
        tracker
            .add_file_to_context_mut("README.md", RecordSource::FileMentioned)
            .unwrap();

        let metadata = tracker.get_task_metadata("test-task").unwrap();
        assert_eq!(metadata.files_in_context.len(), 1);

        let entry = &metadata.files_in_context[0];
        assert_eq!(entry.path, "README.md");
        assert_eq!(entry.record_state, RecordState::Active);
        assert_eq!(entry.record_source, RecordSource::FileMentioned);
        assert!(entry.roo_read_date.is_some());
        assert!(entry.roo_edit_date.is_none());
        assert!(entry.user_edit_date.is_none());
    }

    // --- Stale marking tests ---

    #[test]
    fn test_stale_marking() {
        let mut tracker = make_tracker();
        tracker
            .add_file_to_context_mut("foo.rs", RecordSource::ReadTool)
            .unwrap();

        // First entry should be active
        let metadata = tracker.get_task_metadata("test-task").unwrap();
        assert_eq!(metadata.files_in_context[0].record_state, RecordState::Active);

        // Track the same file again - previous entry should become stale
        tracker
            .add_file_to_context_mut("foo.rs", RecordSource::RooEdited)
            .unwrap();

        let metadata = tracker.get_task_metadata("test-task").unwrap();
        assert_eq!(metadata.files_in_context.len(), 2);
        assert_eq!(metadata.files_in_context[0].record_state, RecordState::Stale);
        assert_eq!(metadata.files_in_context[1].record_state, RecordState::Active);
    }

    #[test]
    fn test_stale_only_affects_same_path() {
        let mut tracker = make_tracker();
        tracker
            .add_file_to_context_mut("a.rs", RecordSource::ReadTool)
            .unwrap();
        tracker
            .add_file_to_context_mut("b.rs", RecordSource::ReadTool)
            .unwrap();

        // Track a.rs again - only a.rs should become stale, b.rs stays active
        tracker
            .add_file_to_context_mut("a.rs", RecordSource::RooEdited)
            .unwrap();

        let metadata = tracker.get_task_metadata("test-task").unwrap();
        // a.rs first entry -> stale
        assert_eq!(metadata.files_in_context[0].path, "a.rs");
        assert_eq!(metadata.files_in_context[0].record_state, RecordState::Stale);
        // b.rs -> still active
        assert_eq!(metadata.files_in_context[1].path, "b.rs");
        assert_eq!(metadata.files_in_context[1].record_state, RecordState::Active);
        // a.rs second entry -> active
        assert_eq!(metadata.files_in_context[2].path, "a.rs");
        assert_eq!(metadata.files_in_context[2].record_state, RecordState::Active);
    }

    // --- Latest date preservation tests ---

    #[test]
    fn test_latest_date_preservation() {
        let mut tracker = make_tracker();

        // First: read_tool sets roo_read_date
        tracker
            .add_file_to_context_mut("main.rs", RecordSource::ReadTool)
            .unwrap();

        let first_read_date = tracker
            .get_task_metadata("test-task")
            .unwrap()
            .files_in_context[0]
            .roo_read_date
            .unwrap();

        // Second: user_edited should preserve roo_read_date from the first entry
        tracker
            .add_file_to_context_mut("main.rs", RecordSource::UserEdited)
            .unwrap();

        let metadata = tracker.get_task_metadata("test-task").unwrap();
        let second_entry = &metadata.files_in_context[1];
        assert_eq!(second_entry.roo_read_date, Some(first_read_date));
        assert!(second_entry.user_edit_date.is_some());
    }

    #[test]
    fn test_latest_date_preserved_across_operations() {
        let mut tracker = make_tracker();

        // Read -> sets roo_read_date
        tracker
            .add_file_to_context_mut("x.rs", RecordSource::ReadTool)
            .unwrap();

        // Roo edit -> sets roo_read_date + roo_edit_date
        tracker
            .add_file_to_context_mut("x.rs", RecordSource::RooEdited)
            .unwrap();

        let roo_edit_date = tracker
            .get_task_metadata("test-task")
            .unwrap()
            .files_in_context[1]
            .roo_edit_date
            .unwrap();

        // User edit -> preserves roo_edit_date
        tracker
            .add_file_to_context_mut("x.rs", RecordSource::UserEdited)
            .unwrap();

        let metadata = tracker.get_task_metadata("test-task").unwrap();
        let third = &metadata.files_in_context[2];
        assert_eq!(third.roo_edit_date, Some(roo_edit_date));
        assert!(third.user_edit_date.is_some());
    }

    // --- Recently modified / checkpoint possible tests ---

    #[test]
    fn test_get_and_clear_recently_modified() {
        let mut tracker = make_tracker();
        tracker
            .add_file_to_context_mut("a.rs", RecordSource::UserEdited)
            .unwrap();
        tracker
            .add_file_to_context_mut("b.rs", RecordSource::UserEdited)
            .unwrap();

        let files = tracker.get_and_clear_recently_modified_files();
        assert_eq!(files.len(), 2);
        assert!(files.contains(&"a.rs".to_string()));
        assert!(files.contains(&"b.rs".to_string()));

        // After clearing, should be empty
        let files2 = tracker.get_and_clear_recently_modified_files();
        assert!(files2.is_empty());
    }

    #[test]
    fn test_get_and_clear_checkpoint_possible() {
        let mut tracker = make_tracker();
        tracker
            .add_file_to_context_mut("a.rs", RecordSource::RooEdited)
            .unwrap();
        tracker
            .add_file_to_context_mut("b.rs", RecordSource::RooEdited)
            .unwrap();

        let files = tracker.get_and_clear_checkpoint_possible_files();
        assert_eq!(files.len(), 2);
        assert!(files.contains(&"a.rs".to_string()));
        assert!(files.contains(&"b.rs".to_string()));

        // After clearing, should be empty
        let files2 = tracker.get_and_clear_checkpoint_possible_files();
        assert!(files2.is_empty());
    }

    #[test]
    fn test_user_edited_adds_to_recently_modified() {
        let mut tracker = make_tracker();
        tracker
            .add_file_to_context_mut("mod.rs", RecordSource::UserEdited)
            .unwrap();

        let files = tracker.get_and_clear_recently_modified_files();
        assert!(files.contains(&"mod.rs".to_string()));
    }

    #[test]
    fn test_roo_edited_sets_checkpoint_possible() {
        let mut tracker = make_tracker();
        tracker
            .add_file_to_context_mut("app.rs", RecordSource::RooEdited)
            .unwrap();

        let files = tracker.get_and_clear_checkpoint_possible_files();
        assert!(files.contains(&"app.rs".to_string()));
    }

    #[test]
    fn test_read_tool_does_not_add_to_recently_modified() {
        let mut tracker = make_tracker();
        tracker
            .add_file_to_context_mut("read.rs", RecordSource::ReadTool)
            .unwrap();

        let files = tracker.get_and_clear_recently_modified_files();
        assert!(files.is_empty());
    }

    #[test]
    fn test_read_tool_does_not_add_to_checkpoint_possible() {
        let mut tracker = make_tracker();
        tracker
            .add_file_to_context_mut("read.rs", RecordSource::ReadTool)
            .unwrap();

        let files = tracker.get_and_clear_checkpoint_possible_files();
        assert!(files.is_empty());
    }

    // --- Mark edited by Roo tests ---

    #[test]
    fn test_mark_file_as_edited_by_roo() {
        let mut tracker = make_tracker();
        assert!(!tracker.is_edited_by_roo("foo.rs"));

        tracker.mark_file_as_edited_by_roo("foo.rs");
        assert!(tracker.is_edited_by_roo("foo.rs"));
    }

    #[test]
    fn test_is_edited_by_roo_false_initially() {
        let tracker = make_tracker();
        assert!(!tracker.is_edited_by_roo("any.rs"));
    }

    #[test]
    fn test_roo_edited_automatically_marks_edited_by_roo() {
        let mut tracker = make_tracker();
        tracker
            .add_file_to_context_mut("auto.rs", RecordSource::RooEdited)
            .unwrap();
        assert!(tracker.is_edited_by_roo("auto.rs"));
    }

    // --- get_files_read_by_roo tests ---

    #[test]
    fn test_get_files_read_by_roo_basic() {
        let mut tracker = make_tracker();
        tracker
            .add_file_to_context_mut("read.rs", RecordSource::ReadTool)
            .unwrap();
        tracker
            .add_file_to_context_mut("mentioned.rs", RecordSource::FileMentioned)
            .unwrap();

        let files = tracker.get_files_read_by_roo(None).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files.contains(&"read.rs".to_string()));
        assert!(files.contains(&"mentioned.rs".to_string()));
    }

    #[test]
    fn test_get_files_read_by_roo_excludes_user_edited() {
        let mut tracker = make_tracker();
        tracker
            .add_file_to_context_mut("read.rs", RecordSource::ReadTool)
            .unwrap();
        tracker
            .add_file_to_context_mut("user.rs", RecordSource::UserEdited)
            .unwrap();

        let files = tracker.get_files_read_by_roo(None).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files.contains(&"read.rs".to_string()));
        assert!(!files.contains(&"user.rs".to_string()));
    }

    #[test]
    fn test_get_files_read_by_roo_excludes_roo_edited() {
        let mut tracker = make_tracker();
        tracker
            .add_file_to_context_mut("read.rs", RecordSource::ReadTool)
            .unwrap();
        tracker
            .add_file_to_context_mut("edited.rs", RecordSource::RooEdited)
            .unwrap();

        let files = tracker.get_files_read_by_roo(None).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files.contains(&"read.rs".to_string()));
        assert!(!files.contains(&"edited.rs".to_string()));
    }

    #[test]
    fn test_get_files_read_by_roo_with_timestamp_filter() {
        let tracker = make_tracker();

        // Manually set up metadata with known timestamps
        let metadata = TaskMetadata {
            files_in_context: vec![
                FileMetadataEntry {
                    path: "old.rs".to_string(),
                    record_state: RecordState::Active,
                    record_source: RecordSource::ReadTool,
                    roo_read_date: Some(1000),
                    roo_edit_date: None,
                    user_edit_date: None,
                },
                FileMetadataEntry {
                    path: "new.rs".to_string(),
                    record_state: RecordState::Active,
                    record_source: RecordSource::ReadTool,
                    roo_read_date: Some(2000),
                    roo_edit_date: None,
                    user_edit_date: None,
                },
            ],
        };
        tracker.save_task_metadata("test-task", &metadata).unwrap();

        // Filter: only files read at or after timestamp 1500
        let files = tracker.get_files_read_by_roo(Some(1500)).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files.contains(&"new.rs".to_string()));
    }

    #[test]
    fn test_get_files_read_by_roo_sorted_by_date() {
        let tracker = make_tracker();

        let metadata = TaskMetadata {
            files_in_context: vec![
                FileMetadataEntry {
                    path: "old.rs".to_string(),
                    record_state: RecordState::Active,
                    record_source: RecordSource::ReadTool,
                    roo_read_date: Some(100),
                    roo_edit_date: None,
                    user_edit_date: None,
                },
                FileMetadataEntry {
                    path: "new.rs".to_string(),
                    record_state: RecordState::Active,
                    record_source: RecordSource::ReadTool,
                    roo_read_date: Some(300),
                    roo_edit_date: None,
                    user_edit_date: None,
                },
                FileMetadataEntry {
                    path: "mid.rs".to_string(),
                    record_state: RecordState::Active,
                    record_source: RecordSource::FileMentioned,
                    roo_read_date: Some(200),
                    roo_edit_date: None,
                    user_edit_date: None,
                },
            ],
        };
        tracker.save_task_metadata("test-task", &metadata).unwrap();

        let files = tracker.get_files_read_by_roo(None).unwrap();
        assert_eq!(files.len(), 3);
        // Should be sorted by roo_read_date descending
        assert_eq!(files[0], "new.rs");
        assert_eq!(files[1], "mid.rs");
        assert_eq!(files[2], "old.rs");
    }

    #[test]
    fn test_get_files_read_by_roo_deduplication() {
        let tracker = make_tracker();

        let metadata = TaskMetadata {
            files_in_context: vec![
                FileMetadataEntry {
                    path: "dup.rs".to_string(),
                    record_state: RecordState::Stale,
                    record_source: RecordSource::ReadTool,
                    roo_read_date: Some(100),
                    roo_edit_date: None,
                    user_edit_date: None,
                },
                FileMetadataEntry {
                    path: "dup.rs".to_string(),
                    record_state: RecordState::Active,
                    record_source: RecordSource::ReadTool,
                    roo_read_date: Some(200),
                    roo_edit_date: None,
                    user_edit_date: None,
                },
            ],
        };
        tracker.save_task_metadata("test-task", &metadata).unwrap();

        let files = tracker.get_files_read_by_roo(None).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], "dup.rs");
    }

    #[test]
    fn test_get_files_read_by_roo_no_entries() {
        let tracker = make_tracker();
        let files = tracker.get_files_read_by_roo(None).unwrap();
        assert!(files.is_empty());
    }

    // --- Multiple files / sequential operations ---

    #[test]
    fn test_multiple_files_tracking() {
        let mut tracker = make_tracker();
        tracker
            .add_file_to_context_mut("a.rs", RecordSource::ReadTool)
            .unwrap();
        tracker
            .add_file_to_context_mut("b.rs", RecordSource::FileMentioned)
            .unwrap();
        tracker
            .add_file_to_context_mut("c.rs", RecordSource::RooEdited)
            .unwrap();

        let metadata = tracker.get_task_metadata("test-task").unwrap();
        assert_eq!(metadata.files_in_context.len(), 3);

        // All should be active (different paths)
        for entry in &metadata.files_in_context {
            assert_eq!(entry.record_state, RecordState::Active);
        }
    }

    #[test]
    fn test_sequential_operations() {
        let mut tracker = make_tracker();

        // Read -> edit -> user edit
        tracker
            .add_file_to_context_mut("flow.rs", RecordSource::ReadTool)
            .unwrap();
        tracker
            .add_file_to_context_mut("flow.rs", RecordSource::RooEdited)
            .unwrap();
        tracker
            .add_file_to_context_mut("flow.rs", RecordSource::UserEdited)
            .unwrap();

        let metadata = tracker.get_task_metadata("test-task").unwrap();
        assert_eq!(metadata.files_in_context.len(), 3);

        // First two should be stale, last should be active
        assert_eq!(metadata.files_in_context[0].record_state, RecordState::Stale);
        assert_eq!(metadata.files_in_context[1].record_state, RecordState::Stale);
        assert_eq!(metadata.files_in_context[2].record_state, RecordState::Active);

        // Last entry should have preserved dates from prior entries
        let last = &metadata.files_in_context[2];
        assert!(last.roo_read_date.is_some()); // preserved from read_tool/roo_edited
        assert!(last.roo_edit_date.is_some()); // preserved from roo_edited
        assert!(last.user_edit_date.is_some()); // set by user_edited
    }

    #[test]
    fn test_track_same_file_multiple_times() {
        let mut tracker = make_tracker();
        for _ in 0..5 {
            tracker
                .add_file_to_context_mut("repeat.rs", RecordSource::ReadTool)
                .unwrap();
        }

        let metadata = tracker.get_task_metadata("test-task").unwrap();
        assert_eq!(metadata.files_in_context.len(), 5);

        // Only the last should be active
        let active_count = metadata
            .files_in_context
            .iter()
            .filter(|e| e.record_state == RecordState::Active)
            .count();
        assert_eq!(active_count, 1);

        let stale_count = metadata
            .files_in_context
            .iter()
            .filter(|e| e.record_state == RecordState::Stale)
            .count();
        assert_eq!(stale_count, 4);
    }

    #[test]
    fn test_multiple_source_types_for_same_file() {
        let mut tracker = make_tracker();

        tracker
            .add_file_to_context_mut("multi.rs", RecordSource::ReadTool)
            .unwrap();
        tracker
            .add_file_to_context_mut("multi.rs", RecordSource::FileMentioned)
            .unwrap();
        tracker
            .add_file_to_context_mut("multi.rs", RecordSource::RooEdited)
            .unwrap();
        tracker
            .add_file_to_context_mut("multi.rs", RecordSource::UserEdited)
            .unwrap();

        let metadata = tracker.get_task_metadata("test-task").unwrap();
        assert_eq!(metadata.files_in_context.len(), 4);

        // Last entry (UserEdited) should be active
        let last = metadata.files_in_context.last().unwrap();
        assert_eq!(last.record_state, RecordState::Active);
        assert_eq!(last.record_source, RecordSource::UserEdited);

        // All prior entries should be stale
        for entry in &metadata.files_in_context[..3] {
            assert_eq!(entry.record_state, RecordState::Stale);
        }
    }

    // --- task_id accessor ---

    #[test]
    fn test_task_id_accessor() {
        let tracker = FileContextTracker::new("my-task-123", InMemoryMetadataStore::new());
        assert_eq!(tracker.task_id(), "my-task-123");
    }

    // --- track_file_context (non-mut) test ---

    #[test]
    fn test_track_file_context_immutable() {
        let tracker = make_tracker();
        tracker
            .track_file_context("immutable.rs", RecordSource::ReadTool)
            .unwrap();

        let metadata = tracker.get_task_metadata("test-task").unwrap();
        assert_eq!(metadata.files_in_context.len(), 1);
        assert_eq!(metadata.files_in_context[0].path, "immutable.rs");
    }

    // --- track_file_context_mut test ---

    #[test]
    fn test_track_file_context_mut_updates_sets() {
        let mut tracker = make_tracker();
        tracker
            .track_file_context_mut("mut.rs", RecordSource::RooEdited)
            .unwrap();

        assert!(tracker.is_edited_by_roo("mut.rs"));

        let checkpoint = tracker.get_and_clear_checkpoint_possible_files();
        assert!(checkpoint.contains(&"mut.rs".to_string()));
    }

    // --- get_files_read_by_roo with no date entries ---

    #[test]
    fn test_get_files_read_by_roo_entries_without_dates() {
        let tracker = make_tracker();

        let metadata = TaskMetadata {
            files_in_context: vec![FileMetadataEntry {
                path: "nodate.rs".to_string(),
                record_state: RecordState::Active,
                record_source: RecordSource::ReadTool,
                roo_read_date: None,
                roo_edit_date: None,
                user_edit_date: None,
            }],
        };
        tracker.save_task_metadata("test-task", &metadata).unwrap();

        let files = tracker.get_files_read_by_roo(None).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0], "nodate.rs");
    }

    // --- save/get task metadata proxy ---

    #[test]
    fn test_save_and_get_task_metadata() {
        let tracker = make_tracker();

        let custom = TaskMetadata {
            files_in_context: vec![FileMetadataEntry {
                path: "custom.rs".to_string(),
                record_state: RecordState::Active,
                record_source: RecordSource::ReadTool,
                roo_read_date: Some(42),
                roo_edit_date: None,
                user_edit_date: None,
            }],
        };
        tracker.save_task_metadata("test-task", &custom).unwrap();

        let loaded = tracker.get_task_metadata("test-task").unwrap();
        assert_eq!(loaded, custom);
    }

    // --- get_files_read_by_roo with timestamp filtering edge cases ---

    #[test]
    fn test_get_files_read_by_roo_timestamp_exactly_matches() {
        let tracker = make_tracker();

        let metadata = TaskMetadata {
            files_in_context: vec![FileMetadataEntry {
                path: "exact.rs".to_string(),
                record_state: RecordState::Active,
                record_source: RecordSource::ReadTool,
                roo_read_date: Some(1000),
                roo_edit_date: None,
                user_edit_date: None,
            }],
        };
        tracker.save_task_metadata("test-task", &metadata).unwrap();

        // Timestamp exactly matches roo_read_date -> should be included (>=)
        let files = tracker.get_files_read_by_roo(Some(1000)).unwrap();
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn test_get_files_read_by_roo_timestamp_just_above() {
        let tracker = make_tracker();

        let metadata = TaskMetadata {
            files_in_context: vec![FileMetadataEntry {
                path: "above.rs".to_string(),
                record_state: RecordState::Active,
                record_source: RecordSource::ReadTool,
                roo_read_date: Some(1000),
                roo_edit_date: None,
                user_edit_date: None,
            }],
        };
        tracker.save_task_metadata("test-task", &metadata).unwrap();

        // Timestamp just above roo_read_date -> should be excluded
        let files = tracker.get_files_read_by_roo(Some(1001)).unwrap();
        assert!(files.is_empty());
    }

    // ---- track_file_edit / track_file_read tests ----

    #[test]
    fn test_track_file_edit_records_correctly() {
        let mut tracker = make_tracker();
        tracker.track_file_edit("src/main.rs", 5000).unwrap();

        let metadata = tracker.get_task_metadata("test-task").unwrap();
        let entry = metadata.files_in_context.last().unwrap();
        assert_eq!(entry.path, "src/main.rs");
        assert_eq!(entry.record_source, RecordSource::RooEdited);
        assert_eq!(entry.roo_edit_date, Some(5000));
        assert_eq!(entry.roo_read_date, Some(5000));
        assert!(tracker.is_edited_by_roo("src/main.rs"));
    }

    #[test]
    fn test_track_file_read_records_correctly() {
        let mut tracker = make_tracker();
        tracker.track_file_read("src/lib.rs", 3000).unwrap();

        let metadata = tracker.get_task_metadata("test-task").unwrap();
        let entry = metadata.files_in_context.last().unwrap();
        assert_eq!(entry.path, "src/lib.rs");
        assert_eq!(entry.record_source, RecordSource::ReadTool);
        assert_eq!(entry.roo_read_date, Some(3000));
        assert_eq!(entry.roo_edit_date, None);
    }

    #[test]
    fn test_track_file_edit_then_read_preserves_edit_date() {
        let mut tracker = make_tracker();
        tracker.track_file_edit("src/app.rs", 1000).unwrap();
        tracker.track_file_read("src/app.rs", 2000).unwrap();

        let metadata = tracker.get_task_metadata("test-task").unwrap();
        // The last entry should be the read
        let read_entry = metadata.files_in_context.iter().rev().find(|e| e.record_state == RecordState::Active).unwrap();
        assert_eq!(read_entry.record_source, RecordSource::ReadTool);
        assert_eq!(read_entry.roo_read_date, Some(2000));
        // roo_edit_date should be preserved from the edit
        assert_eq!(read_entry.roo_edit_date, Some(1000));
    }
}

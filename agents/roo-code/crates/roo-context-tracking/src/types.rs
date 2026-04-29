use serde::{Deserialize, Serialize};

/// Source of a file context record.
///
/// Corresponds to `RecordSource` in `FileContextTrackerTypes.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordSource {
    ReadTool,
    UserEdited,
    RooEdited,
    FileMentioned,
}

/// State of a file context record.
///
/// Corresponds to `record_state` in `FileContextTrackerTypes.ts`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecordState {
    Active,
    Stale,
}

/// A single file metadata entry tracking when and how a file entered context.
///
/// Corresponds to `FileMetadataEntry` in `FileContextTrackerTypes.ts`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileMetadataEntry {
    pub path: String,
    pub record_state: RecordState,
    pub record_source: RecordSource,
    pub roo_read_date: Option<i64>,
    pub roo_edit_date: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_edit_date: Option<i64>,
}

/// Task-level metadata containing all tracked file entries.
///
/// Corresponds to `TaskMetadata` in `FileContextTrackerTypes.ts`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TaskMetadata {
    pub files_in_context: Vec<FileMetadataEntry>,
}

impl Default for TaskMetadata {
    fn default() -> Self {
        Self {
            files_in_context: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_source_serde_all_variants() {
        let variants = [
            RecordSource::ReadTool,
            RecordSource::UserEdited,
            RecordSource::RooEdited,
            RecordSource::FileMentioned,
        ];
        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let deserialized: RecordSource = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, deserialized);
        }
    }

    #[test]
    fn test_record_source_serde_values() {
        assert_eq!(
            "\"read_tool\"",
            serde_json::to_string(&RecordSource::ReadTool).unwrap()
        );
        assert_eq!(
            "\"user_edited\"",
            serde_json::to_string(&RecordSource::UserEdited).unwrap()
        );
        assert_eq!(
            "\"roo_edited\"",
            serde_json::to_string(&RecordSource::RooEdited).unwrap()
        );
        assert_eq!(
            "\"file_mentioned\"",
            serde_json::to_string(&RecordSource::FileMentioned).unwrap()
        );
    }

    #[test]
    fn test_record_state_serde_all_variants() {
        let variants = [RecordState::Active, RecordState::Stale];
        for variant in variants {
            let json = serde_json::to_string(&variant).unwrap();
            let deserialized: RecordState = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, deserialized);
        }
    }

    #[test]
    fn test_record_state_serde_values() {
        assert_eq!(
            "\"active\"",
            serde_json::to_string(&RecordState::Active).unwrap()
        );
        assert_eq!(
            "\"stale\"",
            serde_json::to_string(&RecordState::Stale).unwrap()
        );
    }

    #[test]
    fn test_file_metadata_entry_serde_roundtrip() {
        let entry = FileMetadataEntry {
            path: "src/main.rs".to_string(),
            record_state: RecordState::Active,
            record_source: RecordSource::ReadTool,
            roo_read_date: Some(1234567890),
            roo_edit_date: None,
            user_edit_date: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: FileMetadataEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, deserialized);
    }

    #[test]
    fn test_file_metadata_entry_with_all_dates() {
        let entry = FileMetadataEntry {
            path: "lib.rs".to_string(),
            record_state: RecordState::Active,
            record_source: RecordSource::RooEdited,
            roo_read_date: Some(100),
            roo_edit_date: Some(200),
            user_edit_date: Some(300),
        };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: FileMetadataEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, deserialized);
        assert!(json.contains("\"roo_read_date\":100"));
        assert!(json.contains("\"roo_edit_date\":200"));
        assert!(json.contains("\"user_edit_date\":300"));
    }

    #[test]
    fn test_file_metadata_entry_with_no_dates() {
        let entry = FileMetadataEntry {
            path: "test.rs".to_string(),
            record_state: RecordState::Stale,
            record_source: RecordSource::FileMentioned,
            roo_read_date: None,
            roo_edit_date: None,
            user_edit_date: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: FileMetadataEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(entry, deserialized);
    }

    #[test]
    fn test_file_metadata_entry_user_edit_date_omitted_when_none() {
        let entry = FileMetadataEntry {
            path: "foo.rs".to_string(),
            record_state: RecordState::Active,
            record_source: RecordSource::ReadTool,
            roo_read_date: Some(100),
            roo_edit_date: None,
            user_edit_date: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(!json.contains("user_edit_date"));
    }

    #[test]
    fn test_task_metadata_serde_roundtrip() {
        let metadata = TaskMetadata {
            files_in_context: vec![
                FileMetadataEntry {
                    path: "a.rs".to_string(),
                    record_state: RecordState::Active,
                    record_source: RecordSource::ReadTool,
                    roo_read_date: Some(1000),
                    roo_edit_date: None,
                    user_edit_date: None,
                },
                FileMetadataEntry {
                    path: "b.rs".to_string(),
                    record_state: RecordState::Stale,
                    record_source: RecordSource::UserEdited,
                    roo_read_date: Some(500),
                    roo_edit_date: Some(600),
                    user_edit_date: Some(700),
                },
            ],
        };
        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: TaskMetadata = serde_json::from_str(&json).unwrap();
        assert_eq!(metadata, deserialized);
    }

    #[test]
    fn test_task_metadata_empty() {
        let metadata = TaskMetadata {
            files_in_context: vec![],
        };
        let json = serde_json::to_string(&metadata).unwrap();
        let deserialized: TaskMetadata = serde_json::from_str(&json).unwrap();
        assert!(deserialized.files_in_context.is_empty());
        assert_eq!(metadata, deserialized);
    }

    #[test]
    fn test_task_metadata_default() {
        let metadata = TaskMetadata::default();
        assert!(metadata.files_in_context.is_empty());
    }

    #[test]
    fn test_parse_from_json_string() {
        let json = r#"{
            "path": "src/lib.rs",
            "record_state": "active",
            "record_source": "read_tool",
            "roo_read_date": 1234567890,
            "roo_edit_date": null
        }"#;
        let entry: FileMetadataEntry = serde_json::from_str(json).unwrap();
        assert_eq!(entry.path, "src/lib.rs");
        assert_eq!(entry.record_state, RecordState::Active);
        assert_eq!(entry.record_source, RecordSource::ReadTool);
        assert_eq!(entry.roo_read_date, Some(1234567890));
        assert_eq!(entry.roo_edit_date, None);
        assert_eq!(entry.user_edit_date, None);
    }
}

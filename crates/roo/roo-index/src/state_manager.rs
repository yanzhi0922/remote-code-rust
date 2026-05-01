//! State manager for code indexing.
//!
//! Corresponds to `state-manager.ts` in the TypeScript source.
//!
//! Manages the state machine for the code indexing process,
//! including progress tracking and state transitions.

use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Current state of the code index.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum IndexingState {
    Standby,
    Indexing,
    Indexed,
    Error,
    Stopping,
}

impl std::fmt::Display for IndexingState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Standby => write!(f, "Standby"),
            Self::Indexing => write!(f, "Indexing"),
            Self::Indexed => write!(f, "Indexed"),
            Self::Error => write!(f, "Error"),
            Self::Stopping => write!(f, "Stopping"),
        }
    }
}

/// Current status of the indexing process.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct IndexingStatus {
    pub system_status: IndexingState,
    pub message: String,
    pub processed_items: usize,
    pub total_items: usize,
    pub current_item_unit: String,
}

impl Default for IndexingStatus {
    fn default() -> Self {
        Self {
            system_status: IndexingState::Standby,
            message: "Ready.".to_string(),
            processed_items: 0,
            total_items: 0,
            current_item_unit: "blocks".to_string(),
        }
    }
}

/// Callback type for progress updates.
pub type ProgressCallback = Box<dyn Fn(&IndexingStatus) + Send + Sync>;

/// Manages the state of the code indexing process.
///
/// Corresponds to `CodeIndexStateManager` in `state-manager.ts`.
pub struct CodeIndexStateManager {
    status: Arc<std::sync::Mutex<IndexingStatus>>,
    on_progress: Option<ProgressCallback>,
}

impl CodeIndexStateManager {
    /// Creates a new state manager.
    pub fn new() -> Self {
        Self {
            status: Arc::new(std::sync::Mutex::new(IndexingStatus::default())),
            on_progress: None,
        }
    }

    /// Creates a new state manager with a progress callback.
    pub fn with_callback(callback: ProgressCallback) -> Self {
        let mut manager = Self::new();
        manager.on_progress = Some(callback);
        manager
    }

    /// Returns the current indexing state.
    pub fn state(&self) -> IndexingState {
        self.status.lock().unwrap().system_status.clone()
    }

    /// Returns the current status.
    pub fn get_current_status(&self) -> IndexingStatus {
        self.status.lock().unwrap().clone()
    }

    /// Sets the system state with an optional message.
    pub fn set_system_state(&self, new_state: IndexingState, message: Option<&str>) {
        let mut status = self.status.lock().unwrap();

        let state_changed = new_state != status.system_status
            || message.is_some() && message != Some(status.message.as_str());

        if state_changed {
            status.system_status = new_state.clone();

            if let Some(msg) = message {
                status.message = msg.to_string();
            }

            // Reset progress counters if moving to a non-indexing state
            if status.system_status != IndexingState::Indexing {
                status.processed_items = 0;
                status.total_items = 0;
                status.current_item_unit = "blocks".to_string();

                if message.is_none() {
                    match status.system_status {
                        IndexingState::Standby => status.message = "Ready.".to_string(),
                        IndexingState::Indexed => status.message = "Index up-to-date.".to_string(),
                        IndexingState::Error => status.message = "An error occurred.".to_string(),
                        _ => {}
                    }
                }
            }

            let current = status.clone();
            drop(status);
            self.fire_progress_update(&current);
        }
    }

    /// Reports block indexing progress.
    pub fn report_block_indexing_progress(&self, processed_items: usize, total_items: usize) {
        let mut status = self.status.lock().unwrap();

        // Don't override Stopping state with progress updates
        if status.system_status == IndexingState::Stopping {
            return;
        }

        let progress_changed = processed_items != status.processed_items || total_items != status.total_items;

        if progress_changed || status.system_status != IndexingState::Indexing {
            status.processed_items = processed_items;
            status.total_items = total_items;
            status.current_item_unit = "blocks".to_string();

            let message = format!(
                "Indexed {} / {} {} found",
                status.processed_items, status.total_items, status.current_item_unit
            );

            let old_status = status.system_status.clone();
            let old_message = status.message.clone();

            status.system_status = IndexingState::Indexing;
            status.message = message;

            let state_changed = old_status != status.system_status
                || old_message != status.message
                || progress_changed;

            let current = status.clone();
            drop(status);

            if state_changed {
                self.fire_progress_update(&current);
            }
        }
    }

    /// Reports file queue progress.
    pub fn report_file_queue_progress(
        &self,
        processed_files: usize,
        total_files: usize,
        current_file_basename: Option<&str>,
    ) {
        let mut status = self.status.lock().unwrap();

        // Don't override Stopping state with progress updates
        if status.system_status == IndexingState::Stopping {
            return;
        }

        let progress_changed = processed_files != status.processed_items || total_files != status.total_items;

        if progress_changed || status.system_status != IndexingState::Indexing {
            status.processed_items = processed_files;
            status.total_items = total_files;
            status.current_item_unit = "files".to_string();
            status.system_status = IndexingState::Indexing;

            let message = if total_files > 0 && processed_files < total_files {
                format!(
                    "Processing {} / {} files. Current: {}",
                    processed_files,
                    total_files,
                    current_file_basename.unwrap_or("...")
                )
            } else if total_files > 0 && processed_files == total_files {
                format!("Finished processing {} files from queue.", total_files)
            } else {
                "File queue processed.".to_string()
            };

            let old_message = status.message.clone();
            status.message = message;

            let state_changed = progress_changed || old_message != status.message;

            let current = status.clone();
            drop(status);

            if state_changed {
                self.fire_progress_update(&current);
            }
        }
    }

    /// Fires a progress update to the callback.
    fn fire_progress_update(&self, status: &IndexingStatus) {
        if let Some(ref callback) = self.on_progress {
            callback(status);
        }
    }
}

impl Default for CodeIndexStateManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn test_initial_state() {
        let sm = CodeIndexStateManager::new();
        assert_eq!(sm.state(), IndexingState::Standby);

        let status = sm.get_current_status();
        assert_eq!(status.message, "Ready.");
        assert_eq!(status.processed_items, 0);
        assert_eq!(status.total_items, 0);
    }

    #[test]
    fn test_set_system_state() {
        let sm = CodeIndexStateManager::new();
        sm.set_system_state(IndexingState::Indexing, Some("Starting..."));
        assert_eq!(sm.state(), IndexingState::Indexing);
        assert_eq!(sm.get_current_status().message, "Starting...");
    }

    #[test]
    fn test_set_system_state_default_messages() {
        let sm = CodeIndexStateManager::new();

        sm.set_system_state(IndexingState::Standby, None);
        assert_eq!(sm.get_current_status().message, "Ready.");

        sm.set_system_state(IndexingState::Indexed, None);
        assert_eq!(sm.get_current_status().message, "Index up-to-date.");

        sm.set_system_state(IndexingState::Error, None);
        assert_eq!(sm.get_current_status().message, "An error occurred.");
    }

    #[test]
    fn test_report_block_indexing_progress() {
        let sm = CodeIndexStateManager::new();
        sm.report_block_indexing_progress(10, 100);

        assert_eq!(sm.state(), IndexingState::Indexing);
        let status = sm.get_current_status();
        assert_eq!(status.processed_items, 10);
        assert_eq!(status.total_items, 100);
        assert_eq!(status.current_item_unit, "blocks");
    }

    #[test]
    fn test_report_file_queue_progress() {
        let sm = CodeIndexStateManager::new();
        sm.report_file_queue_progress(5, 10, Some("test.rs"));

        assert_eq!(sm.state(), IndexingState::Indexing);
        let status = sm.get_current_status();
        assert_eq!(status.processed_items, 5);
        assert_eq!(status.total_items, 10);
        assert_eq!(status.current_item_unit, "files");
        assert!(status.message.contains("test.rs"));
    }

    #[test]
    fn test_progress_callback() {
        let updates = Arc::new(Mutex::new(Vec::new()));
        let updates_clone = updates.clone();

        let sm = CodeIndexStateManager::with_callback(Box::new(move |status| {
            updates_clone.lock().unwrap().push(status.system_status.clone());
        }));

        sm.set_system_state(IndexingState::Indexing, Some("Starting"));
        sm.report_block_indexing_progress(50, 100);
        sm.set_system_state(IndexingState::Indexed, Some("Done"));

        let updates = updates.lock().unwrap();
        assert!(updates.contains(&IndexingState::Indexing));
        assert!(updates.contains(&IndexingState::Indexed));
    }

    #[test]
    fn test_stopping_state_ignores_progress() {
        let sm = CodeIndexStateManager::new();
        sm.set_system_state(IndexingState::Stopping, Some("Stopping..."));
        sm.report_block_indexing_progress(50, 100);

        // Should still be Stopping, not Indexing
        assert_eq!(sm.state(), IndexingState::Stopping);
    }

    #[test]
    fn test_state_display() {
        assert_eq!(format!("{}", IndexingState::Standby), "Standby");
        assert_eq!(format!("{}", IndexingState::Indexing), "Indexing");
        assert_eq!(format!("{}", IndexingState::Indexed), "Indexed");
        assert_eq!(format!("{}", IndexingState::Error), "Error");
        assert_eq!(format!("{}", IndexingState::Stopping), "Stopping");
    }
}

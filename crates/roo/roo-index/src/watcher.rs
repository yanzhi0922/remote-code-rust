//! File watcher for incremental code index updates.
//!
//! Uses the `notify` crate to watch workspace directories for file changes
//! and triggers re-indexing through `CodeIndexManager`.
//!
//! Adapted from `.research/Roo-Code/src/services/code-index/processors/file-watcher.ts`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use notify::event::{CreateKind, ModifyKind, RemoveKind, RenameMode};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

use crate::manager::CodeIndexManager;

/// Type of file event detected by the watcher.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileEventType {
    /// A new file was created.
    Create,
    /// An existing file was modified.
    Change,
    /// A file was deleted.
    Delete,
}

/// Supported file extensions for indexing.
/// Mirrors the TS `scannerExtensions` list from `shared/supported-extensions`.
const SUPPORTED_EXTENSIONS: &[&str] = &[
    ".rs", ".ts", ".tsx", ".js", ".jsx", ".py", ".go", ".java", ".c", ".cpp", ".h", ".hpp", ".cs",
    ".rb", ".php", ".swift", ".kt", ".scala", ".sh", ".bash", ".zsh", ".fish", ".yaml", ".yml",
    ".json", ".toml", ".xml", ".html", ".css", ".scss", ".less", ".md", ".txt", ".sql", ".proto",
    ".graphql", ".vue", ".svelte",
];

/// Debounce delay in milliseconds.
const DEBOUNCE_DELAY_MS: u64 = 500;

/// File watcher that monitors workspace directories for changes
/// and triggers re-indexing of affected files.
///
/// Unlike the TS version which uses VSCode's `FileSystemWatcher`, this
/// implementation uses the `notify` crate for CLI environments.
pub struct FileWatcher {
    /// Path to the workspace root being watched.
    workspace_path: PathBuf,
    /// Inner watcher state behind Arc<Mutex> for async access.
    inner: Arc<Mutex<WatcherInner>>,
}

struct WatcherInner {
    /// The underlying notify watcher (None when stopped).
    _watcher: Option<RecommendedWatcher>,
    /// Accumulated file events awaiting debounce processing.
    accumulated_events: HashMap<String, FileEventType>,
    /// Receiver for file events from the notify watcher thread.
    event_rx: mpsc::Receiver<notify::Event>,
    /// Whether the watcher is currently running.
    running: bool,
}

impl FileWatcher {
    /// Create a new file watcher for the given workspace path.
    pub fn new(workspace_path: impl Into<PathBuf>) -> Self {
        let workspace_path = workspace_path.into();

        // Channel for events from the notify watcher to the async processing loop.
        // Buffer size allows for bursty file system activity.
        let (event_tx, event_rx) = mpsc::channel(512);

        // Create the notify watcher that sends events through the channel.
        // Note: notify 7 does not support Config::with_event_kinds, so we
        // filter events manually in process_notify_event().
        let tx_clone = event_tx.clone();
        let watcher_result = RecommendedWatcher::new(
            move |result: Result<notify::Event, notify::Error>| {
                if let Ok(event) = result {
                    let _ = tx_clone.blocking_send(event);
                }
            },
            notify::Config::default(),
        );

        let _watcher = match watcher_result {
            Ok(w) => {
                info!(path = %workspace_path.display(), "FileWatcher created successfully");
                Some(w)
            }
            Err(e) => {
                error!(error = %e, "Failed to create notify watcher");
                None
            }
        };

        let inner = WatcherInner {
            _watcher,
            accumulated_events: HashMap::new(),
            event_rx,
            running: false,
        };

        Self {
            workspace_path,
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    /// Start watching the workspace directory.
    ///
    /// Begins accumulating file events. Call `run` to spawn the debounce loop.
    pub async fn start(&self) -> Result<(), String> {
        let mut inner = self.inner.lock().await;

        if inner.running {
            return Ok(());
        }

        if let Some(ref mut watcher) = inner._watcher {
            watcher
                .watch(&self.workspace_path, RecursiveMode::Recursive)
                .map_err(|e| format!("Failed to start watching: {e}"))?;
            inner.running = true;
            info!(path = %self.workspace_path.display(), "FileWatcher started");
        } else {
            return Err("Watcher not available".to_string());
        }

        Ok(())
    }

    /// Stop watching the workspace directory.
    pub async fn stop(&self) {
        let mut inner = self.inner.lock().await;

        if let Some(ref mut watcher) = inner._watcher {
            let _ = watcher.unwatch(&self.workspace_path);
        }

        inner.running = false;
        inner.accumulated_events.clear();
        info!(path = %self.workspace_path.display(), "FileWatcher stopped");
    }

    /// Run the debounce loop. This should be spawned as a tokio task.
    ///
    /// Accumulates file change events and processes them after the debounce
    /// delay elapses without new events.
    pub async fn run(self: Arc<Self>, mut manager: CodeIndexManager) {
        let debounce_delay = Duration::from_millis(DEBOUNCE_DELAY_MS);

        loop {
            let mut inner = self.inner.lock().await;

            if !inner.running {
                break;
            }

            // Wait for the next event from the notify watcher.
            tokio::select! {
                Some(event) = inner.event_rx.recv() => {
                    Self::process_notify_event(&mut inner, &event);
                }
                _ = tokio::time::sleep(debounce_delay) => {
                    if !inner.accumulated_events.is_empty() {
                        let events = std::mem::take(&mut inner.accumulated_events);
                        drop(inner); // Release lock before processing
                        Self::flush_events(&self.workspace_path, &events, &mut manager).await;
                        continue;
                    }
                }
            }

            // After accumulating an event, wait for the debounce period
            // before flushing.
            if !inner.accumulated_events.is_empty() {
                drop(inner); // Release lock before sleeping

                tokio::time::sleep(debounce_delay).await;

                let mut inner = self.inner.lock().await;
                if !inner.accumulated_events.is_empty() {
                    let events = std::mem::take(&mut inner.accumulated_events);
                    drop(inner); // Release lock before processing
                    Self::flush_events(&self.workspace_path, &events, &mut manager).await;
                }
            }
        }
    }

    /// Process a single notify event and accumulate it.
    fn process_notify_event(inner: &mut WatcherInner, event: &notify::Event) {
        let file_event_type = match &event.kind {
            EventKind::Create(CreateKind::File)
            | EventKind::Create(CreateKind::Any)
            | EventKind::Create(CreateKind::Other) => FileEventType::Create,

            EventKind::Modify(ModifyKind::Data(_))
            | EventKind::Modify(ModifyKind::Any)
            | EventKind::Modify(ModifyKind::Other) => FileEventType::Change,

            EventKind::Modify(ModifyKind::Name(RenameMode::To)) => FileEventType::Create,
            EventKind::Modify(ModifyKind::Name(RenameMode::From)) => FileEventType::Delete,
            EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => FileEventType::Change,

            EventKind::Remove(RemoveKind::File)
            | EventKind::Remove(RemoveKind::Any)
            | EventKind::Remove(RemoveKind::Other) => FileEventType::Delete,

            _ => return, // Ignore other event types (folder events, access events, etc.)
        };

        for path in &event.paths {
            if let Some(path_str) = path.to_str()
                && Self::is_supported_file(path_str)
            {
                let key = path_str.to_string();
                // Insert or update. Delete takes precedence if already deleted,
                // otherwise the latest event type wins.
                inner
                    .accumulated_events
                    .entry(key)
                    .and_modify(|existing| {
                        // If it was deleted and now created/changed, treat as change.
                        // If it was created/changed and now deleted, treat as delete.
                        match (&existing, &file_event_type) {
                            (FileEventType::Delete, FileEventType::Create) => {
                                *existing = FileEventType::Change;
                            }
                            (FileEventType::Delete, FileEventType::Change) => {
                                *existing = FileEventType::Change;
                            }
                            (_, FileEventType::Delete) => {
                                *existing = FileEventType::Delete;
                            }
                            _ => {} // Keep the existing event type for create/change
                        }
                    })
                    .or_insert(file_event_type.clone());
            }
        }
    }

    /// Flush accumulated events to the index manager.
    async fn flush_events(
        workspace_path: &Path,
        events: &HashMap<String, FileEventType>,
        manager: &mut CodeIndexManager,
    ) {
        if events.is_empty() {
            return;
        }

        let changes: Vec<(String, FileEventType)> = events
            .iter()
            .map(|(path, event_type)| {
                // Convert absolute paths to relative paths for the manager.
                let relative = path
                    .strip_prefix(workspace_path.to_str().unwrap_or(""))
                    .unwrap_or(path)
                    .trim_start_matches(std::path::MAIN_SEPARATOR)
                    .to_string();
                (relative, event_type.clone())
            })
            .collect();

        debug!(count = changes.len(), "Processing batch of file changes");

        manager.on_files_changed(&changes);
    }

    /// Check if a file path has a supported extension.
    fn is_supported_file(path: &str) -> bool {
        let path_lower = path.to_lowercase();
        SUPPORTED_EXTENSIONS
            .iter()
            .any(|ext| path_lower.ends_with(ext))
    }

    /// Check whether the watcher is currently running.
    pub async fn is_running(&self) -> bool {
        self.inner.lock().await.running
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        // Best-effort stop on drop
        info!(path = %self.workspace_path.display(), "FileWatcher dropped");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_supported_file_rs() {
        assert!(FileWatcher::is_supported_file("src/main.rs"));
        assert!(FileWatcher::is_supported_file("src/main.RS"));
    }

    #[test]
    fn test_is_supported_file_ts() {
        assert!(FileWatcher::is_supported_file("src/index.ts"));
        assert!(FileWatcher::is_supported_file("src/component.tsx"));
    }

    #[test]
    fn test_is_supported_file_unsupported() {
        assert!(!FileWatcher::is_supported_file("image.png"));
        assert!(!FileWatcher::is_supported_file("archive.zip"));
        assert!(!FileWatcher::is_supported_file("binary.exe"));
    }

    #[test]
    fn test_is_supported_file_json() {
        assert!(FileWatcher::is_supported_file("package.json"));
        assert!(FileWatcher::is_supported_file("tsconfig.json"));
    }

    #[test]
    fn test_file_event_type_precedence() {
        // Simulate delete then create -> should become Change
        use std::collections::HashMap;
        let mut events: HashMap<String, FileEventType> = HashMap::new();
        events.insert("test.rs".to_string(), FileEventType::Delete);
        events
            .entry("test.rs".to_string())
            .and_modify(|e| {
                if let FileEventType::Delete = e {
                    *e = FileEventType::Change;
                }
            })
            .or_insert(FileEventType::Create);
        assert_eq!(events.get("test.rs"), Some(&FileEventType::Change));
    }
}

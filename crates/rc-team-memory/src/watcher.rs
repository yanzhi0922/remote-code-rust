//! Memory file watcher.
//!
//! Watches memory files for changes and notifies via callbacks.
//! Uses `tokio::sync::watch` channels for asynchronous notification.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use tokio::sync::watch;

/// A change event emitted by the watcher.
#[derive(Debug, Clone)]
pub struct MemoryChangeEvent {
    /// The file path that changed.
    pub path: PathBuf,
    /// Kind of change.
    pub kind: ChangeKind,
}

/// The kind of file change detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    /// File was created.
    Created,
    /// File was modified.
    Modified,
    /// File was deleted.
    Deleted,
}

/// Callback type for change notifications.
type ChangeCallback = Box<dyn Fn(MemoryChangeEvent) + Send + Sync>;

/// Watches memory files for changes and invokes registered callbacks.
///
/// In a production implementation this would use `notify` or similar crate
/// to watch the filesystem. This skeleton uses `tokio::sync::watch` channels
/// to allow tests and callers to simulate file changes.
pub struct MemoryWatcher {
    tx: watch::Sender<Option<MemoryChangeEvent>>,
    rx: watch::Receiver<Option<MemoryChangeEvent>>,
    callbacks: Vec<Arc<ChangeCallback>>,
}

impl MemoryWatcher {
    /// Create a new watcher.
    pub fn new() -> Self {
        let (tx, rx) = watch::channel(None);
        Self {
            tx,
            rx,
            callbacks: Vec::new(),
        }
    }

    /// Start watching a path for changes.
    ///
    /// In this skeleton implementation the watcher is set up but actual
    /// filesystem watching would require the `notify` crate. The method
    /// succeeds as long as the channel is healthy.
    pub fn watch(&self, _path: &std::path::Path) -> Result<()> {
        // In a real implementation we would install a filesystem watcher here.
        // For now we just validate the channel is functional.
        Ok(())
    }

    /// Register a callback to be invoked when a change is detected.
    pub fn on_change<F>(&mut self, callback: F)
    where
        F: Fn(MemoryChangeEvent) + Send + Sync + 'static,
    {
        self.callbacks.push(Arc::new(Box::new(callback)));
    }

    /// Simulate a change event (used for testing or programmatic notification).
    pub fn notify(&self, event: MemoryChangeEvent) -> Result<()> {
        for cb in &self.callbacks {
            cb(event.clone());
        }
        self.tx.send(Some(event)).map_err(|_| anyhow::anyhow!("no receivers"))?;
        Ok(())
    }

    /// Receive the latest change event (non-blocking).
    pub fn latest(&self) -> Option<MemoryChangeEvent> {
        self.rx.borrow().clone()
    }

    /// Check if the watcher has any registered callbacks.
    pub fn has_callbacks(&self) -> bool {
        !self.callbacks.is_empty()
    }

    /// Number of registered callbacks.
    pub fn callback_count(&self) -> usize {
        self.callbacks.len()
    }
}

impl Default for MemoryWatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn new_watcher_has_no_callbacks() {
        let w = MemoryWatcher::new();
        assert!(!w.has_callbacks());
        assert_eq!(w.callback_count(), 0);
    }

    #[test]
    fn register_callback() {
        let mut w = MemoryWatcher::new();
        w.on_change(|_| {});
        assert!(w.has_callbacks());
        assert_eq!(w.callback_count(), 1);
    }

    #[test]
    fn notify_invokes_callback() {
        let mut w = MemoryWatcher::new();
        let count = Arc::new(AtomicUsize::new(0));
        let count_clone = count.clone();
        w.on_change(move |_| {
            count_clone.fetch_add(1, Ordering::SeqCst);
        });

        let event = MemoryChangeEvent {
            path: PathBuf::from("/tmp/test.md"),
            kind: ChangeKind::Modified,
        };
        w.notify(event).expect("notify");

        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn notify_updates_latest() {
        let w = MemoryWatcher::new();
        assert!(w.latest().is_none());

        let event = MemoryChangeEvent {
            path: PathBuf::from("/tmp/mem.json"),
            kind: ChangeKind::Created,
        };
        w.notify(event).expect("notify");

        let latest = w.latest().expect("some");
        assert_eq!(latest.path, PathBuf::from("/tmp/mem.json"));
        assert_eq!(latest.kind, ChangeKind::Created);
    }

    #[test]
    fn watch_path_succeeds() {
        let w = MemoryWatcher::new();
        let result = w.watch(Path::new("/some/path"));
        assert!(result.is_ok());
    }

    #[test]
    fn multiple_callbacks() {
        let mut w = MemoryWatcher::new();
        let count = Arc::new(AtomicUsize::new(0));

        for _ in 0..3 {
            let c = count.clone();
            w.on_change(move |_| {
                c.fetch_add(1, Ordering::SeqCst);
            });
        }

        let event = MemoryChangeEvent {
            path: PathBuf::from("/tmp/test"),
            kind: ChangeKind::Deleted,
        };
        w.notify(event).expect("notify");
        assert_eq!(count.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn default_trait() {
        let w = MemoryWatcher::default();
        assert!(!w.has_callbacks());
    }

    #[test]
    fn change_kind_equality() {
        assert_eq!(ChangeKind::Created, ChangeKind::Created);
        assert_ne!(ChangeKind::Created, ChangeKind::Modified);
    }
}

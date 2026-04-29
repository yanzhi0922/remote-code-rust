//! Cache manager for code indexing.
//!
//! Corresponds to `cache-manager.ts` in the TypeScript source.
//!
//! Manages file hash caching to detect changes and avoid re-indexing
//! unchanged files. Uses SHA-256 hashes stored as JSON.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// Error type for cache manager operations.
#[derive(Debug, thiserror::Error)]
pub enum CacheError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),
}

/// Manages the cache for code indexing.
///
/// Stores file path -> content hash mappings to detect file changes
/// and enable incremental indexing.
pub struct CacheManager {
    /// Path to the cache file on disk.
    cache_path: PathBuf,
    /// In-memory map of file paths to their content hashes.
    file_hashes: HashMap<String, String>,
    /// Whether the cache has been modified since last save.
    dirty: bool,
}

impl CacheManager {
    /// Creates a new cache manager.
    ///
    /// The cache file is stored at `cache_dir/roo-index-cache-{workspace_hash}.json`.
    pub fn new(cache_dir: &Path, workspace_path: &str) -> Self {
        let workspace_hash = format!("{:x}", Sha256::digest(workspace_path.as_bytes()));
        let cache_path = cache_dir.join(format!("roo-index-cache-{}.json", workspace_hash));

        Self {
            cache_path,
            file_hashes: HashMap::new(),
            dirty: false,
        }
    }

    /// Creates a new cache manager with a specific cache file path.
    pub fn with_path(cache_path: PathBuf) -> Self {
        Self {
            cache_path,
            file_hashes: HashMap::new(),
            dirty: false,
        }
    }

    /// Initializes the cache manager by loading the cache file from disk.
    pub fn initialize(&mut self) -> Result<(), CacheError> {
        match std::fs::read_to_string(&self.cache_path) {
            Ok(data) => {
                self.file_hashes = serde_json::from_str(&data)?;
            }
            Err(_) => {
                // Cache file doesn't exist yet, start with empty cache
                self.file_hashes = HashMap::new();
            }
        }
        self.dirty = false;
        Ok(())
    }

    /// Saves the cache to disk.
    fn perform_save(&self) -> Result<(), CacheError> {
        if let Some(parent) = self.cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let data = serde_json::to_string_pretty(&self.file_hashes)?;
        std::fs::write(&self.cache_path, data)?;
        Ok(())
    }

    /// Clears the cache file and in-memory hashes.
    pub fn clear_cache_file(&mut self) -> Result<(), CacheError> {
        self.file_hashes.clear();
        self.dirty = false;
        self.perform_save()
    }

    /// Gets the hash for a file path.
    pub fn get_hash(&self, file_path: &str) -> Option<&str> {
        self.file_hashes.get(file_path).map(|s| s.as_str())
    }

    /// Updates the hash for a file path.
    pub fn update_hash(&mut self, file_path: &str, hash: &str) {
        self.file_hashes.insert(file_path.to_string(), hash.to_string());
        self.dirty = true;
    }

    /// Deletes the hash for a file path.
    pub fn delete_hash(&mut self, file_path: &str) {
        self.file_hashes.remove(file_path);
        self.dirty = true;
    }

    /// Flushes any pending cache writes to disk immediately.
    pub fn flush(&self) -> Result<(), CacheError> {
        self.perform_save()
    }

    /// Gets a copy of all file hashes.
    pub fn get_all_hashes(&self) -> HashMap<String, String> {
        self.file_hashes.clone()
    }

    /// Computes the SHA-256 hash of file content.
    pub fn compute_hash(content: &[u8]) -> String {
        format!("{:x}", Sha256::digest(content))
    }

    /// Checks if a file has changed since last indexing.
    ///
    /// Returns `true` if the file is new or its content has changed.
    pub fn has_file_changed(&self, file_path: &str, content: &[u8]) -> bool {
        let current_hash = Self::compute_hash(content);
        match self.get_hash(file_path) {
            Some(cached_hash) => cached_hash != current_hash,
            None => true,
        }
    }

    /// Returns the number of cached file hashes.
    pub fn len(&self) -> usize {
        self.file_hashes.len()
    }

    /// Returns true if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.file_hashes.is_empty()
    }

    /// Returns whether the cache has been modified since last save.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_cache_manager_new() {
        let dir = std::env::temp_dir().join("roo-test-cache-new");
        let _ = fs::remove_dir_all(&dir);
        let cm = CacheManager::new(&dir, "/test/workspace");
        assert!(cm.is_empty());
    }

    #[test]
    fn test_cache_manager_update_and_get() {
        let dir = std::env::temp_dir().join("roo-test-cache-update");
        let _ = fs::remove_dir_all(&dir);
        let mut cm = CacheManager::new(&dir, "/test/workspace");

        cm.update_hash("file1.rs", "hash1");
        assert_eq!(cm.get_hash("file1.rs"), Some("hash1"));
        assert!(cm.is_dirty());
        assert_eq!(cm.len(), 1);
    }

    #[test]
    fn test_cache_manager_delete() {
        let dir = std::env::temp_dir().join("roo-test-cache-delete");
        let _ = fs::remove_dir_all(&dir);
        let mut cm = CacheManager::new(&dir, "/test/workspace");

        cm.update_hash("file1.rs", "hash1");
        cm.delete_hash("file1.rs");
        assert_eq!(cm.get_hash("file1.rs"), None);
        assert!(cm.is_empty());
    }

    #[test]
    fn test_cache_manager_save_and_load() {
        let dir = std::env::temp_dir().join("roo-test-cache-save");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        // Save
        {
            let mut cm = CacheManager::new(&dir, "/test/workspace");
            cm.update_hash("file1.rs", "hash1");
            cm.update_hash("file2.rs", "hash2");
            cm.flush().unwrap();
        }

        // Load
        {
            let mut cm = CacheManager::new(&dir, "/test/workspace");
            cm.initialize().unwrap();
            assert_eq!(cm.get_hash("file1.rs"), Some("hash1"));
            assert_eq!(cm.get_hash("file2.rs"), Some("hash2"));
        }
    }

    #[test]
    fn test_cache_manager_clear() {
        let dir = std::env::temp_dir().join("roo-test-cache-clear");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let mut cm = CacheManager::new(&dir, "/test/workspace");
        cm.update_hash("file1.rs", "hash1");
        cm.clear_cache_file().unwrap();
        assert!(cm.is_empty());
    }

    #[test]
    fn test_compute_hash() {
        let hash1 = CacheManager::compute_hash(b"hello");
        let hash2 = CacheManager::compute_hash(b"hello");
        let hash3 = CacheManager::compute_hash(b"world");

        assert_eq!(hash1, hash2);
        assert_ne!(hash1, hash3);
    }

    #[test]
    fn test_has_file_changed() {
        let dir = std::env::temp_dir().join("roo-test-cache-changed");
        let _ = fs::remove_dir_all(&dir);
        let mut cm = CacheManager::new(&dir, "/test/workspace");

        let content = b"fn main() {}";
        let hash = CacheManager::compute_hash(content);

        // New file should be changed
        assert!(cm.has_file_changed("file.rs", content));

        // After updating, should not be changed
        cm.update_hash("file.rs", &hash);
        assert!(!cm.has_file_changed("file.rs", content));

        // Different content should be changed
        assert!(cm.has_file_changed("file.rs", b"fn other() {}"));
    }

    #[test]
    fn test_get_all_hashes() {
        let dir = std::env::temp_dir().join("roo-test-cache-all");
        let _ = fs::remove_dir_all(&dir);
        let mut cm = CacheManager::new(&dir, "/test/workspace");

        cm.update_hash("a.rs", "h1");
        cm.update_hash("b.rs", "h2");

        let all = cm.get_all_hashes();
        assert_eq!(all.len(), 2);
        assert_eq!(all["a.rs"], "h1");
        assert_eq!(all["b.rs"], "h2");
    }
}

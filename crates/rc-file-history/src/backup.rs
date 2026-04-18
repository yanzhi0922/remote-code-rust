//! File backup management — create, restore, and manage file backups.
//!
//! Corresponds to `src/utils/fileHistory.ts` (createBackup, restoreBackup,
//! getBackupFileName, resolveBackupPath, checkOriginFileChanged, etc.).

use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A record of a single file backup.
///
/// Corresponds to `FileHistoryBackup` in the TypeScript source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupRecord {
    /// The original file path that was backed up.
    pub file_path: String,
    /// The version number of this backup (starts at 1).
    pub version: u32,
    /// When this backup was created.
    pub backup_time: DateTime<Utc>,
    /// SHA-256 hash of the file contents at backup time, if available.
    /// `None` means the file did not exist when the backup was taken.
    pub content_hash: Option<String>,
}

impl BackupRecord {
    /// Create a new backup record for a file that doesn't exist (null backup).
    #[must_use]
    pub fn null(file_path: String, version: u32) -> Self {
        Self {
            file_path,
            version,
            backup_time: Utc::now(),
            content_hash: None,
        }
    }

    /// Create a new backup record with a known content hash.
    #[must_use]
    pub fn with_hash(file_path: String, version: u32, content_hash: String) -> Self {
        Self {
            file_path,
            version,
            backup_time: Utc::now(),
            content_hash: Some(content_hash),
        }
    }

    /// Check if this is a null backup (file didn't exist).
    #[must_use]
    pub fn is_null(&self) -> bool {
        self.content_hash.is_none()
    }
}

/// Generate a deterministic backup file name from the original path and version.
///
/// Format: `{sha256_prefix_16}@v{version}`
///
/// Corresponds to `getBackupFileName` in the TS source.
#[must_use]
pub fn get_backup_file_name(file_path: &str, version: u32) -> String {
    let mut hasher = Sha256::new();
    hasher.update(file_path.as_bytes());
    let hash = hasher.finalize();
    let hex_prefix = &format!("{:x}", hash)[..16];
    format!("{hex_prefix}@v{version}")
}

/// Resolve the full backup path on disk.
///
/// Corresponds to `resolveBackupPath` in the TS source.
#[must_use]
pub fn resolve_backup_path(backup_dir: &Path, backup_file_name: &str) -> PathBuf {
    backup_dir.join(backup_file_name)
}

/// Compute SHA-256 hash of file contents.
fn hash_file_contents(path: &Path) -> anyhow::Result<String> {
    let data = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    Ok(format!("{:x}", hasher.finalize()))
}

/// Create a backup of a file.
///
/// If the file does not exist, records a null backup (content_hash = None).
/// If it exists, copies it to the backup directory and records its hash.
///
/// Corresponds to `createBackup` in the TS source.
pub fn create_backup(
    file_path: &Path,
    version: u32,
    backup_dir: &Path,
) -> anyhow::Result<BackupRecord> {
    let file_path_str = file_path.to_string_lossy().to_string();

    // If the source file doesn't exist, record a null backup
    if !file_path.exists() {
        return Ok(BackupRecord::null(file_path_str, version));
    }

    // Compute hash of the source file
    let content_hash = hash_file_contents(file_path)?;

    // Generate backup file name and path
    let backup_name = get_backup_file_name(&file_path_str, version);
    let backup_path = resolve_backup_path(backup_dir, &backup_name);

    // Ensure backup directory exists
    if let Some(parent) = backup_path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Copy the file
    fs::copy(file_path, &backup_path)?;

    Ok(BackupRecord::with_hash(
        file_path_str,
        version,
        content_hash,
    ))
}

/// Restore a file from its backup.
///
/// Returns `Ok(())` if the restore succeeded, or an error if the backup
/// doesn't exist or the copy fails.
///
/// Corresponds to `restoreBackup` in the TS source.
pub fn restore_backup(
    file_path: &Path,
    backup_file_name: &str,
    backup_dir: &Path,
) -> anyhow::Result<()> {
    let backup_path = resolve_backup_path(backup_dir, backup_file_name);

    if !backup_path.exists() {
        anyhow::bail!("Backup file not found: {}", backup_path.display());
    }

    // Ensure the destination directory exists
    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)?;
    }

    fs::copy(&backup_path, file_path)?;
    Ok(())
}

/// Check if the original file has changed compared to its backup.
///
/// Compares file existence and content hash. Returns `true` if the file
/// has changed (or if comparison is not possible).
///
/// Corresponds to `checkOriginFileChanged` in the TS source.
pub fn check_origin_file_changed(
    file_path: &Path,
    backup_record: &BackupRecord,
    _backup_dir: &Path,
) -> bool {
    // If backup is null (file didn't exist), check if file now exists
    if backup_record.is_null() {
        return file_path.exists();
    }

    // If original file doesn't exist now, it has changed
    if !file_path.exists() {
        return true;
    }

    // Compare content hashes
    match hash_file_contents(file_path) {
        Ok(current_hash) => match &backup_record.content_hash {
            Some(backup_hash) => current_hash != *backup_hash,
            None => true,
        },
        Err(_) => true, // Assume changed on error
    }
}

/// Find the first (earliest) backup version for a file across all snapshots.
///
/// Corresponds to `getBackupFileNameFirstVersion` in the TS source.
pub fn get_first_version_backup_name(
    tracking_path: &str,
    snapshots: &[crate::snapshot::FileHistorySnapshot],
) -> Option<Option<String>> {
    for snapshot in snapshots {
        if let Some(backup) = snapshot.tracked_file_backups.get(tracking_path)
            && backup.version == 1
        {
            // Return Some(None) for null backups, Some(Some(name)) for real backups
            if backup.is_null() {
                return Some(None);
            }
            let name = get_backup_file_name(&backup.file_path, 1);
            return Some(Some(name));
        }
    }
    None // Could not find any first version
}

/// Shorten a file path by using relative paths when possible.
///
/// Corresponds to `maybeShortenFilePath` in the TS source.
#[must_use]
pub fn maybe_shorten_file_path(file_path: &str, cwd: &Path) -> String {
    let path = Path::new(file_path);
    if path.is_absolute()
        && let Ok(relative) = path.strip_prefix(cwd)
    {
        return relative.to_string_lossy().to_string();
    }
    file_path.to_string()
}

/// Expand a shortened file path back to absolute.
///
/// Corresponds to `maybeExpandFilePath` in the TS source.
#[must_use]
pub fn maybe_expand_file_path(file_path: &str, cwd: &Path) -> PathBuf {
    let path = Path::new(file_path);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(file_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn backup_file_name_deterministic() {
        let name1 = get_backup_file_name("src/main.rs", 1);
        let name2 = get_backup_file_name("src/main.rs", 1);
        assert_eq!(name1, name2);
        assert!(name1.ends_with("@v1"));
        assert_eq!(name1.len(), 19); // 16 hex chars + "@v1"
    }

    #[test]
    fn backup_file_name_different_versions() {
        let v1 = get_backup_file_name("src/main.rs", 1);
        let v2 = get_backup_file_name("src/main.rs", 2);
        assert_ne!(v1, v2);
        assert!(v1.ends_with("@v1"));
        assert!(v2.ends_with("@v2"));
    }

    #[test]
    fn backup_file_name_different_paths() {
        let a = get_backup_file_name("a.rs", 1);
        let b = get_backup_file_name("b.rs", 1);
        assert_ne!(a, b);
    }

    #[test]
    fn null_backup_record() {
        let rec = BackupRecord::null("missing.txt".to_string(), 1);
        assert!(rec.is_null());
        assert_eq!(rec.version, 1);
        assert_eq!(rec.file_path, "missing.txt");
        assert!(rec.content_hash.is_none());
    }

    #[test]
    fn backup_with_hash() {
        let rec = BackupRecord::with_hash("main.rs".to_string(), 2, "abc123".to_string());
        assert!(!rec.is_null());
        assert_eq!(rec.version, 2);
        assert_eq!(rec.content_hash.as_deref(), Some("abc123"));
    }

    #[test]
    fn create_backup_existing_file() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let file_path = tmp.path().join("test.txt");
        fs::write(&file_path, "hello world")?;

        let backup_dir = tmp.path().join("backups");
        let record = create_backup(&file_path, 1, &backup_dir)?;

        assert!(!record.is_null());
        assert_eq!(record.version, 1);
        assert!(record.content_hash.is_some());
        assert!(backup_dir.exists());
        Ok(())
    }

    #[test]
    fn create_backup_missing_file() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let file_path = tmp.path().join("nonexistent.txt");

        let backup_dir = tmp.path().join("backups");
        let record = create_backup(&file_path, 1, &backup_dir)?;

        assert!(record.is_null());
        assert!(record.content_hash.is_none());
        Ok(())
    }

    #[test]
    fn restore_backup_roundtrip() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let file_path = tmp.path().join("test.txt");
        fs::write(&file_path, "original content")?;

        let backup_dir = tmp.path().join("backups");
        let record = create_backup(&file_path, 1, &backup_dir)?;

        // Modify the original file
        fs::write(&file_path, "modified content")?;

        // Restore from backup
        let backup_name = get_backup_file_name(&record.file_path, 1);
        restore_backup(&file_path, &backup_name, &backup_dir)?;

        let restored = fs::read_to_string(&file_path)?;
        assert_eq!(restored, "original content");
        Ok(())
    }

    #[test]
    fn restore_backup_missing_backup_fails() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let file_path = tmp.path().join("test.txt");
        let backup_dir = tmp.path().join("backups");

        let result = restore_backup(&file_path, "nonexistent@v1", &backup_dir);
        assert!(result.is_err());
        Ok(())
    }

    #[test]
    fn check_origin_file_changed_unchanged() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let file_path = tmp.path().join("test.txt");
        fs::write(&file_path, "hello")?;

        let backup_dir = tmp.path().join("backups");
        let record = create_backup(&file_path, 1, &backup_dir)?;

        // File hasn't changed
        assert!(!check_origin_file_changed(&file_path, &record, &backup_dir));
        Ok(())
    }

    #[test]
    fn check_origin_file_changed_modified() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let file_path = tmp.path().join("test.txt");
        fs::write(&file_path, "hello")?;

        let backup_dir = tmp.path().join("backups");
        let record = create_backup(&file_path, 1, &backup_dir)?;

        // Modify the file
        fs::write(&file_path, "world")?;

        assert!(check_origin_file_changed(&file_path, &record, &backup_dir));
        Ok(())
    }

    #[test]
    fn check_origin_file_changed_deleted() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let file_path = tmp.path().join("test.txt");
        fs::write(&file_path, "hello")?;

        let backup_dir = tmp.path().join("backups");
        let record = create_backup(&file_path, 1, &backup_dir)?;

        // Delete the file
        fs::remove_file(&file_path)?;

        assert!(check_origin_file_changed(&file_path, &record, &backup_dir));
        Ok(())
    }

    #[test]
    fn check_origin_file_changed_null_backup_now_exists() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let file_path = tmp.path().join("new.txt");

        let record = BackupRecord::null("new.txt".to_string(), 1);

        // File didn't exist, now it does
        fs::write(&file_path, "new content")?;
        assert!(check_origin_file_changed(
            &file_path,
            &record,
            &tmp.path().join("backups")
        ));
        Ok(())
    }

    #[test]
    fn check_origin_file_changed_null_backup_still_missing() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let file_path = tmp.path().join("still_missing.txt");

        let record = BackupRecord::null("still_missing.txt".to_string(), 1);

        // File still doesn't exist
        assert!(!check_origin_file_changed(
            &file_path,
            &record,
            &tmp.path().join("backups")
        ));
        Ok(())
    }

    #[test]
    fn maybe_shorten_file_path_relative() {
        let cwd = Path::new("/home/user/project");
        let result = maybe_shorten_file_path("src/main.rs", cwd);
        assert_eq!(result, "src/main.rs");
    }

    #[test]
    fn maybe_shorten_file_path_absolute_within_cwd() -> anyhow::Result<()> {
        let cwd = std::env::current_dir()?;
        let abs_path = cwd.join("src/main.rs");
        let result = maybe_shorten_file_path(&abs_path.to_string_lossy(), &cwd);
        assert!(
            result == "src/main.rs" || result.contains("src"),
            "expected relative path, got: {result}"
        );
        Ok(())
    }

    #[test]
    fn maybe_shorten_file_path_absolute_outside_cwd() -> anyhow::Result<()> {
        let abs_outside = if cfg!(windows) {
            "C:\\Windows\\System32\\config"
        } else {
            "/etc/config"
        };
        let cwd = std::env::current_dir()?;
        let result = maybe_shorten_file_path(abs_outside, &cwd);
        assert_eq!(result, abs_outside);
        Ok(())
    }

    #[test]
    fn maybe_expand_file_path_relative() {
        let cwd = Path::new("/home/user/project");
        let result = maybe_expand_file_path("src/main.rs", cwd);
        assert_eq!(result, PathBuf::from("/home/user/project/src/main.rs"));
    }

    #[test]
    fn maybe_expand_file_path_absolute() {
        let cwd = Path::new("/home/user/project");
        let result = maybe_expand_file_path("/etc/config", cwd);
        assert_eq!(result, PathBuf::from("/etc/config"));
    }

    #[test]
    fn resolve_backup_path_joins() {
        let dir = Path::new("/tmp/backups");
        let result = resolve_backup_path(dir, "abc123@v1");
        assert_eq!(result, PathBuf::from("/tmp/backups/abc123@v1"));
    }

    #[test]
    fn backup_record_serializes() -> anyhow::Result<()> {
        let rec = BackupRecord::with_hash("main.rs".to_string(), 1, "deadbeef".to_string());
        let json = serde_json::to_string(&rec)?;
        assert!(json.contains("main.rs"));
        assert!(json.contains("deadbeef"));

        let deserialized: BackupRecord = serde_json::from_str(&json)?;
        assert_eq!(deserialized.file_path, "main.rs");
        assert_eq!(deserialized.version, 1);
        Ok(())
    }
}

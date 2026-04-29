//! Diff statistics computation for file history.
//!
//! Corresponds to `src/utils/fileHistory.ts` (DiffStats type,
//! computeDiffStatsForFile, fileHistoryGetDiffStats, fileHistoryHasAnyChanges).

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Statistics about the differences between file versions.
///
/// Corresponds to `DiffStats` in the TypeScript source (lines 55–61).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DiffStats {
    /// List of file paths that have changed.
    pub files_changed: Vec<String>,
    /// Total number of lines inserted.
    pub insertions: u64,
    /// Total number of lines deleted.
    pub deletions: u64,
}

impl DiffStats {
    /// Create empty diff stats.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if there are any changes.
    #[must_use]
    pub fn has_changes(&self) -> bool {
        !self.files_changed.is_empty() || self.insertions > 0 || self.deletions > 0
    }

    /// Get the total number of changed lines.
    #[must_use]
    pub fn total_changes(&self) -> u64 {
        self.insertions + self.deletions
    }

    /// Merge another DiffStats into this one, combining all metrics.
    pub fn merge(&mut self, other: &DiffStats) {
        let existing: HashSet<String> = self.files_changed.iter().cloned().collect();
        for file in &other.files_changed {
            if !existing.contains(file) {
                self.files_changed.push(file.clone());
            }
        }
        self.insertions += other.insertions;
        self.deletions += other.deletions;
    }
}

/// A single line-level diff change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineChange {
    /// Lines were added (present in new, not in old).
    Added { count: usize },
    /// Lines were removed (present in old, not in new).
    Removed { count: usize },
    /// Lines are unchanged.
    Unchanged { count: usize },
}

/// Compute a simple line-level diff between two strings.
///
/// This is a basic longest-common-subsequence diff implementation.
/// For production use, consider integrating a proper diff library.
fn compute_line_diff(old: &str, new: &str) -> Vec<LineChange> {
    let old_lines: Vec<&str> = if old.is_empty() {
        Vec::new()
    } else {
        old.lines().collect()
    };
    let new_lines: Vec<&str> = if new.is_empty() {
        Vec::new()
    } else {
        new.lines().collect()
    };

    let old_len = old_lines.len();
    let new_len = new_lines.len();

    // LCS table
    let mut lcs = vec![vec![0usize; new_len + 1]; old_len + 1];
    for (i, old_line) in old_lines.iter().enumerate() {
        for (j, new_line) in new_lines.iter().enumerate() {
            if old_line == new_line {
                lcs[i + 1][j + 1] = lcs[i][j] + 1;
            } else {
                lcs[i + 1][j + 1] = lcs[i + 1][j].max(lcs[i][j + 1]);
            }
        }
    }

    // Backtrack to produce the diff
    let mut changes = Vec::new();
    let mut i = old_len;
    let mut j = new_len;

    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old_lines[i - 1] == new_lines[j - 1] {
            changes.push(LineChange::Unchanged { count: 1 });
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || lcs[i][j - 1] >= lcs[i - 1][j]) {
            changes.push(LineChange::Added { count: 1 });
            j -= 1;
        } else {
            changes.push(LineChange::Removed { count: 1 });
            i -= 1;
        }
    }

    changes.reverse();

    // Coalesce consecutive changes of the same type
    let mut coalesced = Vec::new();
    for change in changes {
        match change {
            LineChange::Added { count: n } => {
                if let Some(LineChange::Added { count }) = coalesced.last_mut() {
                    *count += n;
                } else {
                    coalesced.push(LineChange::Added { count: n });
                }
            }
            LineChange::Removed { count: n } => {
                if let Some(LineChange::Removed { count }) = coalesced.last_mut() {
                    *count += n;
                } else {
                    coalesced.push(LineChange::Removed { count: n });
                }
            }
            LineChange::Unchanged { count: n } => {
                if let Some(LineChange::Unchanged { count }) = coalesced.last_mut() {
                    *count += n;
                } else {
                    coalesced.push(LineChange::Unchanged { count: n });
                }
            }
        }
    }

    coalesced
}

/// Compute diff stats for a single file by comparing current content
/// with backup content.
///
/// Corresponds to `computeDiffStatsForFile` in the TS source.
pub fn compute_diff_stats_for_file(file_path: &Path, backup_content: Option<&str>) -> DiffStats {
    let mut stats = DiffStats::new();

    let current_content = fs::read_to_string(file_path).ok();

    match (current_content, backup_content) {
        (None, None) => {
            // Both missing — no diff
        }
        (Some(_current), None) => {
            // File exists but no backup — all lines are insertions
            stats
                .files_changed
                .push(file_path.to_string_lossy().to_string());
            let line_count = _current.lines().count();
            stats.insertions = line_count as u64;
        }
        (None, Some(_backup)) => {
            // File missing but backup exists — all lines are deletions
            stats
                .files_changed
                .push(file_path.to_string_lossy().to_string());
            let line_count = _backup.lines().count();
            stats.deletions = line_count as u64;
        }
        (Some(current), Some(backup)) => {
            let changes = compute_line_diff(backup, &current);
            let has_add_or_remove = changes
                .iter()
                .any(|c| matches!(c, LineChange::Added { .. } | LineChange::Removed { .. }));

            if has_add_or_remove {
                stats
                    .files_changed
                    .push(file_path.to_string_lossy().to_string());
                for change in &changes {
                    match change {
                        LineChange::Added { count } => stats.insertions += *count as u64,
                        LineChange::Removed { count } => stats.deletions += *count as u64,
                        LineChange::Unchanged { .. } => {}
                    }
                }
            }
        }
    }

    stats
}

/// Compute diff stats for multiple files.
pub fn compute_diff_stats_for_files(
    files: &[(String, Option<String>)], // (file_path, backup_content)
) -> DiffStats {
    let mut combined = DiffStats::new();
    for (file_path, backup_content) in files {
        let path = Path::new(file_path);
        let file_stats = compute_diff_stats_for_file(path, backup_content.as_deref());
        combined.merge(&file_stats);
    }
    combined
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn empty_diff_stats() {
        let stats = DiffStats::new();
        assert!(!stats.has_changes());
        assert_eq!(stats.total_changes(), 0);
        assert!(stats.files_changed.is_empty());
    }

    #[test]
    fn diff_stats_with_changes() {
        let stats = DiffStats {
            files_changed: vec!["a.rs".to_string(), "b.rs".to_string()],
            insertions: 10,
            deletions: 5,
        };
        assert!(stats.has_changes());
        assert_eq!(stats.total_changes(), 15);
    }

    #[test]
    fn merge_diff_stats() {
        let mut a = DiffStats {
            files_changed: vec!["a.rs".to_string()],
            insertions: 5,
            deletions: 3,
        };
        let b = DiffStats {
            files_changed: vec!["b.rs".to_string()],
            insertions: 2,
            deletions: 1,
        };
        a.merge(&b);
        assert_eq!(a.files_changed.len(), 2);
        assert_eq!(a.insertions, 7);
        assert_eq!(a.deletions, 4);
    }

    #[test]
    fn merge_no_duplicates() {
        let mut a = DiffStats {
            files_changed: vec!["a.rs".to_string()],
            insertions: 5,
            deletions: 0,
        };
        let b = DiffStats {
            files_changed: vec!["a.rs".to_string()],
            insertions: 3,
            deletions: 0,
        };
        a.merge(&b);
        assert_eq!(a.files_changed.len(), 1);
        assert_eq!(a.insertions, 8);
    }

    #[test]
    fn line_diff_identical() {
        let changes = compute_line_diff("hello\nworld", "hello\nworld");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0], LineChange::Unchanged { count: 2 });
    }

    #[test]
    fn line_diff_additions() {
        let changes = compute_line_diff("", "line1\nline2\nline3");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0], LineChange::Added { count: 3 });
    }

    #[test]
    fn line_diff_deletions() {
        let changes = compute_line_diff("line1\nline2", "");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0], LineChange::Removed { count: 2 });
    }

    #[test]
    fn line_diff_mixed() {
        let changes = compute_line_diff("a\nb\nc", "a\nx\nc");
        // Should have: unchanged(a), removed(b), added(x), unchanged(c)
        assert!(
            changes
                .iter()
                .any(|c| matches!(c, LineChange::Removed { .. }))
        );
        assert!(
            changes
                .iter()
                .any(|c| matches!(c, LineChange::Added { .. }))
        );
    }

    #[test]
    fn compute_diff_for_file_both_missing() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let file_path = tmp.path().join("nonexistent.txt");
        let stats = compute_diff_stats_for_file(&file_path, None);
        assert!(!stats.has_changes());
        Ok(())
    }

    #[test]
    fn compute_diff_for_file_new_file() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let file_path = tmp.path().join("new.txt");
        fs::write(&file_path, "line1\nline2\n")?;

        let stats = compute_diff_stats_for_file(&file_path, None);
        assert!(stats.has_changes());
        assert_eq!(stats.insertions, 2); // 2 lines (trailing newline doesn't add a line)
        assert_eq!(stats.deletions, 0);
        Ok(())
    }

    #[test]
    fn compute_diff_for_file_deleted() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let file_path = tmp.path().join("deleted.txt");

        let stats = compute_diff_stats_for_file(&file_path, Some("line1\nline2"));
        assert!(stats.has_changes());
        assert_eq!(stats.insertions, 0);
        assert_eq!(stats.deletions, 2);
        Ok(())
    }

    #[test]
    fn compute_diff_for_file_unchanged() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let file_path = tmp.path().join("same.txt");
        fs::write(&file_path, "hello\nworld\n")?;

        let stats = compute_diff_stats_for_file(&file_path, Some("hello\nworld\n"));
        assert!(!stats.has_changes());
        Ok(())
    }

    #[test]
    fn compute_diff_for_file_modified() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        let file_path = tmp.path().join("mod.txt");
        fs::write(&file_path, "hello\nchanged\nworld\n")?;

        let stats = compute_diff_stats_for_file(&file_path, Some("hello\noriginal\nworld\n"));
        assert!(stats.has_changes());
        assert_eq!(stats.insertions, 1);
        assert_eq!(stats.deletions, 1);
        Ok(())
    }

    #[test]
    fn compute_diff_for_multiple_files() -> anyhow::Result<()> {
        let tmp = TempDir::new()?;
        fs::write(tmp.path().join("a.txt"), "new\n")?;
        fs::write(tmp.path().join("b.txt"), "line1\nline2\n")?;

        let files_with_paths = vec![
            (
                tmp.path().join("a.txt").to_string_lossy().to_string(),
                Some("old\n".to_string()),
            ),
            (tmp.path().join("b.txt").to_string_lossy().to_string(), None),
        ];

        let stats = compute_diff_stats_for_files(&files_with_paths);
        assert!(stats.has_changes());
        assert_eq!(stats.files_changed.len(), 2);
        Ok(())
    }

    #[test]
    fn diff_stats_serializes() -> anyhow::Result<()> {
        let stats = DiffStats {
            files_changed: vec!["main.rs".to_string()],
            insertions: 10,
            deletions: 5,
        };
        let json = serde_json::to_string(&stats)?;
        assert!(json.contains("main.rs"));
        assert!(json.contains("10"));
        assert!(json.contains("5"));

        let deserialized: DiffStats = serde_json::from_str(&json)?;
        assert_eq!(deserialized.insertions, 10);
        assert_eq!(deserialized.deletions, 5);
        Ok(())
    }
}

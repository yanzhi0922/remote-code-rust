//! Diff computation for checkpoint file changes.
//!
//! Produces unified diffs between file versions captured in checkpoints.

use crate::types::*;

/// Computes diffs between file versions in checkpoints.
pub struct CheckpointDiffer;

impl CheckpointDiffer {
    /// Compute a unified diff between two strings.
    pub fn unified_diff(old: &str, new: &str, path: &str) -> String {
        let mut output = String::with_capacity(old.len() + new.len());
        output.push_str(&format!("--- a/{path}\n"));
        output.push_str(&format!("+++ b/{path}\n"));

        for change in similar::TextDiff::from_lines(old, new).iter_all_changes() {
            let sign = match change.tag() {
                similar::ChangeTag::Delete => "-",
                similar::ChangeTag::Insert => "+",
                similar::ChangeTag::Equal => " ",
            };
            output.push_str(sign);
            output.push_str(&change.to_string());
        }

        output
    }

    /// Compute structured diff hunks between two strings.
    pub fn structured_diff(old: &str, new: &str) -> Vec<DiffHunk> {
        let diff = similar::TextDiff::from_lines(old, new);
        let mut hunks = Vec::new();

        for op in diff.ops() {
            let mut diff_hunk = DiffHunk {
                old_start: op.old_range().start,
                old_count: op.old_range().len(),
                new_start: op.new_range().start,
                new_count: op.new_range().len(),
                lines: Vec::new(),
            };

            for change in diff.iter_changes(op) {
                let line_type = match change.tag() {
                    similar::ChangeTag::Equal => DiffLineType::Context,
                    similar::ChangeTag::Delete => DiffLineType::Removed,
                    similar::ChangeTag::Insert => DiffLineType::Added,
                };
                diff_hunk.lines.push(DiffLine {
                    line_type,
                    content: change.to_string(),
                });
            }

            hunks.push(diff_hunk);
        }

        hunks
    }

    /// Compute line-level stats from a diff.
    pub fn compute_stats(old: &str, new: &str) -> (usize, usize) {
        let mut added = 0;
        let mut removed = 0;

        for change in similar::TextDiff::from_lines(old, new).iter_all_changes() {
            match change.tag() {
                similar::ChangeTag::Insert => added += 1,
                similar::ChangeTag::Delete => removed += 1,
                similar::ChangeTag::Equal => {}
            }
        }

        (added, removed)
    }

    /// Build FileDiff objects for a checkpoint by loading cached content.
    pub fn build_file_diffs(
        store: &crate::storage::CheckpointStore,
        checkpoint: &Checkpoint,
    ) -> Vec<FileDiff> {
        checkpoint
            .file_changes
            .iter()
            .filter_map(|change| {
                let hunks = match change.operation {
                    FileOperation::Created => {
                        let new_content = change
                            .hash_after
                            .as_ref()
                            .and_then(|h| store.get_cached_content(h).ok().flatten());
                        let new_str = new_content
                            .map(|c| String::from_utf8_lossy(&c).to_string())
                            .unwrap_or_default();
                        Self::structured_diff("", &new_str)
                    }
                    FileOperation::Deleted => {
                        let old_content = change
                            .hash_before
                            .as_ref()
                            .and_then(|h| store.get_cached_content(h).ok().flatten());
                        let old_str = old_content
                            .map(|c| String::from_utf8_lossy(&c).to_string())
                            .unwrap_or_default();
                        Self::structured_diff(&old_str, "")
                    }
                    FileOperation::Modified => {
                        let old_content = change
                            .hash_before
                            .as_ref()
                            .and_then(|h| store.get_cached_content(h).ok().flatten());
                        let new_content = change
                            .hash_after
                            .as_ref()
                            .and_then(|h| store.get_cached_content(h).ok().flatten());
                        let old_str = old_content
                            .map(|c| String::from_utf8_lossy(&c).to_string())
                            .unwrap_or_default();
                        let new_str = new_content
                            .map(|c| String::from_utf8_lossy(&c).to_string())
                            .unwrap_or_default();
                        Self::structured_diff(&old_str, &new_str)
                    }
                };

                if hunks.is_empty() && change.operation == FileOperation::Modified {
                    return None;
                }

                Some(FileDiff {
                    path: change.path.clone(),
                    operation: change.operation,
                    hunks,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unified_diff_addition() {
        let old = "line 1\nline 2\n";
        let new = "line 1\nline 2\nline 3\n";
        let diff = CheckpointDiffer::unified_diff(old, new, "test.rs");
        assert!(diff.contains("+line 3"));
    }

    #[test]
    fn test_unified_diff_deletion() {
        let old = "line 1\nline 2\nline 3\n";
        let new = "line 1\nline 3\n";
        let diff = CheckpointDiffer::unified_diff(old, new, "test.rs");
        assert!(diff.contains("-line 2"));
    }

    #[test]
    fn test_structured_diff() {
        let old = "a\nb\nc\n";
        let new = "a\nx\nc\n";
        let hunks = CheckpointDiffer::structured_diff(old, new);
        assert!(!hunks.is_empty());
    }

    #[test]
    fn test_compute_stats() {
        let old = "line 1\nline 2\n";
        let new = "line 1\nline 3\nline 4\n";
        let (added, removed) = CheckpointDiffer::compute_stats(old, new);
        assert_eq!(added, 2); // line 3 + line 4
        assert_eq!(removed, 1); // line 2
    }

    #[test]
    fn test_compute_stats_no_change() {
        let text = "same content\n";
        let (added, removed) = CheckpointDiffer::compute_stats(text, text);
        assert_eq!(added, 0);
        assert_eq!(removed, 0);
    }
}

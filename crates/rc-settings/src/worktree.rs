//! Worktree settings types.

use serde::{Deserialize, Serialize};

/// Git worktree configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeSettings {
    /// Directories to symlink from main repository to worktrees.
    pub symlink_directories: Option<Vec<String>>,
    /// Directories to include via git sparse-checkout.
    pub sparse_paths: Option<Vec<String>>,
}

impl WorktreeSettings {
    /// Check if any worktree configuration is set.
    #[must_use]
    pub fn has_config(&self) -> bool {
        self.symlink_directories
            .as_ref()
            .is_some_and(|d| !d.is_empty())
            || self.sparse_paths.as_ref().is_some_and(|p| !p.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_empty() {
        let w = WorktreeSettings::default();
        assert!(!w.has_config());
    }

    #[test]
    fn with_symlink_dirs() {
        let w = WorktreeSettings {
            symlink_directories: Some(vec!["node_modules".to_string()]),
            ..Default::default()
        };
        assert!(w.has_config());
    }

    #[test]
    fn serialization() {
        let w = WorktreeSettings {
            symlink_directories: Some(vec!["node_modules".to_string(), ".cache".to_string()]),
            sparse_paths: None,
        };
        let json = serde_json::to_string(&w).unwrap();
        assert!(json.contains("node_modules"));
    }
}

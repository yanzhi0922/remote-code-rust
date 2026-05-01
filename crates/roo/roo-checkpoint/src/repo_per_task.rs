//! Repo-per-task checkpoint service.
//!
//! A concrete implementation of `ShadowCheckpointService` that uses a
//! task-specific directory layout: `{shadowDir}/tasks/{taskId}/checkpoints`.
//! Ported from `src/services/checkpoints/RepoPerTaskCheckpointService.ts`.

use std::path::PathBuf;

use crate::error::CheckpointError;
use crate::service::ShadowCheckpointService;
use crate::types::CheckpointServiceOptions;

/// A checkpoint service that creates one shadow git repository per task.
///
/// Directory layout: `{shadow_dir}/tasks/{task_id}/checkpoints`
pub struct RepoPerTaskCheckpointService {
    inner: ShadowCheckpointService,
}

impl RepoPerTaskCheckpointService {
    /// Creates a new `RepoPerTaskCheckpointService` from the given options.
    ///
    /// The checkpoints directory is computed as:
    /// `{shadow_dir}/tasks/{task_id}/checkpoints`
    pub fn create(options: CheckpointServiceOptions) -> Result<Self, CheckpointError> {
        let checkpoints_dir = PathBuf::from(&options.shadow_dir)
            .join("tasks")
            .join(&options.task_id)
            .join("checkpoints");

        let inner = ShadowCheckpointService::new(
            options.task_id,
            checkpoints_dir,
            options.workspace_dir,
            options.log,
        )?;

        Ok(Self { inner })
    }

    /// Returns a reference to the underlying `ShadowCheckpointService`.
    pub fn inner(&self) -> &ShadowCheckpointService {
        &self.inner
    }

    /// Returns a mutable reference to the underlying `ShadowCheckpointService`.
    pub fn inner_mut(&mut self) -> &mut ShadowCheckpointService {
        &mut self.inner
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn default_options(task_id: &str, workspace_dir: &str, shadow_dir: &str) -> CheckpointServiceOptions {
        CheckpointServiceOptions {
            task_id: task_id.to_string(),
            workspace_dir: workspace_dir.to_string(),
            shadow_dir: shadow_dir.to_string(),
            log: None,
        }
    }

    #[test]
    fn test_create_sets_correct_checkpoints_dir() {
        let options = default_options("task-abc", "/tmp/my-project", "/tmp/shadow");
        let service = RepoPerTaskCheckpointService::create(options).unwrap();

        let expected = PathBuf::from("/tmp/shadow/tasks/task-abc/checkpoints");
        assert_eq!(service.inner().checkpoints_dir, expected);
    }

    #[test]
    fn test_create_preserves_task_id() {
        let options = default_options("my-task-123", "/tmp/my-project", "/tmp/shadow");
        let service = RepoPerTaskCheckpointService::create(options).unwrap();
        assert_eq!(service.inner().task_id, "my-task-123");
    }

    #[test]
    fn test_create_preserves_workspace_dir() {
        let options = default_options("task-1", "/tmp/my-project", "/tmp/shadow");
        let service = RepoPerTaskCheckpointService::create(options).unwrap();
        assert_eq!(
            service.inner().workspace_dir,
            PathBuf::from("/tmp/my-project")
        );
    }

    #[test]
    fn test_create_rejects_protected_path() {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map(std::path::PathBuf::from)
            .unwrap();

        let options = default_options("task-1", home.to_str().unwrap(), "/tmp/shadow");
        let result = RepoPerTaskCheckpointService::create(options);
        assert!(result.is_err());
    }

    #[test]
    fn test_create_with_different_task_ids() {
        let opts1 = default_options("task-1", "/tmp/project", "/tmp/shadow");
        let opts2 = default_options("task-2", "/tmp/project", "/tmp/shadow");

        let service1 = RepoPerTaskCheckpointService::create(opts1).unwrap();
        let service2 = RepoPerTaskCheckpointService::create(opts2).unwrap();

        assert_ne!(
            service1.inner().checkpoints_dir,
            service2.inner().checkpoints_dir
        );
    }
}

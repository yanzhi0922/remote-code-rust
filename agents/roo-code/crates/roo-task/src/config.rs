//! Task configuration validation and defaults.
//!
//! Provides validation logic for [`TaskConfig`] and default configuration helpers.
//!
//! Source: `src/core/task/Task.ts` — constructor validation, `DEFAULT_CONSECUTIVE_MISTAKE_LIMIT`

use crate::types::{
    TaskConfig, TaskError, MAX_CHECKPOINT_TIMEOUT_SECONDS,
    MIN_CHECKPOINT_TIMEOUT_SECONDS,
};

/// Default maximum consecutive mistakes.
///
/// Source: `packages/types/src/provider-settings.ts` — `DEFAULT_CONSECUTIVE_MISTAKE_LIMIT`
pub const DEFAULT_MAX_MISTAKES: usize = 3;

/// Default mode.
pub const DEFAULT_MODE: &str = "code";

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Validate a task configuration.
///
/// Returns `Ok(())` if the configuration is valid, `Err` otherwise.
///
/// Source: `src/core/task/Task.ts` — constructor validation logic
pub fn validate_config(config: &TaskConfig) -> Result<(), TaskError> {
    if config.task_id.is_empty() {
        return Err(TaskError::General("task_id cannot be empty".to_string()));
    }

    if config.cwd.is_empty() {
        return Err(TaskError::General("cwd cannot be empty".to_string()));
    }

    if config.mode.is_empty() {
        return Err(TaskError::General("mode cannot be empty".to_string()));
    }

    if let Some(max) = config.max_iterations {
        if max == 0 {
            return Err(TaskError::General("max_iterations must be greater than 0".to_string()));
        }
    }

    // Validate checkpoint timeout
    // Source: `src/core/task/Task.ts` — constructor validation
    if config.checkpoint_timeout < MIN_CHECKPOINT_TIMEOUT_SECONDS
        || config.checkpoint_timeout > MAX_CHECKPOINT_TIMEOUT_SECONDS
    {
        return Err(TaskError::General(format!(
            "checkpoint_timeout must be between {} and {} seconds",
            MIN_CHECKPOINT_TIMEOUT_SECONDS, MAX_CHECKPOINT_TIMEOUT_SECONDS
        )));
    }

    // Validate startTask && !task && !images && !historyItem
    // Source: `src/core/task/Task.ts` lines 442-444
    //   `if (startTask && !task && !images && !historyItem) { throw new Error(...) }`
    if config.start_task
        && config.task_text.is_none()
        && config.images.is_empty()
        && config.history_item_id.is_none()
    {
        return Err(TaskError::General(
            "Either history_item_id or task_text/images must be provided when start_task is true"
                .to_string(),
        ));
    }

    Ok(())
}

/// Create a default task config for the given task ID.
pub fn default_config(task_id: impl Into<String>) -> TaskConfig {
    TaskConfig::new(task_id, ".").with_mode(DEFAULT_MODE)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::DEFAULT_CHECKPOINT_TIMEOUT_SECONDS;

    /// Helper: create a valid config with task_text so startTask validation passes.
    fn make_valid_config() -> TaskConfig {
        TaskConfig::new("task-1", "/tmp/work")
            .with_mode("code")
            .with_task_text("hello")
    }

    #[test]
    fn test_validate_valid_config() {
        let config = make_valid_config();
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_empty_task_id() {
        let config = TaskConfig::new("", "/tmp/work")
            .with_mode("code")
            .with_task_text("hello");
        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("task_id"));
    }

    #[test]
    fn test_validate_empty_cwd() {
        let mut config = make_valid_config();
        config.cwd = "".to_string();
        let result = validate_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_empty_mode() {
        let mut config = make_valid_config();
        config.mode = "".to_string();
        let result = validate_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_zero_max_iterations() {
        let config = TaskConfig::new("task-1", "/tmp/work")
            .with_max_iterations(0)
            .with_task_text("hello");
        let result = validate_config(&config);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_none_max_iterations() {
        let config = make_valid_config();
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_checkpoint_timeout_too_low() {
        let config = TaskConfig::new("task-1", "/tmp/work")
            .with_checkpoint_timeout(0)
            .with_task_text("hello");
        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("checkpoint_timeout"));
    }

    #[test]
    fn test_validate_checkpoint_timeout_too_high() {
        let config = TaskConfig::new("task-1", "/tmp/work")
            .with_checkpoint_timeout(601)
            .with_task_text("hello");
        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("checkpoint_timeout"));
    }

    #[test]
    fn test_validate_checkpoint_timeout_valid() {
        let config = TaskConfig::new("task-1", "/tmp/work")
            .with_checkpoint_timeout(30)
            .with_task_text("hello");
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_checkpoint_timeout_min_boundary() {
        let config = TaskConfig::new("task-1", "/tmp/work")
            .with_checkpoint_timeout(1)
            .with_task_text("hello");
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_checkpoint_timeout_max_boundary() {
        let config = TaskConfig::new("task-1", "/tmp/work")
            .with_checkpoint_timeout(600)
            .with_task_text("hello");
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_default_config() {
        let config = default_config("task-1");
        assert_eq!(config.task_id, "task-1");
        assert_eq!(config.mode, DEFAULT_MODE);
        assert_eq!(config.cwd, ".");
    }

    #[test]
    fn test_default_constants() {
        assert_eq!(DEFAULT_MAX_MISTAKES, 3);
        assert_eq!(DEFAULT_MODE, "code");
        assert_eq!(DEFAULT_CHECKPOINT_TIMEOUT_SECONDS, 30);
        assert_eq!(MAX_CHECKPOINT_TIMEOUT_SECONDS, 600);
        assert_eq!(MIN_CHECKPOINT_TIMEOUT_SECONDS, 1);
    }

    // --- New validation tests (R22-B1) ---

    #[test]
    fn test_validate_start_task_without_task_or_history() {
        // Source: TS constructor line 442-444
        // `if (startTask && !task && !images && !historyItem) { throw ... }`
        let config = TaskConfig::new("task-1", "/tmp/work")
            .with_mode("code")
            .with_start_task(true);
        let result = validate_config(&config);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("start_task"));
    }

    #[test]
    fn test_validate_start_task_false_without_task() {
        // When start_task is false, no task/images/history needed
        let config = TaskConfig::new("task-1", "/tmp/work")
            .with_mode("code")
            .with_start_task(false);
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_start_task_with_task_text() {
        let config = TaskConfig::new("task-1", "/tmp/work")
            .with_mode("code")
            .with_start_task(true)
            .with_task_text("do something");
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_start_task_with_images() {
        let config = TaskConfig::new("task-1", "/tmp/work")
            .with_mode("code")
            .with_start_task(true)
            .with_images(vec!["image.png".to_string()]);
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_validate_start_task_with_history_item() {
        let config = TaskConfig::new("task-1", "/tmp/work")
            .with_mode("code")
            .with_start_task(true)
            .with_history_item_id("hist-1");
        assert!(validate_config(&config).is_ok());
    }

    #[test]
    fn test_instance_id_auto_generated() {
        let config = TaskConfig::new("task-1", "/tmp/work");
        assert!(!config.instance_id.is_empty());
        assert_eq!(config.instance_id.len(), 8);
    }

    #[test]
    fn test_new_auto_id_generates_uuidv7() {
        let config = TaskConfig::new_auto_id("/tmp/work");
        assert!(!config.task_id.is_empty());
        // UUIDv7 format: 8-4-4-4-12 (36 chars with hyphens)
        assert_eq!(config.task_id.len(), 36);
    }
}

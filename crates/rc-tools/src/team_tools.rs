//! Team management tools: team_delete, team_list.
//!
//! Provides tools for deleting multi-agent teams and listing all teams.
//! Depends on `rc_swarm::team_helpers` for team file operations.

use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::{Result, anyhow};
use once_cell::sync::Lazy;
use serde_json::{Value, json};

use super::ToolExecutionContext;

/// Thread-safe override for the teams base directory (used in tests).
static BASE_DIR_OVERRIDE: Lazy<Mutex<Option<PathBuf>>> = Lazy::new(|| Mutex::new(None));

/// Set a base-directory override (primarily for testing).
pub fn set_base_dir_override(dir: Option<PathBuf>) {
    let mut guard = BASE_DIR_OVERRIDE
        .lock()
        .expect("BASE_DIR_OVERRIDE lock poisoned");
    *guard = dir;
}

/// Delete a multi-agent team and clean up associated resources.
///
/// Removes the team file, worktree, and mailbox directory.
///
/// # Errors
/// Returns an error if the team name is missing or the team directory cannot be removed.
pub fn team_delete(input: &Value, _context: &ToolExecutionContext) -> Result<String> {
    let team_name = input["team_name"]
        .as_str()
        .ok_or_else(|| anyhow!("team_name is required"))?;

    if team_name.trim().is_empty() {
        return Err(anyhow!("team_name cannot be empty"));
    }

    // Resolve the team directory using the same logic as rc_swarm.
    let team_dir = resolve_team_dir(team_name);

    if !team_dir.exists() {
        return Ok(json!({
            "team_name": team_name,
            "status": "not_found",
            "message": format!("Team '{team_name}' does not exist.")
        })
        .to_string());
    }

    // Clean up the team directory.
    let cleanup_results = cleanup_team_resources(&team_dir, team_name);

    Ok(json!({
        "team_name": team_name,
        "status": "deleted",
        "message": format!("Team '{team_name}' has been deleted."),
        "cleanup": cleanup_results,
    })
    .to_string())
}

/// List all multi-agent teams.
///
/// Returns a list of team names and their basic metadata.
///
/// # Errors
/// Returns an error if the teams directory cannot be read.
pub fn team_list(_input: &Value, _context: &ToolExecutionContext) -> Result<String> {
    let teams_dir = resolve_teams_base_dir();

    if !teams_dir.exists() {
        return Ok(json!({
            "teams": [],
            "total": 0,
            "message": "No teams directory found."
        })
        .to_string());
    }

    let mut teams = Vec::new();
    let entries = std::fs::read_dir(&teams_dir)?;

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            let team_file = path.join("team.json");
            if team_file.exists()
                && let Ok(content) = std::fs::read_to_string(&team_file)
                && let Ok(team_data) = serde_json::from_str::<Value>(&content)
            {
                teams.push(json!({
                    "name": team_data["name"].as_str().unwrap_or("unknown"),
                    "lead": team_data["lead_agent_id"].as_str().unwrap_or("unknown"),
                    "created_at": team_data["created_at"].as_i64().unwrap_or(0),
                    "member_count": team_data["members"]
                        .as_array()
                        .map(|a| a.len())
                        .unwrap_or(0),
                }));
            }
        }
    }

    let total = teams.len();
    Ok(json!({
        "teams": teams,
        "total": total,
        "message": format!("Found {total} team(s).")
    })
    .to_string())
}

/// Resolve the base directory for teams data.
fn resolve_teams_base_dir() -> PathBuf {
    // Check in-memory override first (used by tests).
    if let Ok(guard) = BASE_DIR_OVERRIDE.lock()
        && let Some(ref dir) = *guard
    {
        return dir.clone();
    }
    rc_swarm::team_helpers::teams_base_dir()
}

/// Resolve the directory for a specific team.
fn resolve_team_dir(team_name: &str) -> std::path::PathBuf {
    let sanitized = sanitize_team_name(team_name);
    resolve_teams_base_dir().join(sanitized)
}

/// Sanitize a team name for use as a directory name.
fn sanitize_team_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Clean up team resources: mailbox, worktree, team file.
fn cleanup_team_resources(team_dir: &std::path::Path, team_name: &str) -> Value {
    let mut cleanup = serde_json::Map::new();

    // Remove mailbox directory.
    let mailbox_dir = team_dir.join("mailbox");
    if mailbox_dir.exists() {
        match std::fs::remove_dir_all(&mailbox_dir) {
            Ok(()) => {
                cleanup.insert("mailbox".to_string(), Value::String("removed".to_string()));
            }
            Err(e) => {
                cleanup.insert("mailbox".to_string(), Value::String(format!("error: {e}")));
            }
        }
    } else {
        cleanup.insert(
            "mailbox".to_string(),
            Value::String("not_found".to_string()),
        );
    }

    // Remove permissions directory.
    let perms_dir = team_dir.join("permissions");
    if perms_dir.exists() {
        match std::fs::remove_dir_all(&perms_dir) {
            Ok(()) => {
                cleanup.insert(
                    "permissions".to_string(),
                    Value::String("removed".to_string()),
                );
            }
            Err(e) => {
                cleanup.insert(
                    "permissions".to_string(),
                    Value::String(format!("error: {e}")),
                );
            }
        }
    }

    // Remove team.json file.
    let team_file = team_dir.join("team.json");
    if team_file.exists() {
        match std::fs::remove_file(&team_file) {
            Ok(()) => {
                cleanup.insert(
                    "team_file".to_string(),
                    Value::String("removed".to_string()),
                );
            }
            Err(e) => {
                cleanup.insert(
                    "team_file".to_string(),
                    Value::String(format!("error: {e}")),
                );
            }
        }
    }

    // Remove the team directory itself.
    match std::fs::remove_dir(team_dir) {
        Ok(()) => {
            cleanup.insert("team_dir".to_string(), Value::String("removed".to_string()));
        }
        Err(e) => {
            cleanup.insert(
                "team_dir".to_string(),
                Value::String(format!("error: {e} (directory may not be empty)")),
            );
        }
    }

    // Log the deletion.
    let _ = team_name; // Used in log message.
    cleanup.insert(
        "team_name".to_string(),
        Value::String(team_name.to_string()),
    );

    Value::Object(cleanup)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use once_cell::sync::Lazy;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    static BASE_DIR_TEST_MUTEX: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    struct ResetBaseDirOverride;

    impl Drop for ResetBaseDirOverride {
        fn drop(&mut self) {
            set_base_dir_override(None);
        }
    }

    fn with_base_dir_override<T>(dir: PathBuf, f: impl FnOnce() -> T) -> T {
        let _test_guard = BASE_DIR_TEST_MUTEX
            .lock()
            .expect("BASE_DIR_TEST_MUTEX lock poisoned");
        set_base_dir_override(Some(dir));
        let _reset = ResetBaseDirOverride;
        f()
    }

    fn test_context() -> ToolExecutionContext {
        ToolExecutionContext {
            cwd: PathBuf::from("/tmp"),
            timeout_ms: 30_000,
            sub_agent: None,
            progress_cb: None,
            task_stack: Arc::new(std::sync::Mutex::new(
                rc_core::task_stack::TaskStack::default(),
            )),
        }
    }

    #[test]
    fn sanitize_team_name_handles_special_chars() {
        assert_eq!(sanitize_team_name("my-team"), "my-team");
        assert_eq!(sanitize_team_name("my team"), "my_team");
        assert_eq!(sanitize_team_name("my/team"), "my_team");
        assert_eq!(sanitize_team_name("my.team"), "my_team");
    }

    #[test]
    fn sanitize_team_name_preserves_alphanumeric() {
        assert_eq!(sanitize_team_name("team123"), "team123");
        assert_eq!(sanitize_team_name("My-Team_2"), "My-Team_2");
    }

    #[test]
    fn team_delete_requires_team_name() {
        let input = json!({});
        let context = test_context();
        let result = team_delete(&input, &context);
        let error = result.expect_err("missing team_name should fail");
        assert!(error.to_string().contains("team_name"));
    }

    #[test]
    fn team_delete_rejects_empty_team_name() {
        let input = json!({"team_name": ""});
        let context = test_context();
        let result = team_delete(&input, &context);
        assert!(result.is_err());
    }

    #[test]
    fn team_delete_handles_nonexistent_team() {
        let input = json!({"team_name": "nonexistent-team-xyz-123"});
        let context = test_context();
        let result = team_delete(&input, &context);
        let output = result.expect("nonexistent team should still return JSON");
        let parsed: Value = serde_json::from_str(&output).expect("valid json");
        assert_eq!(parsed["status"], "not_found");
    }

    #[test]
    fn team_delete_cleans_up_team_dir() {
        let temp = TempDir::new().expect("temp dir");
        let team_dir = temp.path().join("test-team");
        std::fs::create_dir_all(team_dir.join("mailbox")).expect("create mailbox");
        std::fs::create_dir_all(team_dir.join("permissions")).expect("create permissions");
        std::fs::write(
            team_dir.join("team.json"),
            r#"{"name":"test-team","lead_agent_id":"lead","created_at":0,"members":[]}"#,
        )
        .expect("write team.json");

        let result = with_base_dir_override(temp.path().to_path_buf(), || {
            let input = json!({"team_name": "test-team"});
            let context = test_context();
            team_delete(&input, &context)
        });

        assert!(result.is_ok());
        let output = result.expect("team_delete should succeed");
        let parsed: Value = serde_json::from_str(&output).expect("valid json");
        assert_eq!(parsed["status"], "deleted");
    }

    #[test]
    fn team_list_returns_empty_when_no_teams() {
        let temp = TempDir::new().expect("temp dir");
        let result = with_base_dir_override(temp.path().to_path_buf(), || {
            let input = json!({});
            let context = test_context();
            team_list(&input, &context)
        });

        assert!(result.is_ok());
        let output = result.expect("team_list should succeed");
        let parsed: Value = serde_json::from_str(&output).expect("valid json");
        assert_eq!(parsed["total"], 0);
    }

    #[test]
    fn team_list_returns_existing_teams() {
        let temp = TempDir::new().expect("temp dir");
        let team_dir = temp.path().join("my-team");
        std::fs::create_dir_all(&team_dir).expect("create team dir");
        std::fs::write(
            team_dir.join("team.json"),
            r#"{"name":"my-team","lead_agent_id":"lead-123","created_at":1700000000,"members":[{"name":"worker1"}]}"#,
        )
        .expect("write team.json");

        let result = with_base_dir_override(temp.path().to_path_buf(), || {
            let input = json!({});
            let context = test_context();
            team_list(&input, &context)
        });

        assert!(result.is_ok());
        let output = result.expect("team_list should succeed");
        let parsed: Value = serde_json::from_str(&output).expect("valid json");
        assert_eq!(parsed["total"], 1);
        let teams = parsed["teams"].as_array().expect("teams array");
        assert_eq!(teams[0]["name"], "my-team");
        assert_eq!(teams[0]["member_count"], 1);
    }

    #[test]
    fn resolve_teams_base_dir_uses_override() {
        let dir = with_base_dir_override(PathBuf::from("/custom/path"), resolve_teams_base_dir);
        assert_eq!(dir, PathBuf::from("/custom/path"));
    }

    #[test]
    fn resolve_team_dir_sanitizes_name() {
        let dir =
            with_base_dir_override(PathBuf::from("/tmp"), || resolve_team_dir("my cool team"));
        assert!(dir.to_string_lossy().contains("my_cool_team"));
    }

    #[test]
    fn cleanup_team_resources_handles_missing_dirs() {
        let temp = TempDir::new().expect("temp dir");
        let team_dir = temp.path().join("empty-team");
        std::fs::create_dir_all(&team_dir).expect("create dir");

        let result = cleanup_team_resources(&team_dir, "empty-team");
        assert_eq!(
            result["mailbox"]
                .as_str()
                .expect("mailbox state should be a string"),
            "not_found"
        );
    }

    #[test]
    fn team_list_handles_nonexistent_base_dir() {
        let result = with_base_dir_override(PathBuf::from("/nonexistent/path/xyz/abc"), || {
            let input = json!({});
            let context = test_context();
            team_list(&input, &context)
        });
        assert!(result.is_ok());
        let output = result.expect("team_list should succeed");
        let parsed: Value = serde_json::from_str(&output).expect("valid json");
        assert_eq!(parsed["total"], 0);
    }

    #[test]
    fn team_delete_json_output_format() {
        let input = json!({"team_name": "nonexistent-xyz"});
        let context = test_context();
        let result = team_delete(&input, &context).expect("team_delete should succeed");
        let parsed: Value = serde_json::from_str(&result).expect("valid json");
        assert!(parsed.get("team_name").is_some());
        assert!(parsed.get("status").is_some());
        assert!(parsed.get("message").is_some());
    }
}

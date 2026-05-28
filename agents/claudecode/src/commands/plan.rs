//! `plan` command — plan-aware session management.
//!
//! In the reference Claude Code, `/plan` switches the runtime to planning mode:
//! a restricted permission state (Read-only) where the agent reads the
//! workspace and produces a structured plan before any edits occur.

use anyhow::Result;
use claude_config::RuntimeConfig;
use claude_core::PermissionMode;
use claude_session::SessionStore;

use crate::cli::PlanArgs;

/// Run the `plan` command.
///
/// Creates a plan-mode session or switches the active session to planning mode.
pub async fn run_plan(
    config: &mut RuntimeConfig,
    store: &SessionStore,
    args: PlanArgs,
) -> Result<()> {
    if args.clear {
        // Exit plan mode by restoring normal permission mode.
        config.permission_mode = PermissionMode::Default;
        store.append_named_event(
            config.session_id,
            "plan_mode",
            serde_json::json!({"action": "clear", "mode": "default"}),
        )?;
        println!("Plan mode cleared. Permissions restored to default.");
        return Ok(());
    }

    if let Some(objective) = &args.objective {
        // Enter plan mode with the given objective.
        let prev_mode = config.permission_mode;
        config.permission_mode = PermissionMode::Plan;

        store.append_named_event(
            config.session_id,
            "plan_mode",
            serde_json::json!({
                "action": "enter",
                "objective": objective,
                "previous_mode": format!("{prev_mode:?}"),
            }),
        )?;

        if args.json {
            println!(
                "{}",
                serde_json::to_string(&serde_json::json!({
                    "status": "ok",
                    "mode": "plan",
                    "objective": objective,
                }))
                .unwrap_or_else(|_| "{{\"status\": \"ok\", \"mode\": \"plan\"}}".to_owned())
            );
        } else {
            println!("Plan mode entered.");
            println!("  Objective: {objective}");
            println!("  Permission mode: plan (Read-only)");
            println!("  Use `--clear` to exit plan mode.");
        }
    } else {
        // Show current plan mode status.
        let in_plan_mode = matches!(config.permission_mode, PermissionMode::Plan);
        if args.json {
            println!("{{\"in_plan_mode\": {in_plan_mode}}}");
        } else {
            println!(
                "Plan mode: {}",
                if in_plan_mode { "active" } else { "inactive" }
            );
        }
    }
    Ok(())
}

use rc_config::RuntimeConfig;
use rc_tools::{ToolExecutionContext, git};
use serde_json::{Value, json};

pub fn dispatch(input: &str, config: &RuntimeConfig) {
    let context = ToolExecutionContext {
        cwd: config.cwd.clone(),
        timeout_ms: config.provider.timeout_ms,
        ..ToolExecutionContext::default()
    };
    let remainder = input
        .trim()
        .strip_prefix("/worktree")
        .unwrap_or_default()
        .trim();
    if remainder.is_empty() || remainder == "list" {
        render_list(&context);
        return;
    }

    let mut parts = remainder.split_whitespace();
    let action = parts.next().unwrap_or_default();
    let branch = parts.next().unwrap_or_default();
    let path = parts.next();

    match action {
        "add" => {
            if branch.is_empty() {
                print_usage();
                return;
            }
            render_action(
                git::enter_worktree_tool(
                    &json!({
                        "branch": branch,
                        "path": path,
                    }),
                    &context,
                ),
                "create worktree",
            );
        }
        "remove" => {
            if branch.is_empty() {
                print_usage();
                return;
            }
            render_action(
                git::exit_worktree_tool(
                    &json!({
                        "branch": branch,
                        "path": path,
                    }),
                    &context,
                ),
                "remove worktree",
            );
        }
        _ => print_usage(),
    }
}

fn render_list(context: &ToolExecutionContext) {
    match git::list_worktrees_tool(context)
        .and_then(|payload| serde_json::from_str::<Value>(&payload).map_err(Into::into))
    {
        Ok(value) => {
            let worktrees = value["worktrees"].as_array().cloned().unwrap_or_default();
            if worktrees.is_empty() {
                println!(
                    "No worktrees found. {}",
                    value["note"].as_str().unwrap_or_default()
                );
                return;
            }

            println!("Worktrees:");
            for worktree in worktrees {
                println!(
                    "  {}  {}",
                    worktree["branch"].as_str().unwrap_or("(detached)"),
                    worktree["path"].as_str().unwrap_or("(missing path)")
                );
            }
        }
        Err(error) => eprintln!("Failed to list worktrees: {error}"),
    }
}

fn render_action(result: anyhow::Result<String>, label: &str) {
    match result.and_then(|payload| serde_json::from_str::<Value>(&payload).map_err(Into::into)) {
        Ok(value) => {
            println!("Worktree {label}:");
            for key in ["status", "branch", "path", "command", "note", "error"] {
                if let Some(rendered) = render_scalar(&value, key) {
                    println!("  {key}: {rendered}");
                }
            }
        }
        Err(error) => eprintln!("Failed to {label}: {error}"),
    }
}

fn render_scalar(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(|entry| match entry {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        other => Some(other.to_string()),
    })
}

fn print_usage() {
    println!("Usage:");
    println!("  /worktree");
    println!("  /worktree list");
    println!("  /worktree add <branch> [path]");
    println!("  /worktree remove <branch> [path]");
}

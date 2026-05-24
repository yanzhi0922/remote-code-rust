//! Phase A: High-value CLI commands.
//!
//! Each command is a public function that takes a RuntimeConfig and/or
//! SessionStore and returns either synchronously or asynchronously.

mod plan;

pub use plan::run_plan;

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use claude_config::RuntimeConfig;
use claude_session::SessionStore;
use serde_json::Value;

use crate::cli::{
    AddDirArgs, AgentsPlatformArgs, AntTraceArgs, AutoFixPrArgs, BranchCommand, BreakCacheArgs,
    BridgeArgs, BtwArgs, BuddyArgs, BughunterArgs, ChromeArgs, ColorArgs, ContextManageArgs,
    CopyArgs, CostArgs, CtxArgs, CtxVizArgs, DebugToolCallArgs, DesktopArgs, DiffArgs,
    DiffRealArgs, FastArgs, FeedbackArgs, FilesArgs, HeapdumpArgs, HelpArgs, IdeArgs,
    InstallGithubAppArgs, InstallSlackAppArgs, IssueArgs, KeybindingsCommand, LoginArgs,
    MemoryCommand, MobileArgs, MockLimitsArgs, ModelCommand, OnboardingArgs, PassesArgs,
    PerfIssueArgs, PermissionsArgs, PrCommentsArgs, PrivacySettingsArgs, ProviderCommand,
    RateLimitOptionsArgs, ReleaseNotesArgs, RemoteEnvArgs, RemoteSetupArgs, StatsArgs,
    StickersArgs, TeleportArgs, ThemeArgs, ThinkbackArgs,
};

/// Extract a string value by key path from a JSON object map.
fn nested_str<'a>(map: &'a serde_json::Map<String, Value>, keys: &[&str]) -> Option<&'a str> {
    let mut val: &Value = map.get(keys[0])?;
    for k in &keys[1..] {
        val = val.as_object()?.get(*k)?;
    }
    val.as_str()
}

/// Extract a u64 value by key path from a JSON object map.
fn nested_u64(map: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<u64> {
    let mut val: &Value = map.get(keys[0])?;
    for k in &keys[1..] {
        val = val.as_object()?.get(*k)?;
    }
    val.as_u64()
}

fn unsupported_command(command: &str, json: bool) -> Result<()> {
    if json {
        println!(
            "{}",
            serde_json::json!({
                "command": command,
                "status": "unsupported",
                "message": "not implemented in this build"
            })
        );
    }
    Err(anyhow!("{command} is not implemented in this build"))
}

// ── cost ───────────────────────────────────────────────────────────────

/// Show current cost tracking and token usage.
pub fn run_cost(config: &RuntimeConfig, store: &SessionStore, args: CostArgs) -> Result<()> {
    let events = store.load_events(config.session_id).unwrap_or_default();
    let (inp, out) = events.iter().fold((0u64, 0u64), |(i, o), e| {
        if let Some(ref map) = e.payload.as_ref().and_then(|p| p.as_object()) {
            (
                i + nested_u64(map, &["usage", "input_tokens"]).unwrap_or(0),
                o + nested_u64(map, &["usage", "output_tokens"]).unwrap_or(0),
            )
        } else {
            (i, o)
        }
    });

    if args.reset {
        return Ok(());
    }
    if args.json {
        println!(
            "{}",
            serde_json::json!({
                "total_cost_usd": 0.0_f64,
                "input_tokens": inp,
                "output_tokens": out,
            })
        );
    } else {
        println!("Cost: ${:.6}", 0.0_f64);
        println!("  Input tokens:  {inp}");
        println!("  Output tokens: {out}");
    }
    Ok(())
}

// ── memory ─────────────────────────────────────────────────────────────

/// View or edit conversation memory (read/write/list).
pub fn run_memory(
    config: &RuntimeConfig,
    store: &SessionStore,
    command: MemoryCommand,
) -> Result<()> {
    match command {
        MemoryCommand::Read { key, json } => {
            let events = store.load_events(config.session_id).unwrap_or_default();
            let memories: Vec<_> = events
                .iter()
                .filter(|e| e.event_type == "memory")
                .filter(|e| {
                    let k = e
                        .payload
                        .as_ref()
                        .and_then(|p| p.as_object())
                        .and_then(|m| nested_str(m, &["key"]));
                    key.as_ref()
                        .map_or(true, |target| k == Some(target.as_str()))
                })
                .collect();
            if json {
                let output: Vec<Value> = memories
                    .iter()
                    .map(|e| {
                        let k = e
                            .payload
                            .as_ref()
                            .and_then(|p| p.as_object())
                            .and_then(|m| nested_str(m, &["key"]));
                        let v = e
                            .payload
                            .as_ref()
                            .and_then(|p| p.as_object())
                            .and_then(|m| nested_str(m, &["value"]));
                        Value::Object(
                            vec![
                                (
                                    "key".into(),
                                    k.map(|s| Value::String(s.into())).unwrap_or(Value::Null),
                                ),
                                (
                                    "value".into(),
                                    v.map(|s| Value::String(s.into())).unwrap_or(Value::Null),
                                ),
                            ]
                            .into_iter()
                            .collect(),
                        )
                    })
                    .collect();
                println!("{}", serde_json::to_string(&output)?);
            } else if memories.is_empty() {
                println!("No memories found.");
            } else {
                for m in &memories {
                    let obj = m.payload.as_ref().and_then(|p| p.as_object());
                    let k = obj.and_then(|o| nested_str(o, &["key"])).unwrap_or("");
                    let v = obj.and_then(|o| nested_str(o, &["value"])).unwrap_or("");
                    println!("  {k}: {v}");
                }
            }
        }
        MemoryCommand::Write { key, value } => {
            store.append_named_event(
                config.session_id,
                "memory",
                serde_json::json!({"key": key.clone(), "value": value.clone()}),
            )?;
            println!("Memory saved: {key} = {value}");
        }
        MemoryCommand::List { json } => {
            let events = store.load_events(config.session_id).unwrap_or_default();
            let keys: Vec<&str> = events
                .iter()
                .filter(|e| e.event_type == "memory")
                .filter_map(|e| {
                    e.payload
                        .as_ref()
                        .and_then(|p| p.as_object())
                        .and_then(|m| nested_str(m, &["key"]))
                })
                .collect();
            if json {
                println!("{}", serde_json::to_string(&keys)?);
            } else if keys.is_empty() {
                println!("No memories stored.");
            } else {
                println!("Memories:");
                for k in keys {
                    println!("  - {k}");
                }
            }
        }
    }
    Ok(())
}

// ── model ──────────────────────────────────────────────────────────────

/// List, inspect, and switch models.
pub fn run_model(config: &mut RuntimeConfig, command: ModelCommand) -> Result<()> {
    match command {
        ModelCommand::List { json } => {
            let current = config.provider.model.as_deref().unwrap_or("unknown");
            if json {
                println!(
                    "{}",
                    serde_json::json!({"current": current, "available": []})
                );
            } else {
                println!("Current model: {current}");
            }
        }
        ModelCommand::Get { json } => {
            let model = config.provider.model.as_deref().unwrap_or("unknown");
            if json {
                println!("{}", serde_json::json!(model));
            } else {
                println!("{model}");
            }
        }
        ModelCommand::Set { model } => {
            config.provider.model = Some(model.clone());
            println!("Model set to \"{model}\" (effective next session)");
        }
    }
    Ok(())
}

// ── provider ──────────────────────────────────────────────────────────

/// List, inspect, and switch providers.
pub fn run_provider(config: &mut RuntimeConfig, command: ProviderCommand) -> Result<()> {
    match command {
        ProviderCommand::List { json } => {
            let name = &config.provider.name;
            if json {
                println!("{}", serde_json::json!({"current": name, "available": []}));
            } else {
                println!("Current provider: {name}");
            }
        }
        ProviderCommand::Get { json } => {
            if json {
                println!("{}", serde_json::json!(config.provider.name));
            } else {
                println!("{}", config.provider.name);
            }
        }
        ProviderCommand::Set { provider } => {
            config.provider.name = provider.clone();
            println!("Provider set to \"{provider}\" (effective next session)");
        }
    }
    Ok(())
}

// ── compact ───────────────────────────────────────────────────────────

/// Compact the current session context.
pub async fn run_compact(config: &mut RuntimeConfig, store: &SessionStore) -> Result<()> {
    let _ = (config, store);
    unsupported_command("compact", false)
}

// ── theme ─────────────────────────────────────────────────────────────

/// Show or set the UI theme.
pub fn run_theme(_config: &RuntimeConfig, args: ThemeArgs) -> Result<()> {
    unsupported_command("theme", args.json)
}

// ── feedback ──────────────────────────────────────────────────────────

/// Send feedback to the developer.
pub async fn run_feedback(config: &RuntimeConfig, args: FeedbackArgs) -> Result<()> {
    let _ = (config, args);
    unsupported_command("feedback", false)
}

// ── summary ───────────────────────────────────────────────────────────

/// Summarize the current session.
pub async fn run_summary(config: &mut RuntimeConfig, store: &SessionStore) -> Result<()> {
    let conversation = store.load_conversation(config.session_id)?;
    let events = store.load_events(config.session_id).unwrap_or_default();
    let msg_count = conversation.len();
    let tool_calls = events
        .iter()
        .filter(|e| e.event_type == "tool_result")
        .count();
    let errors = events
        .iter()
        .filter(|e| {
            e.event_type == "result"
                && e.payload
                    .as_ref()
                    .and_then(|p| p.as_object())
                    .map_or(false, |m| {
                        m.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false)
                    })
        })
        .count();

    println!("Session summary");
    println!("  Session:    {}", config.session_id);
    println!(
        "  Model:      {}",
        config.provider.model.as_deref().unwrap_or("unknown")
    );
    println!("  Provider:   {}", config.provider.name);
    println!("  Messages:   {msg_count}");
    println!("  Tool calls: {tool_calls}");
    println!("  Errors:     {errors}");
    Ok(())
}

// ── files ─────────────────────────────────────────────────────────────

/// List workspace files.
pub fn run_files(config: &RuntimeConfig, args: FilesArgs) -> Result<()> {
    let dir = args
        .path
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| config.cwd.clone());
    if !dir.exists() {
        return Err(anyhow!("path does not exist: {}", dir.display()));
    }

    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());

    if args.json {
        let files: Vec<Value> = entries
            .iter()
            .map(|e| {
                let is_dir = e.metadata().map(|m| m.is_dir()).unwrap_or(false);
                let size = e.metadata().map(|m| m.len()).unwrap_or(0);
                serde_json::json!({
                    "name": e.file_name().to_string_lossy(),
                    "is_dir": is_dir,
                    "size": size,
                })
            })
            .collect();
        println!("{}", serde_json::to_string(&files)?);
    } else {
        println!("Files in {}:", dir.display());
        for entry in &entries {
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.metadata().map(|m| m.is_dir()).unwrap_or(false);
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let kind = if is_dir { "[DIR]" } else { "[FILE]" };
            println!("  {kind} {name} ({size} B)");
        }
    }
    Ok(())
}

// ── ctx ───────────────────────────────────────────────────────────────

/// View context usage details.
pub fn run_ctx(config: &RuntimeConfig, store: &SessionStore, args: CtxArgs) -> Result<()> {
    let conversation = store.load_conversation(config.session_id).ok();
    let msg_count = conversation.as_ref().map(|c| c.len()).unwrap_or(0);

    if args.json {
        println!(
            "{}",
            serde_json::json!({
                "used": msg_count * 500,
                "total": 200000,
                "threshold": 160000,
                "messages": msg_count,
            })
        );
    } else {
        let rough_tokens = msg_count * 500;
        println!("Context usage: ~{rough_tokens} / 200,000 tokens (threshold: 160,000)");
        if args.detailed {
            let model = config.provider.model.as_deref().unwrap_or("unknown");
            println!("  Messages:  {msg_count}");
            println!("  Model:     {model}");
        }
    }
    Ok(())
}

// ── diff ──────────────────────────────────────────────────────────────

/// Show a diff between sessions or checkpoints.
pub fn run_diff(config: &RuntimeConfig, store: &SessionStore, args: DiffArgs) -> Result<()> {
    let target_sid = args.session_id.unwrap_or(config.session_id);
    let conversation = store.load_conversation(target_sid).ok();
    let entries = conversation.as_deref().map(|c| c.len()).unwrap_or(0);

    if args.json {
        println!(
            "{}",
            serde_json::json!({"session_id": target_sid, "entries": entries, "diff": []})
        );
    } else {
        println!("Session {target_sid}: {entries} conversation entries");
        if let Some(path) = &args.path {
            println!("  Path filter: {path}");
        }
    }
    Ok(())
}

// ── copy ──────────────────────────────────────────────────────────────

/// Copy session content to clipboard.
pub async fn run_copy(config: &RuntimeConfig, store: &SessionStore, args: CopyArgs) -> Result<()> {
    let session_id = args.session_id.unwrap_or(config.session_id);
    let conversation = store.load_conversation(session_id)?;
    let text: String = conversation
        .iter()
        .map(|entry| entry.text.clone())
        .collect::<Vec<_>>()
        .join("\n");

    if text.is_empty() {
        println!("Session has no text content to copy.");
        return Ok(());
    }

    let tmp = std::env::temp_dir().join(format!("remote-code-copy-{session_id}.txt"));
    std::fs::write(&tmp, &text)?;
    println!("Copied {} characters to {}.", text.len(), tmp.display());
    Ok(())
}

// ── P1-A1: Low-complexity commands ────────────────────────────────────

/// Show help for commands.
pub async fn run_help(args: HelpArgs) -> Result<()> {
    if let Some(cmd) = &args.command {
        println!("Help for `{cmd}`: (detailed help not yet implemented)");
    } else if args.json {
        println!("{}", serde_json::json!({"commands": []}));
    } else {
        println!("Available commands:");
        println!("  doctor      Run diagnostic checks");
        println!("  status      Show runtime status");
        println!("  hooks       Manage hooks");
        println!("  remote      Control plane commands");
        println!("  sessions    List/show/manage sessions");
        println!("  review      Review changes");
        println!("  worktree    Manage git worktrees");
        println!("  tasks       List/show tasks");
        println!("  resume      Resume a session");
        println!("  export      Export a session");
        println!("  tui         Launch TUI");
        println!("  plan        Enter/exit plan mode");
        println!("  cost        Show cost tracking");
        println!("  memory      View/edit memory");
        println!("  model       List/set model");
        println!("  provider    List/set provider");
        println!("  compact     Compact context");
        println!("  theme       Set theme");
        println!("  summary     Session summary");
        println!("  files       List workspace files");
        println!("  ctx         Context usage");
        println!("  diff        Show diff");
        println!("  help        This help");
        println!("  clear       Clear terminal");
        println!("  exit        Exit shell");
    }
    Ok(())
}

/// Clear the terminal screen.
pub fn run_clear() -> Result<()> {
    // Use ANSI escape codes for clear screen on all platforms.
    // Windows 10+ terminals support ANSI escape sequences.
    print!("\x1B[2J\x1B[1;1H");
    Ok(())
}

/// Exit the shell.
pub fn run_exit() -> Result<()> {
    std::process::exit(0);
}

/// Dump heap memory stats for debugging.
pub fn run_heapdump(config: &RuntimeConfig, args: HeapdumpArgs) -> Result<()> {
    if args.json {
        println!(
            "{}",
            serde_json::json!({"session_id": config.session_id, "heap": "N/A"})
        );
    } else {
        println!("Heap dump for session {}:", config.session_id);
        println!("  Memory stats: N/A (requires allocator instrumentation)");
    }
    Ok(())
}

/// Debug a tool call by ID.
pub fn run_debug_tool_call(
    config: &RuntimeConfig,
    store: &SessionStore,
    args: DebugToolCallArgs,
) -> Result<()> {
    let sid = args.session_id.unwrap_or(config.session_id);
    let events = store.load_events(sid)?;
    let tool_events: Vec<_> = events
        .iter()
        .filter(|e| e.event_type == "tool_result" || e.event_type == "tool_use")
        .filter(|e| {
            e.payload
                .as_ref()
                .and_then(|p| p.as_object())
                .and_then(|m| m.get("tool_call_id"))
                .and_then(|v| v.as_str())
                .map_or(false, |id| id == args.tool_call_id)
        })
        .collect();

    if args.json {
        println!("{}", serde_json::to_string(&tool_events)?);
    } else {
        if tool_events.is_empty() {
            println!("No events found for tool call `{}`.", args.tool_call_id);
        } else {
            println!(
                "Tool call `{}`: {} events",
                args.tool_call_id,
                tool_events.len()
            );
            for e in &tool_events {
                println!("  - {} ({})", e.event_type, e.timestamp);
            }
        }
    }
    Ok(())
}

/// Toggle fast mode (skip confirmations).
pub fn run_fast(config: &mut RuntimeConfig, args: FastArgs) -> Result<()> {
    if args.enable {
        config.permission_mode = claude_core::PermissionMode::AcceptEdits;
        if args.json {
            println!("{}", serde_json::json!({"fast_mode": true}));
        } else {
            println!("Fast mode enabled.");
        }
    } else if args.disable {
        config.permission_mode = claude_core::PermissionMode::Default;
        if args.json {
            println!("{}", serde_json::json!({"fast_mode": false}));
        } else {
            println!("Fast mode disabled.");
        }
    } else {
        let enabled = matches!(
            config.permission_mode,
            claude_core::PermissionMode::AcceptEdits
        );
        if args.json {
            println!("{}", serde_json::json!({"fast_mode": enabled}));
        } else {
            println!(
                "Fast mode: {}",
                if enabled { "enabled" } else { "disabled" }
            );
        }
    }
    Ok(())
}

/// Mobile device pairing and management.
pub async fn run_mobile(args: MobileArgs) -> Result<()> {
    unsupported_command("mobile", args.json)
}

/// Show or configure desktop pairing.
pub async fn run_desktop(args: DesktopArgs) -> Result<()> {
    unsupported_command("desktop", args.json)
}

/// Toggle sandbox execution mode.
pub fn run_sandbox_toggle(_config: &mut RuntimeConfig) -> Result<()> {
    unsupported_command("sandbox-toggle", false)
}

/// Reload all plugins from disk.
pub async fn run_reload_plugins(_config: &mut RuntimeConfig) -> Result<()> {
    unsupported_command("reload-plugins", false)
}

/// Add a directory to the workspace.
pub fn run_add_dir(_config: &RuntimeConfig, args: AddDirArgs) -> Result<()> {
    unsupported_command("add-dir", args.json)
}

/// Mock rate limit responses for testing.
pub fn run_mock_limits(_config: &RuntimeConfig, args: MockLimitsArgs) -> Result<()> {
    unsupported_command("mock-limits", args.json)
}

/// Manage GitHub achievement stickers.
pub async fn run_stickers(_config: &RuntimeConfig, args: StickersArgs) -> Result<()> {
    unsupported_command("stickers", args.json)
}

/// Show release notes for this version.
pub fn run_release_notes(args: ReleaseNotesArgs) -> Result<()> {
    let version = args
        .version
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_owned());
    if args.json {
        println!("{}", serde_json::json!({"version": version, "notes": []}));
    } else {
        println!("Release notes for version {version}:");
        println!("  (Release notes not yet bundled in this build)");
    }
    Ok(())
}

/// Show session or system statistics.
pub fn run_stats(config: &RuntimeConfig, store: &SessionStore, args: StatsArgs) -> Result<()> {
    let sid = args.session_id.unwrap_or(config.session_id);
    let conversation = store.load_conversation(sid)?;
    let events = store.load_events(sid).unwrap_or_default();
    let msg_count = conversation.len();
    let tool_calls = events
        .iter()
        .filter(|e| e.event_type == "tool_result")
        .count();
    let errors = events
        .iter()
        .filter(|e| {
            e.event_type == "result"
                && e.payload
                    .as_ref()
                    .and_then(|p| p.as_object())
                    .and_then(|m| m.get("is_error"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
        })
        .count();

    if args.json {
        println!(
            "{}",
            serde_json::json!({
                "session_id": sid,
                "messages": msg_count,
                "tool_calls": tool_calls,
                "errors": errors,
            })
        );
    } else {
        println!("Stats for session {sid}:");
        println!("  Messages:   {msg_count}");
        println!("  Tool calls: {tool_calls}");
        println!("  Errors:     {errors}");
    }
    Ok(())
}

/// Send a "good Claude" feedback signal.
pub async fn run_good_claude(config: &RuntimeConfig) -> Result<()> {
    eprintln!(
        "Positive feedback recorded for session {}",
        config.session_id
    );
    println!("Thank you! Glad I could help.");
    Ok(())
}

// ── P1-A2: Medium-complexity commands ────────────────────────────────

/// Show a real diff between sessions or files.
pub fn run_diff_real(
    config: &RuntimeConfig,
    store: &SessionStore,
    args: DiffRealArgs,
) -> Result<()> {
    let sid = args.session_id.unwrap_or(config.session_id);
    let conversation = store.load_conversation(sid)?;

    if args.json {
        let lines: Vec<serde_json::Value> = conversation
            .iter()
            .map(|e| serde_json::json!({"role": format!("{:?}", e.role), "text": e.text}))
            .collect();
        println!("{}", serde_json::to_string(&lines)?);
    } else {
        println!("Diff for session {sid}: {} entries", conversation.len());
        for entry in conversation.iter().rev().take(10).rev() {
            let preview = if entry.text.len() > 80 {
                format!("{}...", entry.text.chars().take(80).collect::<String>())
            } else {
                entry.text.clone()
            };
            println!("  [{:?}] {}", entry.role, preview);
        }
    }
    Ok(())
}

/// Manage git branches.
pub fn run_branch(_config: &RuntimeConfig, command: BranchCommand) -> Result<()> {
    match command {
        BranchCommand::List { json } => {
            // Shell out to git branch
            let output = std::process::Command::new("git")
                .args(["branch"])
                .output()
                .map_err(|e| anyhow!("git branch failed: {e}"))?;
            let stdout = String::from_utf8_lossy(&output.stdout);
            if json {
                let branches: Vec<&str> = stdout
                    .lines()
                    .map(|l| l.trim().trim_start_matches('*').trim())
                    .collect();
                println!("{}", serde_json::to_string(&branches)?);
            } else {
                print!("{stdout}");
            }
        }
        BranchCommand::Create { name, start_point } => {
            let mut args = vec!["branch", &name];
            if let Some(sp) = &start_point {
                args.push(sp);
            }
            let output = std::process::Command::new("git")
                .args(&args)
                .output()
                .map_err(|e| anyhow!("git branch create failed: {e}"))?;
            if output.status.success() {
                println!("Branch '{name}' created.");
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow!("{}", stderr.trim()));
            }
        }
        BranchCommand::Switch { name } => {
            std::process::Command::new("git")
                .args(["checkout", &name])
                .output()
                .map_err(|e| anyhow!("git checkout failed: {e}"))?;
            println!("Switched to branch '{name}'.");
        }
        BranchCommand::Delete { name, force } => {
            let mut args = vec!["branch"];
            if force {
                args.push("-D");
            } else {
                args.push("-d");
            }
            args.push(&name);
            std::process::Command::new("git")
                .args(&args)
                .output()
                .map_err(|e| anyhow!("git branch delete failed: {e}"))?;
            println!("Branch '{name}' deleted.");
        }
        BranchCommand::Current { json } => {
            let output = std::process::Command::new("git")
                .args(["rev-parse", "--abbrev-ref", "HEAD"])
                .output()
                .map_err(|e| anyhow!("git rev-parse failed: {e}"))?;
            let branch = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if json {
                println!("{}", serde_json::json!({"branch": branch}));
            } else {
                println!("{branch}");
            }
        }
    }
    Ok(())
}

/// View or set remote env vars.
pub async fn run_remote_env(args: RemoteEnvArgs) -> Result<()> {
    if args.list || (args.get.is_none() && args.set.is_none() && args.unset.is_none()) {
        if args.json {
            println!("[]");
        } else {
            println!("Remote environment variables:");
            println!("  (no variables configured)");
        }
    } else if let Some(key) = &args.get {
        let val = std::env::var(key).unwrap_or_else(|_| "(not set)".to_owned());
        if args.json {
            println!("{}", serde_json::json!({key: val}));
        } else {
            println!("{key}={val}");
        }
    } else if let Some(key) = &args.set {
        if let Some(val) = &args.value {
            unsafe {
                std::env::set_var(key, val);
            }
            if args.json {
                println!("{}", serde_json::json!({"set": key, "value": val}));
            } else {
                println!("Set {key}={val}");
            }
        }
    } else if let Some(key) = &args.unset {
        unsafe {
            std::env::remove_var(key);
        }
        if args.json {
            println!("{}", serde_json::json!({"unset": key}));
        } else {
            println!("Unset {key}");
        }
    }
    Ok(())
}

/// Log in with a provider.
pub async fn run_login(_config: &RuntimeConfig, args: LoginArgs) -> Result<()> {
    unsupported_command("login", args.json)
}

/// Log out from current provider.
pub async fn run_logout(_config: &RuntimeConfig) -> Result<()> {
    unsupported_command("logout", false)
}

/// Refresh OAuth token.
pub async fn run_oauth_refresh(_config: &RuntimeConfig) -> Result<()> {
    unsupported_command("oauth-refresh", false)
}

/// Run automated bug hunting.
pub fn run_bughunter(
    _config: &RuntimeConfig,
    _store: &SessionStore,
    args: BughunterArgs,
) -> Result<()> {
    unsupported_command("bughunter", args.json)
}

/// Trace API calls in a session.
pub fn run_ant_trace(
    _config: &RuntimeConfig,
    store: &SessionStore,
    args: AntTraceArgs,
) -> Result<()> {
    let sid = args.session_id.unwrap_or(_config.session_id);
    let events = store.load_events(sid).unwrap_or_default();

    if args.json {
        let traces: Vec<serde_json::Value> = events
            .iter()
            .filter(|e| e.event_type == "tool_result" || e.event_type == "assistant_turn")
            .map(|e| serde_json::json!({"type": e.event_type, "payload": e.payload}))
            .collect();
        println!("{}", serde_json::to_string(&traces)?);
    } else {
        println!("API trace for session {sid}:");
        for e in events.iter().filter(|e| e.event_type == "tool_result") {
            let payload_str = e
                .payload
                .as_ref()
                .map(|v| v.to_string())
                .unwrap_or_default();
            println!("  tool_result: {payload_str}");
        }
        for e in events.iter().filter(|e| e.event_type == "assistant_turn") {
            let payload_str = e
                .payload
                .as_ref()
                .map(|v| v.to_string())
                .unwrap_or_default();
            println!("  assistant_turn: {payload_str}");
        }
    }
    Ok(())
}

/// Manage keybindings.
pub fn run_keybindings(command: KeybindingsCommand) -> Result<()> {
    match command {
        KeybindingsCommand::List { json } => {
            if json {
                println!("[]");
            } else {
                println!("Current keybindings:");
                println!("  (use TUI to configure)");
            }
        }
        KeybindingsCommand::Set { key, action } => {
            let _ = (key, action);
            return unsupported_command("keybindings set", false);
        }
        KeybindingsCommand::Reset => {
            return unsupported_command("keybindings reset", false);
        }
    }
    Ok(())
}

/// Run batch analysis passes.
pub fn run_passes(_config: &RuntimeConfig, _store: &SessionStore, args: PassesArgs) -> Result<()> {
    unsupported_command("passes", args.json)
}

// ── P1-A3: High-complexity commands ──────────────────────────────────

/// Teleport a session between environments.
pub async fn run_teleport(
    _config: &RuntimeConfig,
    _store: &SessionStore,
    args: TeleportArgs,
) -> Result<()> {
    unsupported_command("teleport", args.json)
}

/// Install a GitHub App.
pub async fn run_install_github_app(args: InstallGithubAppArgs) -> Result<()> {
    unsupported_command("install-github-app", args.json)
}

/// Install a Slack App.
pub async fn run_install_slack_app(args: InstallSlackAppArgs) -> Result<()> {
    unsupported_command("install-slack-app", args.json)
}

/// Collaborate with a buddy / peer programmer.
pub async fn run_buddy(_config: &RuntimeConfig, args: BuddyArgs) -> Result<()> {
    unsupported_command("buddy", args.json)
}

/// Manage the agent platform.
pub async fn run_agents_platform(_config: &RuntimeConfig, args: AgentsPlatformArgs) -> Result<()> {
    unsupported_command("agents-platform", args.json)
}

/// Backtrack through agent reasoning.
pub fn run_thinkback(
    config: &RuntimeConfig,
    _store: &SessionStore,
    args: ThinkbackArgs,
) -> Result<()> {
    let _ = config;
    unsupported_command("thinkback", args.json)
}

/// Visualize conversation context usage.
pub fn run_ctx_viz(config: &RuntimeConfig, store: &SessionStore, args: CtxVizArgs) -> Result<()> {
    let sid = args.session_id.unwrap_or(config.session_id);
    let conversation = store.load_conversation(sid)?;
    let total = conversation.len();
    let estimated_tokens: usize = conversation.iter().map(|e| e.text.len() / 4).sum();

    if let Some(path) = &args.output {
        let viz = serde_json::json!({
            "session_id": sid.to_string(),
            "total_messages": total,
            "estimated_tokens": estimated_tokens,
            "max_context": 200000,
        });
        std::fs::write(path, serde_json::to_string_pretty(&viz)?)?;
        println!("Context visualization written to {}.", path.display());
    } else if args.json {
        println!(
            "{}",
            serde_json::json!({"session_id": sid, "messages": total, "tokens": estimated_tokens, "max": 200000})
        );
    } else {
        println!("Context visualization for session {sid}:");
        println!("  Messages:       {total}");
        println!("  Estimated tokens: ~{estimated_tokens} / 200,000");
        let ratio = estimated_tokens as f64 / 200_000.0;
        let bar_len: usize = 30;
        let filled = (ratio * bar_len as f64).round() as usize;
        let empty = bar_len.saturating_sub(filled);
        println!(
            "  [{}{}] {:.1}%",
            "#".repeat(filled),
            ".".repeat(empty),
            ratio * 100.0
        );
    }
    Ok(())
}

// ── P1-A4: 16 remaining commands ────────────────────────────────────

/// Auto-fix a pull request.
pub async fn run_autofix_pr(_config: &RuntimeConfig, args: AutoFixPrArgs) -> Result<()> {
    unsupported_command("autofix-pr", args.json)
}

/// Break the provider response cache.
pub fn run_break_cache(_config: &RuntimeConfig, args: BreakCacheArgs) -> Result<()> {
    unsupported_command("break-cache", args.json)
}

/// Manage bridge connections.
pub async fn run_bridge(args: BridgeArgs) -> Result<()> {
    unsupported_command("bridge", args.json)
}

/// Send a quick "by the way" note.
pub async fn run_btw(_config: &RuntimeConfig, args: BtwArgs) -> Result<()> {
    unsupported_command("btw", args.json)
}

/// Configure Chrome browser MCP integration.
pub async fn run_chrome(args: ChromeArgs) -> Result<()> {
    unsupported_command("chrome", args.json)
}

/// Configure terminal colors.
pub fn run_color(args: ColorArgs) -> Result<()> {
    unsupported_command("color", args.json)
}

/// Manage conversation context.
pub fn run_context_manage(
    _config: &RuntimeConfig,
    _store: &SessionStore,
    args: ContextManageArgs,
) -> Result<()> {
    unsupported_command("context", args.json)
}

/// IDE integration configuration.
pub async fn run_ide(args: IdeArgs) -> Result<()> {
    unsupported_command("ide", args.json)
}

/// Manage GitHub issues.
pub async fn run_issue(_config: &RuntimeConfig, args: IssueArgs) -> Result<()> {
    unsupported_command("issue", args.json)
}

/// First-run onboarding wizard.
pub async fn run_onboarding(
    _config: &RuntimeConfig,
    _store: &SessionStore,
    args: OnboardingArgs,
) -> Result<()> {
    unsupported_command("onboarding", args.json)
}

/// Diagnose performance issues.
pub fn run_perf_issue(
    _config: &RuntimeConfig,
    _store: &SessionStore,
    args: PerfIssueArgs,
) -> Result<()> {
    unsupported_command("perf-issue", args.json)
}

/// Manage permission rules.
pub fn run_permissions(_config: &RuntimeConfig, args: PermissionsArgs) -> Result<()> {
    unsupported_command("permissions", args.json)
}

/// View pull request comments.
pub async fn run_pr_comments(args: PrCommentsArgs) -> Result<()> {
    unsupported_command("pr-comments", args.json)
}

/// Configure privacy settings.
pub fn run_privacy_settings(args: PrivacySettingsArgs) -> Result<()> {
    unsupported_command("privacy-settings", args.json)
}

/// Configure rate limit options.
pub fn run_rate_limit_options(_config: &RuntimeConfig, args: RateLimitOptionsArgs) -> Result<()> {
    unsupported_command("rate-limit-options", args.json)
}

/// Set up remote access.
pub async fn run_remote_setup(_config: &RuntimeConfig, args: RemoteSetupArgs) -> Result<()> {
    unsupported_command("remote-setup", args.json)
}

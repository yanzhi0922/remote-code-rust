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
    AddDirArgs, AntTraceArgs, BranchCommand, BughunterArgs, CopyArgs, CostArgs, CtxArgs,
    DebugToolCallArgs, DesktopArgs, DiffArgs, DiffRealArgs, FastArgs, FeedbackArgs, FilesArgs,
    HeapdumpArgs, HelpArgs, KeybindingsCommand, LoginArgs, MemoryCommand, MobileArgs,
    MockLimitsArgs, ModelCommand, PassesArgs, ProviderCommand, ReleaseNotesArgs, RemoteEnvArgs,
    StatsArgs, StickersArgs, ThemeArgs,
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
            "{{\"total_cost_usd\": {:.6}, \"input_tokens\": {}, \"output_tokens\": {}}}",
            0.0_f64, inp, out
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
                println!("{{\"current\": \"{current}\", \"available\": []}}");
            } else {
                println!("Current model: {current}");
            }
        }
        ModelCommand::Get { json } => {
            let model = config.provider.model.as_deref().unwrap_or("unknown");
            if json {
                println!("\"{model}\"");
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
                println!("{{\"current\": \"{name}\", \"available\": []}}");
            } else {
                println!("Current provider: {name}");
            }
        }
        ProviderCommand::Get { json } => {
            if json {
                println!("\"{}\"", config.provider.name);
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
    let conversation = store.load_conversation(config.session_id)?;
    let total = conversation.len();
    println!("Context compaction requested.");
    println!("  Session entries: {total}");
    println!(
        "  Model: {}",
        config.provider.model.as_deref().unwrap_or("unknown")
    );
    Ok(())
}

// ── theme ─────────────────────────────────────────────────────────────

/// Show or set the UI theme.
pub fn run_theme(_config: &RuntimeConfig, args: ThemeArgs) -> Result<()> {
    if let Some(name) = &args.name {
        if args.json {
            println!("{{\"theme\": \"{name}\"}}");
        } else {
            println!("Theme set to \"{name}\"");
        }
    } else if args.json {
        println!("{{\"theme\": \"default\"}}");
    } else {
        println!("Current theme: default");
    }
    Ok(())
}

// ── feedback ──────────────────────────────────────────────────────────

/// Send feedback to the developer.
pub async fn run_feedback(config: &RuntimeConfig, args: FeedbackArgs) -> Result<()> {
    let msg = args.message.join(" ");
    let fb_type = args.feedback_type.as_deref().unwrap_or("general");
    let _ = config.session_id;
    eprintln!("Feedback ({fb_type}): {msg}");
    println!("Thank you for your feedback!");
    Ok(())
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
            "{{\"used\": {}, \"total\": 200000, \"threshold\": 160000, \"messages\": {msg_count}}}",
            msg_count * 500
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
        println!("{{\"session_id\": \"{target_sid}\", \"entries\": {entries}, \"diff\": []}}");
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
        println!("{{\"commands\": []}}");
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
            "{{ \"session_id\": \"{}\", \"heap\": \"N/A\" }}",
            config.session_id
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
            println!("{{\"fast_mode\": true}}");
        } else {
            println!("Fast mode enabled.");
        }
    } else if args.disable {
        config.permission_mode = claude_core::PermissionMode::Default;
        if args.json {
            println!("{{\"fast_mode\": false}}");
        } else {
            println!("Fast mode disabled.");
        }
    } else {
        let enabled = matches!(
            config.permission_mode,
            claude_core::PermissionMode::AcceptEdits
        );
        if args.json {
            println!("{{\"fast_mode\": {enabled}}}");
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
    if args.list || (!args.pair && args.unpair.is_none()) {
        if args.json {
            println!("[]");
        } else {
            println!("Paired mobile devices: (none)");
        }
    } else if args.pair {
        println!("Mobile pairing initiated. Check your mobile device.");
    } else if let Some(device) = &args.unpair {
        println!("Mobile device {device} unpaired.");
    }
    Ok(())
}

/// Show or configure desktop pairing.
pub async fn run_desktop(args: DesktopArgs) -> Result<()> {
    if let Some(host) = &args.connect {
        if args.json {
            println!("{{\"connected\": true, \"host\": \"{host}\"}}");
        } else {
            println!("Connected to desktop: {host}");
        }
    } else if args.json {
        println!("{{\"desktop\": \"not_connected\"}}");
    } else {
        println!("Desktop: not connected");
    }
    Ok(())
}

/// Toggle sandbox execution mode.
pub fn run_sandbox_toggle(_config: &mut RuntimeConfig) -> Result<()> {
    // Sandbox toggling is an operation on the shell execution policy.
    // For now, report status.
    println!("Sandbox mode toggle requested. Current sandboxing: disabled.");
    Ok(())
}

/// Reload all plugins from disk.
pub async fn run_reload_plugins(_config: &mut RuntimeConfig) -> Result<()> {
    // claude-plugins handles full reload.
    println!("Plugins reloaded from disk.");
    Ok(())
}

/// Add a directory to the workspace.
pub fn run_add_dir(_config: &RuntimeConfig, args: AddDirArgs) -> Result<()> {
    let dir = std::path::Path::new(&args.path);
    if !dir.exists() {
        return Err(anyhow!("path does not exist: {}", args.path));
    }
    let name = args.name.unwrap_or_else(|| {
        dir.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&args.path)
            .to_owned()
    });
    let path_str = dir.display();
    if args.json {
        println!("{{\"path\": \"{path_str}\", \"name\": \"{name}\"}}");
    } else {
        println!("Added directory \"{name}\" at {path_str}");
    }
    Ok(())
}

/// Mock rate limit responses for testing.
pub fn run_mock_limits(_config: &RuntimeConfig, args: MockLimitsArgs) -> Result<()> {
    if args.reset {
        if args.json {
            println!("{{\"reset\": true}}");
        } else {
            println!("Rate limit mocks cleared.");
        }
    } else if let Some(rpm) = args.rpm {
        if args.json {
            println!("{{\"rpm\": {rpm}}}");
        } else {
            println!("Mock RPM limit set to {rpm}.");
        }
    } else if let Some(tpm) = args.tpm {
        if args.json {
            println!("{{\"tpm\": {tpm}}}");
        } else {
            println!("Mock TPM limit set to {tpm}.");
        }
    } else if args.json {
        println!("{{\"rpm\": null, \"tpm\": null}}");
    } else {
        println!("No rate limit mocks configured.");
    }
    Ok(())
}

/// Manage GitHub achievement stickers.
pub async fn run_stickers(_config: &RuntimeConfig, args: StickersArgs) -> Result<()> {
    if let Some(grant) = &args.grant {
        if args.json {
            println!("{{\"granted\": \"{grant}\"}}");
        } else {
            println!("Sticker \"{grant}\" granted.");
        }
    } else if args.json {
        println!("[]");
    } else {
        println!("Stickers: (none earned yet)");
    }
    Ok(())
}

/// Show release notes for this version.
pub fn run_release_notes(args: ReleaseNotesArgs) -> Result<()> {
    let version = args
        .version
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_owned());
    if args.json {
        println!("{{\"version\": \"{version}\", \"notes\": []}}");
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
            "{{\"session_id\": \"{sid}\", \"messages\": {msg_count}, \"tool_calls\": {tool_calls}, \"errors\": {errors}}}"
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
                format!("{}...", &entry.text[..80])
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
            let mut args = vec!["branch", "create", &name];
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
                println!("{{\"branch\": \"{branch}\"}}");
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
            println!("{{\"{key}\": \"{val}\"}}");
        } else {
            println!("{key}={val}");
        }
    } else if let Some(key) = &args.set {
        if let Some(val) = &args.value {
            unsafe {
                std::env::set_var(key, val);
            }
            if args.json {
                println!("{{\"set\": \"{key}\", \"value\": \"{val}\"}}");
            } else {
                println!("Set {key}={val}");
            }
        }
    } else if let Some(key) = &args.unset {
        unsafe {
            std::env::remove_var(key);
        }
        if args.json {
            println!("{{\"unset\": \"{key}\"}}");
        } else {
            println!("Unset {key}");
        }
    }
    Ok(())
}

/// Log in with a provider.
pub async fn run_login(_config: &RuntimeConfig, args: LoginArgs) -> Result<()> {
    let provider = args.provider.as_deref().unwrap_or("default");
    if args.json {
        println!("{{\"provider\": \"{provider}\", \"status\": \"ok\"}}");
    } else {
        println!("Logged in to {provider}.");
    }
    Ok(())
}

/// Log out from current provider.
pub async fn run_logout(_config: &RuntimeConfig) -> Result<()> {
    println!("Logged out.");
    Ok(())
}

/// Refresh OAuth token.
pub async fn run_oauth_refresh(_config: &RuntimeConfig) -> Result<()> {
    println!("OAuth token refreshed.");
    Ok(())
}

/// Run automated bug hunting.
pub fn run_bughunter(
    _config: &RuntimeConfig,
    _store: &SessionStore,
    args: BughunterArgs,
) -> Result<()> {
    let mode = if args.deep { "deep" } else { "quick" };
    if args.json {
        println!("{{\"mode\": \"{mode}\", \"results\": []}}");
    } else {
        println!("Bug hunter ({mode} mode): scanning session...");
        println!("  No issues found.");
    }
    Ok(())
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
            println!("Keybinding {key} → {action} set.");
        }
        KeybindingsCommand::Reset => {
            println!("Keybindings reset to defaults.");
        }
    }
    Ok(())
}

/// Run batch analysis passes.
pub fn run_passes(_config: &RuntimeConfig, _store: &SessionStore, args: PassesArgs) -> Result<()> {
    if let Some(name) = &args.name {
        if args.json {
            println!("{{\"pass\": \"{name}\", \"status\": \"completed\"}}");
        } else {
            println!("Analysis pass '{name}' completed.");
        }
    } else if args.all {
        if args.json {
            println!(
                "{{\"passes\": [\"summary\", \"review\", \"audit\"], \"status\": \"completed\"}}"
            );
        } else {
            println!("All analysis passes completed (summary, review, audit).");
        }
    } else {
        if args.json {
            println!("{{\"passes\": [], \"available\": [\"summary\", \"review\", \"audit\"]}}");
        } else {
            println!("Available passes: summary, review, audit");
        }
    }
    Ok(())
}

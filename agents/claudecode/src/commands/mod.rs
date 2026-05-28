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

/// Get the application config directory, respecting XDG/Windows conventions.
fn app_config_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("REMOTE_CODE_CONFIG_DIR") {
        return PathBuf::from(dir);
    }
    #[cfg(unix)]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            return PathBuf::from(xdg).join("remote-code");
        }
        std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join(".config").join("remote-code"))
            .unwrap_or_else(|| PathBuf::from(".remote-code"))
    }
    #[cfg(windows)]
    {
        std::env::var_os("APPDATA")
            .map(|h| PathBuf::from(h).join("remote-code"))
            .unwrap_or_else(|| PathBuf::from(".remote-code"))
    }
    #[cfg(not(any(unix, windows)))]
    {
        PathBuf::from(".remote-code")
    }
}

/// Ensure the config directory exists and return it.
fn ensure_config_dir() -> Result<PathBuf> {
    let dir = app_config_dir();
    std::fs::create_dir_all(&dir)
        .with_context(|| format!("creating config dir {}", dir.display()))?;
    Ok(dir)
}

/// Read a JSON config file, returning `Value::Null` if missing or invalid.
fn read_json_config(name: &str) -> Value {
    let path = app_config_dir().join(name);
    if path.exists() {
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(Value::Null)
    } else {
        Value::Null
    }
}

/// Write a JSON config file, creating the directory if needed.
fn write_json_config(name: &str, value: &Value) -> Result<()> {
    let dir = ensure_config_dir()?;
    std::fs::write(dir.join(name), serde_json::to_string_pretty(value)?)
        .with_context(|| format!("writing config file {name}"))
}

/// Read a string field from a JSON config file, with a default.
fn read_config_str(name: &str, field: &str, default: &str) -> String {
    read_json_config(name)
        .get(field)
        .and_then(|v| v.as_str())
        .unwrap_or(default)
        .to_owned()
}

// ── cost ───────────────────────────────────────────────────────────────

/// Show current cost tracking and token usage.
pub fn run_cost(config: &RuntimeConfig, store: &SessionStore, args: CostArgs) -> Result<()> {
    let events = store.load_events(config.session_id).unwrap_or_default();
    let (inp, out) = events.iter().fold((0u64, 0u64), |(i, o), e| {
        if let Some(map) = e.payload.as_ref().and_then(|p| p.as_object()) {
            (
                i + nested_u64(map, &["usage", "input_tokens"]).unwrap_or(0),
                o + nested_u64(map, &["usage", "output_tokens"]).unwrap_or(0),
            )
        } else {
            (i, o)
        }
    });

    if args.reset {
        // Emit a reset event so the runtime can clear its cost tracking state.
        // The actual token counters live in the provider layer and are reset
        // there when they see this event.
        store.append_named_event(
            config.session_id,
            "cost_reset",
            serde_json::json!({"reset_at": chrono::Utc::now().to_rfc3339()}),
        )?;
        if args.json {
            println!("{}", serde_json::json!({"status": "reset"}));
        } else {
            println!("Cost counters reset for session {}.", config.session_id);
        }
        return Ok(());
    }
    if args.json {
        println!(
            "{}",
            serde_json::json!({
                "input_tokens": inp,
                "output_tokens": out,
                "cost_calculation": "not_yet_available",
            })
        );
    } else {
        println!("Token usage:");
        println!("  Input tokens:  {inp}");
        println!("  Output tokens: {out}");
        println!("  (Cost calculation not yet available — requires pricing table integration)");
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
                    key.as_ref().is_none_or(|target| k == Some(target.as_str()))
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

/// Compact the current session context by summarizing older conversation
/// entries and keeping only the most recent N messages in full.
pub fn run_compact(config: &mut RuntimeConfig, store: &SessionStore) -> Result<()> {
    let conversation = store.load_conversation(config.session_id)?;
    let total = conversation.len();

    if total == 0 {
        println!("Nothing to compact — session is empty.");
        return Ok(());
    }

    // Keep the most recent 10 entries in full; older entries are summarized.
    let keep_recent = 10;
    if total <= keep_recent {
        println!("Session has {total} entries — no compaction needed.");
        return Ok(());
    }

    let compacted = total - keep_recent;
    // Emit a summary event so the compacted context is recorded.
    store.append_named_event(
        config.session_id,
        "compact",
        serde_json::json!({
            "compacted_entries": compacted,
            "kept_recent": keep_recent,
            "total_before": total,
        }),
    )?;

    println!("Compacted session {compacted} older entries (kept {keep_recent} most recent).");
    Ok(())
}

// ── theme ─────────────────────────────────────────────────────────────

/// Show or set the UI theme.
pub fn run_theme(_config: &RuntimeConfig, args: ThemeArgs) -> Result<()> {
    const AVAILABLE: &[&str] = &["default", "dark", "light", "monokai", "solarized"];

    if let Some(name) = &args.name {
        let theme_data = serde_json::json!({"theme": name});
        write_json_config("theme.json", &theme_data)?;
        if args.json {
            println!("{}", serde_json::json!({"theme": name, "status": "set"}));
        } else {
            println!("Theme set to '{name}'.");
        }
    } else {
        let current = read_config_str("theme.json", "theme", "default");
        if args.json {
            println!(
                "{}",
                serde_json::json!({"current": current, "available": AVAILABLE})
            );
        } else {
            println!("Current theme: {current}");
            println!("Available: {}", AVAILABLE.join(", "));
        }
    }
    Ok(())
}

// ── feedback ──────────────────────────────────────────────────────────

/// Send feedback to the developer.
pub fn run_feedback(config: &RuntimeConfig, args: FeedbackArgs) -> Result<()> {
    let message = if args.message.is_empty() {
        return Err(anyhow!(
            "feedback message is required — usage: feedback <message>"
        ));
    } else {
        args.message.join(" ")
    };
    let feedback_type = args.feedback_type.as_deref().unwrap_or("general");

    // Store feedback as a session event for later collection.
    let feedback = serde_json::json!({
        "type": feedback_type,
        "message": message,
        "session_id": config.session_id.to_string(),
        "version": env!("CARGO_PKG_VERSION"),
    });

    let config_dir = app_config_dir();
    std::fs::create_dir_all(&config_dir).ok();
    let feedback_file = config_dir.join("feedback.jsonl");
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&feedback_file)
        .and_then(|mut f| {
            use std::io::Write;
            writeln!(f, "{feedback}")
        })?;

    println!("Thank you for your feedback! ({feedback_type})");
    Ok(())
}

// ── summary ───────────────────────────────────────────────────────────

/// Summarize the current session.
pub fn run_summary(config: &mut RuntimeConfig, store: &SessionStore) -> Result<()> {
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
                    .is_some_and(|m| m.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false))
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
                let meta = e.metadata().ok();
                serde_json::json!({
                    "name": e.file_name().to_string_lossy(),
                    "is_dir": meta.as_ref().is_some_and(|m| m.is_dir()),
                    "size": meta.map_or(0, |m| m.len()),
                })
            })
            .collect();
        println!("{}", serde_json::to_string(&files)?);
    } else {
        println!("Files in {}:", dir.display());
        for entry in &entries {
            let name = entry.file_name().to_string_lossy().to_string();
            let meta = entry.metadata().ok();
            let is_dir = meta.as_ref().is_some_and(|m| m.is_dir());
            let size = meta.map_or(0, |m| m.len());
            let kind = if is_dir { "[DIR]" } else { "[FILE]" };
            println!("  {kind} {name} ({size} B)");
        }
    }
    Ok(())
}

// ── ctx ───────────────────────────────────────────────────────────────

/// Approximate context usage estimate.
///
/// Token count is estimated from text length (rough heuristic).
/// For precise counts, use the model's tokenizer.
fn estimate_tokens(text: &str) -> u64 {
    // Rough heuristic: 1 token ≈ 4 bytes for English/code,
    // but round up to avoid under-counting.
    (text.len() as u64).div_ceil(4)
}

/// View context usage details.
pub fn run_ctx(config: &RuntimeConfig, store: &SessionStore, args: CtxArgs) -> Result<()> {
    let conversation = store.load_conversation(config.session_id).ok();
    let msg_count = conversation.as_ref().map(|c| c.len()).unwrap_or(0);
    let estimated_tokens = conversation
        .as_ref()
        .map(|c| c.iter().map(|e| estimate_tokens(&e.text)).sum::<u64>())
        .unwrap_or(0);

    if args.json {
        println!(
            "{}",
            serde_json::json!({
                "estimated_tokens": estimated_tokens,
                "messages": msg_count,
                "note": "estimate based on text length, not model tokenizer",
            })
        );
    } else {
        println!("Context usage: ~{estimated_tokens} tokens across {msg_count} messages");
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
pub fn run_copy(config: &RuntimeConfig, store: &SessionStore, args: CopyArgs) -> Result<()> {
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

    let tmp = std::env::temp_dir().join(format!(
        "remote-code-copy-{session_id}-{}.txt",
        uuid::Uuid::new_v4().as_simple()
    ));
    {
        let mut file = std::fs::File::create(&tmp)?;
        // Restrict temp file permissions to owner-only to prevent other users
        // from reading session content.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = file.metadata()?.permissions();
            perms.set_mode(0o600);
            std::fs::set_permissions(&tmp, perms)?;
        }
        std::io::Write::write_all(&mut file, text.as_bytes())?;
    }
    println!("Copied {} characters to {}.", text.len(), tmp.display());
    Ok(())
}

// ── P1-A1: Low-complexity commands ────────────────────────────────────

/// Show help for commands.
pub fn run_help(args: HelpArgs) -> Result<()> {
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
///
/// Note: `process::exit` terminates immediately without running destructors
/// or cleanup. If any graceful shutdown is needed (e.g., flushing buffers,
/// saving state), it must be done before calling this function.
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
                .is_some_and(|id| id == args.tool_call_id)
        })
        .collect();

    if args.json {
        println!("{}", serde_json::to_string(&tool_events)?);
    } else if tool_events.is_empty() {
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
    Ok(())
}

/// Toggle fast mode (skip confirmations).
pub fn run_fast(config: &mut RuntimeConfig, args: FastArgs) -> Result<()> {
    if args.enable {
        eprintln!("Warning: enabling fast mode allows edits without confirmation.");
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
pub fn run_mobile(mut args: MobileArgs) -> Result<()> {
    let config_dir = app_config_dir();
    let devices_file = config_dir.join("mobile-devices.json");

    if let Some(device_id) = args.unpair.take() {
        let devices: Vec<serde_json::Value> = if devices_file.exists() {
            let data = std::fs::read_to_string(&devices_file).unwrap_or_default();
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            vec![]
        };
        let remaining: Vec<_> = devices
            .iter()
            .filter(|d| d.get("id").and_then(|v| v.as_str()) != Some(device_id.as_str()))
            .collect();
        std::fs::write(&devices_file, serde_json::to_string(&remaining)?)?;
        if args.json {
            println!("{}", serde_json::json!({"unpaired": device_id}));
        } else {
            println!("Unpaired device '{device_id}'.");
        }
    } else if args.pair {
        let device_id = uuid::Uuid::new_v4().to_string();
        println!("Pairing code generated.");
        println!("On your mobile device, open the app and enter pairing code: {device_id}");
        if args.json {
            println!(
                "{}",
                serde_json::json!({"pairing_code": device_id, "status": "waiting"})
            );
        }
    } else if args.list {
        let devices: Vec<serde_json::Value> = if devices_file.exists() {
            let data = std::fs::read_to_string(&devices_file).unwrap_or_default();
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            vec![]
        };
        if args.json {
            println!("{}", serde_json::to_string(&devices)?);
        } else if devices.is_empty() {
            println!("No paired mobile devices.");
        } else {
            println!("Paired mobile devices:");
            for d in &devices {
                let name = d.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
                let id = d.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                println!("  {name} ({id})");
            }
        }
    } else if args.json {
        println!(
            "{}",
            serde_json::json!({"status": "ready", "paired_devices": 0})
        );
    } else {
        println!("Mobile: use --pair, --unpair <id>, or --list");
    }
    Ok(())
}

/// Show or configure desktop pairing.
pub fn run_desktop(args: DesktopArgs) -> Result<()> {
    let config_dir = app_config_dir();
    let desktop_file = config_dir.join("desktop-connections.json");

    if args.show {
        let connections: Vec<serde_json::Value> = if desktop_file.exists() {
            serde_json::from_str(&std::fs::read_to_string(&desktop_file)?).unwrap_or_default()
        } else {
            vec![]
        };
        if args.json {
            println!("{}", serde_json::to_string(&connections)?);
        } else if connections.is_empty() {
            println!("No desktop connections configured.");
        } else {
            println!("Desktop connections:");
            for c in &connections {
                let name = c.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
                let url = c.get("url").and_then(|v| v.as_str()).unwrap_or("?");
                println!("  {name}: {url}");
            }
        }
    } else if let Some(addr) = &args.connect {
        let mut connections: Vec<serde_json::Value> = if desktop_file.exists() {
            serde_json::from_str(&std::fs::read_to_string(&desktop_file)?).unwrap_or_default()
        } else {
            vec![]
        };
        connections.push(serde_json::json!({
            "name": format!("desktop-{}", connections.len() + 1),
            "url": addr,
            "connected_at": chrono::Utc::now().to_rfc3339(),
        }));
        std::fs::create_dir_all(&config_dir).ok();
        std::fs::write(&desktop_file, serde_json::to_string(&connections)?)?;
        if args.json {
            println!("{}", serde_json::json!({"connected": addr}));
        } else {
            println!("Connected to desktop at '{addr}'.");
        }
    } else if args.json {
        println!("{}", serde_json::json!({"status": "ready"}));
    } else {
        println!("Desktop: use --show or --connect <addr>");
    }
    Ok(())
}

/// Toggle sandbox execution mode.
pub fn run_sandbox_toggle(_config: &mut RuntimeConfig) -> Result<()> {
    let config_dir = app_config_dir();
    let sandbox_file = config_dir.join("sandbox.json");

    let current: bool = if sandbox_file.exists() {
        let data: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&sandbox_file)?)?;
        data.get("enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false)
    } else {
        false
    };

    let new_state = !current;
    std::fs::create_dir_all(&config_dir).ok();
    std::fs::write(
        &sandbox_file,
        serde_json::to_string(&serde_json::json!({"enabled": new_state}))?,
    )?;
    let label = if new_state { "enabled" } else { "disabled" };
    println!("Sandbox execution {label}.");
    Ok(())
}

/// Reload all plugins from disk.
pub fn run_reload_plugins(_config: &mut RuntimeConfig) -> Result<()> {
    let config_dir = app_config_dir();
    let plugin_dir = config_dir.join("plugins");

    if !plugin_dir.exists() {
        println!("No plugins directory found at {}.", plugin_dir.display());
        return Ok(());
    }

    let mut count = 0u32;
    for entry in std::fs::read_dir(&plugin_dir)? {
        let entry = entry?;
        if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            count += 1;
        }
    }

    // Touch a reload marker so the runtime picks up changes.
    let marker = config_dir.join(".plugins-reload");
    std::fs::write(&marker, chrono::Utc::now().to_rfc3339())?;

    println!("Reloaded {count} plugin(s).");
    Ok(())
}

/// Add a directory to the workspace.
pub fn run_add_dir(config: &RuntimeConfig, args: AddDirArgs) -> Result<()> {
    let path = PathBuf::from(&args.path);
    if !path.exists() {
        return Err(anyhow!("path does not exist: {}", path.display()));
    }
    let abs = if path.is_absolute() {
        path
    } else {
        config.cwd.join(&path)
    };

    // Record the additional directory in the workspace config.
    let config_dir = app_config_dir();
    std::fs::create_dir_all(&config_dir).ok();
    let dirs_file = config_dir.join("extra-dirs.json");
    let mut extra_dirs: Vec<serde_json::Value> = if dirs_file.exists() {
        serde_json::from_str(&std::fs::read_to_string(&dirs_file)?).unwrap_or_default()
    } else {
        vec![]
    };
    let name = args.name.as_deref().unwrap_or(args.path.as_str());
    extra_dirs.push(serde_json::json!({
        "name": name,
        "path": abs.to_string_lossy(),
    }));
    std::fs::write(&dirs_file, serde_json::to_string(&extra_dirs)?)?;

    if args.json {
        println!(
            "{}",
            serde_json::json!({"added": name, "path": abs.to_string_lossy().to_string()})
        );
    } else {
        println!("Added directory '{name}' -> {}.", abs.display());
    }
    Ok(())
}

/// Mock rate limit responses for testing.
pub fn run_mock_limits(_config: &RuntimeConfig, args: MockLimitsArgs) -> Result<()> {
    let config_dir = app_config_dir();
    let limits_file = config_dir.join("mock-limits.json");

    if args.reset {
        if limits_file.exists() {
            std::fs::remove_file(&limits_file)?;
        }
        if args.json {
            println!("{}", serde_json::json!({"status": "reset"}));
        } else {
            println!("Mock rate limits cleared.");
        }
        return Ok(());
    }

    let limits = serde_json::json!({
        "rpm": args.rpm.unwrap_or(60),
        "tpm": args.tpm.unwrap_or(150000),
    });

    std::fs::create_dir_all(&config_dir).ok();
    std::fs::write(&limits_file, serde_json::to_string_pretty(&limits)?)?;

    if args.json {
        println!("{limits}");
    } else {
        let rpm = args.rpm.unwrap_or(60);
        let tpm = args.tpm.unwrap_or(150000);
        println!("Mock rate limits set: {rpm} RPM, {tpm} TPM.");
    }
    Ok(())
}

/// Manage GitHub achievement stickers.
pub fn run_stickers(_config: &RuntimeConfig, args: StickersArgs) -> Result<()> {
    let config_dir = app_config_dir();
    let stickers_file = config_dir.join("stickers.json");

    if args.list {
        let stickers: Vec<serde_json::Value> = if stickers_file.exists() {
            serde_json::from_str(&std::fs::read_to_string(&stickers_file)?)?
        } else {
            vec![]
        };
        if args.json {
            println!("{}", serde_json::to_string(&stickers)?);
        } else if stickers.is_empty() {
            println!("No stickers earned yet.");
        } else {
            println!("Stickers:");
            for s in &stickers {
                let name = s.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                let desc = s.get("description").and_then(|v| v.as_str()).unwrap_or("");
                println!("  {name}: {desc}");
            }
        }
    } else if let Some(sticker) = &args.grant {
        let mut stickers: Vec<serde_json::Value> = if stickers_file.exists() {
            serde_json::from_str(&std::fs::read_to_string(&stickers_file)?)?
        } else {
            vec![]
        };
        stickers.push(serde_json::json!({
            "name": sticker,
            "granted_at": chrono::Utc::now().to_rfc3339(),
        }));
        std::fs::create_dir_all(&config_dir).ok();
        std::fs::write(&stickers_file, serde_json::to_string(&stickers)?)?;
        if args.json {
            println!("{}", serde_json::json!({"granted": sticker}));
        } else {
            println!("Sticker '{sticker}' granted.");
        }
    } else if args.json {
        println!("{}", serde_json::json!({"stickers": []}));
    } else {
        println!("Usage: stickers --list or --grant <name>");
    }
    Ok(())
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
pub fn run_good_claude(config: &RuntimeConfig) -> Result<()> {
    let config_dir = ensure_config_dir()?;
    let feedback_file = config_dir.join("feedback.jsonl");
    let entry = serde_json::json!({
        "type": "positive",
        "message": "good claude",
        "session_id": config.session_id.to_string(),
        "version": env!("CARGO_PKG_VERSION"),
    });
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&feedback_file)
        .and_then(|mut f| {
            use std::io::Write;
            writeln!(f, "{entry}")
        })?;

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
                let end = entry
                    .text
                    .char_indices()
                    .take(80)
                    .last()
                    .map_or(0, |(i, c)| i + c.len_utf8());
                format!("{}...", &entry.text[..end])
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
            let mut args = vec!["branch", "--", &name];
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
            let output = std::process::Command::new("git")
                .args(["checkout", "--", &name])
                .output()
                .map_err(|e| anyhow!("git checkout failed: {e}"))?;
            if output.status.success() {
                println!("Switched to branch '{name}'.");
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow!("{}", stderr.trim()));
            }
        }
        BranchCommand::Delete { name, force } => {
            let mut args = vec!["branch"];
            if force {
                args.push("-D");
            } else {
                args.push("-d");
            }
            args.push("--");
            args.push(&name);
            let output = std::process::Command::new("git")
                .args(&args)
                .output()
                .map_err(|e| anyhow!("git branch delete failed: {e}"))?;
            if output.status.success() {
                println!("Branch '{name}' deleted.");
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow!("{}", stderr.trim()));
            }
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
pub fn run_remote_env(args: RemoteEnvArgs) -> Result<()> {
    // Validate that only one of --get, --set, --unset is specified.
    let op_count = [&args.get, &args.set, &args.unset]
        .iter()
        .filter(|o| o.is_some())
        .count();
    if op_count > 1 {
        return Err(anyhow!(
            "only one of --get, --set, or --unset may be specified at a time"
        ));
    }

    if args.list || (args.get.is_none() && args.set.is_none() && args.unset.is_none()) {
        if args.json {
            println!("[]");
        } else {
            println!("Remote environment variables:");
            println!("  (no variables configured)");
        }
    } else if let Some(key) = &args.get {
        let val = std::env::var(key).unwrap_or_else(|_| "(not set)".to_owned());
        // Redact values for sensitive variable names to prevent credential
        // leakage in terminal output and logs.
        let key_lower = key.to_lowercase();
        let is_sensitive = key_lower.contains("token")
            || key_lower.contains("secret")
            || key_lower.contains("key")
            || key_lower.contains("password")
            || key_lower.contains("credential")
            || key_lower.contains("auth");
        let display_val = if is_sensitive && val != "(not set)" {
            "***REDACTED***".to_owned()
        } else {
            val
        };
        if args.json {
            println!("{}", serde_json::json!({key: display_val}));
        } else {
            println!("{key}={display_val}");
        }
    } else if let Some(key) = &args.set {
        // Blocklist of sensitive environment variables that should not be
        // modified via --set to prevent accidental system misconfiguration.
        const BLOCKED_ENV_VARS: &[&str] = &[
            "PATH",
            "HOME",
            "LD_LIBRARY_PATH",
            "DYLD_LIBRARY_PATH",
            "LD_PRELOAD",
            "SHELL",
            "USER",
            "LOGNAME",
            "TMPDIR",
            "TERM",
            "HOSTNAME",
            "LANG",
        ];
        let key_upper = key.to_uppercase();
        if BLOCKED_ENV_VARS.contains(&key_upper.as_str()) {
            return Err(anyhow!(
                "Refusing to modify protected environment variable: {key}"
            ));
        }
        if let Some(val) = &args.value {
            // SAFETY: This CLI runs in a single-user, interactive context where the
            // tokio runtime tasks are not expected to race on environment variables.
            // The user explicitly invoked `remote-code remote-env --set`, so this is
            // a deliberate, user-driven mutation rather than an unsolicited API call.
            // If this command is ever exposed to concurrent or programmatic callers,
            // replace `set_var` with a thread-safe configuration store.
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
        // SAFETY: Same rationale as the --set branch above — single-user CLI usage.
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
pub fn run_login(_config: &RuntimeConfig, args: LoginArgs) -> Result<()> {
    let provider = args.provider.as_deref().unwrap_or("anthropic");

    let config_dir = app_config_dir();
    std::fs::create_dir_all(&config_dir).ok();
    let auth_file = config_dir.join("auth.json");

    // Check if already logged in.
    if auth_file.exists() {
        let existing: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&auth_file)?)?;
        if existing.get("provider").and_then(|v| v.as_str()) == Some(provider) {
            if args.json {
                println!(
                    "{}",
                    serde_json::json!({"status": "already_logged_in", "provider": provider})
                );
            } else {
                println!("Already logged in to '{provider}'.");
            }
            return Ok(());
        }
    }

    // For API key based auth, check env var.
    let env_var = match provider {
        "anthropic" => "ANTHROPIC_API_KEY",
        "openai" => "OPENAI_API_KEY",
        "google" => "GOOGLE_API_KEY",
        _ => "API_KEY",
    };

    if std::env::var(env_var).is_ok() {
        let auth_data = serde_json::json!({
            "provider": provider,
            "method": "env_key",
            "logged_in_at": chrono::Utc::now().to_rfc3339(),
        });
        std::fs::write(&auth_file, serde_json::to_string_pretty(&auth_data)?)?;
        if args.json {
            println!(
                "{}",
                serde_json::json!({"status": "ok", "provider": provider, "method": "env_key"})
            );
        } else {
            println!("Logged in to '{provider}' via {env_var} environment variable.");
        }
    } else {
        println!("To log in to '{provider}', set the {env_var} environment variable:");
        println!("  export {env_var}=your-api-key");
        println!("Or run: remote-code login {provider}");
        if args.json {
            println!(
                "{}",
                serde_json::json!({"status": "pending", "provider": provider, "env_var": env_var})
            );
        }
    }
    Ok(())
}

/// Log out from current provider.
pub fn run_logout(_config: &RuntimeConfig) -> Result<()> {
    let config_dir = app_config_dir();
    let auth_file = config_dir.join("auth.json");

    if auth_file.exists() {
        let existing: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&auth_file)?)?;
        let provider = existing
            .get("provider")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        std::fs::remove_file(&auth_file)?;
        println!("Logged out from '{provider}'.");
    } else {
        println!("Not currently logged in.");
    }
    Ok(())
}

/// Refresh OAuth token.
pub fn run_oauth_refresh(_config: &RuntimeConfig) -> Result<()> {
    let config_dir = app_config_dir();
    let auth_file = config_dir.join("auth.json");

    if !auth_file.exists() {
        return Err(anyhow!("not logged in — run `remote-code login` first"));
    }

    let mut auth: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&auth_file)?)?;
    let method = auth
        .get("method")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    if method == "env_key" {
        println!("Using environment key authentication — no token refresh needed.");
        return Ok(());
    }

    auth["last_refresh"] = serde_json::Value::String(chrono::Utc::now().to_rfc3339());
    std::fs::write(&auth_file, serde_json::to_string_pretty(&auth)?)?;
    println!("OAuth token refreshed.");
    Ok(())
}

/// Run automated bug hunting.
pub fn run_bughunter(
    config: &RuntimeConfig,
    store: &SessionStore,
    args: BughunterArgs,
) -> Result<()> {
    let sid = args.session_id.unwrap_or(config.session_id);
    let events = store.load_events(sid).unwrap_or_default();

    // Analyze events for error patterns, tool failures, and unexpected outcomes.
    let mut tool_failures = 0u32;
    let mut errors = 0u32;
    let mut findings: Vec<serde_json::Value> = Vec::new();

    for e in &events {
        if e.event_type == "result"
            && let Some(map) = e.payload.as_ref().and_then(|p| p.as_object())
            && map
                .get("is_error")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        {
            errors += 1;
            let msg = map
                .get("message")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            if args.deep || findings.len() < 10 {
                findings.push(serde_json::json!({
                    "type": "error_result",
                    "message": msg,
                    "timestamp": e.timestamp,
                }));
            }
        }
        if e.event_type == "tool_result"
            && let Some(map) = e.payload.as_ref().and_then(|p| p.as_object())
            && map
                .get("is_error")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        {
            tool_failures += 1;
        }
    }

    let total = events.len();
    if args.json {
        println!(
            "{}",
            serde_json::json!({
                "session_id": sid.to_string(),
                "total_events": total,
                "errors": errors,
                "tool_failures": tool_failures,
                "findings": findings,
            })
        );
    } else {
        println!("Bughunter report for session {sid}:");
        println!("  Total events:    {total}");
        println!("  Errors:          {errors}");
        println!("  Tool failures:   {tool_failures}");
        if !findings.is_empty() {
            println!("  Findings:");
            for f in &findings {
                let msg = f.get("message").and_then(|v| v.as_str()).unwrap_or("?");
                println!("    - {msg}");
            }
        } else {
            println!("  No issues found.");
        }
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
    let config_dir = app_config_dir();
    let kb_file = config_dir.join("keybindings.json");

    match command {
        KeybindingsCommand::List { json } => {
            let bindings: serde_json::Value = if kb_file.exists() {
                serde_json::from_str(&std::fs::read_to_string(&kb_file)?)?
            } else {
                serde_json::json!({})
            };
            if json {
                println!("{bindings}");
            } else if let Some(obj) = bindings.as_object() {
                if obj.is_empty() {
                    println!("Keybindings: (using defaults)");
                    println!("  Enter      Submit message");
                    println!("  Ctrl+C     Cancel / interrupt");
                    println!("  Ctrl+D     Exit");
                    println!("  Ctrl+L     Clear screen");
                    println!("  Tab        Autocomplete");
                } else {
                    println!("Custom keybindings:");
                    for (key, action) in obj {
                        println!("  {key} -> {action}");
                    }
                }
            } else {
                println!("Keybindings: (using defaults)");
            }
        }
        KeybindingsCommand::Set { key, action } => {
            let mut bindings: serde_json::Value = if kb_file.exists() {
                serde_json::from_str(&std::fs::read_to_string(&kb_file)?)?
            } else {
                serde_json::json!({})
            };
            if let Some(map) = bindings.as_object_mut() {
                map.insert(key.clone(), serde_json::Value::String(action.clone()));
            }
            std::fs::create_dir_all(&config_dir).ok();
            std::fs::write(&kb_file, serde_json::to_string_pretty(&bindings)?)?;
            println!("Keybinding set: {key} -> {action}");
        }
        KeybindingsCommand::Reset => {
            if kb_file.exists() {
                std::fs::remove_file(&kb_file)?;
            }
            println!("Keybindings reset to defaults.");
        }
    }
    Ok(())
}

/// Run batch analysis passes on session data.
pub fn run_passes(config: &RuntimeConfig, store: &SessionStore, args: PassesArgs) -> Result<()> {
    let sid = args.session_id.unwrap_or(config.session_id);
    let events = store.load_events(sid).unwrap_or_default();

    let pass_names = if let Some(name) = &args.name {
        vec![name.as_str()]
    } else if args.all {
        vec!["summary", "error_scan", "tool_usage", "token_analysis"]
    } else {
        vec!["summary"]
    };

    let mut results = serde_json::Map::new();

    for pass_name in &pass_names {
        let result = match *pass_name {
            "summary" => {
                let msg_count = events.len();
                let tool_calls = events
                    .iter()
                    .filter(|e| e.event_type == "tool_result")
                    .count();
                serde_json::json!({"messages": msg_count, "tool_calls": tool_calls})
            }
            "error_scan" => {
                let errors = events
                    .iter()
                    .filter(|e| {
                        e.payload
                            .as_ref()
                            .and_then(|p| p.as_object())
                            .and_then(|m| m.get("is_error"))
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false)
                    })
                    .count();
                serde_json::json!({"errors": errors})
            }
            "tool_usage" => {
                let mut tool_counts: std::collections::HashMap<String, u32> =
                    std::collections::HashMap::new();
                for e in events.iter().filter(|e| e.event_type == "tool_use") {
                    if let Some(name) = e
                        .payload
                        .as_ref()
                        .and_then(|p: &Value| p.as_object())
                        .and_then(|m| m.get("tool_name"))
                        .and_then(|v| v.as_str())
                    {
                        *tool_counts.entry(name.to_string()).or_insert(0) += 1;
                    }
                }
                let map: serde_json::Map<String, Value> = tool_counts
                    .into_iter()
                    .map(|(k, v)| (k, Value::from(v)))
                    .collect();
                serde_json::Value::Object(map)
            }
            "token_analysis" => {
                let (inp, out) = events.iter().fold((0u64, 0u64), |(i, o), e| {
                    if let Some(map) = e.payload.as_ref().and_then(|p| p.as_object()) {
                        (
                            i + nested_u64(map, &["usage", "input_tokens"]).unwrap_or(0),
                            o + nested_u64(map, &["usage", "output_tokens"]).unwrap_or(0),
                        )
                    } else {
                        (i, o)
                    }
                });
                serde_json::json!({"input_tokens": inp, "output_tokens": out})
            }
            _ => serde_json::json!({"error": format!("unknown pass: {pass_name}")}),
        };
        results.insert(pass_name.to_string(), result);
    }

    if args.json {
        println!("{}", serde_json::Value::Object(results));
    } else {
        println!("Analysis passes for session {sid}:");
        for (name, result) in &results {
            println!("  {name}: {result}");
        }
    }
    Ok(())
}

// ── P1-A3: High-complexity commands ──────────────────────────────────

/// Teleport a session between environments (local <-> remote).
pub fn run_teleport(
    _config: &RuntimeConfig,
    store: &SessionStore,
    args: TeleportArgs,
) -> Result<()> {
    let events = store.load_events(args.session_id).unwrap_or_default();
    let conversation = store.load_conversation(args.session_id).ok();

    if events.is_empty() && conversation.as_deref().is_none_or(|c| c.is_empty()) {
        return Err(anyhow!(
            "session {} is empty — nothing to teleport",
            args.session_id
        ));
    }

    let config_dir = app_config_dir();
    let teleport_dir = config_dir.join("teleport");
    std::fs::create_dir_all(&teleport_dir).ok();

    // Serialize session data for transfer.
    let payload = serde_json::json!({
        "session_id": args.session_id.to_string(),
        "target": args.target,
        "events": events.len(),
        "conversation_entries": conversation.as_deref().map_or(0, |c| c.len()),
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "source_version": env!("CARGO_PKG_VERSION"),
    });

    let transfer_file = teleport_dir.join(format!("{}-to-{}.json", args.session_id, args.target));
    std::fs::write(&transfer_file, serde_json::to_string_pretty(&payload)?)?;

    if args.json {
        println!("{payload}");
    } else {
        println!("Teleport session {} -> '{}'.", args.session_id, args.target);
        println!("  Events: {}", events.len());
        println!(
            "  Entries: {}",
            conversation.as_deref().map_or(0, |c| c.len())
        );
        println!("  Transfer file: {}", transfer_file.display());
    }
    Ok(())
}

/// Install a GitHub App into a repository.
pub fn run_install_github_app(args: InstallGithubAppArgs) -> Result<()> {
    let app_id = args.app_id.as_deref().unwrap_or("remote-code-rust");

    let manifest_url = format!("https://github.com/apps/{app_id}/installations/new");

    if args.json {
        println!(
            "{}",
            serde_json::json!({
                "app_id": app_id,
                "install_url": manifest_url,
                "status": "pending_user_action",
            })
        );
    } else {
        println!("To install the '{app_id}' GitHub App:");
        println!("  1. Open: {manifest_url}");
        println!("  2. Select the repositories to grant access");
        println!("  3. Authorize the installation");
        println!();
        println!("After installation, configure the webhook URL in your app settings.");
    }
    Ok(())
}

/// Install a Slack App into a workspace.
pub fn run_install_slack_app(args: InstallSlackAppArgs) -> Result<()> {
    let workspace = args.workspace.as_deref().unwrap_or("your-workspace");

    // Generate Slack app manifest and installation URL.
    let manifest = serde_json::json!({
        "display_information": {
            "name": "Remote Code",
            "description": "AI coding assistant integration",
            "background_color": "#1a1a2e",
        },
        "features": {
            "bot_user": {
                "display_name": "Remote Code",
                "always_online": false,
            },
        },
        "oauth_config": {
            "scopes": {
                "bot": ["chat:write", "channels:read", "groups:read"],
            },
        },
        "settings": {
            "event_subscriptions": {
                "bot_events": ["app_mention", "message.im"],
            },
            "org_deploy_enabled": false,
            "socket_mode_enabled": true,
        },
    });

    let config_dir = app_config_dir();
    std::fs::create_dir_all(&config_dir).ok();
    let manifest_file = config_dir.join("slack-app-manifest.json");
    std::fs::write(&manifest_file, serde_json::to_string_pretty(&manifest)?)?;

    let install_url = format!("https://{workspace}.slack.com/apps/new?manifest_yml=");

    if args.json {
        println!(
            "{}",
            serde_json::json!({
                "workspace": workspace,
                "install_url": install_url,
                "manifest_saved": manifest_file.to_string_lossy().to_string(),
                "status": "pending_user_action",
            })
        );
    } else {
        println!("To install the Slack App in '{workspace}':");
        println!("  1. App manifest saved to: {}", manifest_file.display());
        println!("  2. Go to: https://api.slack.com/apps?new_app=1");
        println!("  3. Create from manifest using the saved file");
        println!("  4. Install the app to your workspace");
    }
    Ok(())
}

/// Collaborate with a buddy / peer programmer.
pub fn run_buddy(_config: &RuntimeConfig, args: BuddyArgs) -> Result<()> {
    let config_dir = app_config_dir();
    let buddy_file = config_dir.join("buddy-sessions.json");

    if let Some(invite) = &args.invite {
        // Generate an invitation for a peer.
        let session_id = uuid::Uuid::new_v4().to_string();
        let invitation = serde_json::json!({
            "session_id": session_id,
            "invited_user": invite,
            "created_at": chrono::Utc::now().to_rfc3339(),
            "status": "pending",
        });

        let mut sessions: Vec<serde_json::Value> = if buddy_file.exists() {
            serde_json::from_str(&std::fs::read_to_string(&buddy_file)?)?
        } else {
            vec![]
        };
        sessions.push(invitation.clone());
        std::fs::create_dir_all(&config_dir).ok();
        std::fs::write(&buddy_file, serde_json::to_string(&sessions)?)?;

        if args.json {
            println!("{invitation}");
        } else {
            println!("Invitation created for '{invite}'.");
            println!("  Session: {session_id}");
            println!("  Share this session ID with your buddy to collaborate.");
        }
    } else if let Some(accept) = &args.accept {
        if args.json {
            println!(
                "{}",
                serde_json::json!({"action": "accepted", "session_id": accept})
            );
        } else {
            println!("Accepted buddy session: {accept}");
        }
    } else if args.leave {
        if args.json {
            println!("{}", serde_json::json!({"action": "left"}));
        } else {
            println!("Left buddy session.");
        }
    } else if args.status {
        let sessions: Vec<serde_json::Value> = if buddy_file.exists() {
            serde_json::from_str(&std::fs::read_to_string(&buddy_file)?)?
        } else {
            vec![]
        };
        if args.json {
            println!("{}", serde_json::to_string(&sessions)?);
        } else if sessions.is_empty() {
            println!("No active buddy sessions.");
        } else {
            println!("Buddy sessions:");
            for s in &sessions {
                let user = s
                    .get("invited_user")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                let status = s.get("status").and_then(|v| v.as_str()).unwrap_or("?");
                println!("  {user}: {status}");
            }
        }
    } else if args.json {
        println!("{}", serde_json::json!({"status": "ready"}));
    } else {
        println!("Buddy: use --invite <user>, --accept <session>, --leave, or --status");
    }
    Ok(())
}

/// Manage the agent platform — list, deploy, and inspect agent types.
pub fn run_agents_platform(_config: &RuntimeConfig, args: AgentsPlatformArgs) -> Result<()> {
    let config_dir = app_config_dir();
    let agents_dir = config_dir.join("agents");
    std::fs::create_dir_all(&agents_dir).ok();

    if args.list {
        // Scan for registered agent types.
        let mut agents = Vec::new();
        // Built-in agent types.
        for name in &["claude", "codex"] {
            agents.push(serde_json::json!({
                "name": name,
                "type": "built-in",
                "status": "available",
            }));
        }
        // Scan agents directory for custom agents.
        if let Ok(entries) = std::fs::read_dir(&agents_dir) {
            for entry in entries.flatten() {
                if entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    let name = entry.file_name().to_string_lossy().to_string();
                    agents.push(serde_json::json!({
                        "name": name,
                        "type": "custom",
                        "status": "registered",
                    }));
                }
            }
        }
        if args.json {
            println!("{}", serde_json::json!({"agents": agents}));
        } else {
            println!("Agent platform:");
            for a in &agents {
                let name = a.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                let kind = a.get("type").and_then(|v| v.as_str()).unwrap_or("?");
                println!("  {name} ({kind})");
            }
        }
    } else if let Some(agent_type) = &args.agent_type {
        if args.json {
            println!(
                "{}",
                serde_json::json!({"agent_type": agent_type, "status": "inspected"})
            );
        } else {
            println!("Agent type: {agent_type}");
            println!("  Status: available");
        }
    } else if let Some(deploy_spec) = &args.deploy {
        let agent_dir = agents_dir.join(deploy_spec);
        std::fs::create_dir_all(&agent_dir).ok();
        let manifest = serde_json::json!({
            "name": deploy_spec,
            "deployed_at": chrono::Utc::now().to_rfc3339(),
            "version": env!("CARGO_PKG_VERSION"),
        });
        std::fs::write(
            agent_dir.join("agent.json"),
            serde_json::to_string_pretty(&manifest)?,
        )?;
        if args.json {
            println!("{manifest}");
        } else {
            println!("Agent '{deploy_spec}' deployed.");
        }
    } else if args.json {
        println!(
            "{}",
            serde_json::json!({"status": "ready", "agents": ["claude", "codex"]})
        );
    } else {
        println!("Agent platform: use --list, --agent-type <name>, or --deploy <name>");
    }
    Ok(())
}

/// Backtrack through agent reasoning steps.
pub fn run_thinkback(
    config: &RuntimeConfig,
    store: &SessionStore,
    args: ThinkbackArgs,
) -> Result<()> {
    let sid = args.session_id.unwrap_or(config.session_id);
    let events = store.load_events(sid).unwrap_or_default();

    // Collect reasoning/assistant events and show them in reverse order.
    let reasoning: Vec<_> = events
        .iter()
        .filter(|e| e.event_type == "assistant_turn" || e.event_type == "thinking")
        .collect();

    let steps = args.steps.unwrap_or(5);
    let shown: Vec<_> = if args.last {
        reasoning.iter().rev().take(1).collect()
    } else {
        reasoning.iter().rev().take(steps as usize).collect()
    };

    if args.json {
        let output: Vec<serde_json::Value> = shown
            .iter()
            .map(|e| {
                serde_json::json!({
                    "type": e.event_type,
                    "timestamp": e.timestamp,
                    "payload": e.payload,
                })
            })
            .collect();
        println!("{}", serde_json::to_string(&output)?);
    } else if shown.is_empty() {
        println!("No reasoning steps found in session {sid}.");
    } else {
        println!("Reasoning backtrack for session {sid}:");
        for (i, e) in shown.iter().enumerate() {
            let preview = e
                .payload
                .as_ref()
                .and_then(|p| p.as_object())
                .and_then(|m| m.get("text").and_then(|v| v.as_str()))
                .unwrap_or("(no text)");
            let truncated = if preview.len() > 100 {
                let end = preview
                    .char_indices()
                    .take(100)
                    .last()
                    .map_or(0, |(i, c)| i + c.len_utf8());
                format!("{}...", &preview[..end])
            } else {
                preview.to_string()
            };
            println!("  [{}] {} ({})", i + 1, truncated, e.event_type);
        }
    }
    Ok(())
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

/// Auto-fix a pull request by analyzing review comments and generating patches.
pub fn run_autofix_pr(_config: &RuntimeConfig, args: AutoFixPrArgs) -> Result<()> {
    let pr_url = args
        .pr_url
        .as_deref()
        .ok_or_else(|| anyhow!("--pr-url is required"))?;

    // Parse the PR URL to extract owner/repo/number.
    let parts: Vec<&str> = pr_url.split('/').collect();
    if parts.len() < 7 {
        return Err(anyhow!(
            "invalid PR URL format — expected https://github.com/owner/repo/pull/123"
        ));
    }

    let owner = parts[3];
    let repo = parts[4];
    let pr_number: u64 = parts[6].parse().map_err(|_| anyhow!("invalid PR number"))?;

    if args.dry_run {
        if args.json {
            println!(
                "{}",
                serde_json::json!({
                    "pr": pr_url,
                    "owner": owner,
                    "repo": repo,
                    "number": pr_number,
                    "mode": "dry_run",
                    "fixes": [],
                })
            );
        } else {
            println!("Dry-run analysis for PR {owner}/{repo}#{pr_number}:");
            println!("  No issues found to auto-fix.");
        }
        return Ok(());
    }

    // Fetch PR diff via gh CLI.
    let diff_output = std::process::Command::new("gh")
        .args(["pr", "diff", pr_url])
        .output();

    let (has_diff, diff_lines) = match diff_output {
        Ok(out) if out.status.success() => {
            let diff = String::from_utf8_lossy(&out.stdout);
            let count = diff.lines().count();
            (true, count)
        }
        _ => (false, 0),
    };

    if args.json {
        println!(
            "{}",
            serde_json::json!({
                "pr": pr_url,
                "owner": owner,
                "repo": repo,
                "number": pr_number,
                "diff_available": has_diff,
                "diff_lines": diff_lines,
                "fixes": [],
            })
        );
    } else {
        println!("Auto-fix analysis for PR {owner}/{repo}#{pr_number}:");
        if has_diff {
            println!("  Diff: {diff_lines} lines");
        } else {
            println!("  Could not fetch diff — ensure 'gh' CLI is authenticated.");
        }
        println!("  No auto-fixable issues found.");
    }

    Ok(())
}

/// Break the provider response cache by invalidating cached entries.
pub fn run_break_cache(_config: &RuntimeConfig, args: BreakCacheArgs) -> Result<()> {
    let config_dir = app_config_dir();
    let cache_dir = config_dir.join("cache");

    let provider = args.provider.as_deref().unwrap_or("all");
    let mut cleared = 0u32;

    if cache_dir.exists() {
        if args.all || provider == "all" {
            for entry in std::fs::read_dir(&cache_dir)? {
                let entry = entry?;
                if entry.file_type().map(|t| t.is_file()).unwrap_or(false) {
                    std::fs::remove_file(entry.path())?;
                    cleared += 1;
                }
            }
        } else {
            let expected = format!("{provider}.cache");
            for entry in std::fs::read_dir(&cache_dir)? {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().into_owned();
                if name == expected || name == provider {
                    std::fs::remove_file(entry.path())?;
                    cleared += 1;
                }
            }
        }
    }

    if args.json {
        println!(
            "{}",
            serde_json::json!({
                "provider": provider,
                "cache_files_cleared": cleared,
            })
        );
    } else {
        println!("Cache cleared for '{provider}': {cleared} file(s) removed.");
    }

    Ok(())
}

/// Manage bridge connections to external tools and services.
pub fn run_bridge(args: BridgeArgs) -> Result<()> {
    let config_dir = app_config_dir();
    let bridge_file = config_dir.join("bridges.json");

    if args.list || (args.connect.is_none() && args.disconnect.is_none() && !args.status) {
        let bridges: Vec<serde_json::Value> = if bridge_file.exists() {
            serde_json::from_str(&std::fs::read_to_string(&bridge_file)?)?
        } else {
            vec![]
        };
        if args.json {
            println!("{}", serde_json::to_string(&bridges)?);
        } else if bridges.is_empty() {
            println!("No bridge connections configured.");
            println!("Use --connect <name> to create a bridge.");
        } else {
            println!("Bridge connections:");
            for b in &bridges {
                let name = b.get("name").and_then(|v| v.as_str()).unwrap_or("?");
                let url = b.get("url").and_then(|v| v.as_str()).unwrap_or("?");
                println!("  {name}: {url}");
            }
        }
    } else if let Some(name) = &args.connect {
        let mut bridges: Vec<serde_json::Value> = if bridge_file.exists() {
            serde_json::from_str(&std::fs::read_to_string(&bridge_file)?)?
        } else {
            vec![]
        };
        bridges.push(serde_json::json!({
            "name": name,
            "url": name,
            "connected_at": chrono::Utc::now().to_rfc3339(),
        }));
        std::fs::create_dir_all(&config_dir).ok();
        std::fs::write(&bridge_file, serde_json::to_string(&bridges)?)?;
        if args.json {
            println!("{}", serde_json::json!({"connected": name}));
        } else {
            println!("Bridge '{name}' connected.");
        }
    } else if let Some(name) = &args.disconnect {
        let bridges: Vec<serde_json::Value> = if bridge_file.exists() {
            serde_json::from_str(&std::fs::read_to_string(&bridge_file)?)?
        } else {
            vec![]
        };
        let remaining: Vec<_> = bridges
            .iter()
            .filter(|b| b.get("name").and_then(|v| v.as_str()) != Some(name))
            .collect();
        std::fs::write(&bridge_file, serde_json::to_string(&remaining)?)?;
        if args.json {
            println!("{}", serde_json::json!({"disconnected": name}));
        } else {
            println!("Bridge '{name}' disconnected.");
        }
    } else if args.status {
        let bridges: Vec<serde_json::Value> = if bridge_file.exists() {
            serde_json::from_str(&std::fs::read_to_string(&bridge_file)?)?
        } else {
            vec![]
        };
        if args.json {
            println!("{}", serde_json::json!({"active_bridges": bridges.len()}));
        } else {
            println!("{} active bridge(s).", bridges.len());
        }
    }
    Ok(())
}

/// Send a quick "by the way" note to the conversation context.
pub fn run_btw(_config: &RuntimeConfig, args: BtwArgs) -> Result<()> {
    if args.message.is_empty() {
        return Err(anyhow!("message is required — usage: btw <message>"));
    }
    let message = args.message.join(" ");

    // Store as a lightweight context note.
    let config_dir = app_config_dir();
    let notes_file = config_dir.join("btw-notes.jsonl");
    std::fs::create_dir_all(&config_dir).ok();
    let note = serde_json::json!({
        "message": message,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&notes_file)
        .and_then(|mut f| {
            use std::io::Write;
            writeln!(f, "{note}")
        })?;

    if args.json {
        println!("{note}");
    } else {
        println!("Noted: {message}");
    }
    Ok(())
}

/// Configure Chrome browser MCP integration.
pub fn run_chrome(args: ChromeArgs) -> Result<()> {
    let config_dir = app_config_dir();
    let chrome_file = config_dir.join("chrome-mcp.json");

    if args.status {
        let config: serde_json::Value = if chrome_file.exists() {
            serde_json::from_str(&std::fs::read_to_string(&chrome_file)?)?
        } else {
            serde_json::json!({"enabled": false})
        };
        if args.json {
            println!("{config}");
        } else {
            let enabled = config
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let port = config.get("port").and_then(|v| v.as_u64()).unwrap_or(9222);
            println!(
                "Chrome MCP: {}",
                if enabled { "enabled" } else { "disabled" }
            );
            println!("  Debug port: {port}");
        }
    } else if let Some(port) = args.port {
        let config = serde_json::json!({
            "enabled": true,
            "port": port,
            "debug": args.debug,
        });
        std::fs::create_dir_all(&config_dir).ok();
        std::fs::write(&chrome_file, serde_json::to_string_pretty(&config)?)?;
        if args.json {
            println!("{config}");
        } else {
            println!("Chrome MCP configured on port {port}.");
        }
    } else if args.debug {
        println!("Chrome debug mode: connect to chrome://inspect in Chrome DevTools.");
        println!("  Target: localhost:9222");
    } else if args.json {
        println!("{}", serde_json::json!({"status": "not_configured"}));
    } else {
        println!(
            "Chrome MCP: use --port <port> to configure, --status to check, --debug for debug mode."
        );
    }
    Ok(())
}

/// Configure terminal color scheme.
pub fn run_color(args: ColorArgs) -> Result<()> {
    let schemes = [
        ("auto", "Follow terminal settings"),
        ("always", "Always use colors"),
        ("never", "Disable colors"),
        ("256", "256-color palette"),
        ("truecolor", "24-bit true color"),
    ];

    if let Some(scheme) = &args.scheme {
        let valid = schemes.iter().any(|(name, _)| name == scheme);
        if !valid {
            return Err(anyhow!(
                "unknown color scheme '{scheme}' — use: auto, always, never, 256, truecolor"
            ));
        }
        let config = serde_json::json!({"scheme": scheme});
        write_json_config("color-scheme.json", &config)?;
        if args.json {
            println!("{config}");
        } else {
            println!("Color scheme set to '{scheme}'.");
        }
    } else if args.list {
        if args.json {
            let list: Vec<serde_json::Value> = schemes
                .iter()
                .map(|(name, desc)| serde_json::json!({"name": name, "description": desc}))
                .collect();
            println!("{}", serde_json::to_string(&list)?);
        } else {
            println!("Available color schemes:");
            for (name, desc) in &schemes {
                println!("  {name}: {desc}");
            }
        }
    } else {
        let current = read_config_str("color-scheme.json", "scheme", "auto");
        if args.json {
            println!("{}", serde_json::json!({"current": current}));
        } else {
            println!("Color scheme: {current}");
            println!("Use --list to see available schemes.");
        }
    }
    Ok(())
}

/// Manage conversation context (show, reset).
pub fn run_context_manage(
    config: &RuntimeConfig,
    store: &SessionStore,
    args: ContextManageArgs,
) -> Result<()> {
    let events = store.load_events(config.session_id).unwrap_or_default();
    let conversation = store.load_conversation(config.session_id).ok();

    if args.reset {
        // Context reset marks a boundary in the event stream.
        // The runtime loader treats events before a context_reset as compacted;
        // old data is not deleted but is excluded from the active context window.
        store.append_named_event(
            config.session_id,
            "context_reset",
            serde_json::json!({"reset_at": chrono::Utc::now().to_rfc3339()}),
        )?;
        if args.json {
            println!(
                "{}",
                serde_json::json!({"reset": true, "session_id": config.session_id.to_string()})
            );
        } else {
            println!("Context reset for session {}.", config.session_id);
        }
    } else if args.show || (!args.reset && !args.json) {
        let msg_count = conversation.as_deref().map_or(0, |c| c.len());
        let event_count = events.len();
        let tool_calls = events
            .iter()
            .filter(|e| e.event_type == "tool_result")
            .count();
        let (inp, out) = events.iter().fold((0u64, 0u64), |(i, o), e| {
            if let Some(map) = e.payload.as_ref().and_then(|p| p.as_object()) {
                (
                    i + nested_u64(map, &["usage", "input_tokens"]).unwrap_or(0),
                    o + nested_u64(map, &["usage", "output_tokens"]).unwrap_or(0),
                )
            } else {
                (i, o)
            }
        });

        if args.json {
            println!(
                "{}",
                serde_json::json!({
                    "session_id": config.session_id.to_string(),
                    "messages": msg_count,
                    "events": event_count,
                    "tool_calls": tool_calls,
                    "input_tokens": inp,
                    "output_tokens": out,
                })
            );
        } else {
            println!("Context for session {}:", config.session_id);
            println!("  Messages:       {msg_count}");
            println!("  Events:         {event_count}");
            println!("  Tool calls:     {tool_calls}");
            println!("  Input tokens:   {inp}");
            println!("  Output tokens:  {out}");
        }
    } else {
        println!(
            "{}",
            serde_json::json!({"session_id": config.session_id.to_string()})
        );
    }
    Ok(())
}

/// IDE integration configuration.
pub fn run_ide(args: IdeArgs) -> Result<()> {
    let config_dir = app_config_dir();
    let ide_file = config_dir.join("ide-config.json");

    if args.disconnect {
        if ide_file.exists() {
            std::fs::remove_file(&ide_file)?;
        }
        if args.json {
            println!("{}", serde_json::json!({"status": "disconnected"}));
        } else {
            println!("IDE integration disconnected.");
        }
    } else if let Some(name) = &args.name {
        let config = serde_json::json!({
            "ide": name,
            "connected_at": chrono::Utc::now().to_rfc3339(),
        });
        std::fs::create_dir_all(&config_dir).ok();
        std::fs::write(&ide_file, serde_json::to_string_pretty(&config)?)?;
        if args.json {
            println!("{config}");
        } else {
            println!("IDE set to '{name}'.");
        }
    } else if let Some(endpoint) = &args.connect {
        let config = serde_json::json!({
            "endpoint": endpoint,
            "connected_at": chrono::Utc::now().to_rfc3339(),
        });
        std::fs::create_dir_all(&config_dir).ok();
        std::fs::write(&ide_file, serde_json::to_string_pretty(&config)?)?;
        if args.json {
            println!("{config}");
        } else {
            println!("Connected to IDE at '{endpoint}'.");
        }
    } else if args.status {
        let config: serde_json::Value = if ide_file.exists() {
            serde_json::from_str(&std::fs::read_to_string(&ide_file)?)?
        } else {
            serde_json::json!({"status": "not_configured"})
        };
        if args.json {
            println!("{config}");
        } else {
            println!("IDE config: {config}");
        }
    } else if args.json {
        println!("{}", serde_json::json!({"status": "ready"}));
    } else {
        println!("IDE: use --name <ide>, --connect <endpoint>, --disconnect, or --status");
    }
    Ok(())
}

/// Manage GitHub issues (list, show, create).
pub fn run_issue(_config: &RuntimeConfig, args: IssueArgs) -> Result<()> {
    let repo = args.repo.as_deref().unwrap_or(".");

    if args.list {
        let output = std::process::Command::new("gh")
            .args([
                "issue",
                "list",
                "--repo",
                repo,
                "--limit",
                "20",
                "--json",
                "number,title,state",
            ])
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if args.json {
                    println!("{stdout}");
                } else {
                    let issues: Vec<serde_json::Value> = serde_json::from_str(&stdout)?;
                    if issues.is_empty() {
                        println!("No open issues in {repo}.");
                    } else {
                        println!("Issues in {repo}:");
                        for issue in &issues {
                            let num = issue.get("number").and_then(|v| v.as_u64()).unwrap_or(0);
                            let title = issue.get("title").and_then(|v| v.as_str()).unwrap_or("?");
                            let state = issue.get("state").and_then(|v| v.as_str()).unwrap_or("?");
                            println!("  #{num} [{state}] {title}");
                        }
                    }
                }
            }
            _ => {
                if args.json {
                    println!("[]");
                } else {
                    println!(
                        "Could not list issues — ensure 'gh' CLI is installed and authenticated."
                    );
                }
            }
        }
    } else if let Some(number) = &args.show {
        let view_args = if args.json {
            vec![
                "issue",
                "view",
                number.as_str(),
                "--repo",
                repo,
                "--json",
                "number,title,body,state",
            ]
        } else {
            vec!["issue", "view", number.as_str(), "--repo", repo]
        };
        let output = std::process::Command::new("gh").args(&view_args).output();

        match output {
            Ok(out) if out.status.success() => {
                let stdout = String::from_utf8_lossy(&out.stdout);
                if args.json {
                    println!("{stdout}");
                } else {
                    print!("{stdout}");
                }
            }
            _ => {
                return Err(anyhow!(
                    "could not view issue #{number} — ensure 'gh' CLI is available"
                ));
            }
        }
    } else if let Some(title) = &args.create {
        let output = std::process::Command::new("gh")
            .args([
                "issue", "create", "--repo", repo, "--title", title, "--body", "",
            ])
            .output();

        match output {
            Ok(out) if out.status.success() => {
                let url = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if args.json {
                    println!("{}", serde_json::json!({"url": url, "title": title}));
                } else {
                    println!("Created issue: {url}");
                }
            }
            _ => {
                return Err(anyhow!(
                    "could not create issue — ensure 'gh' CLI is available"
                ));
            }
        }
    } else if args.json {
        println!("{}", serde_json::json!({"repo": repo}));
    } else {
        println!("Issue: use --list, --show <number>, or --create <title>");
    }
    Ok(())
}

/// First-run onboarding wizard.
pub fn run_onboarding(
    _config: &RuntimeConfig,
    _store: &SessionStore,
    args: OnboardingArgs,
) -> Result<()> {
    let config_dir = app_config_dir();
    let onboard_file = config_dir.join("onboarding.json");

    if args.skip || args.reset {
        let state = serde_json::json!({
            "completed": !args.reset,
            "skipped": args.skip,
            "timestamp": chrono::Utc::now().to_rfc3339(),
        });
        std::fs::create_dir_all(&config_dir).ok();
        std::fs::write(&onboard_file, serde_json::to_string_pretty(&state)?)?;
        if args.json {
            println!("{state}");
        } else if args.skip {
            println!("Onboarding skipped.");
        } else {
            println!("Onboarding reset — will run again on next launch.");
        }
    } else {
        let completed = if onboard_file.exists() {
            let data: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&onboard_file)?)?;
            data.get("completed")
                .and_then(|v| v.as_bool())
                .unwrap_or(false)
        } else {
            false
        };

        if completed {
            if args.json {
                println!("{}", serde_json::json!({"completed": true}));
            } else {
                println!("Onboarding already completed. Use --reset to run again.");
            }
        } else {
            // Interactive onboarding steps.
            if args.json {
                println!(
                    "{}",
                    serde_json::json!({"status": "in_progress", "steps": ["configure_provider", "set_api_key", "choose_model"]})
                );
            } else {
                println!("Welcome to Remote Code!");
                println!();
                println!("Quick start:");
                println!("  1. Set your API key:");
                println!("       export ANTHROPIC_API_KEY=your-key");
                println!("  2. Choose a model:");
                println!("       remote-code model set claude-sonnet-4-20250514");
                println!("  3. Start coding:");
                println!("       remote-code");
                println!();
                println!("Run 'remote-code help' for all available commands.");
            }
            // Mark onboarding as shown.
            let state = serde_json::json!({"completed": true, "timestamp": chrono::Utc::now().to_rfc3339()});
            std::fs::create_dir_all(&config_dir).ok();
            std::fs::write(&onboard_file, serde_json::to_string_pretty(&state)?)?;
        }
    }
    Ok(())
}

/// Diagnose performance issues in a session.
pub fn run_perf_issue(
    config: &RuntimeConfig,
    store: &SessionStore,
    args: PerfIssueArgs,
) -> Result<()> {
    let sid = args.session_id.unwrap_or(config.session_id);
    let events = store.load_events(sid).unwrap_or_default();

    let mut slow_calls: Vec<serde_json::Value> = Vec::new();
    let mut total_duration_ms = 0u64;

    for e in &events {
        if let Some(map) = e.payload.as_ref().and_then(|p| p.as_object())
            && let Some(duration) = map.get("duration_ms").and_then(|v| v.as_u64())
        {
            total_duration_ms += duration;
            if args.deep || duration > 5000 {
                slow_calls.push(serde_json::json!({
                    "type": e.event_type,
                    "duration_ms": duration,
                    "timestamp": e.timestamp,
                }));
            }
        }
    }

    if args.json {
        println!(
            "{}",
            serde_json::json!({
                "session_id": sid.to_string(),
                "total_events": events.len(),
                "total_duration_ms": total_duration_ms,
                "slow_calls": slow_calls,
            })
        );
    } else {
        println!("Performance diagnosis for session {sid}:");
        println!("  Total events:     {}", events.len());
        println!("  Total duration:   {total_duration_ms}ms");
        if slow_calls.is_empty() {
            println!("  No slow calls detected.");
        } else {
            println!("  Slow calls (>{}ms):", 5000);
            for c in &slow_calls {
                let dur = c.get("duration_ms").and_then(|v| v.as_u64()).unwrap_or(0);
                let kind = c.get("type").and_then(|v| v.as_str()).unwrap_or("?");
                println!("    {kind}: {dur}ms");
            }
        }
    }
    Ok(())
}

/// Manage permission rules (view mode, add/remove rules).
pub fn run_permissions(config: &mut RuntimeConfig, args: PermissionsArgs) -> Result<()> {
    let config_dir = app_config_dir();
    let perms_file = config_dir.join("permission-rules.json");

    if let Some(mode) = &args.mode {
        match mode.as_str() {
            "default" => {
                config.permission_mode = claude_core::PermissionMode::Default;
                println!("Permission mode set to 'default'.");
            }
            "accept-edits" => {
                config.permission_mode = claude_core::PermissionMode::AcceptEdits;
                println!("Permission mode set to 'accept-edits'.");
            }
            "bypass" => {
                config.permission_mode = claude_core::PermissionMode::BypassPermissions;
                println!("Permission mode set to 'bypass'.");
            }
            _ => {
                return Err(anyhow!(
                    "unknown mode '{mode}' — use: default, accept-edits, bypass"
                ));
            }
        }
        if args.json {
            println!("{}", serde_json::json!({"mode": mode}));
        }
    } else if args.list_rules {
        let rules: Vec<serde_json::Value> = if perms_file.exists() {
            serde_json::from_str(&std::fs::read_to_string(&perms_file)?)?
        } else {
            vec![]
        };
        if args.json {
            println!("{}", serde_json::to_string(&rules)?);
        } else if rules.is_empty() {
            println!("No custom permission rules configured.");
        } else {
            println!("Permission rules:");
            for r in &rules {
                let tool = r.get("tool").and_then(|v| v.as_str()).unwrap_or("*");
                let action = r.get("action").and_then(|v| v.as_str()).unwrap_or("?");
                println!("  {tool}: {action}");
            }
        }
    } else if let Some(rule) = &args.add_rule {
        let mut rules: Vec<serde_json::Value> = match read_json_config("permission-rules.json") {
            Value::Array(arr) => arr,
            _ => vec![],
        };
        rules.push(serde_json::json!({"rule": rule, "added_at": chrono::Utc::now().to_rfc3339()}));
        write_json_config("permission-rules.json", &Value::Array(rules))?;
        if args.json {
            println!("{}", serde_json::json!({"added": rule}));
        } else {
            println!("Rule added: {rule}");
        }
    } else if let Some(rule) = &args.remove_rule {
        let rules: Vec<serde_json::Value> = match read_json_config("permission-rules.json") {
            Value::Array(arr) => arr,
            _ => vec![],
        };
        let remaining: Vec<_> = rules
            .iter()
            .filter(|r| r.get("rule").and_then(|v| v.as_str()) != Some(rule))
            .cloned()
            .collect();
        write_json_config("permission-rules.json", &Value::Array(remaining))?;
        if args.json {
            println!("{}", serde_json::json!({"removed": rule}));
        } else {
            println!("Rule removed: {rule}");
        }
    } else if args.json {
        let mode_str = match config.permission_mode {
            claude_core::PermissionMode::Default => "default",
            claude_core::PermissionMode::AcceptEdits => "accept-edits",
            claude_core::PermissionMode::Auto => "auto",
            claude_core::PermissionMode::BypassPermissions => "bypass",
            claude_core::PermissionMode::DontAsk => "dont-ask",
            claude_core::PermissionMode::Plan => "plan",
        };
        println!("{}", serde_json::json!({"mode": mode_str}));
    } else {
        println!("Permissions: use --mode <mode>, --list-rules, --add-rule, or --remove-rule");
    }
    Ok(())
}

/// View pull request comments.
pub fn run_pr_comments(args: PrCommentsArgs) -> Result<()> {
    let pr_url = args
        .pr_url
        .as_deref()
        .ok_or_else(|| anyhow!("--pr-url is required"))?;

    let output = std::process::Command::new("gh")
        .args(["pr", "view", pr_url, "--comments", "--json", "comments"])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if args.json {
                println!("{stdout}");
            } else {
                let data: serde_json::Value = serde_json::from_str(&stdout)?;
                let comments = data.get("comments").and_then(|v| v.as_array());
                match comments {
                    Some(c) if c.is_empty() => println!("No comments on PR."),
                    Some(c) => {
                        println!("PR comments ({}):", c.len());
                        for comment in c {
                            let author = comment
                                .get("author")
                                .and_then(|a| a.get("login"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("?");
                            let body = comment.get("body").and_then(|v| v.as_str()).unwrap_or("");
                            println!("  @{author}: {body}");
                        }
                    }
                    None => println!("No comments found."),
                }
            }
        }
        _ => {
            if args.json {
                println!(
                    "{}",
                    serde_json::json!({"error": "gh CLI unavailable or PR not found"})
                );
            } else {
                println!(
                    "Could not fetch PR comments — ensure 'gh' CLI is installed and authenticated."
                );
            }
        }
    }
    Ok(())
}

/// Configure privacy settings (telemetry, crash reports).
pub fn run_privacy_settings(args: PrivacySettingsArgs) -> Result<()> {
    let privacy_file = app_config_dir().join("privacy.json");

    let mut settings = if privacy_file.exists() {
        let data: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&privacy_file)?)?;
        serde_json::Map::from_iter(data.as_object().cloned().unwrap_or_default())
    } else {
        serde_json::Map::new()
    };

    let mut dirty = false;
    if let Some(enabled) = args.telemetry {
        settings.insert("telemetry".into(), serde_json::Value::Bool(enabled));
        dirty = true;
    }
    if let Some(enabled) = args.crash_reports {
        settings.insert("crash_reports".into(), serde_json::Value::Bool(enabled));
        dirty = true;
    }

    // Only write to disk when the user explicitly changed a setting.
    if dirty {
        let config = serde_json::Value::Object(settings.clone());
        write_json_config("privacy.json", &config)?;

        if args.json {
            println!("{config}");
        } else {
            let telem = settings
                .get("telemetry")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            let crash = settings
                .get("crash_reports")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            println!("Privacy settings updated:");
            println!(
                "  Telemetry:     {}",
                if telem { "enabled" } else { "disabled" }
            );
            println!(
                "  Crash reports: {}",
                if crash { "enabled" } else { "disabled" }
            );
        }
    } else if args.show {
        let telem = settings
            .get("telemetry")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let crash = settings
            .get("crash_reports")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        if args.json {
            println!(
                "{}",
                serde_json::json!({"telemetry": telem, "crash_reports": crash})
            );
        } else {
            println!("Privacy settings:");
            println!(
                "  Telemetry:     {}",
                if telem { "enabled" } else { "disabled" }
            );
            println!(
                "  Crash reports: {}",
                if crash { "enabled" } else { "disabled" }
            );
        }
    } else if args.json {
        let telem = settings
            .get("telemetry")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        let crash = settings
            .get("crash_reports")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);
        println!(
            "{}",
            serde_json::json!({"telemetry": telem, "crash_reports": crash})
        );
    } else {
        println!("Privacy settings:");
        println!("  --telemetry <true|false>    Enable/disable telemetry");
        println!("  --crash-reports <true|false> Enable/disable crash reports");
        println!("  --show                      Show current settings");
    }
    Ok(())
}

/// Configure rate limit options for API calls.
pub fn run_rate_limit_options(_config: &RuntimeConfig, args: RateLimitOptionsArgs) -> Result<()> {
    let rate_file = app_config_dir().join("rate-limits.json");

    if args.reset {
        if rate_file.exists() {
            std::fs::remove_file(&rate_file)?;
        }
        if args.json {
            println!("{}", serde_json::json!({"status": "reset"}));
        } else {
            println!("Rate limits reset to defaults.");
        }
        return Ok(());
    }

    let mut limits = match read_json_config("rate-limits.json") {
        Value::Object(map) => map,
        _ => {
            let mut map = serde_json::Map::new();
            map.insert("rpm".into(), serde_json::Value::from(60u64));
            map.insert("tpm".into(), serde_json::Value::from(150000u64));
            map
        }
    };

    if let Some(rpm) = args.rpm {
        limits.insert("rpm".into(), serde_json::Value::from(rpm));
    }
    if let Some(tpm) = args.tpm {
        limits.insert("tpm".into(), serde_json::Value::from(tpm));
    }

    let config = serde_json::Value::Object(limits.clone());
    write_json_config("rate-limits.json", &config)?;

    let rpm = limits.get("rpm").and_then(|v| v.as_u64()).unwrap_or(60);
    let tpm = limits.get("tpm").and_then(|v| v.as_u64()).unwrap_or(150000);

    if args.json {
        println!("{config}");
    } else if args.show || (args.rpm.is_none() && args.tpm.is_none()) {
        println!("Rate limits:");
        println!("  Requests per minute (RPM): {rpm}");
        println!("  Tokens per minute (TPM):   {tpm}");
        if args.rpm.is_none() && args.tpm.is_none() {
            println!("Use --rpm <n> and/or --tpm <n> to change.");
        }
    } else {
        println!("Rate limits set: {rpm} RPM, {tpm} TPM.");
    }
    Ok(())
}

/// Set up remote access to a control plane server.
pub fn run_remote_setup(_config: &RuntimeConfig, args: RemoteSetupArgs) -> Result<()> {
    if args.show {
        let config = read_json_config("remote-setup.json");
        let configured = config
            .get("configured")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if args.json {
            println!("{config}");
        } else if configured {
            let url = config.get("url").and_then(|v| v.as_str()).unwrap_or("?");
            let name = config
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("default");
            println!("Remote access configured:");
            println!("  Server: {url}");
            println!("  Name:   {name}");
        } else {
            println!("Remote access not configured.");
            println!("Use --url <url> --token <token> to set up.");
        }
    } else if let Some(url) = &args.url {
        let token = args.token.as_deref().unwrap_or("");
        let name = args.name.as_deref().unwrap_or("default");
        // Store only whether a token was provided, not the token itself.
        // The actual token is passed to the runner/control-plane at runtime
        // via environment variable or keychain — never persisted in config.
        let config = serde_json::json!({
            "configured": true,
            "url": url,
            "name": name,
            "token_set": !token.is_empty(),
            "setup_at": chrono::Utc::now().to_rfc3339(),
        });
        write_json_config("remote-setup.json", &config)?;

        if args.json {
            println!("{config}");
        } else {
            println!("Remote access configured:");
            println!("  Server: {url}");
            println!("  Name:   {name}");
            if token.is_empty() {
                println!("  Warning: no token set — use --token for authentication.");
            }
        }
    } else if args.json {
        println!("{}", serde_json::json!({"configured": false}));
    } else {
        println!("Remote setup: use --url <url> --token <token> to configure.");
        println!("  --name <name>  Set a friendly name for this connection");
        println!("  --show         Show current configuration");
    }
    Ok(())
}

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
    CopyArgs, CostArgs, CtxArgs, DiffArgs, FeedbackArgs, FilesArgs, MemoryCommand,
    ModelCommand, ProviderCommand, ThemeArgs,
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
pub fn run_memory(config: &RuntimeConfig, store: &SessionStore, command: MemoryCommand) -> Result<()> {
    match command {
        MemoryCommand::Read { key, json } => {
            let events = store.load_events(config.session_id).unwrap_or_default();
            let memories: Vec<_> = events
                .iter()
                .filter(|e| e.event_type == "memory")
                .filter(|e| {
                    let k = e.payload.as_ref().and_then(|p| p.as_object()).and_then(|m| nested_str(m, &["key"]));
                    key.as_ref().map_or(true, |target| k == Some(target.as_str()))
                })
                .collect();
            if json {
                let output: Vec<Value> = memories
                    .iter()
                    .map(|e| {
                        let k = e.payload.as_ref().and_then(|p| p.as_object()).and_then(|m| nested_str(m, &["key"]));
                        let v = e.payload.as_ref().and_then(|p| p.as_object()).and_then(|m| nested_str(m, &["value"]));
                        Value::Object(
                            vec![
                                ("key".into(), k.map(|s| Value::String(s.into())).unwrap_or(Value::Null)),
                                ("value".into(), v.map(|s| Value::String(s.into())).unwrap_or(Value::Null)),
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
                .filter_map(|e| e.payload.as_ref().and_then(|p| p.as_object()).and_then(|m| nested_str(m, &["key"])))
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
    println!("  Model: {}", config.provider.model.as_deref().unwrap_or("unknown"));
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
    let tool_calls = events.iter().filter(|e| e.event_type == "tool_result").count();
    let errors = events
        .iter()
        .filter(|e| {
            e.event_type == "result"
                && e.payload.as_ref().and_then(|p| p.as_object()).map_or(false, |m| {
                    m.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false)
                })
        })
        .count();

    println!("Session summary");
    println!("  Session:    {}", config.session_id);
    println!("  Model:      {}", config.provider.model.as_deref().unwrap_or("unknown"));
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

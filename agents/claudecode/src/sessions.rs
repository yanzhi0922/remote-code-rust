use anyhow::Result;
use claude_core::ConversationRole;
use claude_session::{SessionBundle, SessionStore, SessionSummary};
use serde::Serialize;
use uuid::Uuid;

use crate::cli::{
    ExportArgs, ExportFormat, SessionBackfillArgs, SessionRenameArgs, SessionRewindArgs,
    SessionShareArgs, SessionTagArgs, SessionsCommand, SessionsStatsArgs,
};
use crate::conversation::truncate_preview;

#[derive(Debug, Clone, Serialize)]
struct SessionStatsRow {
    session_id: Uuid,
    title: String,
    provider_name: String,
    model: Option<String>,
    updated_at: String,
    archived: bool,
    total_events: usize,
    conversation_entries: usize,
    tool_call_count: usize,
    error_count: usize,
    input_tokens: u64,
    output_tokens: u64,
    last_stop_reason: Option<String>,
}

pub(crate) fn run_sessions(store: &SessionStore, command: Option<SessionsCommand>) -> Result<()> {
    match command.unwrap_or(SessionsCommand::List) {
        SessionsCommand::List => {
            let sessions = store.list_sessions()?;
            if sessions.is_empty() {
                println!("No sessions found.");
                return Ok(());
            }
            for session in sessions {
                println!(
                    "{}  {}  {}  {}",
                    session.session_id, session.updated_at, session.provider_name, session.title
                );
            }
            Ok(())
        }
        SessionsCommand::Show(args) => {
            let bundle = store.load_session_bundle(args.session_id)?;
            if args.json {
                println!("{}", serde_json::to_string_pretty(&bundle)?);
            } else {
                print_session_summary(&bundle.summary);
                println!("- transcript: {}", bundle.summary.transcript_path.display());
                println!("- events: {}", bundle.stats.total_events);
                println!("- messages: {}", bundle.stats.conversation_entries);
                println!(
                    "- usage: {} input / {} output",
                    bundle.stats.usage.input_tokens, bundle.stats.usage.output_tokens
                );
                if let Some(stop_reason) = &bundle.stats.last_stop_reason {
                    println!("- last stop reason: {stop_reason}");
                }
                if !bundle.conversation.is_empty() {
                    println!("\nRecent conversation:");
                    for entry in bundle.conversation.iter().rev().take(5).rev() {
                        println!(
                            "  {}: {}",
                            entry_role_label(&entry.role),
                            truncate_preview(&entry.history_text(), 120)
                        );
                    }
                }
            }
            Ok(())
        }
        SessionsCommand::Stats(args) => run_session_stats(store, args),
        SessionsCommand::Rename(args) => run_session_rename(store, args),
        SessionsCommand::Tag(args) => run_session_tag(store, args),
        SessionsCommand::Share(args) => run_session_share(store, args),
        SessionsCommand::Backfill(args) => run_session_backfill(store, args),
        SessionsCommand::Rewind(args) => run_session_rewind(store, args),
    }
}

pub(crate) fn run_export(store: &SessionStore, args: ExportArgs) -> Result<()> {
    let path = match args.format {
        ExportFormat::Ndjson => store.export_session(args.session_id, args.output)?,
        ExportFormat::Json => store.export_session_bundle_json(args.session_id, args.output)?,
    };
    println!("{}", path.display());
    Ok(())
}

pub(crate) fn print_session_summary(summary: &SessionSummary) {
    println!("Session {}", summary.session_id);
    println!("- title: {}", summary.title);
    println!("- cwd: {}", summary.cwd.display());
    println!("- provider: {}", summary.provider_name);
    println!(
        "- model: {}",
        summary.model.as_deref().unwrap_or("(missing)")
    );
    println!("- created: {}", summary.created_at);
    println!("- updated: {}", summary.updated_at);
}

fn entry_role_label(role: &ConversationRole) -> &'static str {
    match role {
        ConversationRole::System => "system",
        ConversationRole::User => "user",
        ConversationRole::Assistant => "assistant",
        ConversationRole::Tool => "tool",
    }
}

fn run_session_stats(store: &SessionStore, args: SessionsStatsArgs) -> Result<()> {
    let rows = collect_session_stats(store, args.session_id, args.limit.max(1))?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&rows)?);
        return Ok(());
    }
    if rows.is_empty() {
        println!("No session stats available.");
        return Ok(());
    }

    let total_input = rows.iter().map(|row| row.input_tokens).sum::<u64>();
    let total_output = rows.iter().map(|row| row.output_tokens).sum::<u64>();
    let total_tools = rows.iter().map(|row| row.tool_call_count).sum::<usize>();
    let total_errors = rows.iter().map(|row| row.error_count).sum::<usize>();
    println!(
        "Sessions: {}  input={}  output={}  tools={}  errors={}",
        rows.len(),
        total_input,
        total_output,
        total_tools,
        total_errors
    );
    for row in rows {
        println!(
            "{}  {}  {}  {}  in={} out={} tools={} err={} stop={}",
            row.session_id,
            row.updated_at,
            row.provider_name,
            row.title,
            row.input_tokens,
            row.output_tokens,
            row.tool_call_count,
            row.error_count,
            row.last_stop_reason.as_deref().unwrap_or("(none)")
        );
    }
    Ok(())
}

fn collect_session_stats(
    store: &SessionStore,
    session_id: Option<Uuid>,
    limit: usize,
) -> Result<Vec<SessionStatsRow>> {
    if let Some(session_id) = session_id {
        return Ok(vec![session_stats_row(store, session_id)?]);
    }

    store
        .list_sessions()?
        .into_iter()
        .take(limit)
        .map(|summary| session_stats_row(store, summary.session_id))
        .collect()
}

fn session_stats_row(store: &SessionStore, session_id: Uuid) -> Result<SessionStatsRow> {
    let bundle = store.load_session_bundle(session_id)?;
    Ok(SessionStatsRow {
        session_id: bundle.summary.session_id,
        title: bundle.summary.title,
        provider_name: bundle.summary.provider_name,
        model: bundle.summary.model,
        updated_at: bundle.summary.updated_at.to_rfc3339(),
        archived: bundle.summary.archived,
        total_events: bundle.stats.total_events,
        conversation_entries: bundle.stats.conversation_entries,
        tool_call_count: bundle.stats.tool_call_count,
        error_count: bundle.stats.error_count,
        input_tokens: bundle.stats.usage.input_tokens,
        output_tokens: bundle.stats.usage.output_tokens,
        last_stop_reason: bundle.stats.last_stop_reason,
    })
}

fn run_session_backfill(store: &SessionStore, args: SessionBackfillArgs) -> Result<()> {
    let bundle = store.load_session_bundle(args.session_id)?;
    if args.dry_run {
        if args.json {
            println!(
                "{{\"dry_run\": true, \"events\": {}}}",
                bundle.stats.total_events
            );
        } else {
            println!(
                "Backfill dry-run for session {}: {} events to process",
                args.session_id, bundle.stats.total_events
            );
        }
    } else {
        // Re-iterate events to rebuild accumulated stats and emit corrected
        // result events.  Uses load_session_bundle which internally uses
        // build_session_stats for accurate re-indexed stats.
        let events = store.load_events(args.session_id)?;
        let SessionBundle { stats, .. } = store.load_session_bundle(args.session_id)?;

        // Rewrite the transcript with the re-indexed events.
        store.rewrite_events(args.session_id, &events)?;

        if args.json {
            println!(
                "{{\"re_indexed\": true, \"session_id\": \"{}\", \"events\": {}, \"conversation_entries\": {}, \"tool_calls\": {}, \"errors\": {}, \"input_tokens\": {}, \"output_tokens\": {}}}",
                args.session_id,
                stats.total_events,
                stats.conversation_entries,
                stats.tool_call_count,
                stats.error_count,
                stats.usage.input_tokens,
                stats.usage.output_tokens
            );
        } else {
            println!(
                "Session {} backfill complete: {} events, {} conversation entries, {} input / {} output tokens",
                args.session_id,
                stats.total_events,
                stats.conversation_entries,
                stats.usage.input_tokens,
                stats.usage.output_tokens
            );
        }
    }
    Ok(())
}

fn run_session_rewind(store: &SessionStore, args: SessionRewindArgs) -> Result<()> {
    let events = store.load_events(args.session_id)?;
    let steps = args.steps.unwrap_or(1) as usize;

    // Find the truncation point.
    let truncate_at = if let Some(ref _cp) = args.to_checkpoint {
        // Find the named checkpoint event — keep events up to and including it.
        // For now, checkpoints are stored as named events with event_type "checkpoint".
        let mut found_idx: Option<usize> = None;
        for (i, event) in events.iter().enumerate().rev() {
            if event.event_type == "checkpoint" {
                if let Some(ref payload) = event.payload {
                    if payload.get("name").and_then(|v| v.as_str()) == Some(_cp.as_str()) {
                        found_idx = Some(i + 1); // keep including this event
                        break;
                    }
                }
            }
        }
        match found_idx {
            Some(idx) => idx,
            None => {
                anyhow::bail!("checkpoint '{}' not found in session {}", _cp, args.session_id);
            }
        }
    } else {
        // Rewind N conversation entries from the end.
        let mut conversation_count = 0usize;
        let mut truncate_idx = events.len();
        for (i, event) in events.iter().enumerate().rev() {
            if event.conversation.is_some() {
                conversation_count += 1;
                if conversation_count == steps {
                    truncate_idx = i;
                    break;
                }
            }
        }
        if conversation_count < steps {
            anyhow::bail!(
                "session {} only has {} conversation entries, cannot rewind {} steps",
                args.session_id,
                conversation_count,
                steps
            );
        }
        truncate_idx
    };

    let kept: Vec<_> = events.into_iter().take(truncate_at).collect();
    let removed_count = kept.len(); // Will be computed after
    let total = store.load_events(args.session_id)?.len();
    let removed = total - kept.len();

    store.rewrite_events(args.session_id, &kept)?;

    if args.json {
        println!("{}", serde_json::to_string(&serde_json::json!({
            "rewound": true,
            "session_id": args.session_id.to_string(),
            "kept_events": kept.len(),
            "removed_events": removed,
        }))?);
    } else {
        println!(
            "Session {} rewound: {} events kept, {} removed.",
            args.session_id,
            kept.len(),
            removed
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::collect_session_stats;
    use claude_config::{ProviderOverrides, RuntimeOverrides, load_runtime_config};
    use claude_core::{ConversationEntry, InputFormat, OutputFormat, PermissionMode};
    use claude_session::SessionStore;
    use tempfile::tempdir;

    #[test]
    fn collect_session_stats_reports_usage_and_stop_reason() {
        let temp = tempdir().expect("tempdir should work");
        let config = load_runtime_config(
            Some(temp.path().to_path_buf()),
            Some(temp.path().join(".remote-code-rust")),
            None,
            PermissionMode::Default,
            InputFormat::Text,
            OutputFormat::Text,
            false,
            false,
            false,
            false,
            8,
            ProviderOverrides {
                provider: Some("glm".to_owned()),
                base_url: Some("https://open.bigmodel.cn/api/paas/v4".to_owned()),
                api_key: Some("secret".to_owned()),
                model: Some("glm-5.1".to_owned()),
                protocol: Some(claude_core::ProviderProtocol::OpenAi),
            },
            RuntimeOverrides::default(),
        )
        .expect("config should load");
        let store = SessionStore::open(config.paths.clone()).expect("store should open");
        let entry = ConversationEntry::assistant("done");
        store
            .ensure_session(
                config.session_id,
                &config.cwd,
                &config.provider.name,
                config.provider.model.as_deref(),
                Some("stats-test"),
            )
            .expect("session should exist");
        store
            .append_conversation_entry(config.session_id, &entry)
            .expect("conversation append should work");
        store
            .append_named_event(
                config.session_id,
                "result",
                serde_json::json!({
                    "stop_reason": "end_turn",
                    "usage": {"input_tokens": 10, "output_tokens": 5}
                }),
            )
            .expect("usage append should work");

        let rows = collect_session_stats(&store, Some(config.session_id), 10)
            .expect("stats collection should work");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].input_tokens, 10);
        assert_eq!(rows[0].output_tokens, 5);
        assert_eq!(rows[0].last_stop_reason.as_deref(), Some("end_turn"));
    }
}

// ── Session subcommand implementations ────────────────────────────────

fn run_session_rename(store: &SessionStore, args: SessionRenameArgs) -> Result<()> {
    // Persist the name as a named event on the session.
    store.append_named_event(
        args.session_id,
        "session_name",
        serde_json::json!({"name": &args.name}),
    )?;
    println!("Session {} renamed to \"{}\".", args.session_id, args.name);
    Ok(())
}

fn run_session_tag(store: &SessionStore, args: SessionTagArgs) -> Result<()> {
    if let Some(tag_to_remove) = &args.remove {
        store.append_named_event(
            args.session_id,
            "tag_remove",
            serde_json::json!({"tag": tag_to_remove}),
        )?;
        println!(
            "Tag \"{tag_to_remove}\" removed from session {}.",
            args.session_id
        );
    } else if !args.tags.is_empty() {
        for tag in &args.tags {
            store.append_named_event(
                args.session_id,
                "tag_add",
                serde_json::json!({"tag": tag}),
            )?;
        }
        println!("Tags {:?} added to session {}.", args.tags, args.session_id);
    } else {
        let events = store.load_events(args.session_id)?;
        let tags: Vec<&str> = events
            .iter()
            .filter(|e| e.event_type == "tag_add")
            .filter_map(|e| {
                e.payload
                    .as_ref()
                    .and_then(|payload| payload.get("tag"))
                    .and_then(|v| v.as_str())
            })
            .collect();
        if tags.is_empty() {
            println!("Session {} has no tags.", args.session_id);
        } else {
            println!("Tags for session {}: {:?}", args.session_id, tags);
        }
    }
    Ok(())
}

fn run_session_share(store: &SessionStore, args: SessionShareArgs) -> Result<()> {
    let session_id = args.session_id.unwrap_or_else(|| {
        store
            .latest_active_session()
            .ok()
            .flatten()
            .map(|s| s.session_id)
            .unwrap_or(uuid::Uuid::nil())
    });
    if session_id.is_nil() {
        println!("No active session found to share.");
        return Ok(());
    }
    let format = args.format.unwrap_or(ExportFormat::Json);
    let path = match format {
        ExportFormat::Ndjson => store.export_session(session_id, args.output)?,
        ExportFormat::Json => store.export_session_bundle_json(session_id, args.output)?,
    };
    println!("Session {} shared to {}.", session_id, path.display());
    Ok(())
}

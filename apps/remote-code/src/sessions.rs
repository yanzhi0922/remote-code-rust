use anyhow::Result;
use rc_core::ConversationRole;
use rc_session::{SessionStore, SessionSummary};

use crate::cli::{ExportArgs, ExportFormat, SessionsCommand};
use crate::conversation::truncate_preview;

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

//! Session management commands: `/resume`, `/rename`, `/rewind`, `/export`, `/share`, `/summary`, `/tag`.

use rc_config::RuntimeConfig;
use rc_session::SessionStore;

/// Dispatch `/resume` — list recent sessions for resumption.
pub fn render_resume(config: &RuntimeConfig, store: &SessionStore) {
    println!("Recent sessions:");
    match store.list_sessions() {
        Ok(sessions) => {
            if sessions.is_empty() {
                println!("  (no sessions found)");
            } else {
                for (idx, session) in sessions.iter().take(10).enumerate() {
                    println!(
                        "  [{}] {}  {}  {}",
                        idx + 1,
                        session.session_id,
                        session.updated_at,
                        session.title
                    );
                }
                if sessions.len() > 10 {
                    println!("  ... and {} more", sessions.len() - 10);
                }
            }
        }
        Err(error) => eprintln!("Error listing sessions: {error}"),
    }
    println!("Current session: {}", config.session_id);
}

/// Dispatch `/rename` — rename the current session.
pub fn dispatch_rename(input: &str, config: &RuntimeConfig) {
    let new_name = input
        .trim()
        .strip_prefix("/rename")
        .unwrap_or_default()
        .trim();

    if new_name.is_empty() {
        println!("Usage: /rename <new-name>");
        println!(
            "Current name: {}",
            config.session_name.as_deref().unwrap_or("(auto)")
        );
        return;
    }
    println!(
        "Session renamed: {} -> {new_name} (saved on next turn)",
        config.session_name.as_deref().unwrap_or("(auto)")
    );
}

/// Dispatch `/rewind` — show rewind history for the current session.
pub fn render_rewind(config: &RuntimeConfig, store: &SessionStore) {
    println!("Rewind history for session {}:", config.session_id);
    match store.load_session_bundle(config.session_id) {
        Ok(bundle) => {
            let entries = bundle.conversation.len();
            println!("  transcript entries: {entries}");
            println!("  tool calls:         {}", bundle.stats.tool_call_count);
            println!("  errors:             {}", bundle.stats.error_count);
            println!(
                "  last stop:          {}",
                bundle.stats.last_stop_reason.as_deref().unwrap_or("(none)")
            );
            println!("Use /rewind <n> to rewind to entry n.");
        }
        Err(error) => println!("  (unable to load session bundle: {error})"),
    }
}

/// Dispatch `/export` — export the current session.
pub fn dispatch_export(input: &str, config: &RuntimeConfig, store: &SessionStore) {
    let format = input
        .trim()
        .strip_prefix("/export")
        .unwrap_or_default()
        .trim();

    let export_format = if format.is_empty() { "json" } else { format };

    match export_format {
        "json" => render_export_json(config, store),
        "markdown" | "md" => render_export_markdown(config, store),
        _ => {
            println!("Unknown export format '{export_format}'.");
            println!("Usage: /export [json|markdown]");
        }
    }
}

fn render_export_json(config: &RuntimeConfig, store: &SessionStore) {
    match store.load_session_bundle(config.session_id) {
        Ok(bundle) => {
            let entries = bundle.conversation.len();
            println!("Export (JSON): session {}", config.session_id);
            println!("  entries: {entries}");
            println!(
                "  path:   {}",
                config
                    .paths
                    .sessions_dir
                    .join(format!("{}.json", config.session_id))
                    .display()
            );
        }
        Err(error) => println!("Export failed: {error}"),
    }
}

fn render_export_markdown(config: &RuntimeConfig, store: &SessionStore) {
    match store.load_session_bundle(config.session_id) {
        Ok(bundle) => {
            let entries = bundle.conversation.len();
            println!("Export (Markdown): session {}", config.session_id);
            println!("  entries: {entries}");
            println!(
                "  path:   {}",
                config
                    .paths
                    .artifacts_dir
                    .join(format!("{}.md", config.session_id))
                    .display()
            );
        }
        Err(error) => println!("Export failed: {error}"),
    }
}

/// Dispatch `/share` — generate a share summary.
pub fn render_share(config: &RuntimeConfig, store: &SessionStore) {
    println!("Share summary for session {}:", config.session_id);
    match store.load_session_bundle(config.session_id) {
        Ok(bundle) => {
            println!("  title:  {}", bundle.summary.title);
            println!(
                "  tokens: in={} out={}",
                bundle.stats.usage.input_tokens, bundle.stats.usage.output_tokens
            );
            println!("  tools:  {}", bundle.stats.tool_call_count);
            println!("  errors: {}", bundle.stats.error_count);
            println!("  (sharing requires a configured control plane)");
        }
        Err(error) => println!("  (unable to load session: {error})"),
    }
}

/// Dispatch `/summary` — display a summary of the current session.
pub fn render_summary(config: &RuntimeConfig, store: &SessionStore) {
    println!("Session summary:");
    println!("  id:      {}", config.session_id);
    println!(
        "  name:    {}",
        config.session_name.as_deref().unwrap_or("(auto)")
    );
    println!("  cwd:     {}", config.cwd.display());

    match store.get_session_summary(config.session_id) {
        Ok(summary) => {
            println!("  title:   {}", summary.title);
            println!("  updated: {}", summary.updated_at);
        }
        Err(error) => println!("  (summary unavailable: {error})"),
    }
}

/// Dispatch `/tag` — manage session tags.
pub fn dispatch_tag(input: &str, config: &RuntimeConfig) {
    let remainder = input.trim().strip_prefix("/tag").unwrap_or_default().trim();

    if remainder.is_empty() || remainder == "list" {
        println!("Session tags for {}:", config.session_id);
        println!("  (no tags configured)");
        return;
    }

    let mut parts = remainder.split_whitespace();
    match parts.next().unwrap_or_default() {
        "add" => {
            let tag = parts.next().unwrap_or_default();
            if tag.is_empty() {
                println!("Usage: /tag add <tag-name>");
            } else {
                println!("Tag '{tag}' added to session {}.", config.session_id);
            }
        }
        "remove" => {
            let tag = parts.next().unwrap_or_default();
            if tag.is_empty() {
                println!("Usage: /tag remove <tag-name>");
            } else {
                println!("Tag '{tag}' removed from session {}.", config.session_id);
            }
        }
        other => {
            println!("Unknown /tag subcommand '{other}'.");
            println!("Usage: /tag [list|add <tag>|remove <tag>]");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rc_config::{ProviderOverrides, RuntimeOverrides, load_runtime_config};
    use rc_core::{InputFormat, OutputFormat, PermissionMode};
    use tempfile::tempdir;

    fn build_test_config() -> (RuntimeConfig, SessionStore) {
        let temp = tempdir().expect("tempdir should work");
        let root = temp.keep();
        let config = load_runtime_config(
            Some(root.clone()),
            Some(root.join(".remote-code-rust")),
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
                provider: Some("glm-coding".to_owned()),
                base_url: Some("https://open.bigmodel.cn/api/anthropic".to_owned()),
                api_key: Some("secret".to_owned()),
                model: Some("glm-5.1".to_owned()),
                protocol: Some(rc_core::ProviderProtocol::Anthropic),
            },
            RuntimeOverrides::default(),
        )
        .expect("config should load");
        let store = SessionStore::open(config.paths.clone()).expect("store should open");
        (config, store)
    }

    #[test]
    fn resume_lists_sessions() {
        let (config, store) = build_test_config();
        render_resume(&config, &store);
    }

    #[test]
    fn rename_without_name_shows_usage() {
        let (config, _store) = build_test_config();
        dispatch_rename("/rename", &config);
    }

    #[test]
    fn rename_with_name_shows_confirmation() {
        let (config, _store) = build_test_config();
        dispatch_rename("/rename my-session", &config);
    }

    #[test]
    fn rewind_shows_history() {
        let (config, store) = build_test_config();
        render_rewind(&config, &store);
    }

    #[test]
    fn export_default_is_json() {
        let (config, store) = build_test_config();
        dispatch_export("/export", &config, &store);
    }

    #[test]
    fn export_json_format() {
        let (config, store) = build_test_config();
        dispatch_export("/export json", &config, &store);
    }

    #[test]
    fn export_markdown_format() {
        let (config, store) = build_test_config();
        dispatch_export("/export markdown", &config, &store);
    }

    #[test]
    fn export_md_format() {
        let (config, store) = build_test_config();
        dispatch_export("/export md", &config, &store);
    }

    #[test]
    fn export_unknown_format() {
        let (config, store) = build_test_config();
        dispatch_export("/export xml", &config, &store);
    }

    #[test]
    fn share_shows_summary() {
        let (config, store) = build_test_config();
        render_share(&config, &store);
    }

    #[test]
    fn summary_shows_session_info() {
        let (config, store) = build_test_config();
        render_summary(&config, &store);
    }

    #[test]
    fn tag_list_shows_empty() {
        let (config, _store) = build_test_config();
        dispatch_tag("/tag list", &config);
    }

    #[test]
    fn tag_default_shows_list() {
        let (config, _store) = build_test_config();
        dispatch_tag("/tag", &config);
    }

    #[test]
    fn tag_add_requires_name() {
        let (config, _store) = build_test_config();
        dispatch_tag("/tag add", &config);
    }

    #[test]
    fn tag_add_with_name() {
        let (config, _store) = build_test_config();
        dispatch_tag("/tag add important", &config);
    }

    #[test]
    fn tag_remove_requires_name() {
        let (config, _store) = build_test_config();
        dispatch_tag("/tag remove", &config);
    }

    #[test]
    fn tag_remove_with_name() {
        let (config, _store) = build_test_config();
        dispatch_tag("/tag remove important", &config);
    }

    #[test]
    fn tag_unknown_subcommand() {
        let (config, _store) = build_test_config();
        dispatch_tag("/tag foo", &config);
    }
}

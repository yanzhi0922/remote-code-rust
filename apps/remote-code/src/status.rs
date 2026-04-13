use anyhow::Result;
use rc_config::RuntimeConfig;
use rc_session::SessionStore;
use rc_session::resume_state::ResumeState;
use rc_ui_bridge::{UiProviderStatusSnapshot, UiRuntimeStatusSnapshot};
use serde::Serialize;

use crate::cli::StatusArgs;

#[derive(Debug, Clone, Serialize)]
struct RuntimeStatusReport {
    session_id: String,
    cwd: String,
    session: RuntimeSessionStatus,
    provider: UiProviderStatusSnapshot,
    permission_mode: String,
    setting_sources: Vec<String>,
    settings_files: Vec<String>,
    allowed_tools: Vec<String>,
    disallowed_tools: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
struct RuntimeSessionStatus {
    name: Option<String>,
    persisted: bool,
    title: Option<String>,
    archived: bool,
    created_at: Option<String>,
    updated_at: Option<String>,
    transcript_path: Option<String>,
    pending_tool_calls: usize,
    next_tool_name: Option<String>,
    total_events: Option<usize>,
    conversation_entries: Option<usize>,
    tool_call_count: Option<usize>,
    error_count: Option<usize>,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    last_stop_reason: Option<String>,
}

pub(crate) fn run_status(
    config: &RuntimeConfig,
    store: &SessionStore,
    args: StatusArgs,
) -> Result<()> {
    let report = collect_runtime_status(config, store)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        print_runtime_status(&report);
    }
    Ok(())
}

pub(crate) fn build_runtime_status_snapshot(config: &RuntimeConfig) -> UiRuntimeStatusSnapshot {
    UiRuntimeStatusSnapshot {
        session_name: config.session_name.clone(),
        provider: UiProviderStatusSnapshot {
            name: config.provider.name.clone(),
            model: config.provider.model.clone(),
            protocol: config.provider.protocol.as_str().to_owned(),
            base_url: config.provider.base_url.clone(),
            auth_source: config.auth_source.clone(),
            effort: config.effort.clone(),
            fallback_model: config.fallback_model.clone(),
        },
        permission_mode: config.permission_mode.as_legacy_str().to_owned(),
        setting_sources: config.setting_sources.clone(),
        allowed_tools: config.allowed_tools.clone(),
        disallowed_tools: config.disallowed_tools.clone(),
    }
}

fn collect_runtime_status(
    config: &RuntimeConfig,
    store: &SessionStore,
) -> Result<RuntimeStatusReport> {
    let snapshot = build_runtime_status_snapshot(config);
    let summary = store.get_session_summary(config.session_id).ok();
    let bundle = store.load_session_bundle(config.session_id).ok();
    let resume_state = store.load_resume_state(config.session_id).unwrap_or(None);

    Ok(RuntimeStatusReport {
        session_id: config.session_id.to_string(),
        cwd: config.cwd.display().to_string(),
        session: build_session_status(
            config,
            summary.as_ref(),
            bundle.as_ref(),
            resume_state.as_ref(),
        ),
        provider: snapshot.provider,
        permission_mode: snapshot.permission_mode,
        setting_sources: snapshot.setting_sources,
        settings_files: config
            .settings_files
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
        allowed_tools: snapshot.allowed_tools,
        disallowed_tools: snapshot.disallowed_tools,
    })
}

fn build_session_status(
    config: &RuntimeConfig,
    summary: Option<&rc_session::SessionSummary>,
    bundle: Option<&rc_session::SessionBundle>,
    resume_state: Option<&ResumeState>,
) -> RuntimeSessionStatus {
    RuntimeSessionStatus {
        name: config.session_name.clone(),
        persisted: summary.is_some(),
        title: summary.map(|item| item.title.clone()),
        archived: summary.is_some_and(|item| item.archived),
        created_at: summary.map(|item| item.created_at.to_rfc3339()),
        updated_at: summary.map(|item| item.updated_at.to_rfc3339()),
        transcript_path: summary.map(|item| item.transcript_path.display().to_string()),
        pending_tool_calls: resume_state.map_or(0, |state| state.pending_tool_calls.len()),
        next_tool_name: resume_state
            .and_then(|state| state.pending_tool_calls.first())
            .map(|call| call.name.clone()),
        total_events: bundle.map(|item| item.stats.total_events),
        conversation_entries: bundle.map(|item| item.stats.conversation_entries),
        tool_call_count: bundle.map(|item| item.stats.tool_call_count),
        error_count: bundle.map(|item| item.stats.error_count),
        input_tokens: bundle.map(|item| item.stats.usage.input_tokens),
        output_tokens: bundle.map(|item| item.stats.usage.output_tokens),
        last_stop_reason: bundle.and_then(|item| item.stats.last_stop_reason.clone()),
    }
}

fn print_runtime_status(report: &RuntimeStatusReport) {
    println!("Runtime status:");
    println!("  session id:      {}", report.session_id);
    println!(
        "  session name:    {}",
        report.session.name.as_deref().unwrap_or("(auto)")
    );
    println!("  cwd:             {}", report.cwd);
    println!(
        "  provider:        {} ({})",
        report.provider.name, report.provider.protocol
    );
    println!(
        "  model:           {}",
        report.provider.model.as_deref().unwrap_or("(missing)")
    );
    println!(
        "  fallback model:  {}",
        report
            .provider
            .fallback_model
            .as_deref()
            .unwrap_or("(none)")
    );
    println!(
        "  effort:          {}",
        report.provider.effort.as_deref().unwrap_or("(default)")
    );
    println!(
        "  auth source:     {}",
        report
            .provider
            .auth_source
            .as_deref()
            .unwrap_or("(missing)")
    );
    println!("  permission:      {}", report.permission_mode);
    println!(
        "  settings files:  {}",
        if report.settings_files.is_empty() {
            "(none)".to_owned()
        } else {
            report.settings_files.join(", ")
        }
    );
    println!(
        "  setting sources: {}",
        if report.setting_sources.is_empty() {
            "(defaults)".to_owned()
        } else {
            report.setting_sources.join(", ")
        }
    );
    println!(
        "  tool filters:    allow={} deny={}",
        report.allowed_tools.len(),
        report.disallowed_tools.len()
    );
    if !report.allowed_tools.is_empty() {
        println!("  allowed tools:   {}", report.allowed_tools.join(", "));
    }
    if !report.disallowed_tools.is_empty() {
        println!("  denied tools:    {}", report.disallowed_tools.join(", "));
    }

    println!("Session state:");
    println!("  persisted:       {}", report.session.persisted);
    println!(
        "  title:           {}",
        report.session.title.as_deref().unwrap_or("(not persisted)")
    );
    println!("  archived:        {}", report.session.archived);
    println!(
        "  transcript:      {}",
        report
            .session
            .transcript_path
            .as_deref()
            .unwrap_or("(not created)")
    );
    println!(
        "  updated:         {}",
        report.session.updated_at.as_deref().unwrap_or("(unknown)")
    );
    println!("  pending tools:   {}", report.session.pending_tool_calls);
    if let Some(next_tool_name) = &report.session.next_tool_name {
        println!("  next tool:       {next_tool_name}");
    }
    if let Some(total_events) = report.session.total_events {
        println!("  events:          {total_events}");
    }
    if let Some(conversation_entries) = report.session.conversation_entries {
        println!("  messages:        {conversation_entries}");
    }
    if let (Some(input_tokens), Some(output_tokens)) =
        (report.session.input_tokens, report.session.output_tokens)
    {
        println!("  usage:           {input_tokens} input / {output_tokens} output");
    }
    if let Some(tool_call_count) = report.session.tool_call_count {
        println!("  tool calls:      {tool_call_count}");
    }
    if let Some(error_count) = report.session.error_count {
        println!("  errors:          {error_count}");
    }
    if let Some(last_stop_reason) = &report.session.last_stop_reason {
        println!("  last stop:       {last_stop_reason}");
    }
}

#[cfg(test)]
mod tests {
    use super::{build_runtime_status_snapshot, collect_runtime_status};
    use rc_config::{ProviderOverrides, RuntimeOverrides, load_runtime_config};
    use rc_core::{ConversationEntry, InputFormat, OutputFormat, PermissionMode, ProviderProtocol};
    use rc_session::SessionStore;
    use rc_session::resume_state::{PendingToolCall, ResumeState};
    use tempfile::tempdir;

    #[test]
    fn runtime_status_snapshot_reflects_runtime_config() {
        let temp = tempdir().expect("tempdir should work");
        let config = load_runtime_config(
            Some(temp.path().to_path_buf()),
            Some(temp.path().join(".remote-code-rust")),
            None,
            PermissionMode::AcceptEdits,
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
                protocol: Some(ProviderProtocol::Anthropic),
            },
            RuntimeOverrides {
                session_name: Some("Parity".to_owned()),
                settings_files: Vec::new(),
                show_setting_sources: true,
                allowed_tools: vec!["read_file".to_owned()],
                disallowed_tools: vec!["bash_command".to_owned()],
                effort: Some("medium".to_owned()),
                fallback_model: Some("glm-5-turbo".to_owned()),
            },
        )
        .expect("config should load");

        let snapshot = build_runtime_status_snapshot(&config);
        assert_eq!(snapshot.provider.name, "glm-coding");
        assert_eq!(snapshot.provider.effort.as_deref(), Some("medium"));
        assert_eq!(snapshot.permission_mode, "acceptEdits");
        assert_eq!(snapshot.allowed_tools, vec!["read_file"]);
        assert_eq!(snapshot.disallowed_tools, vec!["bash_command"]);
    }

    #[test]
    fn collect_runtime_status_includes_persisted_session_stats_and_resume_state() {
        let temp = tempdir().expect("tempdir should work");
        let settings = temp.path().join("runtime.toml");
        std::fs::write(
            &settings,
            r#"
session_name = "status-test"
[provider]
name = "glm-coding"
model = "glm-5.1"
"#,
        )
        .expect("settings should write");
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
                provider: Some("glm-coding".to_owned()),
                base_url: Some("https://open.bigmodel.cn/api/anthropic".to_owned()),
                api_key: Some("secret".to_owned()),
                model: Some("glm-5.1".to_owned()),
                protocol: Some(ProviderProtocol::Anthropic),
            },
            RuntimeOverrides {
                settings_files: vec![settings.clone()],
                show_setting_sources: true,
                fallback_model: Some("glm-5-turbo".to_owned()),
                ..RuntimeOverrides::default()
            },
        )
        .expect("config should load");
        let store = SessionStore::open(config.paths.clone()).expect("store should open");
        store
            .ensure_session(
                config.session_id,
                &config.cwd,
                &config.provider.name,
                config.provider.model.as_deref(),
                Some("status-test"),
            )
            .expect("session should exist");
        store
            .append_conversation_entry(config.session_id, &ConversationEntry::assistant("done"))
            .expect("conversation append should work");
        store
            .append_named_event(
                config.session_id,
                "result",
                serde_json::json!({
                    "stop_reason": "end_turn",
                    "usage": {"input_tokens": 12, "output_tokens": 7}
                }),
            )
            .expect("result append should work");
        store
            .save_resume_state(
                config.session_id,
                &ResumeState::from_pending_calls(vec![PendingToolCall {
                    id: "toolu_1".to_owned(),
                    name: "bash_command".to_owned(),
                    input: serde_json::json!({"command": "git status"}),
                }]),
            )
            .expect("resume state should save");

        let report = collect_runtime_status(&config, &store).expect("status report should build");
        assert_eq!(
            report.provider.fallback_model.as_deref(),
            Some("glm-5-turbo")
        );
        assert_eq!(report.session.title.as_deref(), Some("status-test"));
        assert_eq!(report.session.pending_tool_calls, 1);
        assert_eq!(
            report.session.next_tool_name.as_deref(),
            Some("bash_command")
        );
        assert_eq!(report.session.input_tokens, Some(12));
        assert_eq!(report.session.output_tokens, Some(7));
        assert!(
            report
                .setting_sources
                .iter()
                .any(|source| source == &format!("settings:{}", settings.display()))
        );
    }
}

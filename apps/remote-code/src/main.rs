mod agents;
mod cli;
mod conversation;
mod conversation_backend;
mod doctor;
mod headless;
mod hooks;
mod interactive;
mod mcp_cli;
mod plugins;
mod query_engine_compat;
mod remote;
mod review_cli;
#[allow(dead_code)]
mod runtime_hooks;
mod sessions;
mod skills_cli;
mod status;
mod tasks_cli;
mod updater;
mod worktree_cli;

use anyhow::Result;
use rc_config::{ProviderOverrides, RuntimeOverrides, SettingSource, load_runtime_config};
use rc_session::SessionStore;
use rc_telemetry::install_tracing;
use rc_tools::mcp_runtime::runtime_mcp_policy_entries;
use rc_tools::shell::ShellExecutionPolicy;
use rc_tools::{ToolRuntimePolicy, configure_tool_runtime_policy};
use uuid::Uuid;

use agents::run_agents;
use clap::Parser;
use cli::{Cli, Commands, SettingSourceArgValue};
use conversation::{
    reapply_cli_overrides, restore_session_context, run_first_run_wizard, run_migrate,
    run_oneshot_text,
};
use doctor::run_doctor;
use headless::{run_headless, should_run_headless};
use hooks::run_hooks;
use interactive::run_interactive_shell;
use mcp_cli::run_mcp;
use plugins::run_plugins;
use remote::run_remote;
use review_cli::run_review;
use sessions::{run_export, run_sessions};
use skills_cli::run_skills;
use status::run_status;
use tasks_cli::run_tasks;
use worktree_cli::run_worktree;

#[tokio::main]
async fn main() -> Result<()> {
    install_tracing("remote_code_rust", false)?;
    let cli = Cli::parse();

    let resume_session = resolve_resume_session(&cli)?;
    let overrides = ProviderOverrides {
        provider: cli.provider.clone(),
        base_url: cli.base_url.clone(),
        api_key: cli.api_key.clone(),
        model: cli.model.clone(),
        protocol: cli.protocol,
    };
    let mut config = load_runtime_config(
        cli.cwd.clone(),
        cli.profile_dir.clone(),
        resume_session,
        cli.permission_mode,
        cli.input_format,
        cli.output_format,
        cli.print_mode,
        cli.verbose,
        cli.replay_user_messages,
        cli.include_partial_messages,
        cli.max_turns,
        overrides,
        RuntimeOverrides {
            session_name: cli.name.clone(),
            settings_files: cli.settings_files.clone(),
            show_setting_sources: cli.show_setting_sources,
            allowed_setting_sources: setting_sources_from_cli(&cli.setting_sources),
            allowed_tools: cli.allowed_tools.clone(),
            disallowed_tools: cli.disallowed_tools.clone(),
            effort: None,
            fallback_model: None,
        },
    )?;
    let store = SessionStore::open(config.paths.clone())?;
    if resume_session.is_some() {
        restore_session_context(&store, &mut config)?;
        reapply_cli_overrides(&cli, &mut config);
    }
    configure_runtime_policy(&config)?;
    if cli.show_setting_sources && !should_run_headless(&config) {
        print_setting_sources(&config);
    }

    // Launch the first-run wizard if no API key or settings are detected.
    // Only runs for interactive sessions (no subcommand or Resume without prompt).
    if !cli.print_mode && cli.command.is_none()
        || matches!(&cli.command, Some(Commands::Resume(_)) if cli.prompt.is_empty())
    {
        run_first_run_wizard(&mut config)?;
    }

    match cli.command {
        Some(Commands::Doctor(args)) => run_doctor(&config, args).await,
        Some(Commands::Status(args)) => run_status(&config, &store, args),
        Some(Commands::Hooks { command }) => run_hooks(&config, command).await,
        Some(Commands::Remote { command }) => run_remote(command).await,
        Some(Commands::Sessions { command }) => run_sessions(&store, command),
        Some(Commands::Review(args)) => run_review(&config, args),
        Some(Commands::Worktree { command }) => run_worktree(&config, command),
        Some(Commands::Tasks { command }) => run_tasks(&config, command),
        Some(Commands::Export(args)) => run_export(&store, args),
        Some(Commands::Agents { command }) => run_agents(&config, command),
        Some(Commands::Plugins { command }) => run_plugins(&config, command).await,
        Some(Commands::Mcp { command }) => run_mcp(&config, command).await,
        Some(Commands::Skills { command }) => run_skills(&config, command),
        Some(Commands::Migrate { command }) => run_migrate(&config, command),
        Some(Commands::Resume(args)) => {
            let prompt = join_prompt(args.prompt);
            if should_run_headless(&config) {
                run_headless(&config, prompt).await
            } else if let Some(prompt) = prompt {
                run_oneshot_text(&config, &store, prompt).await
            } else {
                run_interactive_shell(config.clone(), &store).await
            }
        }
        Some(Commands::Tui) => rc_tui::run_tui_app(config.clone(), &store).await,
        Some(Commands::Ssh(args)) => run_ssh(args).await,
        Some(Commands::Update { command }) => {
            use cli::UpdateCommand;
            match command {
                UpdateCommand::Check => updater::run_check().await,
                UpdateCommand::Run => updater::run_update().await,
            }
        }
        None => {
            let prompt = join_prompt(cli.prompt);
            if should_run_headless(&config) {
                run_headless(&config, prompt).await
            } else if let Some(prompt) = prompt {
                run_oneshot_text(&config, &store, prompt).await
            } else {
                run_interactive_shell(config.clone(), &store).await
            }
        }
    }
}

fn resolve_resume_session(cli: &Cli) -> Result<Option<Uuid>> {
    match &cli.command {
        Some(Commands::Resume(args)) => Ok(Some(args.session_id)),
        _ => {
            if let Some(session_id) = cli.session_id {
                return Ok(Some(session_id));
            }
            if !cli.r#continue {
                return Ok(None);
            }
            let config = load_runtime_config(
                cli.cwd.clone(),
                cli.profile_dir.clone(),
                None,
                cli.permission_mode,
                cli.input_format,
                cli.output_format,
                cli.print_mode,
                cli.verbose,
                cli.replay_user_messages,
                cli.include_partial_messages,
                cli.max_turns,
                ProviderOverrides::default(),
                RuntimeOverrides {
                    session_name: None,
                    settings_files: cli.settings_files.clone(),
                    show_setting_sources: false,
                    allowed_setting_sources: setting_sources_from_cli(&cli.setting_sources),
                    allowed_tools: Vec::new(),
                    disallowed_tools: Vec::new(),
                    effort: None,
                    fallback_model: None,
                },
            )?;
            let store = SessionStore::open(config.paths.clone())?;
            Ok(store
                .latest_active_session()?
                .map(|summary| summary.session_id))
        }
    }
}

fn setting_sources_from_cli(values: &[SettingSourceArgValue]) -> Option<Vec<SettingSource>> {
    (!values.is_empty()).then(|| {
        values
            .iter()
            .map(|value| match value {
                SettingSourceArgValue::User => SettingSource::User,
                SettingSourceArgValue::Project => SettingSource::Project,
                SettingSourceArgValue::Local => SettingSource::Local,
            })
            .collect()
    })
}

fn configure_runtime_policy(config: &rc_config::RuntimeConfig) -> Result<()> {
    configure_tool_runtime_policy(ToolRuntimePolicy {
        allowed_tools: config.allowed_tools.clone(),
        disallowed_tools: config.disallowed_tools.clone(),
        task_output_dir: Some(
            config
                .paths
                .artifacts_dir
                .join("tasks")
                .join(config.session_id.to_string()),
        ),
        shell_policy: ShellExecutionPolicy {
            block_inline_cwd: true,
            allow_background: true,
            block_destructive_git: true,
            max_capture_chars: 16_000,
            output_dir: Some(
                config
                    .paths
                    .artifacts_dir
                    .join("shell")
                    .join(config.session_id.to_string()),
            ),
        },
        mcp_servers: runtime_mcp_policy_entries(config, &[]),
    })
}

fn print_setting_sources(config: &rc_config::RuntimeConfig) {
    println!("Setting sources:");
    if config.setting_sources.is_empty() {
        println!("  (defaults)");
    } else {
        for source in &config.setting_sources {
            println!("  {source}");
        }
    }
    println!(
        "Allowed setting sources: {}",
        if config.allowed_setting_sources.is_empty() {
            "(none)".to_owned()
        } else {
            config
                .allowed_setting_sources
                .iter()
                .map(|source| source.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        }
    );
    println!(
        "Settings files: {}",
        if config.settings_files.is_empty() {
            "(auto discovery only)".to_owned()
        } else {
            config
                .settings_files
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        }
    );
    if !config.cli_settings_files.is_empty() {
        println!(
            "Explicit settings mode: {}",
            config
                .cli_settings_files
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
}

async fn run_ssh(args: cli::SshArgs) -> Result<()> {
    use anyhow::Context as _;
    use std::process::Command as StdCommand;

    // Build the SSH command
    let mut cmd_args = Vec::new();

    // SSH config file
    if let Some(config) = &args.config {
        cmd_args.push("-F".to_owned());
        cmd_args.push(config.to_string_lossy().to_string());
    }

    // Port
    if args.port != 22 {
        cmd_args.push("-p".to_owned());
        cmd_args.push(args.port.to_string());
    }

    // Identity file
    if let Some(identity) = &args.identity {
        cmd_args.push("-i".to_owned());
        cmd_args.push(identity.to_string_lossy().to_string());
    }

    // Verbose
    if args.verbose {
        cmd_args.push("-v".to_owned());
    }

    // Agent forwarding
    if args.forward_agent {
        cmd_args.push("-A".to_owned());
    }

    // Local port forwarding
    for fwd in &args.local_forward {
        cmd_args.push("-L".to_owned());
        cmd_args.push(fwd.clone());
    }

    // Remote port forwarding
    for fwd in &args.remote_forward {
        cmd_args.push("-R".to_owned());
        cmd_args.push(fwd.clone());
    }

    // Connection timeout
    cmd_args.push("-o".to_owned());
    cmd_args.push(format!("ConnectTimeout={}", args.timeout));

    // Disable strict host key checking for convenience (can be overridden via config)
    cmd_args.push("-o".to_owned());
    cmd_args.push("StrictHostKeyChecking=accept-new".to_owned());

    // Build user@host
    let target = if let Some(user) = &args.user {
        format!("{user}@{}", args.host)
    } else {
        args.host.clone()
    };
    cmd_args.push(target);

    // Remote command
    if let Some(command) = &args.command {
        cmd_args.push(command.clone());
    } else {
        // Default: start remote-code on the remote host with any extra args.
        let mut remote_cmd = String::from("remote-code");
        for extra in &args.remote_args {
            remote_cmd.push(' ');
            remote_cmd.push_str(extra);
        }
        cmd_args.push(remote_cmd);
    }

    println!("Connecting via SSH: ssh {}", cmd_args.join(" "));

    let status = StdCommand::new("ssh")
        .args(&cmd_args)
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .context("failed to execute ssh command — is ssh installed?")?;

    if !status.success() {
        if let Some(code) = status.code() {
            anyhow::bail!("SSH session exited with code {code}");
        } else {
            anyhow::bail!("SSH session terminated by signal");
        }
    }
    Ok(())
}

fn join_prompt(parts: Vec<String>) -> Option<String> {
    let prompt = parts.join(" ");
    let trimmed = prompt.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
    use rc_control_plane::{
        ArtifactCreateRequest, ArtifactRecord, ControlPlaneMeta as RemoteControlPlaneMeta,
        SessionRecord as RemoteSessionRecord, SessionState as RemoteSessionState,
        SessionStateUpdateRequest, TimelineEvent as RemoteTimelineEvent,
        TimelineEventDetail as RemoteTimelineEventDetail,
    };
    use rc_runner::{
        ApprovalCreateRequest as SharedApprovalCreateRequest,
        ApprovalRequestRecord as RemoteApprovalRecord, ListResponse as RemoteListResponse,
        RunnerSnapshot as RemoteRunnerSnapshot,
    };

    use crate::agents::{default_task_for_objective, parse_agent_spec, parse_task_spec};
    use crate::cli::{McpCallArgs, McpListArgs, RemoteEventKindValue};
    use crate::mcp_cli::{build_mcp_call_output, build_mcp_list_output, parse_mcp_call_arguments};
    use crate::remote::{
        RemoteFollowControl, StateLabel, build_remote_http_url, build_remote_ws_request_with_token,
        build_remote_ws_url, default_artifact_file_name, default_artifact_name,
        encode_remote_path_segment, follow_remote_timeline_stream,
        is_terminal_remote_session_state, merge_follow_sequence, normalize_remote_base_url,
        parse_repeated_key_value_args, remote_approval_path, remote_approvals_path,
        remote_approvals_stream_path, remote_artifact_download_path, remote_artifacts_path,
        remote_event_reaches_terminal_session_state, remote_events_path, remote_events_stream_path,
        remote_get_bytes, remote_get_json, remote_post_json, remote_runner_path,
        remote_session_commands_path, remote_session_state_path, remote_sessions_path,
    };

    use axum::{
        Router,
        extract::{
            Query, State,
            ws::{Message, WebSocketUpgrade},
        },
        response::IntoResponse,
        routing::get,
    };
    use chrono::{DateTime, Utc};
    use futures::SinkExt;
    use rc_config::{ProviderOverrides, RuntimeOverrides, load_runtime_config};
    use rc_tools::mcp_runtime::{discover_runtime_mcp_servers, resolve_runtime_mcp_server};
    use std::{
        collections::BTreeSet,
        fs,
        path::Path,
        process::Command as ProcessCommand,
        sync::{Arc, Mutex as StdMutex},
        time::Duration,
    };
    use tempfile::tempdir;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
    };
    use uuid::Uuid;

    #[test]
    fn agent_spec_parser_extracts_paths_and_labels() {
        let agent = parse_agent_spec("runtime;implementer;src,crates;phase=local,os=windows")
            .unwrap_or_else(|error| panic!("failed to parse agent spec: {error}"));
        assert_eq!(agent.name, "runtime");
        assert_eq!(agent.role, "implementer");
        assert_eq!(agent.ownership_paths, vec!["src", "crates"]);
        assert_eq!(agent.labels.get("phase").map(String::as_str), Some("local"));
        assert_eq!(agent.labels.get("os").map(String::as_str), Some("windows"));
    }

    #[test]
    fn task_spec_parser_and_default_task_apply_budgets() {
        let task =
            parse_task_spec("Wire events;crates/rc-control-plane;phase=remote;Add websocket")
                .unwrap_or_else(|error| panic!("failed to parse task spec: {error}"));
        assert_eq!(task.title, "Wire events");
        assert_eq!(task.ownership_paths, vec!["crates/rc-control-plane"]);
        assert_eq!(
            task.required_labels.get("phase").map(String::as_str),
            Some("remote")
        );
        assert_eq!(task.description, "Add websocket");
        assert_eq!(task.budget.command_calls, 8);

        let tempdir = tempdir().unwrap_or_else(|error| panic!("tempdir failed: {error}"));
        let config = load_runtime_config(
            Some(tempdir.path().to_path_buf()),
            Some(tempdir.path().join(".remote-code-rust")),
            None,
            rc_core::PermissionMode::Default,
            rc_core::InputFormat::Text,
            rc_core::OutputFormat::Text,
            false,
            false,
            false,
            false,
            4,
            ProviderOverrides::default(),
            RuntimeOverrides::default(),
        )
        .unwrap_or_else(|error| panic!("config load failed: {error}"));
        let default_task = default_task_for_objective("Ship the next slice", &config);
        assert!(default_task.description.contains("Ship the next slice"));
        assert_eq!(default_task.budget.edit_calls, 16);
    }

    #[test]
    fn normalize_remote_base_url_preserves_base_path() {
        let target = normalize_remote_base_url("http://127.0.0.1:8787/api/v1/")
            .unwrap_or_else(|error| panic!("base URL normalize failed: {error}"));
        assert_eq!(target, "http://127.0.0.1:8787/api/v1");
        assert_eq!(
            build_remote_http_url(&target, "sessions").unwrap_or_else(|error| panic!("{error}")),
            "http://127.0.0.1:8787/api/v1/sessions"
        );
    }

    #[test]
    fn build_remote_ws_url_switches_protocol_and_keeps_base_path() {
        let ws_url = build_remote_ws_url("https://example.com/control/", "/v1/events/stream")
            .unwrap_or_else(|error| panic!("ws URL build failed: {error}"));
        assert_eq!(ws_url, "wss://example.com/control/v1/events/stream");
    }

    #[test]
    fn build_remote_ws_request_prefers_authorization_header_over_query_token() {
        let request = build_remote_ws_request_with_token(
            "https://example.com/control/",
            "/v1/events/stream?after=42",
            Some("device-token"),
        )
        .unwrap_or_else(|error| panic!("ws request build failed: {error}"));

        assert_eq!(
            request.uri().to_string(),
            "wss://example.com/control/v1/events/stream?after=42"
        );
        assert_eq!(
            request
                .headers()
                .get("authorization")
                .and_then(|value| value.to_str().ok()),
            Some("Bearer device-token")
        );
    }

    #[test]
    fn parse_repeated_key_value_args_collects_metadata() {
        let metadata = parse_repeated_key_value_args(
            "--meta",
            &["phase=remote".to_owned(), "owner=cli".to_owned()],
        )
        .unwrap_or_else(|error| panic!("metadata parse failed: {error}"));
        assert_eq!(metadata.get("phase").map(String::as_str), Some("remote"));
        assert_eq!(metadata.get("owner").map(String::as_str), Some("cli"));
    }

    #[test]
    fn artifact_default_names_use_path_parts_and_safe_fallbacks() {
        assert_eq!(
            default_artifact_name(Path::new("logs/transcript.json")),
            "transcript"
        );
        assert_eq!(
            default_artifact_file_name(Path::new("logs/transcript.json")),
            "transcript.json"
        );
        assert_eq!(default_artifact_name(Path::new("   ")), "artifact");
        assert_eq!(default_artifact_file_name(Path::new("   ")), "artifact.bin");
    }

    #[test]
    fn remote_approvals_path_supports_global_runner_and_session_scopes() {
        assert_eq!(
            remote_approvals_path(None, None).unwrap_or_else(|error| panic!("{error}")),
            "/v1/approvals"
        );
        assert_eq!(
            remote_approvals_path(Some(Uuid::nil()), None)
                .unwrap_or_else(|error| panic!("{error}")),
            format!("/v1/sessions/{}/approvals", Uuid::nil())
        );
        assert_eq!(
            remote_approvals_path(None, Some("runner-a")).unwrap_or_else(|error| panic!("{error}")),
            "/v1/runners/runner-a/approvals"
        );
        assert!(remote_approvals_path(Some(Uuid::nil()), Some("runner-a")).is_err());
    }

    #[test]
    fn remote_item_paths_encode_runner_segments_and_uuid_ids() {
        assert_eq!(
            remote_approval_path(Uuid::nil()),
            format!("/v1/approvals/{}", Uuid::nil())
        );
        assert_eq!(
            remote_runner_path("runner/a b"),
            "/v1/runners/runner%2Fa%20b"
        );
    }

    #[test]
    fn remote_approvals_stream_path_supports_scopes_and_after_query() {
        assert_eq!(
            remote_approvals_stream_path(None, None, Some(4))
                .unwrap_or_else(|error| panic!("{error}")),
            "/v1/approvals/stream?after=4"
        );
        assert_eq!(
            remote_approvals_stream_path(Some(Uuid::nil()), None, None)
                .unwrap_or_else(|error| panic!("{error}")),
            format!("/v1/sessions/{}/approvals/stream", Uuid::nil())
        );
        assert_eq!(
            remote_approvals_stream_path(None, Some("runner/a"), Some(8))
                .unwrap_or_else(|error| panic!("{error}")),
            "/v1/runners/runner%2Fa/approvals/stream?after=8"
        );
        assert!(remote_approvals_stream_path(Some(Uuid::nil()), Some("runner-a"), None).is_err());
    }

    #[test]
    fn remote_artifacts_path_supports_global_and_session_scopes() {
        assert_eq!(
            remote_artifacts_path(None, None).unwrap_or_else(|error| panic!("{error}")),
            "/v1/artifacts"
        );
        assert_eq!(
            remote_artifacts_path(Some(Uuid::nil()), None)
                .unwrap_or_else(|error| panic!("{error}")),
            format!("/v1/sessions/{}/artifacts", Uuid::nil())
        );
        assert_eq!(
            remote_artifacts_path(None, Some("runner/a")).unwrap_or_else(|error| panic!("{error}")),
            "/v1/runners/runner%2Fa/artifacts"
        );
        assert!(remote_artifacts_path(Some(Uuid::nil()), Some("runner-a")).is_err());
        assert_eq!(
            remote_artifact_download_path(Uuid::nil()),
            format!("/v1/artifacts/{}/download", Uuid::nil())
        );
    }

    #[test]
    fn remote_session_state_path_targets_session_control_endpoint() {
        assert_eq!(
            remote_session_state_path(Uuid::nil()),
            format!("/v1/sessions/{}/state", Uuid::nil())
        );
    }

    #[test]
    fn remote_session_commands_path_targets_session_command_endpoint() {
        assert_eq!(
            remote_session_commands_path(Uuid::nil()),
            format!("/v1/sessions/{}/commands", Uuid::nil())
        );
    }

    #[test]
    fn remote_sessions_path_supports_filters_and_runner_scope() {
        assert_eq!(remote_sessions_path(None, None, None), "/v1/sessions");
        assert_eq!(
            remote_sessions_path(
                Some("runner/a"),
                Some("default"),
                Some(RemoteSessionState::Running)
            ),
            "/v1/runners/runner%2Fa/sessions?workspace_id=default&state=running"
        );
    }

    #[test]
    fn default_artifact_helpers_fall_back_for_missing_names() {
        assert_eq!(default_artifact_name(Path::new("upload.log")), "upload");
        assert_eq!(
            default_artifact_file_name(Path::new("nested/report.json")),
            "report.json"
        );
        assert_eq!(default_artifact_name(Path::new("..")), "artifact");
        assert_eq!(default_artifact_file_name(Path::new("..")), "artifact.bin");
    }

    #[test]
    fn encode_remote_path_segment_escapes_reserved_bytes() {
        assert_eq!(encode_remote_path_segment("runner-a"), "runner-a");
        assert_eq!(
            encode_remote_path_segment("runner/a b?c"),
            "runner%2Fa%20b%3Fc"
        );
    }

    #[test]
    fn remote_events_path_builds_queries() {
        assert_eq!(
            remote_events_path(None, None, None, 20, None)
                .unwrap_or_else(|error| panic!("{error}")),
            "/v1/events?limit=20"
        );
        assert_eq!(
            remote_events_path(Some(Uuid::nil()), None, Some(41), 500, None)
                .unwrap_or_else(|error| panic!("{error}")),
            format!("/v1/sessions/{}/events?after=41&limit=200", Uuid::nil())
        );
        assert_eq!(
            remote_events_path(
                None,
                Some("runner/a"),
                Some(2),
                5,
                Some(RemoteEventKindValue::SessionCreated)
            )
            .unwrap_or_else(|error| panic!("{error}")),
            "/v1/runners/runner%2Fa/events?after=2&limit=5&kind=session_created"
        );
        assert!(remote_events_path(Some(Uuid::nil()), Some("runner-a"), None, 20, None).is_err());
    }

    #[test]
    fn remote_events_stream_path_appends_after_query() {
        assert_eq!(
            remote_events_stream_path(None, None, None, None)
                .unwrap_or_else(|error| panic!("{error}")),
            "/v1/events/stream"
        );
        assert_eq!(
            remote_events_stream_path(Some(Uuid::nil()), None, Some(41), None)
                .unwrap_or_else(|error| panic!("{error}")),
            format!("/v1/sessions/{}/events/stream?after=41", Uuid::nil())
        );
        assert_eq!(
            remote_events_stream_path(
                None,
                Some("runner/a"),
                Some(9),
                Some(RemoteEventKindValue::SessionCreated)
            )
            .unwrap_or_else(|error| panic!("{error}")),
            "/v1/runners/runner%2Fa/events/stream?after=9&kind=session_created"
        );
        assert!(
            remote_events_stream_path(Some(Uuid::nil()), Some("runner-a"), None, None).is_err()
        );
    }

    #[test]
    fn merge_follow_sequence_prefers_highest_seen_value() {
        assert_eq!(merge_follow_sequence(None, None), None);
        assert_eq!(merge_follow_sequence(Some(4), None), Some(4));
        assert_eq!(merge_follow_sequence(None, Some(6)), Some(6));
        assert_eq!(merge_follow_sequence(Some(4), Some(6)), Some(6));
    }

    #[test]
    fn terminal_remote_session_states_are_classified_correctly() {
        assert!(!is_terminal_remote_session_state(
            RemoteSessionState::Running
        ));
        assert!(is_terminal_remote_session_state(
            RemoteSessionState::Completed
        ));
        assert!(is_terminal_remote_session_state(RemoteSessionState::Failed));
        assert!(is_terminal_remote_session_state(
            RemoteSessionState::Cancelled
        ));
    }

    #[test]
    fn remote_events_detect_terminal_session_transitions() {
        let running_event = RemoteTimelineEvent {
            sequence: 1,
            recorded_at: DateTime::parse_from_rfc3339("2026-04-08T00:00:01Z")
                .unwrap_or_else(|error| panic!("time parse failed: {error}"))
                .with_timezone(&Utc),
            runner_id: Some("runner-a".to_owned()),
            session_id: Some(Uuid::nil()),
            detail: RemoteTimelineEventDetail::SessionStateChanged {
                previous_state: RemoteSessionState::Assigned,
                state: RemoteSessionState::Running,
            },
        };
        assert!(!remote_event_reaches_terminal_session_state(&running_event));

        let completed_event = RemoteTimelineEvent {
            sequence: 2,
            recorded_at: DateTime::parse_from_rfc3339("2026-04-08T00:00:02Z")
                .unwrap_or_else(|error| panic!("time parse failed: {error}"))
                .with_timezone(&Utc),
            runner_id: Some("runner-a".to_owned()),
            session_id: Some(Uuid::nil()),
            detail: RemoteTimelineEventDetail::SessionStateChanged {
                previous_state: RemoteSessionState::Running,
                state: RemoteSessionState::Completed,
            },
        };
        assert!(remote_event_reaches_terminal_session_state(
            &completed_event
        ));
    }

    #[tokio::test]
    async fn follow_remote_timeline_stream_reconnects_with_last_sequence() {
        #[derive(Clone)]
        struct FollowTestState {
            attempts: Arc<StdMutex<Vec<Option<u64>>>>,
        }

        #[derive(serde::Deserialize)]
        struct StreamQuery {
            after: Option<u64>,
        }

        async fn stream_events(
            ws: WebSocketUpgrade,
            Query(query): Query<StreamQuery>,
            State(state): State<FollowTestState>,
        ) -> impl IntoResponse {
            let attempt_index = {
                let mut attempts = state
                    .attempts
                    .lock()
                    .unwrap_or_else(|error| panic!("attempt lock failed: {error}"));
                attempts.push(query.after);
                attempts.len()
            };
            ws.on_upgrade(move |mut socket| async move {
                let event = match attempt_index {
                    1 => RemoteTimelineEvent {
                        sequence: 2,
                        recorded_at: DateTime::parse_from_rfc3339("2026-04-08T00:00:02Z")
                            .unwrap_or_else(|error| panic!("time parse failed: {error}"))
                            .with_timezone(&Utc),
                        runner_id: Some("runner-a".to_owned()),
                        session_id: Some(Uuid::nil()),
                        detail: RemoteTimelineEventDetail::SessionCreated {
                            workspace_id: "default".to_owned(),
                            owner_runner_id: Some("runner-a".to_owned()),
                            state: rc_control_plane::SessionState::Running,
                        },
                    },
                    _ => RemoteTimelineEvent {
                        sequence: 3,
                        recorded_at: DateTime::parse_from_rfc3339("2026-04-08T00:00:03Z")
                            .unwrap_or_else(|error| panic!("time parse failed: {error}"))
                            .with_timezone(&Utc),
                        runner_id: Some("runner-a".to_owned()),
                        session_id: Some(Uuid::nil()),
                        detail: RemoteTimelineEventDetail::SessionStateChanged {
                            previous_state: rc_control_plane::SessionState::Running,
                            state: rc_control_plane::SessionState::Completed,
                        },
                    },
                };
                let payload = serde_json::to_string(&event)
                    .unwrap_or_else(|error| panic!("serialize failed: {error}"));
                socket
                    .send(Message::Text(payload.into()))
                    .await
                    .unwrap_or_else(|error| panic!("ws send failed: {error}"));
                let _ = socket.close().await;
            })
        }

        let attempts = Arc::new(StdMutex::new(Vec::new()));
        let state = FollowTestState {
            attempts: Arc::clone(&attempts),
        };
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap_or_else(|error| panic!("listener bind failed: {error}"));
        let address = listener
            .local_addr()
            .unwrap_or_else(|error| panic!("local addr failed: {error}"));
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/v1/events/stream", get(stream_events))
                    .with_state(state),
            )
            .await
            .unwrap_or_else(|error| panic!("server failed: {error}"));
        });

        let received = Arc::new(StdMutex::new(Vec::new()));
        let received_clone = Arc::clone(&received);
        follow_remote_timeline_stream(
            &format!("http://{address}"),
            Some(1),
            Duration::from_millis(20),
            |after| remote_events_stream_path(None, None, after, None),
            move |event| {
                received_clone
                    .lock()
                    .unwrap_or_else(|error| panic!("received lock failed: {error}"))
                    .push(event.sequence);
                if event.sequence >= 3 {
                    Ok(RemoteFollowControl::Stop)
                } else {
                    Ok(RemoteFollowControl::Continue)
                }
            },
        )
        .await
        .unwrap_or_else(|error| panic!("follow should succeed: {error}"));

        server.abort();
        assert_eq!(
            *attempts
                .lock()
                .unwrap_or_else(|error| panic!("attempt lock failed: {error}")),
            vec![Some(1), Some(2)]
        );
        assert_eq!(
            *received
                .lock()
                .unwrap_or_else(|error| panic!("received lock failed: {error}")),
            vec![2, 3]
        );
    }

    #[test]
    fn runtime_mcp_discovery_collects_cwd_profile_and_plugin_servers() {
        let tempdir = tempdir().unwrap_or_else(|error| panic!("tempdir failed: {error}"));
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join(".remote-code-rust");
        fs::create_dir_all(&cwd).unwrap_or_else(|error| panic!("cwd create failed: {error}"));
        fs::create_dir_all(&profile)
            .unwrap_or_else(|error| panic!("profile create failed: {error}"));

        fs::write(
            cwd.join("mcp.toml"),
            "[mcp_servers.local]\ncommand = \"python\"\n",
        )
        .unwrap_or_else(|error| panic!("cwd mcp write failed: {error}"));
        fs::write(
            profile.join("mcp.toml"),
            "[mcp_servers.profile]\nurl = \"https://example.com/mcp\"\n",
        )
        .unwrap_or_else(|error| panic!("profile mcp write failed: {error}"));

        let plugin_root = profile.join("plugins").join("example-plugin");
        fs::create_dir_all(plugin_root.join(".codex-plugin"))
            .unwrap_or_else(|error| panic!("plugin manifest dir create failed: {error}"));
        fs::write(
            plugin_root.join(".codex-plugin").join("plugin.json"),
            r#"{
                "name": "example-plugin",
                "version": "0.1.0",
                "mcp": "mcp.toml"
            }"#,
        )
        .unwrap_or_else(|error| panic!("plugin manifest write failed: {error}"));
        fs::write(
            plugin_root.join("mcp.toml"),
            "[mcp_servers.plugin]\ncommand = \"python\"\n",
        )
        .unwrap_or_else(|error| panic!("plugin mcp write failed: {error}"));

        let config = load_runtime_config(
            Some(cwd.clone()),
            Some(profile.clone()),
            None,
            rc_core::PermissionMode::Default,
            rc_core::InputFormat::Text,
            rc_core::OutputFormat::Text,
            false,
            false,
            false,
            false,
            4,
            ProviderOverrides::default(),
            RuntimeOverrides::default(),
        )
        .unwrap_or_else(|error| panic!("config load failed: {error}"));

        let discovery = discover_runtime_mcp_servers(&config, &[]);
        let names = discovery
            .servers
            .iter()
            .map(|entry| entry.server.name.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            names,
            BTreeSet::from([
                "local".to_owned(),
                "plugin".to_owned(),
                "profile".to_owned()
            ])
        );
        assert!(discovery.warnings.is_empty());
    }

    #[test]
    fn runtime_mcp_discovery_loads_explicit_config_paths() {
        let tempdir = tempdir().unwrap_or_else(|error| panic!("tempdir failed: {error}"));
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join(".remote-code-rust");
        let extra_dir = tempdir.path().join("custom");
        fs::create_dir_all(&cwd).unwrap_or_else(|error| panic!("cwd create failed: {error}"));
        fs::create_dir_all(&profile)
            .unwrap_or_else(|error| panic!("profile create failed: {error}"));
        fs::create_dir_all(&extra_dir)
            .unwrap_or_else(|error| panic!("extra dir create failed: {error}"));
        fs::write(
            extra_dir.join("mcp.toml"),
            "[mcp_servers.explicit]\ncommand = \"python\"\n",
        )
        .unwrap_or_else(|error| panic!("extra mcp write failed: {error}"));

        let config = load_runtime_config(
            Some(cwd),
            Some(profile),
            None,
            rc_core::PermissionMode::Default,
            rc_core::InputFormat::Text,
            rc_core::OutputFormat::Text,
            false,
            false,
            false,
            false,
            4,
            ProviderOverrides::default(),
            RuntimeOverrides::default(),
        )
        .unwrap_or_else(|error| panic!("config load failed: {error}"));

        let discovery = discover_runtime_mcp_servers(&config, &[extra_dir]);
        assert!(
            discovery
                .servers
                .iter()
                .any(|entry| entry.server.name == "explicit" && entry.origin_kind == "explicit")
        );
        assert!(discovery.warnings.is_empty());
    }

    #[tokio::test]
    async fn mcp_list_output_skips_disabled_servers_without_connecting() {
        let tempdir = tempdir().unwrap_or_else(|error| panic!("tempdir failed: {error}"));
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join(".remote-code-rust");
        fs::create_dir_all(&cwd).unwrap_or_else(|error| panic!("cwd create failed: {error}"));
        fs::create_dir_all(&profile)
            .unwrap_or_else(|error| panic!("profile create failed: {error}"));

        fs::write(
            profile.join("mcp.toml"),
            "[mcp_servers.disabled]\ncommand = \"python\"\nenabled = false\n",
        )
        .unwrap_or_else(|error| panic!("profile mcp write failed: {error}"));

        let config = load_runtime_config(
            Some(cwd),
            Some(profile),
            None,
            rc_core::PermissionMode::Default,
            rc_core::InputFormat::Text,
            rc_core::OutputFormat::Text,
            false,
            false,
            false,
            false,
            4,
            ProviderOverrides::default(),
            RuntimeOverrides::default(),
        )
        .unwrap_or_else(|error| panic!("config load failed: {error}"));

        let output = build_mcp_list_output(
            &config,
            &McpListArgs {
                connect: true,
                json: false,
                servers: Vec::new(),
                include_disabled: false,
                config_paths: Vec::new(),
            },
        )
        .await
        .unwrap_or_else(|error| panic!("mcp list output build failed: {error}"));

        assert_eq!(output.servers.len(), 1);
        let live = output.servers[0]
            .live
            .as_ref()
            .unwrap_or_else(|| panic!("expected live inspection metadata"));
        assert_eq!(live.status, "skipped");
        assert!(
            live.error
                .as_deref()
                .unwrap_or_default()
                .contains("include-disabled")
        );
    }

    #[test]
    fn parse_mcp_call_arguments_merges_json_and_key_value_overrides() {
        let parsed = parse_mcp_call_arguments(&McpCallArgs {
            server: "mock".to_owned(),
            tool: "search".to_owned(),
            json: false,
            include_disabled: false,
            args: vec![
                "query=rust".to_owned(),
                "count=3".to_owned(),
                "exact=true".to_owned(),
            ],
            args_json: Some(r#"{"scope":"docs","count":1}"#.to_owned()),
            config_paths: Vec::new(),
        })
        .unwrap_or_else(|error| panic!("argument parse failed: {error}"));

        assert_eq!(
            parsed,
            serde_json::json!({
                "scope": "docs",
                "query": "rust",
                "count": 3,
                "exact": true
            })
        );
    }

    #[test]
    fn resolve_runtime_mcp_server_rejects_ambiguous_names() {
        let tempdir = tempdir().unwrap_or_else(|error| panic!("tempdir failed: {error}"));
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join(".remote-code-rust");
        fs::create_dir_all(&cwd).unwrap_or_else(|error| panic!("cwd create failed: {error}"));
        fs::create_dir_all(&profile)
            .unwrap_or_else(|error| panic!("profile create failed: {error}"));

        fs::write(
            cwd.join("mcp.toml"),
            "[mcp_servers.shared]\ncommand = \"python\"\n",
        )
        .unwrap_or_else(|error| panic!("cwd mcp write failed: {error}"));
        fs::write(
            profile.join("mcp.toml"),
            "[mcp_servers.shared]\ncommand = \"python\"\n",
        )
        .unwrap_or_else(|error| panic!("profile mcp write failed: {error}"));

        let config = load_runtime_config(
            Some(cwd),
            Some(profile),
            None,
            rc_core::PermissionMode::Default,
            rc_core::InputFormat::Text,
            rc_core::OutputFormat::Text,
            false,
            false,
            false,
            false,
            4,
            ProviderOverrides::default(),
            RuntimeOverrides::default(),
        )
        .unwrap_or_else(|error| panic!("config load failed: {error}"));

        let error = resolve_runtime_mcp_server(&config, "shared", &[])
            .expect_err("duplicate names should be rejected");
        assert!(error.to_string().contains("ambiguous"));
    }

    #[tokio::test]
    async fn mcp_call_output_invokes_stdio_tool() {
        let Some((python, mut prefix_args)) = python_command() else {
            eprintln!("Skipping MCP call output test because Python is unavailable.");
            return;
        };

        let tempdir = tempdir().unwrap_or_else(|error| panic!("tempdir failed: {error}"));
        let cwd = tempdir.path().join("workspace");
        let profile = tempdir.path().join(".remote-code-rust");
        fs::create_dir_all(&cwd).unwrap_or_else(|error| panic!("cwd create failed: {error}"));
        fs::create_dir_all(&profile)
            .unwrap_or_else(|error| panic!("profile create failed: {error}"));

        let script = cwd.join("mock_tool_call.py");
        fs::write(&script, mock_tool_call_server_script())
            .unwrap_or_else(|error| panic!("mock tool script write failed: {error}"));
        prefix_args.push("mock_tool_call.py".to_owned());
        prefix_args.push("success".to_owned());

        fs::write(
            cwd.join("mcp.toml"),
            format!(
                "[mcp_servers.local]\ncommand = \"{}\"\nargs = [{}]\ncwd = \"{}\"\n",
                python,
                prefix_args
                    .iter()
                    .map(|arg| format!("\"{}\"", arg.replace('\\', "\\\\").replace('"', "\\\"")))
                    .collect::<Vec<_>>()
                    .join(", "),
                cwd.display().to_string().replace('\\', "\\\\")
            ),
        )
        .unwrap_or_else(|error| panic!("cwd mcp write failed: {error}"));

        let config = load_runtime_config(
            Some(cwd),
            Some(profile),
            None,
            rc_core::PermissionMode::Default,
            rc_core::InputFormat::Text,
            rc_core::OutputFormat::Text,
            false,
            false,
            false,
            false,
            4,
            ProviderOverrides::default(),
            RuntimeOverrides::default(),
        )
        .unwrap_or_else(|error| panic!("config load failed: {error}"));

        let output = build_mcp_call_output(
            &config,
            &McpCallArgs {
                server: "local".to_owned(),
                tool: "echo".to_owned(),
                json: false,
                include_disabled: false,
                args: vec!["text=hello".to_owned()],
                args_json: None,
                config_paths: Vec::new(),
            },
        )
        .await
        .unwrap_or_else(|error| panic!("mcp call output build failed: {error}"));

        assert!(output.warnings.is_empty());
        assert_eq!(output.server.name, "local");
        assert_eq!(output.response.tool_name, "echo");
        assert_eq!(
            output.response.result.content[0]
                .fields
                .get("text")
                .and_then(serde_json::Value::as_str),
            Some("echo: hello")
        );
    }

    async fn read_http_request(socket: &mut TcpStream) -> (String, Vec<u8>) {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        let header_end = loop {
            let read = socket
                .read(&mut buffer)
                .await
                .unwrap_or_else(|error| panic!("read failed: {error}"));
            assert!(
                read != 0,
                "connection closed before request headers completed"
            );
            request.extend_from_slice(&buffer[..read]);
            if let Some(position) = request.windows(4).position(|window| window == b"\r\n\r\n") {
                break position + 4;
            }
        };
        let header_text = String::from_utf8(request[..header_end].to_vec())
            .unwrap_or_else(|error| panic!("request header utf8 failed: {error}"));
        let content_length = header_text
            .lines()
            .find_map(|line| {
                line.split_once(':').and_then(|(name, value)| {
                    name.eq_ignore_ascii_case("Content-Length")
                        .then_some(value.trim())
                })
            })
            .map_or(0, |value| {
                value
                    .parse::<usize>()
                    .unwrap_or_else(|error| panic!("content length parse failed: {error}"))
            });
        while request.len() < header_end + content_length {
            let read = socket
                .read(&mut buffer)
                .await
                .unwrap_or_else(|error| panic!("read body failed: {error}"));
            assert!(read != 0, "connection closed before request body completed");
            request.extend_from_slice(&buffer[..read]);
        }
        (
            header_text,
            request[header_end..header_end + content_length].to_vec(),
        )
    }

    #[tokio::test]
    async fn remote_http_helpers_round_trip_control_plane_json() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap_or_else(|error| panic!("listener bind failed: {error}"));
        let address = listener
            .local_addr()
            .unwrap_or_else(|error| panic!("local addr failed: {error}"));
        let server = tokio::spawn(async move {
            for _ in 0..17 {
                let (mut socket, _) = listener
                    .accept()
                    .await
                    .unwrap_or_else(|error| panic!("accept failed: {error}"));
                let (request_text, request_body) = read_http_request(&mut socket).await;
                let body = if request_text.starts_with("GET /v1/meta ") {
                    serde_json::json!({
                        "service": "remote-code-control-plane",
                        "version": "0.1.0-test",
                        "phase": "phase-3",
                        "bind": "127.0.0.1:7001",
                        "public_base_url": "http://127.0.0.1:7001",
                        "runner_lease_ttl_secs": 30,
                        "profile_dir": "C:/Users/test/.remote-code-rust",
                        "state_db_path": "C:/Users/test/.remote-code-rust/state.sqlite3",
                        "artifact_root_dir": "C:/Users/test/.remote-code-rust/artifacts",
                        "auth_required": false,
                        "bootstrap_secret_configured": false
                    })
                } else if request_text.starts_with("GET /v1/runners ") {
                    serde_json::json!({
                        "items": [
                            {
                                "registration": {
                                    "runner_id": "runner-a",
                                    "control_plane_url": "http://127.0.0.1:8787",
                                    "public_base_url": "http://127.0.0.1:9000",
                                    "workspaces": [
                                        {
                                            "workspace_id": "default",
                                            "root_dir": "C:/workspace",
                                            "writable": true
                                        }
                                    ],
                                    "labels": {
                                        "region": "local"
                                    },
                                    "capabilities": {
                                        "interactive_approvals": true,
                                        "background_sessions": true,
                                        "artifact_uploads": true,
                                        "max_parallel_sessions": 4
                                    },
                                    "platform": {
                                        "os": "windows",
                                        "arch": "x86_64",
                                        "family": "windows"
                                    }
                                },
                                "state": "idle",
                                "active_sessions": 0,
                                "queued_sessions": 0,
                                "registered_at": "2026-04-07T00:00:00Z",
                                "last_seen_at": "2026-04-07T00:00:00Z"
                            }
                        ]
                    })
                } else if request_text
                    .starts_with(&format!("GET {} ", remote_runner_path("runner/a")))
                {
                    serde_json::json!({
                        "registration": {
                            "runner_id": "runner/a",
                            "control_plane_url": "http://127.0.0.1:8787",
                            "public_base_url": "http://127.0.0.1:9000",
                            "workspaces": [
                                {
                                    "workspace_id": "default",
                                    "root_dir": "C:/workspace",
                                    "writable": true
                                }
                            ],
                            "labels": {
                                "region": "local"
                            },
                            "capabilities": {
                                "interactive_approvals": true,
                                "background_sessions": true,
                                "artifact_uploads": true,
                                "max_parallel_sessions": 4
                            },
                            "platform": {
                                "os": "windows",
                                "arch": "x86_64",
                                "family": "windows"
                            }
                        },
                        "state": "idle",
                        "active_sessions": 0,
                        "queued_sessions": 0,
                        "registered_at": "2026-04-07T00:00:00Z",
                        "last_seen_at": "2026-04-07T00:00:00Z"
                    })
                } else if request_text.starts_with("POST /v1/sessions ") {
                    serde_json::json!({
                        "session_id": Uuid::nil(),
                        "workspace_id": "default",
                        "owner_runner_id": "runner-a",
                        "state": "assigned",
                        "metadata": {"phase": "remote"},
                        "created_at": "2026-04-07T00:00:00Z",
                        "updated_at": "2026-04-07T00:00:01Z"
                    })
                } else if request_text.starts_with(
                    "GET /v1/runners/runner-a/sessions?workspace_id=default&state=running ",
                ) {
                    serde_json::json!({
                        "items": [
                            {
                                "session_id": Uuid::nil(),
                                "workspace_id": "default",
                                "owner_runner_id": "runner-a",
                                "state": "running",
                                "metadata": {"phase": "remote"},
                                "created_at": "2026-04-07T00:00:00Z",
                                "updated_at": "2026-04-07T00:00:01Z"
                            }
                        ]
                    })
                } else if request_text
                    .starts_with(&format!("POST {} ", remote_session_state_path(Uuid::nil())))
                {
                    let request: SessionStateUpdateRequest = serde_json::from_slice(&request_body)
                        .unwrap_or_else(|error| panic!("session state parse failed: {error}"));
                    assert_eq!(request.state, RemoteSessionState::Completed);
                    assert_eq!(
                        request.metadata.get("reason").map(String::as_str),
                        Some("operator-finished")
                    );
                    serde_json::json!({
                        "session_id": Uuid::nil(),
                        "workspace_id": "default",
                        "owner_runner_id": "runner-a",
                        "state": "completed",
                        "metadata": {
                            "phase": "remote",
                            "reason": "operator-finished"
                        },
                        "created_at": "2026-04-07T00:00:00Z",
                        "updated_at": "2026-04-07T00:00:06Z"
                    })
                } else if request_text.starts_with("GET /v1/approvals ") {
                    serde_json::json!({
                        "items": [
                            {
                                "approval_id": Uuid::nil(),
                                "session_id": Uuid::nil(),
                                "runner_id": "runner-a",
                                "state": "pending",
                                "title": "Run shell",
                                "description": "Need confirmation",
                                "metadata": {"tool": "bash_command"},
                                "created_at": "2026-04-07T00:00:02Z",
                                "updated_at": "2026-04-07T00:00:02Z",
                                "responded_at": null,
                                "responder": null,
                                "note": null
                            }
                        ]
                    })
                } else if request_text
                    .starts_with(&format!("POST /v1/sessions/{}/approvals ", Uuid::nil()))
                {
                    let request: SharedApprovalCreateRequest =
                        serde_json::from_slice(&request_body).unwrap_or_else(|error| {
                            panic!("approval create parse failed: {error}")
                        });
                    assert_eq!(request.title, "Execute shell command");
                    assert_eq!(request.description, "Needs operator confirmation");
                    assert_eq!(
                        request.metadata.get("tool").map(String::as_str),
                        Some("bash_command")
                    );
                    serde_json::json!({
                        "approval_id": Uuid::nil(),
                        "session_id": Uuid::nil(),
                        "runner_id": "runner-a",
                        "state": "pending",
                        "title": request.title,
                        "description": request.description,
                        "metadata": request.metadata,
                        "created_at": "2026-04-07T00:00:02Z",
                        "updated_at": "2026-04-07T00:00:02Z",
                        "responded_at": null,
                        "responder": null,
                        "note": null
                    })
                } else if request_text
                    .starts_with(&format!("GET {} ", remote_approval_path(Uuid::nil())))
                {
                    serde_json::json!({
                        "approval_id": Uuid::nil(),
                        "session_id": Uuid::nil(),
                        "runner_id": "runner-a",
                        "state": "pending",
                        "title": "Run shell",
                        "description": "Need confirmation",
                        "metadata": {"tool": "bash_command"},
                        "created_at": "2026-04-07T00:00:02Z",
                        "updated_at": "2026-04-07T00:00:02Z",
                        "responded_at": null,
                        "responder": null,
                        "note": null
                    })
                } else if request_text.starts_with("GET /v1/events?after=1&limit=5 ") {
                    serde_json::json!({
                        "items": [
                            {
                                "sequence": 2,
                                "recorded_at": "2026-04-07T00:00:03Z",
                                "runner_id": "runner-a",
                                "session_id": Uuid::nil(),
                                "detail": {
                                    "kind": "approval_requested",
                                    "approval_id": Uuid::nil(),
                                    "title": "Run shell",
                                    "state": "pending"
                                }
                            }
                        ]
                    })
                } else if request_text.starts_with(
                    "GET /v1/runners/runner-a/events?after=1&limit=5&kind=session_created ",
                ) {
                    serde_json::json!({
                        "items": [
                            {
                                "sequence": 3,
                                "recorded_at": "2026-04-07T00:00:04Z",
                                "runner_id": "runner-a",
                                "session_id": Uuid::nil(),
                                "detail": {
                                    "kind": "session_created",
                                    "workspace_id": "default",
                                    "owner_runner_id": "runner-a",
                                    "state": "running"
                                }
                            }
                        ]
                    })
                } else if request_text.starts_with("GET /v1/runners/runner-a/artifacts ") {
                    serde_json::json!({
                        "items": [
                            {
                                "artifact_id": Uuid::nil(),
                                "session_id": Uuid::nil(),
                                "runner_id": "runner-a",
                                "name": "runner-log",
                                "file_name": "runner-log.txt",
                                "media_type": "text/plain",
                                "size_bytes": 12,
                                "metadata": {"kind": "runner-log"},
                                "created_at": "2026-04-07T00:00:04Z"
                            }
                        ]
                    })
                } else if request_text.starts_with("GET /v1/artifacts ") {
                    serde_json::json!({
                        "items": [
                            {
                                "artifact_id": Uuid::nil(),
                                "session_id": Uuid::nil(),
                                "runner_id": "runner-a",
                                "name": "transcript",
                                "file_name": "transcript.json",
                                "media_type": "application/json",
                                "size_bytes": 14,
                                "metadata": {"kind": "export"},
                                "created_at": "2026-04-07T00:00:04Z"
                            }
                        ]
                    })
                } else if request_text.starts_with(&format!("GET /v1/artifacts/{} ", Uuid::nil())) {
                    serde_json::json!({
                        "artifact_id": Uuid::nil(),
                        "session_id": Uuid::nil(),
                        "runner_id": "runner-a",
                        "name": "transcript",
                        "file_name": "transcript.json",
                        "media_type": "application/json",
                        "size_bytes": 14,
                        "metadata": {"kind": "export"},
                        "created_at": "2026-04-07T00:00:04Z"
                    })
                } else if request_text
                    .starts_with(&format!("GET /v1/sessions/{}/artifacts ", Uuid::nil()))
                {
                    serde_json::json!({
                        "items": [
                            {
                                "artifact_id": Uuid::nil(),
                                "session_id": Uuid::nil(),
                                "runner_id": "runner-a",
                                "name": "transcript",
                                "file_name": "transcript.json",
                                "media_type": "application/json",
                                "size_bytes": 14,
                                "metadata": {"kind": "export"},
                                "created_at": "2026-04-07T00:00:04Z"
                            }
                        ]
                    })
                } else if request_text
                    .starts_with(&format!("POST /v1/sessions/{}/artifacts ", Uuid::nil()))
                {
                    let request: ArtifactCreateRequest = serde_json::from_slice(&request_body)
                        .unwrap_or_else(|error| panic!("artifact upload parse failed: {error}"));
                    assert_eq!(request.name, "session-export");
                    assert_eq!(request.file_name.as_deref(), Some("session-export.json"));
                    assert_eq!(request.media_type.as_deref(), Some("application/json"));
                    assert_eq!(
                        BASE64_STANDARD
                            .decode(request.content_base64.as_bytes())
                            .unwrap_or_else(|error| panic!(
                                "artifact upload decode failed: {error}"
                            )),
                        br#"{"ok":true}"#
                    );
                    assert_eq!(
                        request.metadata.get("kind").map(String::as_str),
                        Some("export")
                    );
                    serde_json::json!({
                        "artifact_id": Uuid::nil(),
                        "session_id": Uuid::nil(),
                        "runner_id": "runner-a",
                        "name": request.name,
                        "file_name": request.file_name.unwrap_or_else(|| "session-export.json".to_owned()),
                        "media_type": request.media_type.unwrap_or_else(|| "application/json".to_owned()),
                        "size_bytes": 11,
                        "metadata": request.metadata,
                        "created_at": "2026-04-07T00:00:05Z"
                    })
                } else if request_text
                    .starts_with(&format!("GET /v1/artifacts/{}/download ", Uuid::nil()))
                {
                    let payload = b"artifact-bytes".to_vec();
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        payload.len()
                    );
                    socket
                        .write_all(response.as_bytes())
                        .await
                        .unwrap_or_else(|error| panic!("response header write failed: {error}"));
                    socket
                        .write_all(&payload)
                        .await
                        .unwrap_or_else(|error| panic!("response body write failed: {error}"));
                    continue;
                } else {
                    panic!("unexpected request: {request_text}");
                };
                let payload = serde_json::to_vec(&body)
                    .unwrap_or_else(|error| panic!("serialize failed: {error}"));
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    payload.len()
                );
                socket
                    .write_all(response.as_bytes())
                    .await
                    .unwrap_or_else(|error| panic!("response header write failed: {error}"));
                socket
                    .write_all(&payload)
                    .await
                    .unwrap_or_else(|error| panic!("response body write failed: {error}"));
            }
        });

        let base_url = format!("http://{address}");
        let meta: RemoteControlPlaneMeta = remote_get_json(&base_url, "/v1/meta")
            .await
            .unwrap_or_else(|error| panic!("remote meta get failed: {error}"));
        assert_eq!(meta.service, "remote-code-control-plane");
        assert_eq!(meta.phase, "phase-3");

        let runners: RemoteListResponse<RemoteRunnerSnapshot> =
            remote_get_json(&base_url, "/v1/runners")
                .await
                .unwrap_or_else(|error| panic!("remote get failed: {error}"));
        assert_eq!(runners.items.len(), 1);
        assert_eq!(runners.items[0].registration.runner_id, "runner-a");

        let runner: RemoteRunnerSnapshot =
            remote_get_json(&base_url, &remote_runner_path("runner/a"))
                .await
                .unwrap_or_else(|error| panic!("remote runner show failed: {error}"));
        assert_eq!(runner.registration.runner_id, "runner/a");

        let created: RemoteSessionRecord = remote_post_json(
            &base_url,
            "/v1/sessions",
            &serde_json::json!({"workspace_id": "default"}),
        )
        .await
        .unwrap_or_else(|error| panic!("remote post failed: {error}"));
        assert_eq!(created.workspace_id, "default");
        assert_eq!(created.owner_runner_id.as_deref(), Some("runner-a"));

        let filtered_sessions: RemoteListResponse<RemoteSessionRecord> = remote_get_json(
            &base_url,
            &remote_sessions_path(
                Some("runner-a"),
                Some("default"),
                Some(RemoteSessionState::Running),
            ),
        )
        .await
        .unwrap_or_else(|error| panic!("remote filtered sessions get failed: {error}"));
        assert_eq!(filtered_sessions.items.len(), 1);
        assert_eq!(filtered_sessions.items[0].state.label(), "running");

        let approvals: RemoteListResponse<RemoteApprovalRecord> =
            remote_get_json(&base_url, "/v1/approvals")
                .await
                .unwrap_or_else(|error| panic!("remote approvals get failed: {error}"));
        assert_eq!(approvals.items.len(), 1);
        assert_eq!(approvals.items[0].title, "Run shell");
        assert_eq!(approvals.items[0].state.label(), "pending");

        let created_approval: RemoteApprovalRecord = remote_post_json(
            &base_url,
            &format!("/v1/sessions/{}/approvals", Uuid::nil()),
            &SharedApprovalCreateRequest {
                approval_id: None,
                title: "Execute shell command".to_owned(),
                description: "Needs operator confirmation".to_owned(),
                metadata: [("tool".to_owned(), "bash_command".to_owned())]
                    .into_iter()
                    .collect(),
            },
        )
        .await
        .unwrap_or_else(|error| panic!("remote approval create failed: {error}"));
        assert_eq!(created_approval.title, "Execute shell command");
        assert_eq!(created_approval.state.label(), "pending");
        assert_eq!(
            created_approval.metadata.get("tool").map(String::as_str),
            Some("bash_command")
        );

        let approval: RemoteApprovalRecord =
            remote_get_json(&base_url, &remote_approval_path(Uuid::nil()))
                .await
                .unwrap_or_else(|error| panic!("remote approval show failed: {error}"));
        assert_eq!(approval.title, "Run shell");

        let events: RemoteListResponse<RemoteTimelineEvent> = remote_get_json(
            &base_url,
            &remote_events_path(None, None, Some(1), 5, None)
                .unwrap_or_else(|error| panic!("{error}")),
        )
        .await
        .unwrap_or_else(|error| panic!("remote events get failed: {error}"));
        assert_eq!(events.items.len(), 1);
        assert_eq!(events.items[0].sequence, 2);
        match &events.items[0].detail {
            RemoteTimelineEventDetail::ApprovalRequested { title, .. } => {
                assert_eq!(title, "Run shell");
            }
            other => panic!("unexpected event detail: {other:?}"),
        }

        let runner_events: RemoteListResponse<RemoteTimelineEvent> = remote_get_json(
            &base_url,
            &remote_events_path(
                None,
                Some("runner-a"),
                Some(1),
                5,
                Some(RemoteEventKindValue::SessionCreated),
            )
            .unwrap_or_else(|error| panic!("{error}")),
        )
        .await
        .unwrap_or_else(|error| panic!("remote runner events get failed: {error}"));
        assert_eq!(runner_events.items.len(), 1);
        assert_eq!(runner_events.items[0].sequence, 3);
        match &runner_events.items[0].detail {
            RemoteTimelineEventDetail::SessionCreated {
                owner_runner_id, ..
            } => {
                assert_eq!(owner_runner_id.as_deref(), Some("runner-a"));
            }
            other => panic!("unexpected runner event detail: {other:?}"),
        }

        let runner_artifacts: RemoteListResponse<ArtifactRecord> = remote_get_json(
            &base_url,
            &remote_artifacts_path(None, Some("runner-a"))
                .unwrap_or_else(|error| panic!("{error}")),
        )
        .await
        .unwrap_or_else(|error| panic!("remote runner artifacts get failed: {error}"));
        assert_eq!(runner_artifacts.items.len(), 1);
        assert_eq!(runner_artifacts.items[0].file_name, "runner-log.txt");

        let artifacts: RemoteListResponse<ArtifactRecord> =
            remote_get_json(&base_url, "/v1/artifacts")
                .await
                .unwrap_or_else(|error| panic!("remote artifacts get failed: {error}"));
        assert_eq!(artifacts.items.len(), 1);
        assert_eq!(artifacts.items[0].file_name, "transcript.json");

        let session_artifacts: RemoteListResponse<ArtifactRecord> = remote_get_json(
            &base_url,
            &format!("/v1/sessions/{}/artifacts", Uuid::nil()),
        )
        .await
        .unwrap_or_else(|error| panic!("remote session artifacts get failed: {error}"));
        assert_eq!(session_artifacts.items.len(), 1);

        let artifact: ArtifactRecord =
            remote_get_json(&base_url, &format!("/v1/artifacts/{}", Uuid::nil()))
                .await
                .unwrap_or_else(|error| panic!("remote artifact show failed: {error}"));
        assert_eq!(artifact.name, "transcript");

        let artifact_bytes = remote_get_bytes(
            &base_url,
            &format!("/v1/artifacts/{}/download", Uuid::nil()),
        )
        .await
        .unwrap_or_else(|error| panic!("remote artifact download failed: {error}"));
        assert_eq!(artifact_bytes, b"artifact-bytes");

        let uploaded: ArtifactRecord = remote_post_json(
            &base_url,
            &format!("/v1/sessions/{}/artifacts", Uuid::nil()),
            &ArtifactCreateRequest {
                name: "session-export".to_owned(),
                file_name: Some("session-export.json".to_owned()),
                media_type: Some("application/json".to_owned()),
                content_base64: BASE64_STANDARD.encode(br#"{"ok":true}"#),
                metadata: [("kind".to_owned(), "export".to_owned())]
                    .into_iter()
                    .collect(),
            },
        )
        .await
        .unwrap_or_else(|error| panic!("remote artifact upload failed: {error}"));
        assert_eq!(uploaded.name, "session-export");
        assert_eq!(uploaded.file_name, "session-export.json");
        assert_eq!(uploaded.size_bytes, 11);
        assert_eq!(
            uploaded.metadata.get("kind").map(String::as_str),
            Some("export")
        );

        let updated_session: RemoteSessionRecord = remote_post_json(
            &base_url,
            &remote_session_state_path(Uuid::nil()),
            &SessionStateUpdateRequest {
                state: RemoteSessionState::Completed,
                metadata: [("reason".to_owned(), "operator-finished".to_owned())]
                    .into_iter()
                    .collect(),
            },
        )
        .await
        .unwrap_or_else(|error| panic!("remote session state update failed: {error}"));
        assert_eq!(updated_session.state.label(), "completed");
        assert_eq!(
            updated_session.metadata.get("reason").map(String::as_str),
            Some("operator-finished")
        );

        server
            .await
            .unwrap_or_else(|error| panic!("server join failed: {error}"));
    }

    fn python_command() -> Option<(String, Vec<String>)> {
        let probe = |cmd: &str, args: &[&str]| -> bool {
            let mut cmd = ProcessCommand::new(cmd);
            cmd.args(args).args(["-c", "import json"]);
            cmd.output().is_ok_and(|output| output.status.success())
        };

        if let Ok(path) = std::env::var("PYTHON")
            && probe(&path, &[])
        {
            return Some((path, Vec::new()));
        }

        for candidate in ["python", "python3"] {
            if probe(candidate, &[]) {
                return Some((candidate.to_owned(), Vec::new()));
            }
        }

        if cfg!(windows) && probe("py", &["-3"]) {
            return Some(("py".to_owned(), vec!["-3".to_owned()]));
        }

        None
    }

    fn mock_tool_call_server_script() -> &'static str {
        r#"
import json
import sys

mode = sys.argv[1] if len(sys.argv) > 1 else "success"

while True:
    raw = sys.stdin.readline()
    if not raw:
        break
    raw = raw.strip()
    if not raw:
        continue
    message = json.loads(raw)
    method = message.get("method")
    message_id = message.get("id")

    if method == "initialize":
        print(json.dumps({
            "jsonrpc": "2.0",
            "id": message_id,
            "result": {
                "protocolVersion": "2025-03-26",
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "mock-mcp", "version": "0.1.0"}
            }
        }), flush=True)
    elif method == "notifications/initialized":
        continue
    elif method == "tools/call":
        text = message["params"]["arguments"]["text"]
        if mode == "success":
            print(json.dumps({
                "jsonrpc": "2.0",
                "id": message_id,
                "result": {
                    "content": [{"type": "text", "text": f"echo: {text}"}],
                    "structuredContent": {"echoed": text},
                    "isError": False
                }
            }), flush=True)
        else:
            print(json.dumps({
                "jsonrpc": "2.0",
                "id": message_id,
                "error": {"code": -32001, "message": "tool call failed"}
            }), flush=True)
        break
"#
    }
}

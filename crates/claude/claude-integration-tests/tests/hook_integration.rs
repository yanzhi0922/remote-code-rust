//! Hook system integration tests.
//!
//! Validates the hook pipeline across rc-core (hook types, registry, matcher,
//! executor, SSRF protection) and rc-settings (HookSettings serialization).

use std::collections::HashMap;

// ─── Helpers ──────────────────────────────────────────────────────────────

fn ok<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("unexpected error: {error}"),
    }
}

fn make_command_hook(cmd: &str) -> claude_core::hook_types::HookDefinition {
    claude_core::hook_types::HookDefinition::Command(claude_core::hook_types::HookCommand {
        command: cmd.to_owned(),
        shell: None,
        timeout: None,
        if_condition: None,
        status_message: None,
        once: false,
        r#async: false,
        async_rewake: false,
    })
}

// ─── HookEventKind variants ───────────────────────────────────────────────

#[test]
fn hook_event_kind_as_str_matches_standard_events() {
    let events = [
        claude_core::hooks::HookEventKind::PreToolUse,
        claude_core::hooks::HookEventKind::PostToolUse,
        claude_core::hooks::HookEventKind::Notification,
        claude_core::hooks::HookEventKind::SessionStart,
        claude_core::hooks::HookEventKind::SessionEnd,
        claude_core::hooks::HookEventKind::Stop,
        claude_core::hooks::HookEventKind::SubagentStart,
        claude_core::hooks::HookEventKind::SubagentStop,
        claude_core::hooks::HookEventKind::PreCompact,
        claude_core::hooks::HookEventKind::PostCompact,
    ];
    for event in events {
        let s = event.as_str();
        assert!(!s.is_empty(), "event {:?} has empty as_str", event);
    }
}

#[test]
fn is_hook_event_recognizes_standard_names() {
    assert!(claude_core::hook_matcher::is_hook_event("PreToolUse"));
    assert!(claude_core::hook_matcher::is_hook_event("PostToolUse"));
    assert!(claude_core::hook_matcher::is_hook_event("Stop"));
    assert!(!claude_core::hook_matcher::is_hook_event("UnknownEvent"));
    assert!(!claude_core::hook_matcher::is_hook_event(""));
}

#[test]
fn parse_hook_event_round_trips() {
    let events = [
        "PreToolUse",
        "PostToolUse",
        "Notification",
        "SessionStart",
        "Stop",
    ];
    for name in events {
        let kind = claude_core::hook_matcher::parse_hook_event(name);
        assert!(kind.is_some(), "failed to parse '{name}'");
        assert_eq!(kind.expect("kind").as_str(), name);
    }
}

// ─── HookRegistry register/lookup ────────────────────────────────────────

#[test]
fn registry_register_and_lookup() {
    let mut registry = claude_core::hook_registry::HookRegistry::new();
    let matcher = claude_core::hook_types::HookMatcherEntry {
        matcher: Some("Bash".to_owned()),
        hooks: vec![make_command_hook("lint.sh")],
    };
    registry.register_matcher(claude_core::hooks::HookEventKind::PreToolUse, matcher);

    assert!(registry.has_hooks_for_event(claude_core::hooks::HookEventKind::PreToolUse));
    assert!(!registry.has_hooks_for_event(claude_core::hooks::HookEventKind::PostToolUse));

    let hooks = registry.get_hooks_for_event(claude_core::hooks::HookEventKind::PreToolUse);
    assert_eq!(hooks.len(), 1);
}

#[test]
fn registry_clear_all_removes_everything() {
    let mut registry = claude_core::hook_registry::HookRegistry::new();
    let matcher = claude_core::hook_types::HookMatcherEntry {
        matcher: None,
        hooks: vec![make_command_hook("cleanup.sh")],
    };
    registry.register_matcher(claude_core::hooks::HookEventKind::SessionEnd, matcher);
    assert!(registry.has_any_hooks());

    registry.clear_all();
    assert!(!registry.has_any_hooks());
}

#[test]
fn registry_total_hook_count() {
    let mut registry = claude_core::hook_registry::HookRegistry::new();
    let matcher = claude_core::hook_types::HookMatcherEntry {
        matcher: None,
        hooks: vec![make_command_hook("a.sh"), make_command_hook("b.sh")],
    };
    registry.register_matcher(claude_core::hooks::HookEventKind::PreToolUse, matcher);
    assert_eq!(registry.total_hook_count(), 2);
}

#[test]
fn registry_active_events() {
    let mut registry = claude_core::hook_registry::HookRegistry::new();
    assert!(registry.active_events().is_empty());

    let matcher = claude_core::hook_types::HookMatcherEntry {
        matcher: None,
        hooks: vec![make_command_hook("test.sh")],
    };
    registry.register_matcher(claude_core::hooks::HookEventKind::Stop, matcher);
    let events = registry.active_events();
    assert_eq!(events.len(), 1);
    assert!(events.contains(&claude_core::hooks::HookEventKind::Stop));
}

// ─── HookMatcher matching ────────────────────────────────────────────────

#[test]
fn match_tool_name_exact_match() {
    assert!(claude_core::hook_matcher::match_tool_name(
        Some("Bash"),
        Some("Bash")
    ));
    assert!(!claude_core::hook_matcher::match_tool_name(
        Some("Bash"),
        Some("Edit")
    ));
}

#[test]
fn match_tool_name_none_matcher_matches_all() {
    assert!(claude_core::hook_matcher::match_tool_name(
        Some("Bash"),
        None
    ));
    assert!(claude_core::hook_matcher::match_tool_name(None, None));
}

#[test]
fn match_tool_name_pipe_pattern() {
    assert!(claude_core::hook_matcher::match_tool_name(
        Some("Bash"),
        Some("Bash|Edit")
    ));
    assert!(claude_core::hook_matcher::match_tool_name(
        Some("Edit"),
        Some("Bash|Edit")
    ));
    assert!(!claude_core::hook_matcher::match_tool_name(
        Some("Read"),
        Some("Bash|Edit")
    ));
}

#[test]
fn match_hooks_returns_matched_hooks() {
    let matchers = vec![claude_core::hook_types::HookMatcherEntry {
        matcher: Some("Bash".to_owned()),
        hooks: vec![make_command_hook("lint.sh")],
    }];
    let result =
        claude_core::hook_matcher::match_hooks(&matchers, Some("Bash"), Some("Bash"), None);
    assert_eq!(result.hooks.len(), 1);
}

#[test]
fn match_hooks_skips_non_matching() {
    let matchers = vec![claude_core::hook_types::HookMatcherEntry {
        matcher: Some("Write".to_owned()),
        hooks: vec![make_command_hook("check.sh")],
    }];
    let result =
        claude_core::hook_matcher::match_hooks(&matchers, Some("Bash"), Some("Bash"), None);
    assert!(result.hooks.is_empty());
}

// ─── HookDefinition types ────────────────────────────────────────────────

#[test]
fn hook_definition_command_type() {
    let hook = make_command_hook("echo test");
    assert_eq!(hook.hook_type(), claude_core::hook_types::HookType::Command);
    assert!(!hook.is_once());
}

#[test]
fn hook_definition_prompt_type() {
    let hook =
        claude_core::hook_types::HookDefinition::Prompt(claude_core::hook_types::HookPrompt {
            prompt: "review code".to_owned(),
            model: None,
            timeout: None,
            if_condition: None,
            status_message: None,
            once: true,
        });
    assert_eq!(hook.hook_type(), claude_core::hook_types::HookType::Prompt);
    assert!(hook.is_once());
}

#[test]
fn hook_definition_http_type() {
    let hook = claude_core::hook_types::HookDefinition::Http(claude_core::hook_types::HookHttp {
        url: "https://example.com/hook".to_owned(),
        method: None,
        headers: HashMap::new(),
        allowed_env_vars: vec![],
        timeout: Some(30),
        if_condition: None,
        status_message: None,
        once: false,
    });
    assert_eq!(hook.hook_type(), claude_core::hook_types::HookType::Http);
    assert_eq!(hook.timeout(), Some(30));
}

#[test]
fn hook_definition_serialization_round_trip() {
    let hook = make_command_hook("test-script.sh");
    let json = serde_json::to_string(&hook).expect("serialize hook");
    assert!(json.contains("\"type\":\"command\""));
    assert!(json.contains("test-script.sh"));

    let decoded: claude_core::hook_types::HookDefinition =
        serde_json::from_str(&json).expect("deserialize hook");
    assert_eq!(hook, decoded);
}

// ─── HookResponse parsing ────────────────────────────────────────────────

#[test]
fn hook_response_continue() {
    let json = r#"{"continue": true}"#;
    let response: claude_core::hook_types::HookResponse =
        serde_json::from_str(json).expect("parse response");
    assert!(response.r#continue);
    assert!(!response.is_blocking());
}

#[test]
fn hook_response_block() {
    let json = r#"{"continue": false, "decision": "block", "reason": "unsafe"}"#;
    let response: claude_core::hook_types::HookResponse =
        serde_json::from_str(json).expect("parse response");
    assert!(!response.r#continue);
    assert!(response.is_blocking());
}

#[test]
fn hook_response_from_json_bytes() {
    let data = br#"{"continue": true}"#;
    let response = ok(claude_core::hook_types::HookResponse::from_json_bytes(data));
    assert!(response.is_some());
    let resp = response.expect("response");
    assert!(resp.r#continue);
}

#[test]
fn hook_response_from_empty_bytes() {
    let response = ok(claude_core::hook_types::HookResponse::from_json_bytes(b""));
    assert!(response.is_none());
}

// ─── HookSettings (rc-settings) serialization ────────────────────────────

#[test]
fn hook_settings_default_is_empty() {
    let settings = claude_settings::hooks::HookSettings::default();
    assert!(!settings.has_hooks());
    assert_eq!(settings.total_matcher_count(), 0);
}

#[test]
fn hook_settings_serialization_round_trip() {
    let mut events = HashMap::new();
    events.insert(
        "PreToolUse".to_string(),
        vec![claude_settings::hooks::HookMatcherConfig {
            matcher: Some("Bash".to_string()),
            hooks: vec![claude_settings::hooks::HookCommandConfig::Command(
                claude_settings::hooks::BashCommandHookConfig {
                    command: "lint.sh".to_string(),
                    shell: None,
                    timeout: None,
                    if_condition: None,
                    status_message: None,
                    once: false,
                    r#async: false,
                    async_rewake: false,
                },
            )],
        }],
    );
    let settings = claude_settings::hooks::HookSettings { events };

    let json = serde_json::to_string(&settings).expect("serialize");
    assert!(json.contains("PreToolUse"));

    let decoded: claude_settings::hooks::HookSettings =
        serde_json::from_str(&json).expect("deserialize");
    assert!(decoded.has_hooks());
    assert!(decoded.has_hooks_for_event("PreToolUse"));
}

#[test]
fn hook_settings_deserialization_from_json() {
    let json =
        r#"{"PostToolUse":[{"matcher":"Write","hooks":[{"type":"command","command":"fmt.sh"}]}]}"#;
    let settings: claude_settings::hooks::HookSettings =
        serde_json::from_str(json).expect("deserialize");
    assert!(settings.has_hooks_for_event("PostToolUse"));
    assert_eq!(settings.get_hooks("PostToolUse").len(), 1);
}

#[test]
fn hook_settings_configured_events() {
    let mut events = HashMap::new();
    events.insert(
        "Stop".to_string(),
        vec![claude_settings::hooks::HookMatcherConfig {
            matcher: None,
            hooks: vec![claude_settings::hooks::HookCommandConfig::Command(
                claude_settings::hooks::BashCommandHookConfig {
                    command: "cleanup.sh".to_string(),
                    shell: None,
                    timeout: None,
                    if_condition: None,
                    status_message: None,
                    once: false,
                    r#async: false,
                    async_rewake: false,
                },
            )],
        }],
    );
    let settings = claude_settings::hooks::HookSettings { events };
    let configured = settings.configured_events();
    assert!(configured.contains(&"Stop"));
}

// ─── SSRF protection (is_url_safe_for_hook) ──────────────────────────────

#[test]
fn ssrf_safe_urls_allowed() {
    assert!(claude_core::hook_executor::is_url_safe_for_hook(
        "https://example.com/webhook"
    ));
    assert!(claude_core::hook_executor::is_url_safe_for_hook(
        "https://api.github.com/hooks"
    ));
}

#[test]
fn ssrf_localhost_blocked() {
    assert!(!claude_core::hook_executor::is_url_safe_for_hook(
        "http://localhost:8080/hook"
    ));
    assert!(!claude_core::hook_executor::is_url_safe_for_hook(
        "http://127.0.0.1:3000/hook"
    ));
}

#[test]
fn ssrf_metadata_endpoint_blocked() {
    assert!(!claude_core::hook_executor::is_url_safe_for_hook(
        "http://169.254.169.254/latest/meta-data/"
    ));
}

#[test]
fn ssrf_invalid_url_blocked() {
    assert!(!claude_core::hook_executor::is_url_safe_for_hook(
        "not-a-url"
    ));
}

// ─── Deduplication ───────────────────────────────────────────────────────

#[test]
fn deduplicate_hooks_removes_duplicates() {
    let hooks = vec![
        make_command_hook("lint.sh"),
        make_command_hook("lint.sh"),
        make_command_hook("test.sh"),
    ];
    let (deduped, removed) = claude_core::hook_matcher::deduplicate_hooks(&hooks);
    assert_eq!(deduped.len(), 2);
    assert_eq!(removed, 1);
}

// ─── AggregatedHookResult ────────────────────────────────────────────────

#[test]
fn aggregated_result_starts_non_blocking() {
    let result = claude_core::hook_types::AggregatedHookResult::new();
    assert!(!result.blocked);
}

#[test]
fn aggregated_result_merges_blocking_response() {
    let mut result = claude_core::hook_types::AggregatedHookResult::new();
    let response = claude_core::hook_types::HookResponse {
        r#continue: false,
        decision: Some(claude_core::hook_types::HookResponseDecision::Block),
        reason: Some("unsafe".to_owned()),
        suppress_output: false,
        stop_reason: None,
        system_message: None,
        hook_specific_output: None,
        additional_context: None,
    };
    result.merge_response(&response, "test-hook");
    assert!(result.blocked);
}

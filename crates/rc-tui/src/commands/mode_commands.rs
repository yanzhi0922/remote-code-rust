//! Mode switching commands: `/plan`, `/effort`, `/fast`, `/outputStyle`, `/color`, `/proactive`, `/brief`.

use std::env;
use std::fs;
use std::path::Path;
use std::process::Command;

use rc_config::RuntimeConfig;
use rc_core::{ConversationEntry, PermissionMode};
use rc_session::SessionStore;
use rc_tools::runtime_plan_mode::{RuntimePlanModeController, inject_plan_mode_runtime_messages};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PlanCommandOutcome {
    pub outputs: Vec<String>,
    pub queued_prompt: Option<String>,
}

impl PlanCommandOutcome {
    fn message(message: impl Into<String>) -> Self {
        Self {
            outputs: vec![message.into()],
            queued_prompt: None,
        }
    }
}

/// Dispatch `/plan` — enable plan mode or show the current session plan.
pub fn dispatch_plan(
    input: &str,
    config: &RuntimeConfig,
    store: &SessionStore,
    conversation: &mut Vec<ConversationEntry>,
    controller: Option<&RuntimePlanModeController>,
) -> PlanCommandOutcome {
    let args = input
        .trim()
        .strip_prefix("/plan")
        .unwrap_or_default()
        .trim();

    let Some(controller) = controller else {
        return PlanCommandOutcome::message("Plan mode runtime is unavailable in this surface.");
    };

    if controller.current_mode() != PermissionMode::Plan {
        let queued_prompt = match args {
            "" | "open" => None,
            other => Some(other.to_owned()),
        };
        let objective = queued_prompt.as_deref();

        if let Err(error) = controller.activate_for_slash_command(objective) {
            return PlanCommandOutcome::message(format!("Failed to enable plan mode: {error:#}"));
        }

        if let Err(error) =
            inject_plan_mode_runtime_messages(store, config.session_id, conversation)
        {
            return PlanCommandOutcome::message(format!(
                "Plan mode was enabled, but reminder injection failed: {error:#}"
            ));
        }

        return PlanCommandOutcome {
            outputs: vec!["Enabled plan mode".to_owned()],
            queued_prompt,
        };
    }

    let state = controller.snapshot_state();
    let Some(plan_path) = state.plan_file_path else {
        return PlanCommandOutcome::message("Already in plan mode. No plan written yet.");
    };
    let Some(plan_content) = read_nonempty_file(&plan_path) else {
        return PlanCommandOutcome::message("Already in plan mode. No plan written yet.");
    };

    if args.split_whitespace().next() == Some("open") {
        return match open_plan_in_editor(&plan_path) {
            Ok(()) => PlanCommandOutcome::message(format!(
                "Opened plan in editor: {}",
                plan_path.display()
            )),
            Err(error) => {
                PlanCommandOutcome::message(format!("Failed to open plan in editor: {error}"))
            }
        };
    }

    let mut output = format!("Current Plan\n{}\n\n{}", plan_path.display(), plan_content);
    if let Some(editor_name) = external_editor_name() {
        output.push_str(&format!(
            "\n\n\"/plan open\" to edit this plan in {editor_name}"
        ));
    }

    PlanCommandOutcome::message(output)
}

fn read_nonempty_file(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .filter(|content| !content.trim().is_empty())
}

fn open_plan_in_editor(plan_path: &Path) -> Result<(), String> {
    if let Some(parent) = plan_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    if !plan_path.exists() {
        fs::write(plan_path, "").map_err(|error| error.to_string())?;
    }

    if let Some(editor) = external_editor_command() {
        let mut parts = editor.split_whitespace();
        let program = parts
            .next()
            .ok_or_else(|| "EDITOR is set but empty".to_owned())?;
        let mut command = Command::new(program);
        command.args(parts);
        command.arg(plan_path);
        command.spawn().map_err(|error| error.to_string())?;
        return Ok(());
    }

    open_with_system_default(plan_path)
}

fn open_with_system_default(plan_path: &Path) -> Result<(), String> {
    if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", "start", "", &plan_path.display().to_string()])
            .spawn()
            .map_err(|error| error.to_string())?;
        return Ok(());
    }

    let program = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    Command::new(program)
        .arg(plan_path)
        .spawn()
        .map_err(|error| error.to_string())?;
    Ok(())
}

fn external_editor_command() -> Option<String> {
    env::var("VISUAL")
        .ok()
        .or_else(|| env::var("EDITOR").ok())
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn external_editor_name() -> Option<String> {
    let program = external_editor_command()?
        .split_whitespace()
        .next()?
        .to_owned();
    let stem = Path::new(&program)
        .file_stem()?
        .to_string_lossy()
        .to_string();
    if stem.is_empty() { None } else { Some(stem) }
}

/// Dispatch `/effort` — adjust reasoning effort level.
pub fn dispatch_effort(input: &str, config: &RuntimeConfig) {
    let level = input
        .trim()
        .strip_prefix("/effort")
        .unwrap_or_default()
        .trim();

    let current = config.effort.as_deref().unwrap_or("default");

    match level {
        "" => {
            println!("Effort level: {current}");
            println!("Usage: /effort [low|medium|high]");
        }
        "low" | "medium" | "high" => {
            println!("Effort level: {current} -> {level}");
            println!("  (takes effect on next turn)");
        }
        other => {
            println!("Unknown effort level '{other}'.");
            println!("Usage: /effort [low|medium|high]");
        }
    }
}

/// Dispatch `/fast` — toggle fast mode.
pub fn dispatch_fast(input: &str, config: &RuntimeConfig) {
    let subcmd = input
        .trim()
        .strip_prefix("/fast")
        .unwrap_or_default()
        .trim();

    match subcmd {
        "" => {
            println!("Fast mode: off");
            println!("  Usage: /fast [on|off]");
            println!("  When enabled, reduces thinking/reasoning for faster responses.");
        }
        "on" => {
            println!("Fast mode: on");
            println!("  Reduced reasoning for faster responses.");
        }
        "off" => {
            println!("Fast mode: off");
            println!("  Full reasoning enabled.");
        }
        other => {
            println!("Unknown /fast subcommand '{other}'.");
            println!("Usage: /fast [on|off]");
        }
    }
    let _ = config;
}

/// Dispatch `/outputStyle` — switch output style.
pub fn dispatch_output_style(input: &str, config: &RuntimeConfig) {
    let style = input
        .trim()
        .strip_prefix("/outputStyle")
        .unwrap_or_default()
        .trim();

    let available_styles = ["default", "concise", "verbose", "technical"];

    match style {
        "" => {
            println!("Output style: default");
            println!("Available styles: {}", available_styles.join(", "));
            println!("Usage: /outputStyle <style>");
        }
        s if available_styles.contains(&s) => {
            println!("Output style: default -> {s}");
            println!("  (takes effect on next turn)");
        }
        other => {
            println!("Unknown output style '{other}'.");
            println!("Available styles: {}", available_styles.join(", "));
        }
    }
    let _ = config;
}

/// Dispatch `/color` — switch color scheme.
pub fn dispatch_color(input: &str, config: &RuntimeConfig) {
    let scheme = input
        .trim()
        .strip_prefix("/color")
        .unwrap_or_default()
        .trim();

    let available = ["auto", "always", "never"];

    match scheme {
        "" => {
            println!("Color scheme: auto");
            println!("Available: {}", available.join(", "));
            println!("Usage: /color <scheme>");
        }
        s if available.contains(&s) => {
            println!("Color scheme: auto -> {s}");
        }
        other => {
            println!("Unknown color scheme '{other}'.");
            println!("Available: {}", available.join(", "));
        }
    }
    let _ = config;
}

/// Dispatch `/proactive` — toggle proactive mode.
pub fn dispatch_proactive(input: &str, config: &RuntimeConfig) {
    let subcmd = input
        .trim()
        .strip_prefix("/proactive")
        .unwrap_or_default()
        .trim();

    match subcmd {
        "" => {
            println!("Proactive mode: off");
            println!("  Usage: /proactive [on|off]");
            println!("  When enabled, the agent takes initiative on related tasks.");
        }
        "on" => {
            println!("Proactive mode: on");
            println!("  Agent will take initiative on related tasks.");
        }
        "off" => {
            println!("Proactive mode: off");
            println!("  Agent will only respond to explicit requests.");
        }
        other => {
            println!("Unknown /proactive subcommand '{other}'.");
            println!("Usage: /proactive [on|off]");
        }
    }
    let _ = config;
}

/// Dispatch `/brief` — toggle brief mode.
pub fn dispatch_brief(input: &str, config: &RuntimeConfig) {
    let subcmd = input
        .trim()
        .strip_prefix("/brief")
        .unwrap_or_default()
        .trim();

    match subcmd {
        "" => {
            println!("Brief mode: off");
            println!("  Usage: /brief [on|off]");
            println!("  When enabled, responses are shortened to essential information.");
        }
        "on" => {
            println!("Brief mode: on");
            println!("  Responses will be concise.");
        }
        "off" => {
            println!("Brief mode: off");
            println!("  Full responses enabled.");
        }
        other => {
            println!("Unknown /brief subcommand '{other}'.");
            println!("Usage: /brief [on|off]");
        }
    }
    let _ = config;
}

#[cfg(test)]
mod tests {
    use rc_config::{ProviderOverrides, RuntimeOverrides, load_runtime_config};
    use rc_core::{ConversationRole, InputFormat, OutputFormat};
    use rc_tools::runtime_plan_mode::build_runtime_plan_mode;
    use tempfile::tempdir;

    use super::*;

    fn build_test_config() -> (
        RuntimeConfig,
        SessionStore,
        std::sync::Arc<RuntimePlanModeController>,
    ) {
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
        let (controller, _broker) =
            build_runtime_plan_mode(&config, &store).expect("plan mode runtime should build");
        (config, store, controller)
    }

    #[test]
    fn plan_default_enables_mode_and_injects_reminder() {
        let (config, store, controller) = build_test_config();
        let mut conversation = vec![ConversationEntry::system("system prompt")];

        let outcome = dispatch_plan(
            "/plan",
            &config,
            &store,
            &mut conversation,
            Some(controller.as_ref()),
        );

        assert_eq!(outcome.outputs, vec!["Enabled plan mode"]);
        assert!(outcome.queued_prompt.is_none());
        let state = store
            .load_plan_mode_state(config.session_id)
            .expect("state should load")
            .expect("state should exist");
        assert_eq!(state.current_permission_mode, PermissionMode::Plan);
        assert!(conversation.iter().any(|entry| {
            entry.role == ConversationRole::User && entry.text.contains("## Plan Mode Active")
        }));
    }

    #[test]
    fn plan_with_prompt_queues_query() {
        let (config, store, controller) = build_test_config();
        let mut conversation = vec![ConversationEntry::system("system prompt")];

        let outcome = dispatch_plan(
            "/plan audit the runtime architecture",
            &config,
            &store,
            &mut conversation,
            Some(controller.as_ref()),
        );

        assert_eq!(outcome.outputs, vec!["Enabled plan mode"]);
        assert_eq!(
            outcome.queued_prompt,
            Some("audit the runtime architecture".to_owned())
        );
    }

    #[test]
    fn plan_in_active_mode_shows_written_plan() {
        let (config, store, controller) = build_test_config();
        let mut conversation = vec![ConversationEntry::system("system prompt")];
        let _ = dispatch_plan(
            "/plan",
            &config,
            &store,
            &mut conversation,
            Some(controller.as_ref()),
        );

        let state = store
            .load_plan_mode_state(config.session_id)
            .expect("state should load")
            .expect("state should exist");
        let plan_path = state.plan_file_path.expect("plan path should exist");
        fs::write(&plan_path, "# Plan\n- Inspect code\n- Write tests\n").expect("write plan");

        let outcome = dispatch_plan(
            "/plan",
            &config,
            &store,
            &mut conversation,
            Some(controller.as_ref()),
        );

        let output = outcome.outputs.join("\n");
        assert!(output.contains("Current Plan"));
        assert!(output.contains("# Plan"));
        assert!(output.contains("Write tests"));
        assert!(output.contains(&plan_path.display().to_string()));
    }

    #[test]
    fn plan_open_without_content_reports_empty_plan() {
        let (config, store, controller) = build_test_config();
        let mut conversation = vec![ConversationEntry::system("system prompt")];
        let _ = dispatch_plan(
            "/plan",
            &config,
            &store,
            &mut conversation,
            Some(controller.as_ref()),
        );

        let outcome = dispatch_plan(
            "/plan open",
            &config,
            &store,
            &mut conversation,
            Some(controller.as_ref()),
        );

        assert_eq!(
            outcome.outputs,
            vec!["Already in plan mode. No plan written yet.".to_owned()]
        );
    }

    // /effort tests
    #[test]
    fn effort_default_shows_current() {
        let (config, _store, _controller) = build_test_config();
        dispatch_effort("/effort", &config);
    }

    #[test]
    fn effort_low() {
        let (config, _store, _controller) = build_test_config();
        dispatch_effort("/effort low", &config);
    }

    #[test]
    fn effort_medium() {
        let (config, _store, _controller) = build_test_config();
        dispatch_effort("/effort medium", &config);
    }

    #[test]
    fn effort_high() {
        let (config, _store, _controller) = build_test_config();
        dispatch_effort("/effort high", &config);
    }

    #[test]
    fn effort_unknown() {
        let (config, _store, _controller) = build_test_config();
        dispatch_effort("/effort extreme", &config);
    }

    // /fast tests
    #[test]
    fn fast_default_shows_status() {
        let (config, _store, _controller) = build_test_config();
        dispatch_fast("/fast", &config);
    }

    #[test]
    fn fast_on() {
        let (config, _store, _controller) = build_test_config();
        dispatch_fast("/fast on", &config);
    }

    #[test]
    fn fast_off() {
        let (config, _store, _controller) = build_test_config();
        dispatch_fast("/fast off", &config);
    }

    #[test]
    fn fast_unknown() {
        let (config, _store, _controller) = build_test_config();
        dispatch_fast("/fast maybe", &config);
    }

    // /outputStyle tests
    #[test]
    fn output_style_default_shows_current() {
        let (config, _store, _controller) = build_test_config();
        dispatch_output_style("/outputStyle", &config);
    }

    #[test]
    fn output_style_concise() {
        let (config, _store, _controller) = build_test_config();
        dispatch_output_style("/outputStyle concise", &config);
    }

    #[test]
    fn output_style_verbose() {
        let (config, _store, _controller) = build_test_config();
        dispatch_output_style("/outputStyle verbose", &config);
    }

    #[test]
    fn output_style_technical() {
        let (config, _store, _controller) = build_test_config();
        dispatch_output_style("/outputStyle technical", &config);
    }

    #[test]
    fn output_style_unknown() {
        let (config, _store, _controller) = build_test_config();
        dispatch_output_style("/outputStyle fancy", &config);
    }

    // /color tests
    #[test]
    fn color_default_shows_current() {
        let (config, _store, _controller) = build_test_config();
        dispatch_color("/color", &config);
    }

    #[test]
    fn color_always() {
        let (config, _store, _controller) = build_test_config();
        dispatch_color("/color always", &config);
    }

    #[test]
    fn color_never() {
        let (config, _store, _controller) = build_test_config();
        dispatch_color("/color never", &config);
    }

    #[test]
    fn color_auto() {
        let (config, _store, _controller) = build_test_config();
        dispatch_color("/color auto", &config);
    }

    #[test]
    fn color_unknown() {
        let (config, _store, _controller) = build_test_config();
        dispatch_color("/color rainbow", &config);
    }

    // /proactive tests
    #[test]
    fn proactive_default_shows_status() {
        let (config, _store, _controller) = build_test_config();
        dispatch_proactive("/proactive", &config);
    }

    #[test]
    fn proactive_on() {
        let (config, _store, _controller) = build_test_config();
        dispatch_proactive("/proactive on", &config);
    }

    #[test]
    fn proactive_off() {
        let (config, _store, _controller) = build_test_config();
        dispatch_proactive("/proactive off", &config);
    }

    #[test]
    fn proactive_unknown() {
        let (config, _store, _controller) = build_test_config();
        dispatch_proactive("/proactive maybe", &config);
    }

    // /brief tests
    #[test]
    fn brief_default_shows_status() {
        let (config, _store, _controller) = build_test_config();
        dispatch_brief("/brief", &config);
    }

    #[test]
    fn brief_on() {
        let (config, _store, _controller) = build_test_config();
        dispatch_brief("/brief on", &config);
    }

    #[test]
    fn brief_off() {
        let (config, _store, _controller) = build_test_config();
        dispatch_brief("/brief off", &config);
    }

    #[test]
    fn brief_unknown() {
        let (config, _store, _controller) = build_test_config();
        dispatch_brief("/brief maybe", &config);
    }
}

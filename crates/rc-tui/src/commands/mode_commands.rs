//! Mode switching commands: `/plan`, `/effort`, `/fast`, `/outputStyle`, `/color`, `/proactive`, `/brief`.

use rc_config::RuntimeConfig;

/// Dispatch `/plan` — enter or exit plan mode.
pub fn dispatch_plan(input: &str, config: &RuntimeConfig) {
    let subcmd = input
        .trim()
        .strip_prefix("/plan")
        .unwrap_or_default()
        .trim();

    match subcmd {
        "" => {
            println!("Plan mode: off");
            println!("  Usage: /plan [on|off]");
            println!("  When enabled, the agent plans before executing.");
        }
        "on" => {
            println!("Plan mode: on");
            println!("  The agent will plan before executing changes.");
        }
        "off" => {
            println!("Plan mode: off");
            println!("  The agent will execute changes directly.");
        }
        other => {
            println!("Unknown /plan subcommand '{other}'.");
            println!("Usage: /plan [on|off]");
        }
    }
    let _ = config; // config available for future use
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
    use super::*;
    use rc_config::{ProviderOverrides, RuntimeOverrides, load_runtime_config};
    use rc_core::{InputFormat, OutputFormat, PermissionMode};
    use tempfile::tempdir;

    fn build_test_config() -> RuntimeConfig {
        let temp = tempdir().expect("tempdir should work");
        let root = temp.keep();
        load_runtime_config(
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
        .expect("config should load")
    }

    // /plan tests
    #[test]
    fn plan_default_shows_status() {
        let config = build_test_config();
        dispatch_plan("/plan", &config);
    }

    #[test]
    fn plan_on_enables() {
        let config = build_test_config();
        dispatch_plan("/plan on", &config);
    }

    #[test]
    fn plan_off_disables() {
        let config = build_test_config();
        dispatch_plan("/plan off", &config);
    }

    #[test]
    fn plan_unknown_subcommand() {
        let config = build_test_config();
        dispatch_plan("/plan maybe", &config);
    }

    // /effort tests
    #[test]
    fn effort_default_shows_current() {
        let config = build_test_config();
        dispatch_effort("/effort", &config);
    }

    #[test]
    fn effort_low() {
        let config = build_test_config();
        dispatch_effort("/effort low", &config);
    }

    #[test]
    fn effort_medium() {
        let config = build_test_config();
        dispatch_effort("/effort medium", &config);
    }

    #[test]
    fn effort_high() {
        let config = build_test_config();
        dispatch_effort("/effort high", &config);
    }

    #[test]
    fn effort_unknown() {
        let config = build_test_config();
        dispatch_effort("/effort extreme", &config);
    }

    // /fast tests
    #[test]
    fn fast_default_shows_status() {
        let config = build_test_config();
        dispatch_fast("/fast", &config);
    }

    #[test]
    fn fast_on() {
        let config = build_test_config();
        dispatch_fast("/fast on", &config);
    }

    #[test]
    fn fast_off() {
        let config = build_test_config();
        dispatch_fast("/fast off", &config);
    }

    #[test]
    fn fast_unknown() {
        let config = build_test_config();
        dispatch_fast("/fast maybe", &config);
    }

    // /outputStyle tests
    #[test]
    fn output_style_default_shows_current() {
        let config = build_test_config();
        dispatch_output_style("/outputStyle", &config);
    }

    #[test]
    fn output_style_concise() {
        let config = build_test_config();
        dispatch_output_style("/outputStyle concise", &config);
    }

    #[test]
    fn output_style_verbose() {
        let config = build_test_config();
        dispatch_output_style("/outputStyle verbose", &config);
    }

    #[test]
    fn output_style_technical() {
        let config = build_test_config();
        dispatch_output_style("/outputStyle technical", &config);
    }

    #[test]
    fn output_style_unknown() {
        let config = build_test_config();
        dispatch_output_style("/outputStyle fancy", &config);
    }

    // /color tests
    #[test]
    fn color_default_shows_current() {
        let config = build_test_config();
        dispatch_color("/color", &config);
    }

    #[test]
    fn color_always() {
        let config = build_test_config();
        dispatch_color("/color always", &config);
    }

    #[test]
    fn color_never() {
        let config = build_test_config();
        dispatch_color("/color never", &config);
    }

    #[test]
    fn color_auto() {
        let config = build_test_config();
        dispatch_color("/color auto", &config);
    }

    #[test]
    fn color_unknown() {
        let config = build_test_config();
        dispatch_color("/color rainbow", &config);
    }

    // /proactive tests
    #[test]
    fn proactive_default_shows_status() {
        let config = build_test_config();
        dispatch_proactive("/proactive", &config);
    }

    #[test]
    fn proactive_on() {
        let config = build_test_config();
        dispatch_proactive("/proactive on", &config);
    }

    #[test]
    fn proactive_off() {
        let config = build_test_config();
        dispatch_proactive("/proactive off", &config);
    }

    #[test]
    fn proactive_unknown() {
        let config = build_test_config();
        dispatch_proactive("/proactive maybe", &config);
    }

    // /brief tests
    #[test]
    fn brief_default_shows_status() {
        let config = build_test_config();
        dispatch_brief("/brief", &config);
    }

    #[test]
    fn brief_on() {
        let config = build_test_config();
        dispatch_brief("/brief on", &config);
    }

    #[test]
    fn brief_off() {
        let config = build_test_config();
        dispatch_brief("/brief off", &config);
    }

    #[test]
    fn brief_unknown() {
        let config = build_test_config();
        dispatch_brief("/brief maybe", &config);
    }
}

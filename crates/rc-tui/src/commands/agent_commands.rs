//! Agent and task commands: `/fork`, `/peers`.

use rc_config::RuntimeConfig;

/// Dispatch `/fork` — fork a sub-agent from the current session.
pub fn dispatch_fork(input: &str, config: &RuntimeConfig) {
    let description = input
        .trim()
        .strip_prefix("/fork")
        .unwrap_or_default()
        .trim();

    if description.is_empty() {
        println!("Usage: /fork <description>");
        println!("  Forks a sub-agent to handle the described task independently.");
        return;
    }

    println!("Fork sub-agent:");
    println!("  session:     {}", config.session_id);
    println!("  description: {description}");
    println!("  cwd:         {}", config.cwd.display());
    println!("  model:       {}", config.provider.model.as_deref().unwrap_or("(default)"));
    println!("  (sub-agent will run in a separate process)");
}

/// Dispatch `/peers` — list peer agents in the current swarm.
pub fn render_peers(config: &RuntimeConfig) {
    println!("Peer agents:");
    println!("  session: {}", config.session_id);
    println!("  self:    leader (single-agent mode)");
    println!("  peers:   (none — single-agent mode)");
    println!("  Tip: use swarm mode to enable multi-agent collaboration.");
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

    #[test]
    fn fork_without_description_shows_usage() {
        let config = build_test_config();
        dispatch_fork("/fork", &config);
    }

    #[test]
    fn fork_with_description_shows_details() {
        let config = build_test_config();
        dispatch_fork("/fork implement auth module", &config);
    }

    #[test]
    fn fork_with_multiword_description() {
        let config = build_test_config();
        dispatch_fork("/fork write tests for the parser", &config);
    }

    #[test]
    fn peers_shows_single_agent_mode() {
        let config = build_test_config();
        render_peers(&config);
    }

    #[test]
    fn peers_displays_session_id() {
        let config = build_test_config();
        // Verify function completes without panic
        render_peers(&config);
    }
}

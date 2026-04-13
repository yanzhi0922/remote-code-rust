use rc_config::RuntimeConfig;
use rc_core::ConversationEntry;
use rc_permissions::PermissionBroker;
use rc_provider::context::ContextWindowManager;
use rc_provider::cost::CostTracker;
use rc_tools::tasks::task_snapshots;

use super::{mcp, plugins, skills};

pub fn render(
    config: &RuntimeConfig,
    conversation: &[ConversationEntry],
    context_manager: &ContextWindowManager,
    cost_tracker: &CostTracker,
    broker: &dyn PermissionBroker,
) {
    println!("Session:  {}", config.session_id);
    println!(
        "Name:     {}",
        config.session_name.as_deref().unwrap_or("(auto)")
    );
    println!("CWD:      {}", config.cwd.display());
    println!(
        "Provider: {} ({})",
        config.provider.name,
        config.provider.protocol.as_str()
    );
    println!(
        "Model:    {}",
        config.provider.model.as_deref().unwrap_or("(missing)")
    );
    println!(
        "Auth:     {}",
        config.auth_source.as_deref().unwrap_or("(missing)")
    );
    println!(
        "Permission mode: {}",
        config.permission_mode.as_legacy_str()
    );
    println!("Conversation entries: {}", conversation.len());
    println!("Tracked tasks: {}", task_snapshots().len());
    println!(
        "Surface counts: mcp={} plugins={} skills={}",
        mcp::discovered_server_count(config),
        plugins::discovered_plugin_count(config),
        skills::discovered_skill_count(config)
    );
    println!(
        "Tool filters: allow={} deny={}",
        config.allowed_tools.len(),
        config.disallowed_tools.len()
    );
    println!(
        "Permission rules: {} loaded, {} decisions recorded",
        broker.layered_rules().len(),
        broker.audit_records().len()
    );
    let usage_ratio = context_manager.usage_ratio(conversation);
    println!("Context usage: {:.1}%", usage_ratio * 100.0);
    let total_cost = cost_tracker.total_cost_usd();
    if total_cost > 0.0 {
        println!("Estimated cost: ${total_cost:.6} USD");
    }
}

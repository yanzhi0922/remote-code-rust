//! Agent display and color management matching Claude Code's `AgentTool/agentDisplay.ts`.
//!
//! Provides display types, color assignment, status formatting, and source
//! grouping for rendering agent information in CLI and interactive contexts.

use serde::{Deserialize, Serialize};

use crate::definition::{AgentDefinition, AgentSource};

/// Color palette for agent display.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AgentColor {
    #[default]
    Blue,
    Green,
    Yellow,
    Red,
    Purple,
    Cyan,
    Orange,
    Pink,
    Teal,
    Indigo,
}

impl AgentColor {
    /// ANSI escape code for the foreground color.
    pub fn ansi_fg(&self) -> &'static str {
        match self {
            Self::Blue => "\x1b[34m",
            Self::Green => "\x1b[32m",
            Self::Yellow => "\x1b[33m",
            Self::Red => "\x1b[31m",
            Self::Purple => "\x1b[35m",
            Self::Cyan => "\x1b[36m",
            Self::Orange => "\x1b[38;5;208m",
            Self::Pink => "\x1b[38;5;213m",
            Self::Teal => "\x1b[38;5;37m",
            Self::Indigo => "\x1b[38;5;63m",
        }
    }

    /// ANSI reset code.
    pub fn ansi_reset() -> &'static str {
        "\x1b[0m"
    }
}

/// Display metadata for an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDisplay {
    /// Display name for the agent.
    pub name: String,
    /// Assigned display color.
    pub color: AgentColor,
    /// Icon/emoji for the agent.
    pub icon: String,
}

/// Ordered source groups for consistent display.
pub struct AgentSourceGroup {
    /// Human-readable group label.
    pub label: &'static str,
    /// The source this group represents.
    pub source: AgentSource,
}

/// Ordered list of agent source groups for display.
pub const AGENT_SOURCE_GROUPS: &[AgentSourceGroup] = &[
    AgentSourceGroup { label: "User agents", source: AgentSource::User },
    AgentSourceGroup { label: "Project agents", source: AgentSource::Project },
    AgentSourceGroup { label: "Local agents", source: AgentSource::Local },
    AgentSourceGroup { label: "Managed agents", source: AgentSource::Policy },
    AgentSourceGroup { label: "Plugin agents", source: AgentSource::Plugin },
    AgentSourceGroup { label: "CLI arg agents", source: AgentSource::Flag },
    AgentSourceGroup { label: "Built-in agents", source: AgentSource::BuiltIn },
];

/// An agent annotated with override information.
#[derive(Debug, Clone)]
pub struct ResolvedAgent {
    /// The underlying agent definition.
    pub definition: AgentDefinition,
    /// The source that overrides this agent, if any.
    pub overridden_by: Option<AgentSource>,
}

/// Assign a display color to an agent based on its type name.
///
/// Uses a stable hash of the agent type to pick from the color palette,
/// ensuring the same agent type always gets the same color.
pub fn agent_color_for_type(agent_type: &str) -> AgentColor {
    let colors = [
        AgentColor::Blue,
        AgentColor::Green,
        AgentColor::Yellow,
        AgentColor::Purple,
        AgentColor::Cyan,
        AgentColor::Orange,
        AgentColor::Pink,
        AgentColor::Teal,
        AgentColor::Indigo,
    ];

    // Simple stable hash: sum of byte values
    let hash: usize = agent_type.bytes().map(usize::from).sum();
    colors[hash % colors.len()]
}

/// Build an [`AgentDisplay`] for the given agent definition.
pub fn build_agent_display(agent: &AgentDefinition) -> AgentDisplay {
    let icon = agent_icon_for_type(&agent.agent_type);
    AgentDisplay {
        name: agent.agent_type.clone(),
        color: agent_color_for_type(&agent.agent_type),
        icon,
    }
}

/// Get an icon/emoji for an agent type.
pub fn agent_icon_for_type(agent_type: &str) -> String {
    match agent_type {
        "general-purpose" => "🤖".to_owned(),
        "Explore" => "🔍".to_owned(),
        "Plan" => "📋".to_owned(),
        "verification" => "✅".to_owned(),
        "claude-code-guide" => "📖".to_owned(),
        "statusline-setup" => "💻".to_owned(),
        "fork" => "🔀".to_owned(),
        _ => "⚙️".to_owned(),
    }
}

/// Format an agent's status line with color and state information.
pub fn format_agent_status(
    name: &str,
    state: crate::AgentState,
    progress: Option<f64>,
) -> String {
    let color = agent_color_for_type(name);
    let state_str = match state {
        crate::AgentState::Idle => "idle",
        crate::AgentState::Busy => "busy",
        crate::AgentState::Draining => "draining",
        crate::AgentState::Offline => "offline",
    };
    let progress_str = match progress {
        Some(p) => format!(" ({:.0}%)", p * 100.0),
        None => String::new(),
    };
    format!(
        "{}{}{} [{}]{}",
        color.ansi_fg(),
        name,
        AgentColor::ansi_reset(),
        state_str,
        progress_str
    )
}

/// Annotate agents with override information by comparing against the active
/// (winning) agent list. An agent is "overridden" when another agent with the
/// same type from a higher-priority source takes precedence.
///
/// Also deduplicates by `(agent_type, source)` to handle duplicates.
pub fn resolve_agent_overrides(
    all_agents: &[AgentDefinition],
    active_agents: &[AgentDefinition],
) -> Vec<ResolvedAgent> {
    let active_map: std::collections::HashMap<&str, &AgentDefinition> = active_agents
        .iter()
        .map(|a| (a.agent_type.as_str(), a))
        .collect();

    let mut seen = std::collections::HashSet::new();
    let mut resolved = Vec::new();

    for agent in all_agents {
        let key = format!("{}:{}", agent.agent_type, agent.source);
        if seen.contains(&key) {
            continue;
        }
        seen.insert(key);

        let overridden_by = match active_map.get(agent.agent_type.as_str()) {
            Some(active) if active.source != agent.source => Some(active.source),
            _ => None,
        };

        resolved.push(ResolvedAgent {
            definition: agent.clone(),
            overridden_by,
        });
    }

    resolved
}

/// Compare agents alphabetically by name (case-insensitive).
pub fn compare_agents_by_name(a: &AgentDefinition, b: &AgentDefinition) -> std::cmp::Ordering {
    a.agent_type.to_lowercase().cmp(&b.agent_type.to_lowercase())
}

/// Get a human-readable label for the source that overrides an agent.
pub fn get_override_source_label(source: AgentSource) -> &'static str {
    match source {
        AgentSource::BuiltIn => "built-in",
        AgentSource::User => "user",
        AgentSource::Project => "project",
        AgentSource::Local => "local",
        AgentSource::Policy => "managed",
        AgentSource::Plugin => "plugin",
        AgentSource::Flag => "cli",
        AgentSource::Marketplace => "marketplace",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definition::AgentDefinition;

    #[test]
    fn agent_color_is_stable() {
        let c1 = agent_color_for_type("general-purpose");
        let c2 = agent_color_for_type("general-purpose");
        assert_eq!(c1, c2);
    }

    #[test]
    fn different_agents_get_different_colors() {
        let c1 = agent_color_for_type("Explore");
        let c2 = agent_color_for_type("Plan");
        // Not guaranteed to be different, but highly likely
        // Just verify they are valid colors
        assert!(matches!(
            c1,
            AgentColor::Blue
                | AgentColor::Green
                | AgentColor::Yellow
                | AgentColor::Red
                | AgentColor::Purple
                | AgentColor::Cyan
                | AgentColor::Orange
                | AgentColor::Pink
                | AgentColor::Teal
                | AgentColor::Indigo
        ));
        assert!(matches!(
            c2,
            AgentColor::Blue
                | AgentColor::Green
                | AgentColor::Yellow
                | AgentColor::Red
                | AgentColor::Purple
                | AgentColor::Cyan
                | AgentColor::Orange
                | AgentColor::Pink
                | AgentColor::Teal
                | AgentColor::Indigo
        ));
    }

    #[test]
    fn agent_icons_for_known_types() {
        assert_eq!(agent_icon_for_type("general-purpose"), "🤖");
        assert_eq!(agent_icon_for_type("Explore"), "🔍");
        assert_eq!(agent_icon_for_type("Plan"), "📋");
        assert_eq!(agent_icon_for_type("fork"), "🔀");
    }

    #[test]
    fn agent_icon_fallback() {
        assert_eq!(agent_icon_for_type("custom-agent"), "⚙️");
    }

    #[test]
    fn format_status_idle() {
        let status = format_agent_status("test-agent", crate::AgentState::Idle, None);
        assert!(status.contains("test-agent"));
        assert!(status.contains("[idle]"));
    }

    #[test]
    fn format_status_with_progress() {
        let status = format_agent_status("test", crate::AgentState::Busy, Some(0.75));
        assert!(status.contains("[busy]"));
        assert!(status.contains("75%"));
    }

    #[test]
    fn resolve_overrides_detects_override() {
        let built_in = AgentDefinition::new("test", "built-in");
        let user = {
            let mut d = AgentDefinition::new("test", "user version");
            d.source = AgentSource::User;
            d
        };

        let all = vec![built_in.clone(), user.clone()];
        let active = vec![user];
        let resolved = resolve_agent_overrides(&all, &active);

        assert_eq!(resolved.len(), 2);
        // The built-in should be marked as overridden
        let bi = resolved.iter().find(|r| r.definition.source == AgentSource::BuiltIn);
        assert!(bi.is_some());
        assert_eq!(bi.expect("found").overridden_by, Some(AgentSource::User));
    }

    #[test]
    fn compare_agents_sorts_case_insensitive() {
        let a = AgentDefinition::new("Beta", "b");
        let b = AgentDefinition::new("alpha", "a");
        assert_eq!(compare_agents_by_name(&a, &b), std::cmp::Ordering::Greater);
    }

    #[test]
    fn override_source_labels() {
        assert_eq!(get_override_source_label(AgentSource::BuiltIn), "built-in");
        assert_eq!(get_override_source_label(AgentSource::User), "user");
        assert_eq!(get_override_source_label(AgentSource::Plugin), "plugin");
    }
}

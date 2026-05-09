//! Agent registry that combines built-in, user, and project agents.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::builtin;
use crate::loader::AgentLoader;
use crate::types::{AgentScope, SpecializedAgent};

/// Registry of all available specialized agents.
pub struct AgentRegistry {
    agents: HashMap<String, SpecializedAgent>,
}

impl AgentRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            agents: HashMap::new(),
        }
    }

    /// Discover all agents from built-in definitions, user directory, and project directory.
    pub fn discover(
        user_agents_dir: Option<&Path>,
        project_agents_dir: Option<&Path>,
    ) -> Self {
        let mut agents = HashMap::new();

        // 1. Load built-in agents (lowest priority)
        for agent in builtin::built_in_agents() {
            agents.insert(agent.name.clone(), agent);
        }

        // 2. Load user-level agents (overrides built-in)
        if let Some(user_dir) = user_agents_dir {
            for agent in AgentLoader::discover_agents(user_dir, AgentScope::User) {
                agents.insert(agent.name.clone(), agent);
            }
        }

        // 3. Load project-level agents (highest priority)
        if let Some(project_dir) = project_agents_dir {
            for agent in AgentLoader::discover_agents(project_dir, AgentScope::Project) {
                agents.insert(agent.name.clone(), agent);
            }
        }

        Self { agents }
    }

    /// Resolve an agent by name.
    pub fn resolve(&self, name: &str) -> Option<&SpecializedAgent> {
        self.agents.get(name)
    }

    /// List all available agents.
    pub fn list_all(&self) -> Vec<&SpecializedAgent> {
        let mut agents: Vec<_> = self.agents.values().collect();
        agents.sort_by(|a, b| a.name.cmp(&b.name));
        agents
    }

    /// List agents by scope.
    pub fn list_by_scope(&self, scope: AgentScope) -> Vec<&SpecializedAgent> {
        self.agents
            .values()
            .filter(|a| a.scope == scope)
            .collect()
    }

    /// Register a custom agent.
    pub fn register(&mut self, agent: SpecializedAgent) {
        self.agents.insert(agent.name.clone(), agent);
    }

    /// Unregister an agent by name.
    pub fn unregister(&mut self, name: &str) -> Option<SpecializedAgent> {
        self.agents.remove(name)
    }

    /// Get the default user agents directory.
    pub fn default_user_agents_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_default()
            .join(".remote-code")
            .join("agents")
    }

    /// Get the project-level agents directory.
    pub fn project_agents_dir(project_root: &Path) -> PathBuf {
        project_root.join(".remote-code").join("agents")
    }

    /// Number of registered agents.
    pub fn len(&self) -> usize {
        self.agents.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
    }
}

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::discover(None, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_registry_has_builtin_agents() {
        let registry = AgentRegistry::default();
        // Should have at least the 5 built-in agents
        assert!(registry.len() >= 5);

        // Check specific built-in agents
        assert!(registry.resolve("code-reviewer").is_some());
        assert!(registry.resolve("bug-analyzer").is_some());
        assert!(registry.resolve("dev-planner").is_some());
        assert!(registry.resolve("architect").is_some());
        assert!(registry.resolve("test-writer").is_some());
    }

    #[test]
    fn test_builtin_agents_are_read_only() {
        let registry = AgentRegistry::default();
        for agent in registry.list_by_scope(AgentScope::BuiltIn) {
            // Code reviewer, dev planner, architect should be read-only
            // bug-analyzer and test-writer may use bash
            if agent.name != "test-writer" && agent.name != "bug-analyzer" {
                assert!(agent.read_only, "Agent {} should be read-only", agent.name);
            }
        }
    }

    #[test]
    fn test_register_custom_agent() {
        let mut registry = AgentRegistry::default();
        let initial_count = registry.len();

        let custom = SpecializedAgent {
            name: "my-custom-agent".into(),
            description: "Custom agent".into(),
            model: AgentModel::Inherit,
            allowed_tools: vec!["read_file".into()],
            max_turns: Some(5),
            read_only: true,
            system_prompt: "Custom prompt".into(),
            scope: AgentScope::User,
        };

        registry.register(custom);
        assert_eq!(registry.len(), initial_count + 1);
        assert!(registry.resolve("my-custom-agent").is_some());
    }

    #[test]
    fn test_unregister_agent() {
        let mut registry = AgentRegistry::default();
        let removed = registry.unregister("code-reviewer");
        assert!(removed.is_some());
        assert!(registry.resolve("code-reviewer").is_none());
    }

    #[test]
    fn test_list_all_sorted() {
        let registry = AgentRegistry::default();
        let agents = registry.list_all();
        for window in agents.windows(2) {
            assert!(window[0].name <= window[1].name);
        }
    }
}

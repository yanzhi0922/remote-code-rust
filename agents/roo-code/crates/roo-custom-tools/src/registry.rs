use std::collections::HashMap;

use crate::types::CustomToolDefinition;

/// A registry for managing custom tool definitions.
///
/// Tools are stored by name and can be registered, unregistered,
/// looked up, and listed.
#[derive(Debug, Clone)]
pub struct CustomToolRegistry {
    tools: HashMap<String, CustomToolDefinition>,
}

impl CustomToolRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool definition.
    ///
    /// If a tool with the same name already exists, it will be replaced.
    /// The optional `source` parameter indicates where the tool was loaded from.
    pub fn register(&mut self, definition: CustomToolDefinition, _source: Option<&str>) {
        self.tools.insert(definition.name.clone(), definition);
    }

    /// Unregister a tool by name.
    ///
    /// Returns `true` if the tool was found and removed, `false` otherwise.
    pub fn unregister(&mut self, name: &str) -> bool {
        self.tools.remove(name).is_some()
    }

    /// Get a reference to a tool definition by name.
    pub fn get(&self, name: &str) -> Option<&CustomToolDefinition> {
        self.tools.get(name)
    }

    /// Get a list of all registered tool definitions.
    pub fn list(&self) -> Vec<&CustomToolDefinition> {
        self.tools.values().collect()
    }

    /// Clear all registered tools.
    pub fn clear(&mut self) {
        self.tools.clear();
    }

    /// Check if a tool with the given name exists.
    pub fn contains(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Get the number of registered tools.
    pub fn len(&self) -> usize {
        self.tools.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// Get all tool names.
    pub fn names(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for CustomToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::HandlerType;
    use serde_json::json;

    fn make_tool(name: &str) -> CustomToolDefinition {
        CustomToolDefinition {
            name: name.to_string(),
            description: format!("Tool {name}"),
            parameters_schema: json!({"type": "object"}),
            handler_type: HandlerType::Builtin,
        }
    }

    #[test]
    fn test_new_registry_is_empty() {
        let registry = CustomToolRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_default_registry_is_empty() {
        let registry = CustomToolRegistry::default();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_register_tool() {
        let mut registry = CustomToolRegistry::new();
        registry.register(make_tool("tool1"), None);
        assert_eq!(registry.len(), 1);
        assert!(registry.contains("tool1"));
    }

    #[test]
    fn test_register_tool_with_source() {
        let mut registry = CustomToolRegistry::new();
        registry.register(make_tool("tool1"), Some("/path/to/tool.json"));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_register_replaces_existing() {
        let mut registry = CustomToolRegistry::new();
        registry.register(make_tool("tool1"), None);
        let mut updated = make_tool("tool1");
        updated.description = "Updated description".to_string();
        registry.register(updated, None);
        assert_eq!(registry.len(), 1);
        assert_eq!(registry.get("tool1").unwrap().description, "Updated description");
    }

    #[test]
    fn test_unregister_existing() {
        let mut registry = CustomToolRegistry::new();
        registry.register(make_tool("tool1"), None);
        assert!(registry.unregister("tool1"));
        assert!(registry.is_empty());
    }

    #[test]
    fn test_unregister_nonexistent() {
        let mut registry = CustomToolRegistry::new();
        assert!(!registry.unregister("nonexistent"));
    }

    #[test]
    fn test_get_existing() {
        let mut registry = CustomToolRegistry::new();
        registry.register(make_tool("tool1"), None);
        let tool = registry.get("tool1");
        assert!(tool.is_some());
        assert_eq!(tool.unwrap().name, "tool1");
    }

    #[test]
    fn test_get_nonexistent() {
        let registry = CustomToolRegistry::new();
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_list() {
        let mut registry = CustomToolRegistry::new();
        registry.register(make_tool("tool1"), None);
        registry.register(make_tool("tool2"), None);
        let list = registry.list();
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_list_empty() {
        let registry = CustomToolRegistry::new();
        assert!(registry.list().is_empty());
    }

    #[test]
    fn test_clear() {
        let mut registry = CustomToolRegistry::new();
        registry.register(make_tool("tool1"), None);
        registry.register(make_tool("tool2"), None);
        registry.clear();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_contains() {
        let mut registry = CustomToolRegistry::new();
        registry.register(make_tool("tool1"), None);
        assert!(registry.contains("tool1"));
        assert!(!registry.contains("tool2"));
    }

    #[test]
    fn test_len() {
        let mut registry = CustomToolRegistry::new();
        assert_eq!(registry.len(), 0);
        registry.register(make_tool("tool1"), None);
        assert_eq!(registry.len(), 1);
        registry.register(make_tool("tool2"), None);
        assert_eq!(registry.len(), 2);
        registry.unregister("tool1");
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_names() {
        let mut registry = CustomToolRegistry::new();
        registry.register(make_tool("alpha"), None);
        registry.register(make_tool("beta"), None);
        let mut names = registry.names();
        names.sort();
        assert_eq!(names, vec!["alpha", "beta"]);
    }

    #[test]
    fn test_register_multiple_and_unregister() {
        let mut registry = CustomToolRegistry::new();
        registry.register(make_tool("tool1"), None);
        registry.register(make_tool("tool2"), None);
        registry.register(make_tool("tool3"), None);
        assert_eq!(registry.len(), 3);
        registry.unregister("tool2");
        assert_eq!(registry.len(), 2);
        assert!(registry.contains("tool1"));
        assert!(!registry.contains("tool2"));
        assert!(registry.contains("tool3"));
    }

    #[test]
    fn test_clone() {
        let mut registry = CustomToolRegistry::new();
        registry.register(make_tool("tool1"), None);
        let cloned = registry.clone();
        assert_eq!(cloned.len(), 1);
        assert!(cloned.contains("tool1"));
    }
}

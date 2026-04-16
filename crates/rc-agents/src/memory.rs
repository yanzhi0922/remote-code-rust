//! Agent memory management matching Claude Code's `AgentTool/agentMemory.ts`.
//!
//! Persistent agent memory allows agents to store facts and learnings across
//! sessions. Memory is scoped to user, project, or local contexts and stored
//! as markdown files in the agent memory directory.

use std::fs;
use std::path::Path;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::definition::AgentMemoryScope;

/// In-memory representation of an agent's persistent memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMemory {
    /// The agent type this memory belongs to.
    pub agent_type: String,
    /// Collected facts/learnings.
    pub facts: Vec<String>,
    /// When this memory was last updated.
    pub last_updated: DateTime<Utc>,
}

impl AgentMemory {
    /// Create a new empty memory for the given agent type.
    #[must_use]
    pub fn new(agent_type: impl Into<String>) -> Self {
        Self {
            agent_type: agent_type.into(),
            facts: Vec::new(),
            last_updated: Utc::now(),
        }
    }

    /// Load agent memory from the given directory.
    ///
    /// Looks for a `MEMORY.md` file in the directory. Returns `Ok(None)` if
    /// no memory file exists.
    pub fn load(agent_type: &str, dir: &Path) -> Result<Option<Self>> {
        let memory_file = dir.join("MEMORY.md");
        if !memory_file.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&memory_file)?;
        let facts = parse_memory_content(&content);
        Ok(Some(Self {
            agent_type: agent_type.to_owned(),
            facts,
            last_updated: fs::metadata(&memory_file)
                .and_then(|m| m.modified())
                .map(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .map(|d| DateTime::from_timestamp(d.as_secs() as i64, 0))
                        .ok()
                        .flatten()
                })
                .ok()
                .flatten()
                .unwrap_or_else(Utc::now),
        }))
    }

    /// Save agent memory to the given directory.
    ///
    /// Creates the directory if it doesn't exist. Writes a `MEMORY.md` file
    /// with the current facts.
    pub fn save(&self, dir: &Path) -> Result<()> {
        fs::create_dir_all(dir)?;
        let content = self.render_markdown();
        fs::write(dir.join("MEMORY.md"), content)?;
        Ok(())
    }

    /// Add a new fact, deduplicating against existing entries.
    pub fn add_fact(&mut self, fact: impl Into<String>) {
        let fact = fact.into();
        if !self.facts.contains(&fact) {
            self.facts.push(fact);
            self.last_updated = Utc::now();
        }
    }

    /// Remove facts matching a predicate.
    pub fn remove_facts(&mut self, predicate: impl Fn(&str) -> bool) {
        let before = self.facts.len();
        self.facts.retain(|f| !predicate(f));
        if self.facts.len() != before {
            self.last_updated = Utc::now();
        }
    }

    /// Render the memory as a prompt section for injection into agent context.
    pub fn to_prompt_section(&self) -> String {
        if self.facts.is_empty() {
            return String::new();
        }
        let mut out = String::from("# Persistent Agent Memory\n\n");
        out.push_str("The following facts were learned in previous sessions:\n\n");
        for fact in &self.facts {
            out.push_str("- ");
            out.push_str(fact);
            out.push('\n');
        }
        out
    }

    /// Render the memory as a markdown document.
    fn render_markdown(&self) -> String {
        let mut out = String::from("# Agent Memory\n\n");
        out.push_str(&format!(
            "_Last updated: {}_\n\n",
            self.last_updated.format("%Y-%m-%d %H:%M:%S UTC")
        ));
        for fact in &self.facts {
            out.push_str("- ");
            out.push_str(fact);
            out.push('\n');
        }
        out
    }
}

/// Parse facts from a markdown memory file.
fn parse_memory_content(content: &str) -> Vec<String> {
    content
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            trimmed.strip_prefix("- ").map(|s| s.to_owned())
        })
        .collect()
}

/// Sanitize an agent type name for use as a directory name.
/// Replaces colons (invalid on Windows) with dashes.
pub fn sanitize_agent_type_for_path(agent_type: &str) -> String {
    agent_type.replace(':', "-")
}

/// Get the agent memory directory for a given agent type and scope.
///
/// - `Project` scope: `<base>/.claude/agent-memory/<agent_type>/`
/// - `User` scope: `<config_home>/agent-memory/<agent_type>/`
/// - `Local` scope: `<base>/.claude/agent-memory-local/<agent_type>/`
pub fn get_agent_memory_dir(
    agent_type: &str,
    scope: AgentMemoryScope,
    base: &Path,
    config_home: &Path,
) -> std::path::PathBuf {
    let dir_name = sanitize_agent_type_for_path(agent_type);
    match scope {
        AgentMemoryScope::Project => base.join(".claude").join("agent-memory").join(dir_name),
        AgentMemoryScope::User => config_home.join("agent-memory").join(dir_name),
        AgentMemoryScope::Local => base
            .join(".claude")
            .join("agent-memory-local")
            .join(dir_name),
    }
}

/// Check if a path is within an agent memory directory (any scope).
///
/// The path should be absolute or relative to the current working directory.
/// Does not resolve symlinks or canonicalize the path.
pub fn is_agent_memory_path(path: &Path, base: &Path, config_home: &Path) -> bool {
    // User scope
    let user_memory = config_home.join("agent-memory");
    if path.starts_with(&user_memory) {
        return true;
    }

    // Project scope
    let project_memory = base.join(".claude").join("agent-memory");
    if path.starts_with(&project_memory) {
        return true;
    }

    // Local scope
    let local_memory = base.join(".claude").join("agent-memory-local");
    if path.starts_with(&local_memory) {
        return true;
    }

    false
}

/// Build the memory prompt for an agent with memory enabled.
///
/// Creates the memory directory if needed and returns a prompt section
/// describing how to use persistent memory.
pub fn build_memory_prompt(
    agent_type: &str,
    scope: AgentMemoryScope,
    base: &Path,
    config_home: &Path,
) -> String {
    let scope_note = match scope {
        AgentMemoryScope::User => {
            "Since this memory is user-scope, keep learnings general since they apply across all projects"
        }
        AgentMemoryScope::Project => {
            "Since this memory is project-scope and shared with your team via version control, tailor your memories to this project"
        }
        AgentMemoryScope::Local => {
            "Since this memory is local-scope (not checked into version control), tailor your memories to this project and machine"
        }
    };

    let memory_dir = get_agent_memory_dir(agent_type, scope, base, config_home);
    let memory_dir_str = memory_dir.display();

    format!(
        r#"# Persistent Agent Memory

You have access to persistent memory stored at `{memory_dir_str}`.

Guidelines:
- {scope_note}
- Write facts as individual bullet points in MEMORY.md
- Keep entries concise and factual
- Remove outdated entries when they no longer apply
"#
    )
}

/// Get a display string for the memory scope.
pub fn get_memory_scope_display(scope: &Option<AgentMemoryScope>, _base: &Path) -> &'static str {
    match scope {
        Some(AgentMemoryScope::User) => "User (~/.claude/agent-memory/)",
        Some(AgentMemoryScope::Project) => "Project (.claude/agent-memory/)",
        Some(AgentMemoryScope::Local) => "Local (.claude/agent-memory-local/)",
        None => "None",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::PathBuf;

    #[test]
    fn new_memory_is_empty() {
        let mem = AgentMemory::new("test-agent");
        assert_eq!(mem.agent_type, "test-agent");
        assert!(mem.facts.is_empty());
    }

    #[test]
    fn add_fact_deduplicates() {
        let mut mem = AgentMemory::new("test");
        mem.add_fact("The project uses Rust");
        mem.add_fact("The project uses Rust");
        assert_eq!(mem.facts.len(), 1);
    }

    #[test]
    fn add_fact_different_entries() {
        let mut mem = AgentMemory::new("test");
        mem.add_fact("Uses Rust");
        mem.add_fact("Uses Tokio");
        assert_eq!(mem.facts.len(), 2);
    }

    #[test]
    fn remove_facts_by_predicate() {
        let mut mem = AgentMemory::new("test");
        mem.add_fact("keep this");
        mem.add_fact("remove this");
        mem.add_fact("also keep");
        mem.remove_facts(|f| f.contains("remove"));
        assert_eq!(mem.facts.len(), 2);
        assert!(mem.facts.contains(&"keep this".to_owned()));
        assert!(mem.facts.contains(&"also keep".to_owned()));
    }

    #[test]
    fn prompt_section_empty() {
        let mem = AgentMemory::new("test");
        assert!(mem.to_prompt_section().is_empty());
    }

    #[test]
    fn prompt_section_with_facts() {
        let mut mem = AgentMemory::new("test");
        mem.add_fact("fact one");
        let section = mem.to_prompt_section();
        assert!(section.contains("# Persistent Agent Memory"));
        assert!(section.contains("fact one"));
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut mem = AgentMemory::new("test-agent");
        mem.add_fact("fact 1");
        mem.add_fact("fact 2");

        mem.save(dir.path()).expect("save");
        let loaded = AgentMemory::load("test-agent", dir.path())
            .expect("load")
            .expect("some");

        let loaded_facts: HashSet<_> = loaded.facts.into_iter().collect();
        let expected_facts: HashSet<_> =
            vec!["fact 1".to_owned(), "fact 2".to_owned()].into_iter().collect();
        assert_eq!(loaded_facts, expected_facts);
    }

    #[test]
    fn load_nonexistent_returns_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        let result = AgentMemory::load("nonexistent", dir.path()).expect("load");
        assert!(result.is_none());
    }

    #[test]
    fn sanitize_agent_type() {
        assert_eq!(sanitize_agent_type_for_path("my-plugin:my-agent"), "my-plugin-my-agent");
        assert_eq!(sanitize_agent_type_for_path("simple"), "simple");
    }

    #[test]
    fn memory_dir_project_scope() {
        let base = PathBuf::from("/project");
        let config = PathBuf::from("/home/.config");
        let dir = get_agent_memory_dir("test", AgentMemoryScope::Project, &base, &config);
        assert_eq!(dir, PathBuf::from("/project/.claude/agent-memory/test"));
    }

    #[test]
    fn memory_dir_user_scope() {
        let base = PathBuf::from("/project");
        let config = PathBuf::from("/home/.config");
        let dir = get_agent_memory_dir("test", AgentMemoryScope::User, &base, &config);
        assert_eq!(dir, PathBuf::from("/home/.config/agent-memory/test"));
    }

    #[test]
    fn is_agent_memory_path_check() {
        let base = PathBuf::from("/project");
        let config = PathBuf::from("/home/.config");
        let project_mem = PathBuf::from("/project/.claude/agent-memory/test/MEMORY.md");
        assert!(is_agent_memory_path(&project_mem, &base, &config));

        let user_mem = PathBuf::from("/home/.config/agent-memory/test/MEMORY.md");
        assert!(is_agent_memory_path(&user_mem, &base, &config));

        let random = PathBuf::from("/tmp/other.md");
        assert!(!is_agent_memory_path(&random, &base, &config));
    }

    #[test]
    fn parse_memory_content_extracts_facts() {
        let content = "# Agent Memory\n\n- fact one\n- fact two\n\nSome other text\n";
        let facts = parse_memory_content(content);
        assert_eq!(facts, vec!["fact one", "fact two"]);
    }
}

//! Agent directory loader matching Claude Code's `AgentTool/loadAgentsDir.ts`.
//!
//! Loads agent definitions from `.claude/agents/` directories (user, project,
//! and local settings) as well as from JSON configuration files.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::builtins::get_built_in_agents;
use crate::definition::{AgentDefinition, AgentSource};

/// JSON schema for agent definitions loaded from files.
#[derive(Debug, Deserialize)]
struct AgentFileEntry {
    description: String,
    #[serde(default)]
    tools: Vec<String>,
    #[serde(default)]
    disallowed_tools: Vec<String>,
    prompt: String,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    permission_mode: Option<String>,
    #[serde(default)]
    max_turns: Option<u32>,
    #[serde(default)]
    skills: Vec<String>,
    #[serde(default)]
    memory: Option<crate::definition::AgentMemoryScope>,
    #[serde(default)]
    background: bool,
}

/// Result of loading all agent definitions.
#[derive(Debug)]
pub struct AgentDefinitionsResult {
    /// Agents that are currently active (winning overrides).
    pub active_agents: Vec<AgentDefinition>,
    /// All agents including overridden ones.
    pub all_agents: Vec<AgentDefinition>,
    /// Files that failed to load with error messages.
    pub failed_files: Vec<(String, String)>,
}

/// Load all agent definitions from a directory.
///
/// Looks for `.md` files with YAML frontmatter and `.json` files containing
/// agent definitions. Each file becomes an agent definition with the filename
/// (without extension) as the `agent_type`.
pub fn load_agents_from_dir(dir: &Path, source: AgentSource) -> Result<Vec<AgentDefinition>> {
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut agents = Vec::new();
    let entries = fs::read_dir(dir)
        .with_context(|| format!("Failed to read agent directory: {}", dir.display()))?;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                // Skip entries we can't read but continue
                tracing::warn!("Skipping unreadable dir entry: {}", e);
                continue;
            }
        };

        let path = entry.path();
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");

        match extension {
            "md" => {
                if let Ok(agent) = load_agent_from_markdown(&path, source) {
                    agents.push(agent);
                }
            }
            "json" => {
                if let Ok(agent_list) = load_agents_from_json(&path, source) {
                    agents.extend(agent_list);
                }
            }
            _ => {}
        }
    }

    agents.sort_by(|a, b| a.agent_type.cmp(&b.agent_type));
    Ok(agents)
}

/// Load a single agent definition from a markdown file.
///
/// Parses YAML frontmatter for metadata (tools, model, etc.) and uses the
/// body as the system prompt.
pub fn load_agent_from_markdown(path: &Path, source: AgentSource) -> Result<AgentDefinition> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read agent file: {}", path.display()))?;

    let filename = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_owned();

    let (frontmatter, body) = parse_frontmatter(&content);

    let mut agent = AgentDefinition::new(
        &filename,
        frontmatter.description.as_deref().unwrap_or(&filename),
    );
    agent.source = source;
    agent.base_dir = path
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("")
        .to_owned();
    agent.filename = Some(filename.clone());
    agent.system_prompt = Some(body.trim().to_owned());

    if let Some(desc) = frontmatter.description {
        agent.when_to_use = desc;
    }
    if !frontmatter.tools.is_empty() {
        agent.tools = frontmatter.tools;
    }
    if !frontmatter.disallowed_tools.is_empty() {
        agent.disallowed_tools = frontmatter.disallowed_tools;
    }
    if let Some(model) = frontmatter.model {
        agent.model = Some(model);
    }
    if let Some(max_turns) = frontmatter.max_turns {
        agent.max_turns = max_turns;
    }
    if !frontmatter.skills.is_empty() {
        agent.skills = frontmatter.skills;
    }
    if let Some(memory) = frontmatter.memory {
        agent.memory = Some(memory);
    }
    if frontmatter.background {
        agent.background = true;
    }

    Ok(agent)
}

/// Load agent definitions from a JSON file.
///
/// The JSON format is a map from agent type names to agent definitions:
/// ```json
/// {
///   "my-agent": {
///     "description": "...",
///     "prompt": "...",
///     "tools": ["Bash", "Read"]
///   }
/// }
/// ```
pub fn load_agents_from_json(path: &Path, source: AgentSource) -> Result<Vec<AgentDefinition>> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read agent JSON file: {}", path.display()))?;

    let entries: std::collections::HashMap<String, AgentFileEntry> = serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse agent JSON file: {}", path.display()))?;

    let base_dir = path
        .parent()
        .and_then(|p| p.to_str())
        .unwrap_or("")
        .to_owned();

    let agents = entries
        .into_iter()
        .map(|(name, entry)| {
            let mut agent = AgentDefinition::new(&name, &entry.description);
            agent.source = source;
            agent.base_dir = base_dir.clone();
            agent.filename = Some(name.clone());
            agent.system_prompt = Some(entry.prompt);
            agent.tools = entry.tools;
            agent.disallowed_tools = entry.disallowed_tools;
            agent.model = entry.model;
            agent.permission_mode = entry.permission_mode;
            agent.max_turns = entry.max_turns.unwrap_or(200);
            agent.skills = entry.skills;
            agent.memory = entry.memory;
            agent.background = entry.background;
            agent
        })
        .collect();

    Ok(agents)
}

/// Load a single agent definition from any supported file format.
pub fn load_agent_from_file(path: &Path, source: AgentSource) -> Result<AgentDefinition> {
    let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    match extension {
        "md" => load_agent_from_markdown(path, source),
        "json" => load_agents_from_json(path, source)?
            .into_iter()
            .next()
            .context("JSON file contained no agent definitions"),
        _ => anyhow::bail!(
            "Unsupported agent file format: .{} (expected .md or .json)",
            extension
        ),
    }
}

/// Parsed frontmatter from a markdown agent file.
#[derive(Debug, Default)]
struct Frontmatter {
    description: Option<String>,
    tools: Vec<String>,
    disallowed_tools: Vec<String>,
    model: Option<String>,
    max_turns: Option<u32>,
    skills: Vec<String>,
    memory: Option<crate::definition::AgentMemoryScope>,
    background: bool,
}

/// Parse YAML frontmatter from a markdown file.
///
/// Expects `---` delimiters at the start and end of the frontmatter block.
fn parse_frontmatter(content: &str) -> (Frontmatter, String) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (Frontmatter::default(), content.to_owned());
    }

    let rest = &trimmed[3..];
    let end = match rest.find("---") {
        Some(i) => i,
        None => return (Frontmatter::default(), content.to_owned()),
    };

    let yaml_str = &rest[..end];
    let body = rest[end + 3..].to_owned();

    let fm = parse_yaml_frontmatter(yaml_str);
    (fm, body)
}

/// Minimal YAML frontmatter parser for agent files.
fn parse_yaml_frontmatter(yaml: &str) -> Frontmatter {
    let mut fm = Frontmatter::default();

    for line in yaml.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();

            match key {
                "description" => {
                    fm.description = Some(unquote(value).to_owned());
                }
                "tools" => {
                    fm.tools = parse_yaml_list(value);
                }
                "disallowedTools" | "disallowed_tools" => {
                    fm.disallowed_tools = parse_yaml_list(value);
                }
                "model" => {
                    fm.model = Some(unquote(value).to_owned());
                }
                "maxTurns" | "max_turns" => {
                    fm.max_turns = value.parse().ok();
                }
                "skills" => {
                    fm.skills = parse_yaml_list(value);
                }
                "background" => {
                    fm.background = value == "true";
                }
                "memory" => {
                    fm.memory = match value {
                        "user" => Some(crate::definition::AgentMemoryScope::User),
                        "project" => Some(crate::definition::AgentMemoryScope::Project),
                        "local" => Some(crate::definition::AgentMemoryScope::Local),
                        _ => None,
                    };
                }
                _ => {}
            }
        }
    }

    fm
}

/// Parse a YAML list value like `[a, b, c]`.
fn parse_yaml_list(value: &str) -> Vec<String> {
    let value = value.trim();
    if value.starts_with('[') && value.ends_with(']') {
        value[1..value.len() - 1]
            .split(',')
            .map(|s| unquote(s.trim()).to_owned())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        Vec::new()
    }
}

/// Remove surrounding quotes from a YAML value.
fn unquote(s: &str) -> &str {
    s.trim()
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| s.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(s)
}

/// Load all agents: built-in + from directories.
///
/// Combines built-in agents with user-loaded agents from the given
/// directories, resolving overrides by agent type.
pub fn load_all_agents(
    user_dir: Option<&Path>,
    project_dir: Option<&Path>,
) -> AgentDefinitionsResult {
    let mut all_agents = get_built_in_agents();
    let mut failed_files = Vec::new();

    if let Some(dir) = user_dir {
        match load_agents_from_dir(dir, AgentSource::User) {
            Ok(agents) => all_agents.extend(agents),
            Err(e) => failed_files.push((dir.display().to_string(), e.to_string())),
        }
    }

    if let Some(dir) = project_dir {
        match load_agents_from_dir(dir, AgentSource::Project) {
            Ok(agents) => all_agents.extend(agents),
            Err(e) => failed_files.push((dir.display().to_string(), e.to_string())),
        }
    }

    let active_agents = resolve_active_agents(&all_agents);

    AgentDefinitionsResult {
        active_agents,
        all_agents,
        failed_files,
    }
}

/// Resolve which agents are active based on priority (later source wins).
fn resolve_active_agents(all_agents: &[AgentDefinition]) -> Vec<AgentDefinition> {
    let mut seen_types: HashSet<String> = HashSet::new();
    let mut active: Vec<AgentDefinition> = Vec::new();

    // Process in reverse order so later entries (higher priority) win
    for agent in all_agents.iter().rev() {
        if !seen_types.contains(&agent.agent_type) {
            seen_types.insert(agent.agent_type.clone());
            active.push(agent.clone());
        }
    }

    active.sort_by(|a, b| a.agent_type.cmp(&b.agent_type));
    active
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_from_nonexistent_dir_returns_empty() {
        let dir = Path::new("/nonexistent/path");
        let result = load_agents_from_dir(dir, AgentSource::User);
        assert!(result.is_ok());
        assert!(result.expect("ok").is_empty());
    }

    #[test]
    fn load_agent_from_json_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let json = r#"{
            "my-agent": {
                "description": "My custom agent",
                "prompt": "You are a custom agent",
                "tools": ["Bash", "Read"]
            }
        }"#;
        let path = dir.path().join("agents.json");
        fs::write(&path, json).expect("write");

        let agents = load_agents_from_json(&path, AgentSource::User).expect("load");
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].agent_type, "my-agent");
        assert_eq!(agents[0].tools, vec!["Bash", "Read"]);
        assert_eq!(agents[0].source, AgentSource::User);
    }

    #[test]
    fn load_agent_from_markdown_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let md = "---\ndescription: Test agent\ntools: [Bash]\n---\nYou are a test agent.\n";
        let path = dir.path().join("test-agent.md");
        fs::write(&path, md).expect("write");

        let agent = load_agent_from_markdown(&path, AgentSource::Project).expect("load");
        assert_eq!(agent.agent_type, "test-agent");
        assert_eq!(agent.when_to_use, "Test agent");
        assert_eq!(agent.tools, vec!["Bash"]);
        assert_eq!(agent.source, AgentSource::Project);
        assert_eq!(
            agent.system_prompt.as_deref(),
            Some("You are a test agent.")
        );
    }

    #[test]
    fn load_agent_from_file_dispatches_by_extension() {
        let dir = tempfile::tempdir().expect("tempdir");

        // JSON
        let json = r#"{"a": {"description": "d", "prompt": "p"}}"#;
        let json_path = dir.path().join("agents.json");
        fs::write(&json_path, json).expect("write");
        let agent = load_agent_from_file(&json_path, AgentSource::User).expect("load json");
        assert_eq!(agent.agent_type, "a");

        // Markdown
        let md = "---\n---\nBody text\n";
        let md_path = dir.path().join("my-agent.md");
        fs::write(&md_path, md).expect("write");
        let agent = load_agent_from_file(&md_path, AgentSource::User).expect("load md");
        assert_eq!(agent.agent_type, "my-agent");
    }

    #[test]
    fn unsupported_extension_returns_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("agent.yaml");
        fs::write(&path, "key: value").expect("write");
        let result = load_agent_from_file(&path, AgentSource::User);
        assert!(result.is_err());
    }

    #[test]
    fn load_all_combines_builtins_and_custom() {
        let dir = tempfile::tempdir().expect("tempdir");
        let json = r#"{"custom": {"description": "Custom", "prompt": "Do stuff"}}"#;
        fs::write(dir.path().join("agents.json"), json).expect("write");

        let result = load_all_agents(Some(dir.path()), None);
        // Should have built-in agents + custom
        assert!(result.active_agents.len() > 6);
        assert!(
            result
                .active_agents
                .iter()
                .any(|a| a.agent_type == "custom")
        );
    }

    #[test]
    fn resolve_active_agents_deduplicates() {
        let agents = vec![AgentDefinition::new("test", "built-in"), {
            let mut d = AgentDefinition::new("test", "user override");
            d.source = AgentSource::User;
            d
        }];

        let active = resolve_active_agents(&agents);
        // Should have exactly one "test" agent
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].source, AgentSource::User);
    }

    #[test]
    fn parse_yaml_list_brackets() {
        let result = parse_yaml_list("[Bash, Read, Write]");
        assert_eq!(result, vec!["Bash", "Read", "Write"]);
    }

    #[test]
    fn parse_yaml_list_empty() {
        let result = parse_yaml_list("[]");
        assert!(result.is_empty());
    }

    #[test]
    fn parse_yaml_list_non_list() {
        let result = parse_yaml_list("not a list");
        assert!(result.is_empty());
    }

    #[test]
    fn unquote_removes_double_quotes() {
        assert_eq!(unquote("\"hello\""), "hello");
    }

    #[test]
    fn unquote_removes_single_quotes() {
        assert_eq!(unquote("'hello'"), "hello");
    }

    #[test]
    fn unquote_no_quotes() {
        assert_eq!(unquote("hello"), "hello");
    }
}

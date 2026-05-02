//! Agent definition loader from Markdown files.
//!
//! Loads agent definitions from:
//! - Built-in definitions (compiled into the binary)
//! - User-level: `~/.remote-code/agents/*.md`
//! - Project-level: `.remote-code/agents/*.md`

use anyhow::Result;
use std::path::Path;
use walkdir::WalkDir;

use crate::types::{AgentFrontmatter, AgentModel, AgentScope, SpecializedAgent};

/// Loads agent definitions from the filesystem.
pub struct AgentLoader;

impl AgentLoader {
    /// Load a single agent definition from a Markdown file.
    pub fn load_from_file(path: &Path, scope: AgentScope) -> Result<SpecializedAgent> {
        let content = std::fs::read_to_string(path)?;
        Self::parse_agent_definition(&content, path.to_string_lossy().as_ref(), scope)
    }

    /// Parse an agent definition from Markdown content.
    pub fn parse_agent_definition(
        content: &str,
        source_path: &str,
        scope: AgentScope,
    ) -> Result<SpecializedAgent> {
        let (frontmatter_str, body) = Self::split_frontmatter(content);

        let frontmatter: AgentFrontmatter = if frontmatter_str.is_empty() {
            return Err(anyhow::anyhow!(
                "No YAML frontmatter found in agent definition: {source_path}"
            ));
        } else {
            serde_yaml::from_str(frontmatter_str).map_err(|e| {
                anyhow::anyhow!("Failed to parse frontmatter in {source_path}: {e}")
            })?
        };

        let model = match frontmatter.model.as_str() {
            "inherit" => AgentModel::Inherit,
            name => AgentModel::Specific(name.to_string()),
        };

        Ok(SpecializedAgent {
            name: frontmatter.name,
            description: frontmatter.description,
            model,
            allowed_tools: frontmatter.tools,
            max_turns: frontmatter.max_turns,
            read_only: frontmatter.read_only,
            system_prompt: body.trim().to_string(),
            scope,
        })
    }

    /// Discover all agent files in a directory.
    pub fn discover_agents(dir: &Path, scope: AgentScope) -> Vec<SpecializedAgent> {
        let mut agents = Vec::new();

        if !dir.exists() {
            return agents;
        }

        for entry in WalkDir::new(dir)
            .max_depth(2)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str());
            if ext != Some("md") {
                continue;
            }

            match Self::load_from_file(path, scope) {
                Ok(agent) => agents.push(agent),
                Err(e) => {
                    tracing::warn!("Failed to load agent from {}: {e}", path.display());
                }
            }
        }

        agents
    }

    /// Split Markdown content into frontmatter and body.
    fn split_frontmatter(content: &str) -> (&str, &str) {
        let trimmed = content.trim_start();
        if !trimmed.starts_with("---") {
            return ("", content);
        }

        let after_first = &trimmed[3..];
        if let Some(end_pos) = after_first.find("\n---") {
            let frontmatter = after_first[..end_pos].trim();
            let body = after_first[end_pos + 4..].trim_start_matches('\n');
            (frontmatter, body)
        } else {
            ("", content)
        }
    }

    /// Parse an @mention from user input.
    /// Returns the agent name and remaining text.
    pub fn parse_mention(input: &str) -> Option<crate::types::AgentMention> {
        let input = input.trim_start();
        if !input.starts_with('@') {
            return None;
        }

        let after_at = &input[1..];
        let name_end = after_at
            .find(|c: char| c.is_whitespace() || c == ':')
            .unwrap_or(after_at.len());
        let agent_name = &after_at[..name_end];

        if agent_name.is_empty() {
            return None;
        }

        let remaining = after_at.get(name_end..).unwrap_or("").trim_start_matches(|c: char| c.is_whitespace() || c == ':').trim_start();

        Some(crate::types::AgentMention {
            agent_name: agent_name.to_string(),
            remaining_text: remaining.to_string(),
            position: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_AGENT: &str = r#"---
name: code-reviewer
description: Code review expert for security and quality
model: inherit
tools:
  - read_file
  - search_files
  - list_files
max_turns: 10
read_only: true
---

You are a code review expert. Analyze code for:
1. Security vulnerabilities
2. Performance issues
3. Code quality
"#;

    #[test]
    fn test_parse_agent_definition() {
        let agent = AgentLoader::parse_agent_definition(SAMPLE_AGENT, "test.md", AgentScope::User)
            .unwrap();

        assert_eq!(agent.name, "code-reviewer");
        assert_eq!(agent.description, "Code review expert for security and quality");
        assert!(matches!(agent.model, AgentModel::Inherit));
        assert_eq!(agent.allowed_tools, vec!["read_file", "search_files", "list_files"]);
        assert_eq!(agent.max_turns, Some(10));
        assert!(agent.read_only);
        assert!(agent.system_prompt.contains("code review expert"));
        assert_eq!(agent.scope, AgentScope::User);
    }

    #[test]
    fn test_parse_agent_with_specific_model() {
        let content = r#"---
name: test-agent
description: Test
model: claude-sonnet-4.5
---
Do stuff."#;
        let agent = AgentLoader::parse_agent_definition(content, "test.md", AgentScope::BuiltIn)
            .unwrap();
        assert!(matches!(agent.model, AgentModel::Specific(ref m) if m == "claude-sonnet-4.5"));
    }

    #[test]
    fn test_parse_no_frontmatter() {
        let content = "Just a regular markdown file";
        let result = AgentLoader::parse_agent_definition(content, "test.md", AgentScope::BuiltIn);
        assert!(result.is_err());
    }

    #[test]
    fn test_split_frontmatter() {
        let (fm, body) = AgentLoader::split_frontmatter("---\nname: test\n---\nBody here\n");
        assert_eq!(fm, "name: test");
        assert_eq!(body, "Body here\n");
    }

    #[test]
    fn test_parse_mention() {
        let mention = AgentLoader::parse_mention("@code-reviewer Review this code").unwrap();
        assert_eq!(mention.agent_name, "code-reviewer");
        assert_eq!(mention.remaining_text, "Review this code");
    }

    #[test]
    fn test_parse_mention_no_space() {
        let mention = AgentLoader::parse_mention("@code-reviewer").unwrap();
        assert_eq!(mention.agent_name, "code-reviewer");
        assert_eq!(mention.remaining_text, "");
    }

    #[test]
    fn test_parse_mention_no_at() {
        let mention = AgentLoader::parse_mention("just a message");
        assert!(mention.is_none());
    }

    #[test]
    fn test_parse_mention_with_colon() {
        let mention = AgentLoader::parse_mention("@bug-analyzer: Check this error").unwrap();
        assert_eq!(mention.agent_name, "bug-analyzer");
        assert_eq!(mention.remaining_text, "Check this error");
    }
}

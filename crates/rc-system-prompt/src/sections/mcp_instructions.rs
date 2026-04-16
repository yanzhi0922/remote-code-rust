//! MCP Server Instructions section — connected MCP server guidance.
//!
//! Matches `getMcpInstructionsSection()` in Claude Code's `prompts.ts`.

use anyhow::Result;

use crate::{McpClientInfo, PromptContext};
use crate::sections::SystemPromptSection;

/// The MCP instructions section.
pub struct McpInstructionsSection;

impl SystemPromptSection for McpInstructionsSection {
    fn name(&self) -> &str {
        "mcp_instructions"
    }

    /// MCP instructions may change between turns as servers connect/disconnect.
    fn is_cacheable(&self) -> bool {
        false
    }

    fn compute(&self, ctx: &PromptContext) -> Result<Option<String>> {
        if ctx.mcp_clients.is_empty() {
            return Ok(None);
        }

        let clients_with_instructions: Vec<&McpClientInfo> = ctx
            .mcp_clients
            .iter()
            .filter(|c| c.instructions.is_some())
            .collect();

        if clients_with_instructions.is_empty() {
            return Ok(None);
        }

        let instruction_blocks: Vec<String> = clients_with_instructions
            .iter()
            .map(|client| {
                let instructions = client.instructions.as_deref().unwrap_or("");
                format!("## {}\n{}", client.name, instructions)
            })
            .collect();

        Ok(Some(format!(
            "# MCP Server Instructions\n\n\
            The following MCP servers have provided instructions for how to use their tools and resources:\n\n\
            {}",
            instruction_blocks.join("\n\n")
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::PathBuf;

    fn test_ctx_with_mcp(clients: Vec<McpClientInfo>) -> PromptContext {
        PromptContext {
            model: "test".to_string(),
            cwd: PathBuf::from("/tmp"),
            is_git: false,
            platform: "linux".to_string(),
            shell: "bash".to_string(),
            os_version: "Linux 6.6".to_string(),
            enabled_tools: HashSet::new(),
            language: None,
            output_style: None,
            mcp_clients: clients,
            is_worktree: false,
            additional_dirs: vec![],
            is_non_interactive: false,
            is_fork_subagent_enabled: false,
            session_start_date: "2025-01-01".to_string(),
        }
    }

    #[test]
    fn mcp_instructions_empty_clients() {
        let section = McpInstructionsSection;
        let result = section
            .compute(&test_ctx_with_mcp(vec![]))
            .expect("compute ok");
        assert!(result.is_none());
    }

    #[test]
    fn mcp_instructions_with_client() {
        let clients = vec![McpClientInfo {
            name: "test-server".to_string(),
            instructions: Some("Use tools carefully.".to_string()),
        }];
        let section = McpInstructionsSection;
        let result = section
            .compute(&test_ctx_with_mcp(clients))
            .expect("compute ok");
        let content = result.expect("should be Some");
        assert!(content.starts_with("# MCP Server Instructions"));
        assert!(content.contains("test-server"));
        assert!(content.contains("Use tools carefully."));
    }

    #[test]
    fn mcp_instructions_client_without_instructions() {
        let clients = vec![McpClientInfo {
            name: "no-instructions".to_string(),
            instructions: None,
        }];
        let section = McpInstructionsSection;
        let result = section
            .compute(&test_ctx_with_mcp(clients))
            .expect("compute ok");
        assert!(result.is_none());
    }

    #[test]
    fn mcp_instructions_not_cacheable() {
        let section = McpInstructionsSection;
        assert!(!section.is_cacheable());
    }
}

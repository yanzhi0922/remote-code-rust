//! Dynamic system prompt builder matching Claude Code's `constants/prompts.ts`.
//!
//! This crate provides a modular, section-based system prompt builder that
//! produces the same prompt structure as Claude Code's TypeScript implementation.
//!
//! # Architecture
//!
//! - [`SystemPromptBuilder`] - orchestrates section computation and cache management
//! - [`PromptContext`] - runtime context passed to each section
//! - [`SystemPromptSection`] - trait implemented by each prompt section
//! - [`SectionCache`] - in-memory cache for computed sections
//!
//! # Section Ordering
//!
//! The sections are ordered to match Claude Code's `getSystemPrompt()`:
//!
//! **Static (cacheable):**
//! 1. Intro
//! 2. System
//! 3. Doing Tasks
//! 4. Actions with Care
//! 5. Using Your Tools
//! 6. Tone and Style
//! 7. Output Efficiency
//!
//! **Boundary marker**
//!
//! **Dynamic (per-session):**
//! 8. Session-specific Guidance
//! 9. Memory
//! 10. Environment Info
//! 11. Language
//! 12. Output Style
//! 13. MCP Instructions
//! 14. Scratchpad
//! 15. Function Result Clearing
//! 16. Summarize Tool Results
//! 17. Proactive (if enabled)

pub mod cache;
pub mod sections;

use std::collections::HashSet;
use std::path::PathBuf;

use anyhow::Result;

use cache::{SYSTEM_PROMPT_DYNAMIC_BOUNDARY, SectionCache};
use sections::SystemPromptSection;
use sections::actions::ActionsSection;
use sections::coordinator::CoordinatorSection;
use sections::doing_tasks::DoingTasksSection;
use sections::env_info::EnvInfoSection;
use sections::hooks::HooksSection;
use sections::intro::IntroSection;
use sections::language::LanguageSection;
use sections::mcp_instructions::McpInstructionsSection;
use sections::memory::MemorySection;
use sections::output_efficiency::OutputEfficiencySection;
use sections::output_style::OutputStyleSection;
use sections::proactive::ProactiveSection;
use sections::scratchpad::ScratchpadSection;
use sections::session_guidance::SessionGuidanceSection;
use sections::system::SystemSection;
use sections::system_reminders::SystemRemindersSection;
use sections::token_budget::TokenBudgetSection;
use sections::tone_style::ToneStyleSection;
use sections::tool_result::ToolResultSection;
use sections::using_tools::UsingToolsSection;

/// Configuration for a custom output style.
#[derive(Debug, Clone)]
pub struct OutputStyleConfig {
    /// Name of the output style.
    pub name: String,
    /// The prompt text describing the output style.
    pub prompt: String,
    /// Whether to keep the default coding instructions alongside this style.
    pub keep_coding_instructions: bool,
}

/// Information about a connected MCP server.
#[derive(Debug, Clone)]
pub struct McpClientInfo {
    /// Name of the MCP server.
    pub name: String,
    /// Optional instructions provided by the server.
    pub instructions: Option<String>,
}

/// Runtime context for system prompt section computation.
///
/// This struct carries all the information that sections need to decide
/// what content to include. It is constructed by the application layer
/// and passed to [`SystemPromptBuilder::build`].
#[derive(Debug, Clone)]
pub struct PromptContext {
    /// Model identifier (e.g. "claude-sonnet-4-6").
    pub model: String,
    /// Current working directory.
    pub cwd: PathBuf,
    /// Whether the cwd is inside a git repository.
    pub is_git: bool,
    /// Platform string (e.g. "linux", "darwin", "win32").
    pub platform: String,
    /// User's shell (e.g. "bash", "zsh").
    pub shell: String,
    /// OS version string (e.g. "Linux 6.6.4", "Darwin 25.3.0").
    pub os_version: String,
    /// Set of enabled tool names.
    pub enabled_tools: HashSet<String>,
    /// User's preferred response language.
    pub language: Option<String>,
    /// Custom output style configuration.
    pub output_style: Option<OutputStyleConfig>,
    /// Connected MCP server clients.
    pub mcp_clients: Vec<McpClientInfo>,
    /// Whether this is a git worktree session.
    pub is_worktree: bool,
    /// Additional working directories beyond cwd.
    pub additional_dirs: Vec<PathBuf>,
    /// Whether this is a non-interactive session.
    pub is_non_interactive: bool,
    /// Whether fork subagent mode is enabled.
    pub is_fork_subagent_enabled: bool,
    /// ISO 8601 date string for when the session started.
    pub session_start_date: String,
}

/// Main system prompt builder.
///
/// Orchestrates the computation of static and dynamic sections,
/// manages caching, and produces the final prompt string array.
pub struct SystemPromptBuilder {
    static_sections: Vec<Box<dyn SystemPromptSection>>,
    dynamic_sections: Vec<Box<dyn SystemPromptSection>>,
    cache: SectionCache,
    use_global_cache_scope: bool,
}

impl SystemPromptBuilder {
    /// Create a new builder with no sections.
    #[must_use]
    pub fn new() -> Self {
        Self {
            static_sections: Vec::new(),
            dynamic_sections: Vec::new(),
            cache: SectionCache::new(),
            use_global_cache_scope: true,
        }
    }

    /// Create a builder pre-loaded with all default sections in the correct order.
    ///
    /// The section ordering matches Claude Code's `getSystemPrompt()`.
    #[must_use]
    pub fn with_default_sections() -> Self {
        let mut builder = Self::new();

        // Static sections (before the boundary marker)
        builder.static_sections.push(Box::new(IntroSection));
        builder.static_sections.push(Box::new(SystemSection));
        builder.static_sections.push(Box::new(DoingTasksSection));
        builder.static_sections.push(Box::new(ActionsSection));
        builder.static_sections.push(Box::new(UsingToolsSection));
        builder.static_sections.push(Box::new(ToneStyleSection));
        builder
            .static_sections
            .push(Box::new(OutputEfficiencySection));

        // Dynamic sections (after the boundary marker)
        builder
            .dynamic_sections
            .push(Box::new(SessionGuidanceSection));
        builder.dynamic_sections.push(Box::new(MemorySection));
        builder.dynamic_sections.push(Box::new(EnvInfoSection));
        builder.dynamic_sections.push(Box::new(LanguageSection));
        builder.dynamic_sections.push(Box::new(OutputStyleSection));
        builder
            .dynamic_sections
            .push(Box::new(McpInstructionsSection));
        builder.dynamic_sections.push(Box::new(ScratchpadSection));
        builder.dynamic_sections.push(Box::new(ToolResultSection));
        builder.dynamic_sections.push(Box::new(TokenBudgetSection));
        builder.dynamic_sections.push(Box::new(HooksSection));
        builder
            .dynamic_sections
            .push(Box::new(SystemRemindersSection));
        builder.dynamic_sections.push(Box::new(CoordinatorSection));
        builder.dynamic_sections.push(Box::new(ProactiveSection));

        builder
    }

    /// Add a custom static section.
    pub fn add_static_section(&mut self, section: Box<dyn SystemPromptSection>) {
        self.static_sections.push(section);
    }

    /// Add a custom dynamic section.
    pub fn add_dynamic_section(&mut self, section: Box<dyn SystemPromptSection>) {
        self.dynamic_sections.push(section);
    }

    /// Set whether to use global cache scope (include the boundary marker).
    pub fn set_global_cache_scope(&mut self, enabled: bool) {
        self.use_global_cache_scope = enabled;
    }

    /// Build the complete system prompt.
    ///
    /// Returns a vector of strings representing the system prompt blocks.
    /// The boundary marker [`SYSTEM_PROMPT_DYNAMIC_BOUNDARY`] separates static
    /// from dynamic content (if global cache scope is enabled).
    pub fn build(&mut self, ctx: &PromptContext) -> Result<Vec<String>> {
        let mut result = Vec::new();

        // Compute static sections
        for section in &self.static_sections {
            let name = section.name().to_string();
            let content = if section.is_cacheable() {
                if let Some(cached) = self.cache.get(&name) {
                    cached.clone()
                } else {
                    let computed = section.compute(ctx)?;
                    self.cache.set(&name, computed.clone());
                    computed
                }
            } else {
                section.compute(ctx)?
            };

            if let Some(text) = content {
                result.push(text);
            }
        }

        // Insert boundary marker
        if self.use_global_cache_scope {
            result.push(SYSTEM_PROMPT_DYNAMIC_BOUNDARY.to_string());
        }

        // Compute dynamic sections
        for section in &self.dynamic_sections {
            let name = section.name().to_string();
            let content = if section.is_cacheable() {
                if let Some(cached) = self.cache.get(&name) {
                    cached.clone()
                } else {
                    let computed = section.compute(ctx)?;
                    self.cache.set(&name, computed.clone());
                    computed
                }
            } else {
                section.compute(ctx)?
            };

            if let Some(text) = content {
                result.push(text);
            }
        }

        Ok(result)
    }

    /// Clear all cached section values.
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }

    /// Get the number of static sections.
    #[must_use]
    pub fn static_section_count(&self) -> usize {
        self.static_sections.len()
    }

    /// Get the number of dynamic sections.
    #[must_use]
    pub fn dynamic_section_count(&self) -> usize {
        self.dynamic_sections.len()
    }
}

impl Default for SystemPromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a minimal prompt context for testing purposes.
#[cfg(test)]
pub fn test_prompt_context() -> PromptContext {
    PromptContext {
        model: "claude-sonnet-4-6".to_string(),
        cwd: PathBuf::from("/home/user/project"),
        is_git: true,
        platform: "linux".to_string(),
        shell: "bash".to_string(),
        os_version: "Linux 6.6.4".to_string(),
        enabled_tools: HashSet::new(),
        language: None,
        output_style: None,
        mcp_clients: vec![],
        is_worktree: false,
        additional_dirs: vec![],
        is_non_interactive: false,
        is_fork_subagent_enabled: false,
        session_start_date: "2025-01-01".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builder_default_has_no_sections() {
        let builder = SystemPromptBuilder::new();
        assert_eq!(builder.static_section_count(), 0);
        assert_eq!(builder.dynamic_section_count(), 0);
    }

    #[test]
    fn builder_with_defaults_has_sections() {
        let builder = SystemPromptBuilder::with_default_sections();
        assert_eq!(builder.static_section_count(), 7);
        assert_eq!(builder.dynamic_section_count(), 13);
    }

    #[test]
    fn build_produces_non_empty_prompt() {
        let mut builder = SystemPromptBuilder::with_default_sections();
        let ctx = test_prompt_context();
        let result = builder.build(&ctx).expect("build should succeed");
        assert!(!result.is_empty());
    }

    #[test]
    fn build_contains_boundary_marker() {
        let mut builder = SystemPromptBuilder::with_default_sections();
        let ctx = test_prompt_context();
        let result = builder.build(&ctx).expect("build should succeed");
        assert!(result.contains(&SYSTEM_PROMPT_DYNAMIC_BOUNDARY.to_string()));
    }

    #[test]
    fn build_boundary_comes_after_static_sections() {
        let mut builder = SystemPromptBuilder::with_default_sections();
        let ctx = test_prompt_context();
        let result = builder.build(&ctx).expect("build should succeed");

        let boundary_idx = result
            .iter()
            .position(|s| s == SYSTEM_PROMPT_DYNAMIC_BOUNDARY)
            .expect("boundary should exist");

        // Static content before boundary
        for i in 0..boundary_idx {
            assert!(
                !result[i].is_empty(),
                "Static section {i} should not be empty"
            );
        }

        // The first static section should be the intro
        assert!(
            result[0].contains("You are an interactive agent"),
            "First section should be intro"
        );
    }

    #[test]
    fn build_without_global_cache_has_no_boundary() {
        let mut builder = SystemPromptBuilder::with_default_sections();
        builder.set_global_cache_scope(false);
        let ctx = test_prompt_context();
        let result = builder.build(&ctx).expect("build should succeed");
        assert!(!result.contains(&SYSTEM_PROMPT_DYNAMIC_BOUNDARY.to_string()));
    }

    #[test]
    fn clear_cache_works() {
        let mut builder = SystemPromptBuilder::with_default_sections();
        let ctx = test_prompt_context();
        let _ = builder.build(&ctx);
        builder.clear_cache();
        // After clearing, a new build should still work
        let result = builder
            .build(&ctx)
            .expect("build after clear should succeed");
        assert!(!result.is_empty());
    }

    #[test]
    fn static_section_ordering_matches_claude_code() {
        let builder = SystemPromptBuilder::with_default_sections();
        let names: Vec<&str> = builder.static_sections.iter().map(|s| s.name()).collect();
        assert_eq!(
            names,
            vec![
                "intro",
                "system",
                "doing_tasks",
                "actions",
                "using_tools",
                "tone_style",
                "output_efficiency"
            ]
        );
    }

    #[test]
    fn dynamic_section_ordering_matches_claude_code() {
        let builder = SystemPromptBuilder::with_default_sections();
        let names: Vec<&str> = builder.dynamic_sections.iter().map(|s| s.name()).collect();
        assert_eq!(
            names,
            vec![
                "session_guidance",
                "memory",
                "env_info",
                "language",
                "output_style",
                "mcp_instructions",
                "scratchpad",
                "tool_result",
                "token_budget",
                "hooks",
                "system_reminders",
                "coordinator",
                "proactive"
            ]
        );
    }

    #[test]
    fn full_prompt_contains_expected_sections() {
        let mut builder = SystemPromptBuilder::with_default_sections();
        let ctx = test_prompt_context();
        let result = builder.build(&ctx).expect("build should succeed");
        let combined = result.join("\n---\n");

        // Static sections
        assert!(combined.contains("You are an interactive agent"), "intro");
        assert!(combined.contains("# System"), "system");
        assert!(combined.contains("# Doing tasks"), "doing_tasks");
        assert!(
            combined.contains("# Executing actions with care"),
            "actions"
        );
        assert!(combined.contains("# Using your tools"), "using_tools");
        assert!(combined.contains("# Tone and style"), "tone_style");
        assert!(
            combined.contains("# Output efficiency"),
            "output_efficiency"
        );

        // Dynamic sections (env_info always present)
        assert!(combined.contains("# Environment"), "env_info");
    }

    #[test]
    fn conditional_language_section() {
        let mut builder = SystemPromptBuilder::with_default_sections();
        let mut ctx = test_prompt_context();
        ctx.language = Some("Japanese".to_string());
        let result = builder.build(&ctx).expect("build should succeed");
        let combined = result.join("\n---\n");
        assert!(combined.contains("# Language"));
        assert!(combined.contains("Japanese"));
    }

    #[test]
    fn conditional_language_section_absent() {
        let mut builder = SystemPromptBuilder::with_default_sections();
        let ctx = test_prompt_context();
        let result = builder.build(&ctx).expect("build should succeed");
        let combined = result.join("\n---\n");
        assert!(!combined.contains("# Language"));
    }

    #[test]
    fn conditional_output_style_section() {
        let mut builder = SystemPromptBuilder::with_default_sections();
        let mut ctx = test_prompt_context();
        ctx.output_style = Some(OutputStyleConfig {
            name: "Concise".to_string(),
            prompt: "Be brief.".to_string(),
            keep_coding_instructions: true,
        });
        let result = builder.build(&ctx).expect("build should succeed");
        let combined = result.join("\n---\n");
        assert!(combined.contains("# Output Style: Concise"));
    }

    #[test]
    fn conditional_mcp_section() {
        let mut builder = SystemPromptBuilder::with_default_sections();
        let mut ctx = test_prompt_context();
        ctx.mcp_clients = vec![McpClientInfo {
            name: "test-mcp".to_string(),
            instructions: Some("Use tools carefully.".to_string()),
        }];
        let result = builder.build(&ctx).expect("build should succeed");
        let combined = result.join("\n---\n");
        assert!(combined.contains("# MCP Server Instructions"));
        assert!(combined.contains("test-mcp"));
    }

    #[test]
    fn add_custom_section() {
        let mut builder = SystemPromptBuilder::new();
        builder.add_static_section(Box::new(IntroSection));
        assert_eq!(builder.static_section_count(), 1);
        let ctx = test_prompt_context();
        let result = builder.build(&ctx).expect("build should succeed");
        assert!(result.len() >= 2); // at least intro + boundary
    }

    #[test]
    fn prompt_context_default_values() {
        let ctx = test_prompt_context();
        assert_eq!(ctx.model, "claude-sonnet-4-6");
        assert!(ctx.is_git);
        assert!(ctx.language.is_none());
        assert!(ctx.output_style.is_none());
        assert!(ctx.mcp_clients.is_empty());
        assert!(!ctx.is_non_interactive);
    }
}

//! Specialized agent registry with built-in agents and Markdown-based custom agent definitions.
//!
//! Inspired by ZCode's @agent system: users can define specialized agents as Markdown files
//! with YAML frontmatter, and invoke them via `@agent-name` in chat.
//!
//! # Agent Definition Format
//!
//! ```markdown
//! ---
//! name: code-reviewer
//! description: Code review expert for security and quality
//! model: inherit
//! tools: [read_file, search_files, list_files]
//! max_turns: 10
//! ---
//!
//! You are a code review expert...
//! ```

pub mod builtin;
pub mod loader;
pub mod registry;
pub mod types;

pub use loader::AgentLoader;
pub use registry::AgentRegistry;
pub use types::*;

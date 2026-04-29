//! # Roo Tools MCP
//!
//! MCP (Model Context Protocol) tool implementations: `use_mcp_tool`
//! and `access_mcp_resource`.
//!
//! ## Architecture
//!
//! - **Validation functions** — synchronous parameter validation
//! - **Execution functions** — async tool execution via [`roo_mcp::McpHub`]
//! - **Response formatting** — convert MCP responses to tool results

pub mod types;
pub mod helpers;
pub mod use_mcp_tool;
pub mod access_mcp_resource;

pub use types::*;
pub use helpers::*;
pub use use_mcp_tool::*;
pub use access_mcp_resource::*;

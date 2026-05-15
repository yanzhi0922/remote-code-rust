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

pub mod access_mcp_resource;
pub mod helpers;
pub mod types;
pub mod use_mcp_tool;

pub use access_mcp_resource::*;
pub use helpers::*;
pub use types::*;
pub use use_mcp_tool::*;

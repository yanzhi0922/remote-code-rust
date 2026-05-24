//! Chrome browser automation MCP server using CDP (Chrome DevTools Protocol).
//!
//! Provides 15 browser automation tools (navigate, click, type, screenshot,
//! evaluate_js, etc.) via the MCP (Model Context Protocol) interface.
//!
//! # Architecture
//!
//! - [`browser_pool`] — lazy-initialized, shared Chromium instance
//! - [`browser_detect`] — finds Chrome/Edge/Chromium on the system
//! - [`tools`] — 15 browser automation tool implementations
//! - [`mcp_server`] — MCP server interface (stdio transport)
//!
//! # Usage
//!
//! ```no_run
//! use claude_chrome_mcp::mcp_server;
//!
//! #[tokio::main]
//! async fn main() {
//!     mcp_server::run_stdio_server().await.unwrap();
//! }
//! ```

pub mod browser_detect;
pub mod browser_pool;
pub mod mcp_server;
pub mod tools;

//! # Roo App — Application Layer
//!
//! The main application controller that ties all Roo Code subsystems together.
//!
//! Source: `src/core/ClineProvider.ts` (TypeScript VS Code extension controller)
//!
//! ## Architecture
//!
//! The `App` struct is the top-level coordinator that manages:
//! - **Provider** — AI model API connections
//! - **Task** — Running conversation tasks
//! - **MCP** — Model Context Protocol server connections
//! - **Config** — User/project configuration
//! - **Skills** — Custom skill management
//! - **Checkpoint** — Git-based checkpoint system
//! - **Terminal** — Terminal process management
//! - **Telemetry** — Usage analytics
//!
//! ## Usage
//!
//! ```rust,ignore
//! use roo_app::App;
//!
//! let app = App::new(AppConfig::default());
//! app.initialize().await?;
//! let task_id = app.create_task("code", None).await?;
//! ```

pub mod app;
pub mod config;
pub mod error;
pub mod state;

pub use app::App;
pub use config::AppConfig;
pub use error::AppError;
pub use state::AppState;

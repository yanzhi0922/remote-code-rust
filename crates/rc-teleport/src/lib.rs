//! `rc-teleport` — Session teleportation system.
//!
//! This crate provides session teleportation capabilities, allowing sessions
//! to be transferred between environments. It matches the functionality of
//! Claude Code's `utils/teleport.tsx` and `utils/teleport/` modules.
//!
//! # Overview
//!
//! - **[`api`]** — Teleport API client for fetching/uploading sessions
//! - **[`environments`]** — Environment types and listing
//! - **[`environment_selection`]** — Environment selection and ranking logic
//! - **[`git_bundle`]** — Git bundle creation for session transfer
//!
//! # Example
//!
//! ```rust,ignore
//! use rc_teleport::{TeleportService, TeleportConfig};
//!
//! let config = TeleportConfig::default();
//! let service = TeleportService::new(config);
//! let environments = service.list_environments().await?;
//! ```

pub mod api;
pub mod environment_selection;
pub mod environments;
pub mod git_bundle;

pub use api::{TeleportConfig, TeleportResult, TeleportService, TeleportSession};
pub use environments::{Environment, EnvironmentStatus};
pub use git_bundle::{GitBundleConfig, GitBundleResult};

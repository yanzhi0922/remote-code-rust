//! # Roo Types
//!
//! Core type definitions for the Roo Code Rust project.
//! These types are derived directly from the TypeScript source at
//! `packages/types/src/` and represent the shared data model used
//! across all crates.

pub mod tool;
pub mod mode;
pub mod message;
pub mod events;
pub mod task;
pub mod mcp;
pub mod provider_settings;
pub mod global_settings;
pub mod model;
pub mod api;
pub mod terminal;
pub mod todo;
pub mod skills;
pub mod history;
pub mod git;
pub mod followup;
pub mod embedding;
pub mod codebase_index;
pub mod context_management;
pub mod cloud;
pub mod marketplace;
pub mod worktree;
pub mod cli;
pub mod experiment;
pub mod image_generation;
pub mod cookie_consent;
pub mod custom_tool;
pub mod ipc;
pub mod telemetry;
pub mod tool_params;
pub mod roomodes_schema;
pub mod vscode;
pub mod vscode_extension_host;
pub mod type_fu;

pub mod error;
pub mod utils_error;

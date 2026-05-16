#![allow(dead_code)]
#![allow(clippy::all)]
#![deny(clippy::dbg_macro, clippy::todo)]
//! # Roo Types
//!
//! Core type definitions for the Roo Code Rust project.
//! These types are derived directly from the TypeScript source at
//! `packages/types/src/` and represent the shared data model used
//! across all crates.

pub mod api;
pub mod cli;
pub mod cloud;
pub mod codebase_index;
pub mod context_management;
pub mod cookie_consent;
pub mod custom_tool;
pub mod embedding;
pub mod events;
pub mod experiment;
pub mod followup;
pub mod git;
pub mod global_settings;
pub mod history;
pub mod image_generation;
pub mod ipc;
pub mod marketplace;
pub mod mcp;
pub mod message;
pub mod mode;
pub mod model;
pub mod openai_codex_rate_limits;
pub mod profile_validator;
pub mod provider_settings;
pub mod roomodes_schema;
pub mod skills;
pub mod task;
pub mod telemetry;
pub mod terminal;
pub mod todo;
pub mod tool;
pub mod tool_params;
pub mod type_fu;
pub mod vscode;
pub mod vscode_extension_host;
pub mod worktree;

pub mod error;
pub mod utils_error;

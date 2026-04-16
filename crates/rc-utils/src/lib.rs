//! General-purpose utility modules for remote-code-rust.
//!
//! This crate provides reusable utility modules that are shared across
//! the workspace, including Git filesystem operations, memory types,
//! session teleport, secure storage, diff parsing, markdown rendering,
//! cron expressions, image processing, and code indexing detection.

pub mod code_indexing;
pub mod cron;
pub mod diff;
pub mod git_fs;
pub mod image_resizer;
pub mod markdown;
pub mod memory_types;
pub mod secure_storage;
pub mod teleport;

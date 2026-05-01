//! # roo-checkpoint
//!
//! Shadow git checkpoint service for Roo Code Rust.
//!
//! This crate provides checkpoint functionality using shadow git repositories
//! to track workspace file changes. Each checkpoint is a git commit that can
//! be saved, restored, and compared.
//!
//! ## Architecture
//!
//! - [`service::ShadowCheckpointService`] — Core service managing shadow git repos
//! - [`repo_per_task::RepoPerTaskCheckpointService`] — Per-task checkpoint service
//! - [`excludes`] — File exclusion patterns for the shadow repo
//! - [`types`] — Type definitions for checkpoint operations
//! - [`error`] — Error types for checkpoint operations

pub mod error;
pub mod excludes;
pub mod repo_per_task;
pub mod service;
pub mod types;

// Re-export primary types for convenience.
pub use error::CheckpointError;
pub use repo_per_task::RepoPerTaskCheckpointService;
pub use service::ShadowCheckpointService;
pub use types::{
    CheckpointDiff, CheckpointEvent, CheckpointResult, CheckpointServiceOptions, CommitSummary,
    ContentPair, GetDiffParams, PathPair, SaveCheckpointOptions,
};

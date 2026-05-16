#![allow(dead_code)]
#![allow(clippy::all)]
#![deny(clippy::dbg_macro, clippy::todo)]
//! # Roo Skills
//!
//! Skills management for Roo Code Rust.
//!
//! Provides skill discovery, loading, querying, and management.
//! Derived from `src/services/skills/SkillsManager.ts` and
//! `src/shared/skills.ts`.

pub mod error;
pub mod frontmatter;
pub mod manager;
pub mod types;
pub mod validate;

// Re-export key types and the manager
pub use error::SkillsError;
pub use frontmatter::{FrontMatter, generate_skill_md, parse_skill_md};
pub use manager::SkillsManager;
pub use types::{
    SkillContent, SkillMetadata, SkillNameValidationError, SkillNameValidationResult, SkillSource,
};
pub use validate::{get_skill_name_error_message, validate_skill_name};

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
pub use frontmatter::{parse_skill_md, generate_skill_md, FrontMatter};
pub use manager::SkillsManager;
pub use types::{
    SkillContent, SkillMetadata, SkillNameValidationError, SkillNameValidationResult, SkillSource,
};
pub use validate::{validate_skill_name, get_skill_name_error_message};

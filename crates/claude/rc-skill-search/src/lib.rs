//! `rc-skill-search` — Skill search system with BM25-like scoring, remote loading, and prefetching.
//!
//! This crate provides a full-text search engine for skill documents, supporting
//! local inverted-index search with TF-IDF scoring, remote skill loading with
//! caching, background prefetching, feature-flag gating, and search telemetry.
//!
//! # Overview
//!
//! - **[`local_search`]** — In-memory inverted index with BM25-like scoring
//! - **[`remote_loader`]** — Remote skill loading with TTL cache
//! - **[`prefetch`]** — Background prefetching of likely-relevant skills
//! - **[`feature_check`]** — Feature-flag gating for search capabilities
//! - **[`signals`]** — Search telemetry and analytics
//! - **[`index`]** — Re-exports of core search types
//!
//! # Example
//!
//! ```rust
//! use rc_skill_search::local_search::{SearchIndex, SkillDocument};
//!
//! let mut index = SearchIndex::new();
//! let skill = SkillDocument {
//!     slug: "azure-deploy".into(),
//!     name: "Azure Deploy".into(),
//!     description: "Deploy applications to Azure".into(),
//!     triggers: vec!["deploy".into(), "azure".into()],
//! };
//! index.index_skill(&skill);
//! let results = index.search("deploy azure", 5);
//! assert!(!results.is_empty());
//! ```

pub mod feature_check;
pub mod local_search;
pub mod prefetch;
pub mod remote_loader;
pub mod signals;

/// Convenience re-exports for the most commonly used types.
pub mod index {
    pub use crate::local_search::{SearchIndex, SearchResult, SkillDocument};
}

pub use index::{SearchIndex, SearchResult, SkillDocument};

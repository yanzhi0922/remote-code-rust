//! # roo-diff
//!
//! Diff strategies for Roo Code Rust.
//!
//! This crate implements multiple diff algorithms:
//!
//! - **MultiSearchReplace**: The primary strategy supporting multiple SEARCH/REPLACE blocks
//!   with exact and fuzzy matching, indentation preservation, and line number handling.
//!
//! - **UnifiedDiff**: Applies standard unified diff format (`@@ ... @@` hunks with `+`/`-`/` `
//!   line prefixes).
//!
//! - **SingleSearchReplace**: A simpler single-block `<<< SEARCH` / `>>> REPLACE` strategy.
//!
//! - **ApplyPatch**: Codex-format patch strategy using `*** Begin Patch` / `*** End Patch`
//!   markers with `*** Update File:` hunks.
//!
//! # Example
//!
//! ```rust
//! use roo_diff::MultiSearchReplaceDiffStrategy;
//!
//! let strategy = MultiSearchReplaceDiffStrategy::new(None, None);
//! let original = "function hello() {\n    console.log(\"hello\")\n}\n";
//! let diff = "\
//! <<<<<<< SEARCH
//! function hello() {
//!     console.log(\"hello\")
//! }
//! =======
//! function hello() {
//!     console.log(\"goodbye\")
//! }
//! >>>>>>> REPLACE";
//!
//! let result = strategy.apply_diff(original, diff);
//! assert!(result.success);
//! ```

mod apply_patch;
mod search_replace;
mod similarity;
mod strategy;
mod text_utils;
mod types;
mod unified_diff;
mod validate;

// Public exports — strategies
pub use strategy::MultiSearchReplaceDiffStrategy;
pub use unified_diff::UnifiedDiffStrategy;
pub use search_replace::SingleSearchReplaceDiffStrategy;
pub use apply_patch::ApplyPatchDiffStrategy;

// Public exports — types
pub use types::{DiffResult, ToolProgressStatus, ToolUse, ToolUseParams};

// Utility function exports
pub use text_utils::{
    add_line_numbers, every_line_has_line_numbers, normalize_string, strip_line_numbers,
};
pub use similarity::{get_similarity, fuzzy_search, FuzzySearchResult};
pub use validate::{validate_marker_sequencing, ValidationResult};

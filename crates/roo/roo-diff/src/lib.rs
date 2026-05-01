//! # roo-diff
//!
//! MultiSearchReplace diff strategy for Roo Code Rust.
//!
//! This crate implements the MultiSearchReplace diff algorithm, which supports:
//! - Exact and fuzzy matching using Levenshtein similarity
//! - Middle-out search for finding the best match location
//! - Indentation preservation across replacements
//! - Multiple diff blocks in a single request
//! - Line number handling (addition, stripping, detection)
//! - Marker validation with helpful error messages
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

mod similarity;
mod strategy;
mod text_utils;
mod types;
mod validate;

// Public exports
pub use strategy::MultiSearchReplaceDiffStrategy;
pub use types::{DiffResult, ToolProgressStatus, ToolUse, ToolUseParams};

// Utility function exports
pub use text_utils::{
    add_line_numbers, every_line_has_line_numbers, normalize_string, strip_line_numbers,
};
pub use similarity::{get_similarity, fuzzy_search, FuzzySearchResult};
pub use validate::{validate_marker_sequencing, ValidationResult};

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
pub use apply_patch::ApplyPatchDiffStrategy;
pub use search_replace::SingleSearchReplaceDiffStrategy;
pub use strategy::MultiSearchReplaceDiffStrategy;
pub use unified_diff::UnifiedDiffStrategy;

// Public exports — types
pub use types::{DiffResult, ToolProgressStatus, ToolUse, ToolUseParams};

// Utility function exports
pub use similarity::{FuzzySearchResult, fuzzy_search, get_similarity};
pub use text_utils::{
    add_line_numbers, every_line_has_line_numbers, normalize_string, strip_line_numbers,
};
pub use validate::{ValidationResult, validate_marker_sequencing};

// ---------------------------------------------------------------------------
// DiffStrategy trait — unified interface for all diff strategies
// ---------------------------------------------------------------------------

/// Trait shared by all diff strategy implementations.
///
/// Source: `src/shared/tools.ts` — `DiffStrategy` interface.
pub trait DiffStrategy {
    /// Returns the name of this strategy (for diagnostics / telemetry).
    fn name(&self) -> &str;

    /// Apply a diff to `original_content`, returning a [`DiffResult`].
    fn apply_diff(&self, original_content: &str, diff_content: &str) -> DiffResult;
}

impl DiffStrategy for MultiSearchReplaceDiffStrategy {
    fn name(&self) -> &str {
        MultiSearchReplaceDiffStrategy::name(self)
    }

    fn apply_diff(&self, original_content: &str, diff_content: &str) -> DiffResult {
        MultiSearchReplaceDiffStrategy::apply_diff(self, original_content, diff_content)
    }
}

impl DiffStrategy for SingleSearchReplaceDiffStrategy {
    fn name(&self) -> &str {
        SingleSearchReplaceDiffStrategy::name(self)
    }

    fn apply_diff(&self, original_content: &str, diff_content: &str) -> DiffResult {
        SingleSearchReplaceDiffStrategy::apply_diff(self, original_content, diff_content)
    }
}

impl DiffStrategy for UnifiedDiffStrategy {
    fn name(&self) -> &str {
        UnifiedDiffStrategy::name(self)
    }

    fn apply_diff(&self, original_content: &str, diff_content: &str) -> DiffResult {
        UnifiedDiffStrategy::apply_diff(self, original_content, diff_content)
    }
}

impl DiffStrategy for ApplyPatchDiffStrategy {
    fn name(&self) -> &str {
        ApplyPatchDiffStrategy::name(self)
    }

    fn apply_diff(&self, original_content: &str, diff_content: &str) -> DiffResult {
        ApplyPatchDiffStrategy::apply_diff(self, original_content, diff_content)
    }
}

// ---------------------------------------------------------------------------
// Strategy selection — mirrors TS getDiffStrategy(modelId)
// ---------------------------------------------------------------------------

/// Select the appropriate diff strategy based on the model ID.
///
/// Matching the TypeScript behaviour in `src/core/task/Task.ts`:
/// - The default (and most common) strategy is [`MultiSearchReplaceDiffStrategy`].
/// - For OpenAI models (gpt-4, gpt-3.5, o1, o3, etc.) the [`ApplyPatchDiffStrategy`]
///   is preferred because those models tend to produce Codex-style patches.
/// - For Claude models with a legacy format prefix, [`SingleSearchReplaceDiffStrategy`]
///   is returned.
pub fn get_diff_strategy(model_id: &str) -> Box<dyn DiffStrategy> {
    let lower = model_id.to_lowercase();

    // OpenAI models — use the Codex-format ApplyPatch strategy
    if lower.starts_with("gpt-")
        || lower.starts_with("o1")
        || lower.starts_with("o3")
        || lower.starts_with("o4")
        || lower.contains("chatgpt")
    {
        return Box::new(ApplyPatchDiffStrategy::new());
    }

    // Claude models with legacy single-block format preference
    // (older Claude models that used the simpler SEARCH/REPLACE markers)
    if (lower.starts_with("claude-2") || lower.starts_with("claude-instant"))
        && !lower.contains("legacy")
    {
        return Box::new(SingleSearchReplaceDiffStrategy::new(None, None));
    }

    // Default: MultiSearchReplace — the primary strategy used by most models
    Box::new(MultiSearchReplaceDiffStrategy::new(None, None))
}

//! Follow-up type definitions.
//!
//! Derived from `packages/types/src/followup.ts`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// SuggestionItem
// ---------------------------------------------------------------------------

/// A suggestion item with optional mode switching.
///
/// Source: `packages/types/src/followup.ts` — `SuggestionItem`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuggestionItem {
    pub answer: String,
    pub mode: Option<String>,
}

// ---------------------------------------------------------------------------
// FollowUpData
// ---------------------------------------------------------------------------

/// Follow-up question data structure.
///
/// Source: `packages/types/src/followup.ts` — `FollowUpData`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FollowUpData {
    pub question: Option<String>,
    pub suggest: Option<Vec<SuggestionItem>>,
}

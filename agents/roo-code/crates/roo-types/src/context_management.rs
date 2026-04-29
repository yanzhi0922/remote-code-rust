//! Context management type definitions.
//!
//! Derived from `packages/types/src/context-management.ts`.

// ---------------------------------------------------------------------------
// ContextManagementEvent
// ---------------------------------------------------------------------------

/// All context management event types.
///
/// Source: `packages/types/src/context-management.ts` — `CONTEXT_MANAGEMENT_EVENTS`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContextManagementEvent {
    CondenseContext,
    CondenseContextError,
    SlidingWindowTruncation,
}

impl ContextManagementEvent {
    /// All context management event types.
    pub const ALL: [ContextManagementEvent; 3] = [
        ContextManagementEvent::CondenseContext,
        ContextManagementEvent::CondenseContextError,
        ContextManagementEvent::SlidingWindowTruncation,
    ];

    /// Returns the string representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CondenseContext => "condense_context",
            Self::CondenseContextError => "condense_context_error",
            Self::SlidingWindowTruncation => "sliding_window_truncation",
        }
    }
}

/// Checks if a string is a valid context management event.
pub fn is_context_management_event(value: &str) -> bool {
    ContextManagementEvent::ALL
        .iter()
        .any(|e| e.as_str() == value)
}

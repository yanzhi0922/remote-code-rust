//! Feedback and request routing utilities for the Codex adapter.

use std::sync::OnceLock;

use tracing_subscriber::prelude::*;

/// Create or retrieve the shared feedback instance.
///
/// If `capture_enabled` is false, returns a no-op feedback instance.
/// Otherwise, lazily initializes the global feedback layer with optional
/// log database layer.
pub(crate) fn shared_feedback(
    capture_enabled: bool,
    log_db: Option<codex_state::log_db::LogDbLayer>,
) -> codex_feedback::CodexFeedback {
    if !capture_enabled {
        return codex_feedback::CodexFeedback::new();
    }

    static FEEDBACK: OnceLock<codex_feedback::CodexFeedback> = OnceLock::new();
    FEEDBACK
        .get_or_init(|| {
            let feedback = codex_feedback::CodexFeedback::new();
            let log_db_layer = log_db.clone().map(|layer| {
                layer.with_filter(
                    tracing_subscriber::filter::Targets::new().with_default(tracing::Level::TRACE),
                )
            });
            let _ = tracing_subscriber::registry()
                .with(feedback.logger_layer())
                .with(feedback.metadata_layer())
                .with(log_db_layer)
                .try_init();
            feedback
        })
        .clone()
}
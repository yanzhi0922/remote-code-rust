//! Application state tracking.
//!
//! Source: `src/core/ClineProvider.ts` — ClineProvider state management

use std::sync::Arc;
use tokio::sync::RwLock;

/// The current state of the application.
///
/// Source: `src/core/ClineProvider.ts` — state tracking fields
#[derive(Debug, Clone, Default)]
pub struct AppState {
    /// Whether the app has been initialized.
    pub initialized: bool,

    /// The current mode slug.
    pub current_mode: String,

    /// Number of active tasks.
    pub active_task_count: usize,

    /// Whether a task is currently running.
    pub task_running: bool,

    /// Whether the app has been disposed.
    pub disposed: bool,

    /// Custom instructions for the current session.
    pub custom_instructions: Option<String>,
}

impl AppState {
    /// Create a new default state.
    pub fn new() -> Self {
        Self {
            initialized: false,
            current_mode: "code".to_string(),
            active_task_count: 0,
            task_running: false,
            disposed: false,
            custom_instructions: None,
        }
    }
}

/// Thread-safe shared state wrapper.
pub type SharedState = Arc<RwLock<AppState>>;

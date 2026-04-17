//! UI components for the ratatui-based TUI.
//!
//! Each module provides a `render` function that draws into a [`ratatui::Frame`]
//! region, reading state from [`App`](crate::app::App).

pub mod agent_panel;
pub mod chat;
pub mod compact_summary;
pub mod completion;
pub mod context_viz;
pub mod diff_viewer;
pub mod effort_indicator;
pub mod help;
pub mod input;
pub mod markdown;
pub mod message_types;
pub mod model_picker;
pub mod permission;
pub mod progress;
pub mod provider_picker;
pub mod sidebar;
pub mod status_bar;
pub mod token_indicator;
pub mod tool_output;

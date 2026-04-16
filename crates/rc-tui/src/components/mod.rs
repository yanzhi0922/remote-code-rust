//! UI components for the ratatui-based TUI.
//!
//! Each module provides a `render` function that draws into a [`ratatui::Frame`]
//! region, reading state from [`App`](crate::app::App).

pub mod chat;
pub mod completion;
pub mod diff_viewer;
pub mod help;
pub mod input;
pub mod markdown;
pub mod permission;
pub mod progress;
pub mod sidebar;
pub mod status_bar;
pub mod tool_output;

//! # Roo Environment
//!
//! Pure-function library for generating Roo Code's `<environment_details>`
//! XML string. All data is passed in through parameters — no VS Code
//! dependency, no global state.

pub mod types;
pub mod reminder;
pub mod terminal;
pub mod time;
pub mod details;

pub use types::*;
pub use reminder::format_reminder_section;
pub use terminal::{format_active_terminals, format_inactive_terminals};
pub use time::{format_current_time, format_current_time_with_tz};
pub use details::build_environment_details;

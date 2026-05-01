//! Roo-ignore: File ignore controller for Roo Code.
//!
//! Provides .rooignore support with standard .gitignore syntax,
//! plus built-in directory ignore patterns for common large directories.

pub mod constants;
pub mod controller;
pub mod patterns;

pub use controller::{RooIgnoreController, RooIgnoreError, LOCK_TEXT_SYMBOL};
pub use patterns::is_path_in_ignored_directory;

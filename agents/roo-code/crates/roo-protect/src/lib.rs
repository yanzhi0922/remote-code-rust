//! `roo-protect` — file write-protection for Roo configuration files.
//!
//! Controls write access to Roo configuration files by enforcing protection
//! patterns. Prevents auto-approved modifications to sensitive Roo
//! configuration files.

pub mod controller;
pub mod patterns;

pub use controller::{PathAnnotation, RooProtectedController};
pub use patterns::{get_protection_description, is_protected_path, PROTECTED_PATTERNS, SHIELD_SYMBOL};

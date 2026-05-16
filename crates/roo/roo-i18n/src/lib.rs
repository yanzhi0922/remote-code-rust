#![allow(dead_code)]
#![allow(clippy::all)]
#![deny(clippy::dbg_macro, clippy::todo)]
//! `roo-i18n` — internationalization support for Roo Code.
//!
//! Provides locale management and string translation with `{{param}}`-style
//! interpolation. Mirrors the `i18next` API used in the TypeScript source.

pub mod loader;
pub mod translations;
pub mod types;

pub use loader::I18n;
pub use types::{Locale, LocaleParseError};

//! # roo-tree-sitter
//!
//! Tree-sitter integration for Roo Code Rust.
//!
//! This crate provides code parsing capabilities using tree-sitter grammars
//! for extracting source code definitions (functions, classes, structs, etc.)
//! from various programming languages.
//!
//! ## Architecture
//!
//! - [`language_parser`] — Manages loading and caching of tree-sitter parsers
//! - [`markdown_parser`] — Special-case Markdown parser (no tree-sitter needed)
//! - [`queries`] — Language-specific tree-sitter query strings
//!
//! ## Usage
//!
//! ```rust,ignore
//! use roo_tree_sitter::markdown_parser::parse_source_code_definitions;
//! use std::path::Path;
//!
//! let definitions = parse_source_code_definitions(
//!     Path::new("src/main.rs"),
//!     &source_code,
//! );
//! ```

pub mod language_parser;
pub mod markdown_parser;
pub mod queries;

pub use language_parser::{
    load_required_language_parsers, parse_file, process_captures,
    LanguageParserError, LanguageParsers, LoadedParser,
};
pub use markdown_parser::{
    format_markdown_captures, is_supported_extension, parse_markdown,
    parse_source_code_definitions, MarkdownCapture, SUPPORTED_EXTENSIONS,
};

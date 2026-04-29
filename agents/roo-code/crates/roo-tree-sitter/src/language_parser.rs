//! Language parser management for tree-sitter.
//!
//! Corresponds to `languageParser.ts` in the TypeScript source.
//!
//! Manages loading and caching of tree-sitter parsers for different languages.
//! Uses native tree-sitter grammars compiled into the binary via feature flags.

use std::collections::HashMap;
use std::path::Path;
use streaming_iterator::StreamingIterator;
use tree_sitter::{Language, Parser, Query, QueryCursor, Node};

use crate::queries;

/// Error type for language parser operations.
#[derive(Debug, thiserror::Error)]
pub enum LanguageParserError {
    #[error("unsupported language: {0}")]
    UnsupportedLanguage(String),

    #[error("failed to create query: {0}")]
    QueryError(String),

    #[error("failed to set language: {0}")]
    LanguageError(String),

    #[error("parser not found for extension: {0}")]
    ParserNotFound(String),
}

/// A loaded language parser with its associated query.
pub struct LoadedParser {
    pub parser: Parser,
    pub query: Query,
}

/// Map of file extension (without dot) to loaded parser.
pub type LanguageParsers = HashMap<String, LoadedParser>;

/// Returns the tree-sitter [`Language`] for a given file extension.
///
/// This function is gated by feature flags. If the corresponding language
/// feature is not enabled, returns `None`.
pub fn language_for_extension(ext: &str) -> Option<Language> {
    match ext {
        #[cfg(feature = "lang_rust")]
        "rs" => Some(tree_sitter_rust::LANGUAGE.into()),
        #[cfg(feature = "lang_javascript")]
        "js" | "jsx" | "json" => Some(tree_sitter_javascript::LANGUAGE.into()),
        #[cfg(feature = "lang_typescript")]
        "ts" => Some(tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into()),
        #[cfg(feature = "lang_typescript")]
        "tsx" => Some(tree_sitter_typescript::LANGUAGE_TSX.into()),
        #[cfg(feature = "lang_python")]
        "py" => Some(tree_sitter_python::LANGUAGE.into()),
        #[cfg(feature = "lang_go")]
        "go" => Some(tree_sitter_go::LANGUAGE.into()),
        #[cfg(feature = "lang_c")]
        "c" | "h" => Some(tree_sitter_c::LANGUAGE.into()),
        #[cfg(feature = "lang_cpp")]
        "cpp" | "hpp" => Some(tree_sitter_cpp::LANGUAGE.into()),
        #[cfg(feature = "lang_java")]
        "java" => Some(tree_sitter_java::LANGUAGE.into()),
        #[cfg(feature = "lang_ruby")]
        "rb" => Some(tree_sitter_ruby::LANGUAGE.into()),
        #[cfg(feature = "lang_csharp")]
        "cs" => Some(tree_sitter_c_sharp::LANGUAGE.into()),
        // Additional languages - enable when grammar crates are available
        // #[cfg(feature = "lang_php")]
        // "php" => Some(tree_sitter_php::LANGUAGE.into()),
        // #[cfg(feature = "lang_swift")]
        // "swift" => Some(tree_sitter_swift::LANGUAGE.into()),
        // #[cfg(feature = "lang_kotlin")]
        // "kt" | "kts" => Some(tree_sitter_kotlin::LANGUAGE.into()),
        // #[cfg(feature = "lang_css")]
        // "css" => Some(tree_sitter_css::LANGUAGE.into()),
        // #[cfg(feature = "lang_html")]
        // "html" | "htm" => Some(tree_sitter_html::LANGUAGE.into()),
        // #[cfg(feature = "lang_ocaml")]
        // "ml" | "mli" => Some(tree_sitter_ocaml::LANGUAGE_OCAML.into()),
        // #[cfg(feature = "lang_lua")]
        // "lua" => Some(tree_sitter_lua::LANGUAGE.into()),
        // #[cfg(feature = "lang_toml")]
        // "toml" => Some(tree_sitter_toml::LANGUAGE.into()),
        // #[cfg(feature = "lang_elixir")]
        // "ex" | "exs" => Some(tree_sitter_elixir::LANGUAGE.into()),
        _ => None,
    }
}

/// Loads required language parsers for the given file paths.
///
/// Extracts unique file extensions, loads the corresponding tree-sitter
/// grammars, and creates parsers with their associated queries.
///
/// Corresponds to `loadRequiredLanguageParsers` in `languageParser.ts`.
pub fn load_required_language_parsers(
    files_to_parse: &[&Path],
) -> Result<LanguageParsers, LanguageParserError> {
    let mut parsers = LanguageParsers::new();
    let mut extensions_loaded: HashMap<String, String> = HashMap::new();

    for file_path in files_to_parse {
        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase())
            .unwrap_or_default();

        if ext.is_empty() || extensions_loaded.contains_key(&ext) {
            continue;
        }

        // Determine the parser key (handles extension aliasing)
        let parser_key = match ext.as_str() {
            "ejs" | "erb" => "embedded_template".to_string(),
            other => other.to_string(),
        };

        // Check if we already loaded a parser for this key
        if extensions_loaded.values().any(|k| k == &parser_key) {
            extensions_loaded.insert(ext.clone(), parser_key);
            continue;
        }

        // Get the language for this extension
        let language = language_for_extension(&ext).ok_or_else(|| {
            LanguageParserError::UnsupportedLanguage(format!(
                "No tree-sitter grammar available for .{}",
                ext
            ))
        })?;

        // Get the query string for this extension
        let query_str = queries::query_for_extension(&ext).ok_or_else(|| {
            LanguageParserError::QueryError(format!("No query defined for .{}", ext))
        })?;

        // Create the query
        let query = Query::new(&language, query_str).map_err(|e| {
            LanguageParserError::QueryError(format!("Failed to create query for .{}: {}", ext, e))
        })?;

        // Create and configure the parser
        let mut parser = Parser::new();
        parser.set_language(&language).map_err(|e| {
            LanguageParserError::LanguageError(format!(
                "Failed to set language for .{}: {}",
                ext, e
            ))
        })?;

        parsers.insert(
            parser_key.clone(),
            LoadedParser { parser, query },
        );
        extensions_loaded.insert(ext.clone(), parser_key);
    }

    Ok(parsers)
}

/// A processed capture with resolved name.
pub struct ProcessedCapture<'a> {
    node: Node<'a>,
    name: String,
}

/// Parse a file and extract code definitions using tree-sitter.
///
/// Corresponds to `parseFile` in `index.ts`.
///
/// Returns a formatted string with code definitions, or `None` if no
/// definitions found.
pub fn parse_file(
    file_content: &str,
    ext: &str,
    language_parsers: &mut LanguageParsers,
) -> Result<Option<String>, LanguageParserError> {
    // Determine the parser key
    let parser_key = match ext {
        "ejs" | "erb" => "embedded_template",
        other => other,
    };

    let loaded = language_parsers.get_mut(parser_key).ok_or_else(|| {
        LanguageParserError::ParserNotFound(format!(
            "No parser loaded for extension '{}'",
            ext
        ))
    })?;

    // Parse the file content into an AST
    let tree = loaded.parser.parse(file_content, None);

    let Some(tree) = tree else {
        return Ok(None);
    };

    // Apply the query to the AST using QueryCursor
    let capture_names = loaded.query.capture_names();
    let mut cursor = QueryCursor::new();
    let mut captures: Vec<ProcessedCapture<'_>> = Vec::new();

    let mut matches_iter = cursor.matches(
        &loaded.query,
        tree.root_node(),
        file_content.as_bytes(),
    );

    while let Some(match_item) = matches_iter.next() {
        for capture in match_item.captures {
            let name_idx = capture.index as usize;
            let name = capture_names
                .get(name_idx)
                .map(|s: &&str| s.to_string())
                .unwrap_or_default();
            captures.push(ProcessedCapture {
                node: capture.node,
                name,
            });
        }
    }

    if captures.is_empty() {
        return Ok(None);
    }

    // Split the file content into individual lines
    let lines: Vec<&str> = file_content.lines().collect();

    // Process the captures
    let result = process_captures(&captures, &lines, ext);

    Ok(result)
}

/// Minimum number of lines for a component to be included in output.
const DEFAULT_MIN_COMPONENT_LINES: usize = 4;

/// Process captures from tree-sitter query results.
///
/// Corresponds to `processCaptures` in `index.ts`.
pub fn process_captures(
    captures: &[ProcessedCapture<'_>],
    lines: &[&str],
    language: &str,
) -> Option<String> {
    if captures.is_empty() {
        return None;
    }

    // Determine if HTML filtering is needed for this language
    let needs_html_filtering = matches!(language, "jsx" | "tsx");

    // Filter function to exclude HTML elements if needed
    let is_not_html_element = |line: &str| -> bool {
        if !needs_html_filtering {
            return true;
        }
        let html_elements_re = regex::Regex::new(
            r"^[^A-Z]*<\/?(?:div|span|button|input|h[1-6]|p|a|img|ul|li|form)\b",
        )
        .unwrap();
        let trimmed = line.trim();
        !html_elements_re.is_match(trimmed)
    };

    let mut formatted_output = String::new();
    let mut processed_lines: std::collections::HashSet<String> = std::collections::HashSet::new();

    for capture in captures {
        let name = &capture.name;
        let node = capture.node;

        // Skip captures that don't represent definitions
        if !name.contains("definition") && !name.contains("name") {
            continue;
        }

        // Get the parent node that contains the full definition
        let definition_node = if name.contains("name") {
            node.parent()
        } else {
            Some(node)
        };

        let Some(def_node) = definition_node else {
            continue;
        };

        // Get the start and end lines of the full definition
        let start_line = def_node.start_position().row;
        let end_line = def_node.end_position().row;
        let line_count = end_line - start_line + 1;

        // Skip components that don't span enough lines
        if line_count < DEFAULT_MIN_COMPONENT_LINES {
            continue;
        }

        // Create unique key for this definition based on line range
        let line_key = format!("{}-{}", start_line, end_line);

        // Skip already processed lines
        if processed_lines.contains(&line_key) {
            continue;
        }

        // Check if this is a valid component definition (not an HTML element)
        let start_line_content = lines.get(start_line).unwrap_or(&"");

        // Special handling for component name definitions
        if name.contains("name.definition") {
            let content_bytes = file_content_bytes(lines);
            let component_name = node.utf8_text(&content_bytes).unwrap_or("");

            if !component_name.is_empty() {
                let line_text = lines.get(start_line).unwrap_or(&"");
                formatted_output.push_str(&format!(
                    "{}--{} | {}\n",
                    start_line + 1,
                    end_line + 1,
                    line_text
                ));
                processed_lines.insert(line_key);
            }
        } else if is_not_html_element(start_line_content) {
            let line_text = lines.get(start_line).unwrap_or(&"");
            formatted_output.push_str(&format!(
                "{}--{} | {}\n",
                start_line + 1,
                end_line + 1,
                line_text
            ));
            processed_lines.insert(line_key);

            // If this is part of a larger definition, include its non-HTML context
            if let Some(parent) = node.parent() {
                let child_count = parent.child_count();
                if child_count > 0 {
                    if let Some(last_child) = parent.child(child_count - 1) {
                        let context_end = last_child.end_position().row;
                        let context_span = context_end - parent.start_position().row + 1;

                        if context_span >= DEFAULT_MIN_COMPONENT_LINES {
                            let range_key = format!(
                                "{}-{}",
                                parent.start_position().row,
                                context_end
                            );
                            if !processed_lines.contains(&range_key) {
                                let parent_line = lines.get(parent.start_position().row).unwrap_or(&"");
                                formatted_output.push_str(&format!(
                                    "{}--{} | {}\n",
                                    parent.start_position().row + 1,
                                    context_end + 1,
                                    parent_line
                                ));
                                processed_lines.insert(range_key);
                            }
                        }
                    }
                }
            }
        }
    }

    if formatted_output.is_empty() {
        None
    } else {
        Some(formatted_output)
    }
}

/// Helper to create a joined byte vector from lines for utf8_text.
fn file_content_bytes(lines: &[&str]) -> Vec<u8> {
    lines.join("\n").into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_for_extension_rust() {
        #[cfg(feature = "lang_rust")]
        {
            let lang = language_for_extension("rs");
            assert!(lang.is_some());
        }
    }

    #[test]
    fn test_language_for_extension_unknown() {
        assert!(language_for_extension("xyz").is_none());
    }

    #[test]
    fn test_language_for_extension_javascript_aliases() {
        #[cfg(feature = "lang_javascript")]
        {
            assert!(language_for_extension("js").is_some());
            assert!(language_for_extension("jsx").is_some());
            assert!(language_for_extension("json").is_some());
        }
    }

    #[test]
    fn test_load_required_language_parsers_empty() {
        let result = load_required_language_parsers(&[]);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_load_required_language_parsers_unsupported() {
        let path = Path::new("test.xyz");
        let result = load_required_language_parsers(&[path]);
        assert!(result.is_err());
    }

    #[test]
    fn test_process_captures_empty() {
        let captures = vec![];
        let lines = vec!["fn main() {}"];
        let result = process_captures(&captures, &lines, "rust");
        assert!(result.is_none());
    }

    #[test]
    fn test_load_required_language_parsers_rust() {
        #[cfg(feature = "lang_rust")]
        {
            let path = Path::new("test.rs");
            let result = load_required_language_parsers(&[path]);
            assert!(result.is_ok());
            let parsers = result.unwrap();
            assert!(parsers.contains_key("rs"));
        }
    }

    #[test]
    fn test_parse_file_rust() {
        #[cfg(feature = "lang_rust")]
        {
            let path = Path::new("test.rs");
            let mut parsers = load_required_language_parsers(&[path]).unwrap();
            let content = r#"fn main() {
    println!("hello");
    println!("world");
    println!("test");
}

struct MyStruct {
    field1: String,
    field2: i32,
    field3: bool,
}
"#;
            let result = parse_file(content, "rs", &mut parsers).unwrap();
            assert!(result.is_some());
            let output = result.unwrap();
            assert!(output.contains("fn main"));
        }
    }
}

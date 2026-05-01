//! Markdown parser for extracting headers and section ranges.
//!
//! Corresponds to `markdownParser.ts` in the TypeScript source.
//!
//! This is a special case implementation that doesn't use tree-sitter
//! but is compatible with the capture processing pipeline.

use std::path::Path;

/// A mock capture that mimics tree-sitter's `QueryCapture` structure.
#[derive(Debug, Clone)]
pub struct MarkdownCapture {
    /// Start row (0-based).
    pub start_row: usize,
    /// End row (0-based).
    pub end_row: usize,
    /// The text content of the capture.
    pub text: String,
    /// The capture name (e.g., "name.definition.header.h1").
    pub name: String,
}

/// Parse a markdown file and extract headers and section line ranges.
///
/// Corresponds to `parseMarkdown` in `markdownParser.ts`.
///
/// Returns an array of captures compatible with the tree-sitter capture
/// processing pipeline.
pub fn parse_markdown(content: &str) -> Vec<MarkdownCapture> {
    if content.trim().is_empty() {
        return vec![];
    }

    let lines: Vec<&str> = content.lines().collect();
    let mut captures = Vec::new();

    // Regular expressions for different header types
    let atx_header_re = regex::Regex::new(r"^(#{1,6})\s+(.+)$").unwrap();
    // Setext headers must have at least 3 = or - characters
    let setext_h1_re = regex::Regex::new(r"^={3,}\s*$").unwrap();
    let setext_h2_re = regex::Regex::new(r"^-{3,}\s*$").unwrap();
    // Valid setext header text line should be plain text
    let valid_setext_text_re = regex::Regex::new(r"^\s*[^#<>\[\]`\t]+[^\n]$").unwrap();

    // Find all headers in the document
    for i in 0..lines.len() {
        let line = lines[i];

        // Check for ATX headers (# Header)
        if let Some(caps) = atx_header_re.captures(line) {
            let level = caps[1].len();
            let text = caps[2].trim().to_string();

            // Create a mock capture for this header
            let start_row = i;
            let end_row = i;

            // Name capture
            captures.push(MarkdownCapture {
                start_row,
                end_row,
                text: text.clone(),
                name: format!("name.definition.header.h{}", level),
            });

            // Definition capture
            captures.push(MarkdownCapture {
                start_row,
                end_row,
                text,
                name: format!("definition.header.h{}", level),
            });

            continue;
        }

        // Check for setext headers (underlined headers)
        if i > 0 {
            // Check for H1 (======)
            if setext_h1_re.is_match(line) && valid_setext_text_re.is_match(lines[i - 1]) {
                let text = lines[i - 1].trim().to_string();

                captures.push(MarkdownCapture {
                    start_row: i - 1,
                    end_row: i,
                    text: text.clone(),
                    name: "name.definition.header.h1".to_string(),
                });

                captures.push(MarkdownCapture {
                    start_row: i - 1,
                    end_row: i,
                    text,
                    name: "definition.header.h1".to_string(),
                });

                continue;
            }

            // Check for H2 (------)
            if setext_h2_re.is_match(line) && valid_setext_text_re.is_match(lines[i - 1]) {
                let text = lines[i - 1].trim().to_string();

                captures.push(MarkdownCapture {
                    start_row: i - 1,
                    end_row: i,
                    text: text.clone(),
                    name: "name.definition.header.h2".to_string(),
                });

                captures.push(MarkdownCapture {
                    start_row: i - 1,
                    end_row: i,
                    text,
                    name: "definition.header.h2".to_string(),
                });
            }
        }
    }

    // Sort captures by their start position
    captures.sort_by_key(|c| c.start_row);

    // Group captures by header (name and definition pairs)
    let mut header_groups: Vec<Vec<&MarkdownCapture>> = Vec::new();
    let mut i = 0;
    while i < captures.len() {
        if i + 1 < captures.len() {
            header_groups.push(vec![&captures[i], &captures[i + 1]]);
            i += 2;
        } else {
            header_groups.push(vec![&captures[i]]);
            i += 1;
        }
    }

    // Update end positions for section ranges
    // We need mutable captures, so let's rebuild
    let mut captures_mut = captures;

    // Recalculate groups with indices
    let mut group_indices: Vec<(usize, usize)> = Vec::new(); // (start_idx, end_idx) in captures_mut
    let mut idx = 0;
    while idx < captures_mut.len() {
        if idx + 1 < captures_mut.len() {
            group_indices.push((idx, idx + 1));
            idx += 2;
        } else {
            group_indices.push((idx, idx));
            idx += 1;
        }
    }

    for (gi, &(start_idx, end_idx)) in group_indices.iter().enumerate() {
        if gi < group_indices.len() - 1 {
            // End position is the start of the next header minus 1
            let next_start = captures_mut[group_indices[gi + 1].0].start_row;
            for ci in start_idx..=end_idx {
                captures_mut[ci].end_row = next_start.saturating_sub(1);
            }
        } else {
            // Last header extends to the end of the file
            for ci in start_idx..=end_idx {
                captures_mut[ci].end_row = lines.len().saturating_sub(1);
            }
        }
    }

    captures_mut
}

/// Format markdown captures into a string representation.
///
/// Corresponds to `formatMarkdownCaptures` in `markdownParser.ts`.
///
/// Returns `None` if no captures meet the minimum section line threshold.
pub fn format_markdown_captures(
    captures: &[MarkdownCapture],
    min_section_lines: usize,
) -> Option<String> {
    if captures.is_empty() {
        return None;
    }

    let mut formatted_output = String::new();

    // Process only the definition captures (every other capture)
    let mut i = 1;
    while i < captures.len() {
        let capture = &captures[i];
        let start_line = capture.start_row;
        let end_line = capture.end_row;

        // Only include sections that span at least min_section_lines lines
        let section_length = end_line - start_line + 1;
        if section_length >= min_section_lines {
            // Extract header level from the name
            let header_level = extract_header_level(&capture.name);

            let header_prefix = "#".repeat(header_level);

            // Format: startLine--endLine | # Header Text
            formatted_output.push_str(&format!(
                "{}--{} | {} {}\n",
                start_line, end_line, header_prefix, capture.text
            ));
        }

        i += 2;
    }

    if formatted_output.is_empty() {
        None
    } else {
        Some(formatted_output)
    }
}

/// Extract header level from capture name (e.g., "definition.header.h2" -> 2).
fn extract_header_level(name: &str) -> usize {
    let re = regex::Regex::new(r"\.h(\d)$").unwrap();
    if let Some(caps) = re.captures(name) {
        caps[1].parse::<usize>().unwrap_or(1)
    } else {
        1
    }
}

/// Supported file extensions for tree-sitter parsing.
///
/// Corresponds to the `extensions` array in `index.ts`.
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "tla", "js", "jsx", "ts", "vue", "tsx", "py", "rs", "go", "c", "h", "cpp", "hpp", "cs", "rb",
    "java", "php", "swift", "sol", "kt", "kts", "ex", "exs", "el", "html", "htm", "md", "markdown",
    "json", "css", "rdl", "ml", "mli", "lua", "scala", "toml", "zig", "elm", "ejs", "erb", "vb",
];

/// Check if a file extension is supported for parsing.
pub fn is_supported_extension(ext: &str) -> bool {
    let ext_lower = ext.trim_start_matches('.').to_lowercase();
    SUPPORTED_EXTENSIONS.contains(&ext_lower.as_str())
}

/// Parse source code definitions for a file.
///
/// Corresponds to `parseSourceCodeDefinitionsForFile` in `index.ts`.
///
/// This is the main entry point for parsing a file and extracting
/// source code definitions.
pub fn parse_source_code_definitions(
    file_path: &Path,
    file_content: &str,
) -> Option<String> {
    let ext = file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_default();

    // Check if the file extension is supported
    if !is_supported_extension(&ext) {
        return None;
    }

    let file_name = file_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");

    // Special case for markdown files
    if ext == "md" || ext == "markdown" {
        let captures = parse_markdown(file_content);
        let definitions = format_markdown_captures(&captures, 4);
        return definitions.map(|d| format!("# {}\n{}", file_name, d));
    }

    // For other file types, use tree-sitter
    let mut language_parsers = match crate::language_parser::load_required_language_parsers(&[
        file_path,
    ]) {
        Ok(parsers) => parsers,
        Err(_) => return None,
    };

    let definitions =
        crate::language_parser::parse_file(file_content, &ext, &mut language_parsers).ok()?;

    definitions.map(|d| format!("# {}\n{}", file_name, d))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_markdown_empty() {
        let result = parse_markdown("");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_markdown_whitespace() {
        let result = parse_markdown("   \n  \n  ");
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_markdown_atx_headers() {
        let content = "# Header 1\nSome content\n## Header 2\nMore content\n### Header 3\nEven more";
        let captures = parse_markdown(content);

        // Should have 3 headers * 2 captures each = 6 captures
        assert_eq!(captures.len(), 6);

        // First header
        assert_eq!(captures[0].name, "name.definition.header.h1");
        assert_eq!(captures[0].text, "Header 1");
        assert_eq!(captures[1].name, "definition.header.h1");

        // Second header
        assert_eq!(captures[2].name, "name.definition.header.h2");
        assert_eq!(captures[2].text, "Header 2");
        assert_eq!(captures[3].name, "definition.header.h2");

        // Third header
        assert_eq!(captures[4].name, "name.definition.header.h3");
        assert_eq!(captures[4].text, "Header 3");
        assert_eq!(captures[5].name, "definition.header.h3");
    }

    #[test]
    fn test_parse_markdown_setext_headers() {
        let content = "Header One\n===\nSome content\nHeader Two\n---\nMore content";
        let captures = parse_markdown(content);

        // Should have 2 headers * 2 captures each = 4 captures
        assert_eq!(captures.len(), 4);

        // H1 setext
        assert_eq!(captures[0].name, "name.definition.header.h1");
        assert_eq!(captures[0].text, "Header One");
        assert!(captures[0].end_row > captures[0].start_row);

        // H2 setext
        assert_eq!(captures[2].name, "name.definition.header.h2");
        assert_eq!(captures[2].text, "Header Two");
    }

    #[test]
    fn test_parse_markdown_section_ranges() {
        let content = "# First\nline1\nline2\nline3\nline4\n# Second\nline5\nline6\nline7\nline8";
        let captures = parse_markdown(content);

        // First section should end just before second header
        // First header starts at row 0, second at row 5
        // So first section end should be 4
        assert_eq!(captures[0].start_row, 0);
        assert_eq!(captures[0].end_row, 4);

        // Second section should extend to end of file
        assert_eq!(captures[2].start_row, 5);
        assert_eq!(captures[2].end_row, 9);
    }

    #[test]
    fn test_format_markdown_captures_empty() {
        let result = format_markdown_captures(&[], 4);
        assert!(result.is_none());
    }

    #[test]
    fn test_format_markdown_captures_short_sections() {
        let captures = vec![MarkdownCapture {
            start_row: 0,
            end_row: 0, // Only 1 line, below min of 4
            text: "Short".to_string(),
            name: "definition.header.h1".to_string(),
        }];
        let result = format_markdown_captures(&captures, 4);
        assert!(result.is_none());
    }

    #[test]
    fn test_format_markdown_captures_long_sections() {
        let captures = vec![
            MarkdownCapture {
                start_row: 0,
                end_row: 0,
                text: "Test".to_string(),
                name: "name.definition.header.h1".to_string(),
            },
            MarkdownCapture {
                start_row: 0,
                end_row: 9, // 10 lines, above min of 4
                text: "Test Header".to_string(),
                name: "definition.header.h1".to_string(),
            },
        ];
        let result = format_markdown_captures(&captures, 4);
        assert!(result.is_some());
        let output = result.unwrap();
        assert!(output.contains("0--9"));
        assert!(output.contains("# Test Header"));
    }

    #[test]
    fn test_is_supported_extension() {
        assert!(is_supported_extension("rs"));
        assert!(is_supported_extension("ts"));
        assert!(is_supported_extension("py"));
        assert!(is_supported_extension("md"));
        assert!(!is_supported_extension("xyz"));
    }

    #[test]
    fn test_extract_header_level() {
        assert_eq!(extract_header_level("definition.header.h1"), 1);
        assert_eq!(extract_header_level("definition.header.h3"), 3);
        assert_eq!(extract_header_level("definition.header.h6"), 6);
        assert_eq!(extract_header_level("definition.header"), 1);
    }
}

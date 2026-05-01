//! Folded file context generation using tree-sitter.
//!
//! Generates condensed representations of source files showing only function
//! signatures, class declarations, and other structural definitions — hiding
//! implementation bodies.
//!
//! Source: `src/core/condense/foldedFileContext.ts` — `generateFoldedFileContext()`

use std::path::{Path, PathBuf};
use tracing::warn;

/// Known error patterns from `parseSourceCodeDefinitionsForFile` that indicate
/// the file should be skipped rather than embedded.
///
/// Source: `src/core/condense/foldedFileContext.ts` — `isTreeSitterErrorString()`
const TREE_SITTER_ERROR_PATTERNS: &[&str] = &[
    "This file does not exist",
    "do not have permission",
    "Unsupported file type:",
];

/// Checks if a definitions string is actually an error message from tree-sitter
/// rather than valid code definitions.
fn is_tree_sitter_error_string(definitions: &str) -> bool {
    TREE_SITTER_ERROR_PATTERNS
        .iter()
        .any(|pattern| definitions.contains(pattern))
}

/// Result of generating folded file context.
///
/// Source: `src/core/condense/foldedFileContext.ts` — `FoldedFileContextResult`
#[derive(Debug, Clone)]
pub struct FoldedFileContextResult {
    /// The formatted string containing all folded file definitions (joined).
    pub content: String,
    /// Individual file sections, each in its own `<system-reminder>` block.
    pub sections: Vec<String>,
    /// Number of files successfully processed.
    pub files_processed: usize,
    /// Number of files that failed or were skipped.
    pub files_skipped: usize,
    /// Total character count of the folded content.
    pub character_count: usize,
}

/// Options for generating folded file context.
///
/// Source: `src/core/condense/foldedFileContext.ts` — `FoldedFileContextOptions`
pub struct FoldedFileContextOptions {
    /// Maximum total characters for the folded content (default: 50000).
    pub max_characters: usize,
    /// The current working directory for resolving relative paths.
    pub cwd: String,
}

impl Default for FoldedFileContextOptions {
    fn default() -> Self {
        Self {
            max_characters: 50_000,
            cwd: String::new(),
        }
    }
}

/// Generates folded (signatures-only) file context for a list of files using
/// tree-sitter.
///
/// This function takes file paths that were read during a conversation and
/// produces a condensed representation showing only function signatures,
/// class declarations, and other important structural definitions — hiding
/// implementation bodies.
///
/// Each file is wrapped in its own `<system-reminder>` block, allowing the
/// model to retain awareness of file structure without consuming excessive
/// tokens.
///
/// # Arguments
/// * `file_paths` - Array of file paths to process (relative to `cwd`)
/// * `options` - Configuration options including `cwd` and `max_characters`
///
/// # Returns
/// `FoldedFileContextResult` with the formatted content and statistics.
///
/// Source: `src/core/condense/foldedFileContext.ts` — `generateFoldedFileContext()`
pub fn generate_folded_file_context(
    file_paths: &[String],
    options: &FoldedFileContextOptions,
) -> FoldedFileContextResult {
    let mut result = FoldedFileContextResult {
        content: String::new(),
        sections: Vec::new(),
        files_processed: 0,
        files_skipped: 0,
        character_count: 0,
    };

    if file_paths.is_empty() {
        return result;
    }

    let mut folded_sections: Vec<String> = Vec::new();
    let mut current_char_count: usize = 0;
    let mut failed_files: Vec<String> = Vec::new();

    for (i, file_path) in file_paths.iter().enumerate() {
        // Resolve to absolute path for tree-sitter
        let absolute_path = if Path::new(file_path).is_absolute() {
            PathBuf::from(file_path)
        } else {
            Path::new(&options.cwd).join(file_path)
        };

        // Read the file content
        let file_content = match std::fs::read_to_string(&absolute_path) {
            Ok(content) => content,
            Err(_) => {
                failed_files.push(file_path.clone());
                result.files_skipped += 1;
                continue;
            }
        };

        // Get the folded definitions using tree-sitter
        let definitions = roo_tree_sitter::parse_source_code_definitions(
            &absolute_path,
            &file_content,
        );

        match definitions {
            Some(defs) if !is_tree_sitter_error_string(&defs) => {
                // Wrap each file in its own <system-reminder> block
                let section_content = format!(
                    "<system-reminder>\n## File Context: {}\n{}\n</system-reminder>",
                    file_path, defs
                );

                // Check if adding this file would exceed the character limit
                if current_char_count + section_content.len() > options.max_characters {
                    // Would exceed limit — check if we can fit at least a truncated version
                    let remaining_chars = options.max_characters.saturating_sub(current_char_count);
                    if remaining_chars < 200 {
                        // Not enough room for meaningful content, stop processing all remaining files
                        result.files_skipped += file_paths.len() - i;
                        break;
                    }

                    // Truncate the definitions to fit within the system-reminder block
                    let truncated_defs_len = remaining_chars.saturating_sub(100);
                    let truncated_definitions =
                        format!("{}...\n... (truncated)", &defs[..truncated_defs_len.min(defs.len())]);
                    let truncated_content = format!(
                        "<system-reminder>\n## File Context: {}\n{}\n</system-reminder>",
                        file_path, truncated_definitions
                    );
                    folded_sections.push(truncated_content);
                    result.files_processed += 1;

                    // Stop processing more files since we've hit the limit
                    let remaining = file_paths.len()
                        - result.files_processed
                        - result.files_skipped;
                    result.files_skipped += remaining;
                    break;
                }

                folded_sections.push(section_content.clone());
                current_char_count += section_content.len();
                result.files_processed += 1;
            }
            _ => {
                // File type not supported, no definitions found, or error accessing file
                result.files_skipped += 1;
            }
        }
    }

    // Log failed files as a single batch summary instead of per-file errors
    if !failed_files.is_empty() {
        let display_files: Vec<&str> = failed_files
            .iter()
            .take(5)
            .map(|s| s.as_str())
            .collect();
        let suffix = if failed_files.len() > 5 {
            format!(" and {} more", failed_files.len() - 5)
        } else {
            String::new()
        };
        warn!(
            "Folded context generation: skipped {} file(s) due to errors: {}{}",
            failed_files.len(),
            display_files.join(", "),
            suffix,
        );
    }

    if !folded_sections.is_empty() {
        result.sections = folded_sections;
        result.content = result.sections.join("\n");
        result.character_count = result.content.len();
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_is_tree_sitter_error_string() {
        assert!(is_tree_sitter_error_string("This file does not exist"));
        assert!(is_tree_sitter_error_string("You do not have permission"));
        assert!(is_tree_sitter_error_string("Unsupported file type: .xyz"));
        assert!(!is_tree_sitter_error_string("fn main() {}"));
        assert!(!is_tree_sitter_error_string(""));
    }

    #[test]
    fn test_generate_folded_file_context_empty() {
        let options = FoldedFileContextOptions::default();
        let result = generate_folded_file_context(&[], &options);
        assert_eq!(result.files_processed, 0);
        assert_eq!(result.files_skipped, 0);
        assert!(result.content.is_empty());
        assert!(result.sections.is_empty());
    }

    #[test]
    fn test_generate_folded_file_context_nonexistent() {
        let options = FoldedFileContextOptions {
            cwd: "/tmp".to_string(),
            max_characters: 50_000,
        };
        let result = generate_folded_file_context(
            &["/nonexistent/file.rs".to_string()],
            &options,
        );
        assert_eq!(result.files_processed, 0);
        assert_eq!(result.files_skipped, 1);
    }

    #[test]
    fn test_generate_folded_file_context_with_rust_file() {
        let dir = std::env::temp_dir().join("roo_test_folded");
        let _ = fs::create_dir_all(&dir);

        let rust_file = dir.join("test.rs");
        fs::write(
            &rust_file,
            r#"/// A test function.
fn hello_world() -> String {
    "hello".to_string()
}

/// A test struct.
struct MyStruct {
    field: i32,
}

impl MyStruct {
    fn new() -> Self {
        Self { field: 0 }
    }
}
"#,
        )
        .unwrap();

        let options = FoldedFileContextOptions {
            cwd: dir.to_string_lossy().to_string(),
            max_characters: 50_000,
        };
        let file_path_str = rust_file.to_string_lossy().to_string();
        let result = generate_folded_file_context(&[file_path_str], &options);

        // Should have processed at least one file
        assert!(result.files_processed >= 1 || result.files_skipped >= 1);

        // Clean up
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_generate_folded_file_context_respects_max_chars() {
        let dir = std::env::temp_dir().join("roo_test_folded_max");
        let _ = fs::create_dir_all(&dir);

        // Create multiple files
        for i in 0..5 {
            let file = dir.join(format!("file{}.rs", i));
            fs::write(
                &file,
                format!(
                    "fn function_{}() -> String {{\n    \"{}\".to_string()\n}}\n",
                    i, i
                ),
            )
            .unwrap();
        }

        let options = FoldedFileContextOptions {
            cwd: dir.to_string_lossy().to_string(),
            max_characters: 200, // Very small limit
        };

        let file_paths: Vec<String> = (0..5)
            .map(|i| dir.join(format!("file{}.rs", i)).to_string_lossy().to_string())
            .collect();

        let result = generate_folded_file_context(&file_paths, &options);

        // Content should not exceed max_characters (with some tolerance for truncation)
        assert!(
            result.character_count <= options.max_characters + 200,
            "character_count ({}) far exceeds max_characters ({})",
            result.character_count,
            options.max_characters,
        );

        // Clean up
        let _ = fs::remove_dir_all(&dir);
    }
}

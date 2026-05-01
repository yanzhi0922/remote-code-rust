//! File processor for code indexing.
//!
//! Corresponds to the `processors/` directory in the TypeScript source.
//!
//! Handles parsing files into code blocks, computing hashes, and preparing
//! data for embedding and indexing.

use crate::cache_manager::CacheManager;
use crate::types::IndexError;

/// A parsed code block ready for indexing.
#[derive(Clone, Debug)]
pub struct CodeBlock {
    /// The file path this block came from.
    pub file_path: String,
    /// The content of the code block.
    pub content: String,
    /// 1-based start line.
    pub start_line: usize,
    /// 1-based end line.
    pub end_line: usize,
    /// The language of the file.
    pub language: String,
}

/// Result of processing a file.
#[derive(Clone, Debug)]
pub struct FileProcessingResult {
    /// The file path.
    pub file_path: String,
    /// The code blocks extracted from the file.
    pub blocks: Vec<CodeBlock>,
    /// The content hash of the file.
    pub content_hash: String,
}

/// Summary of a batch processing operation.
#[derive(Clone, Debug, Default)]
pub struct BatchProcessingSummary {
    /// Files processed successfully.
    pub processed_files: Vec<FileProcessingResult>,
    /// Any errors encountered.
    pub errors: Vec<String>,
}

/// Trait for code parsers that extract blocks from source files.
pub trait CodeParser: Send + Sync {
    /// Parse a file and extract code blocks.
    fn parse_file(&self, file_path: &str, content: &str) -> Result<Vec<CodeBlock>, IndexError>;
}

/// A simple code parser that splits files into fixed-size chunks.
pub struct SimpleCodeParser {
    /// Maximum number of lines per block.
    pub max_lines_per_block: usize,
    /// Minimum number of lines for a block to be included.
    pub min_lines_per_block: usize,
}

impl Default for SimpleCodeParser {
    fn default() -> Self {
        Self {
            max_lines_per_block: 100,
            min_lines_per_block: 3,
        }
    }
}

impl SimpleCodeParser {
    pub fn new(max_lines_per_block: usize, min_lines_per_block: usize) -> Self {
        Self {
            max_lines_per_block,
            min_lines_per_block,
        }
    }

    /// Determines the language from a file extension.
    pub fn language_from_extension(ext: &str) -> String {
        match ext {
            "rs" => "rust".to_string(),
            "ts" => "typescript".to_string(),
            "tsx" => "tsx".to_string(),
            "js" | "jsx" => "javascript".to_string(),
            "py" => "python".to_string(),
            "go" => "go".to_string(),
            "java" => "java".to_string(),
            "c" | "h" => "c".to_string(),
            "cpp" | "hpp" => "cpp".to_string(),
            "cs" => "csharp".to_string(),
            "rb" => "ruby".to_string(),
            "php" => "php".to_string(),
            "swift" => "swift".to_string(),
            "kt" | "kts" => "kotlin".to_string(),
            "css" => "css".to_string(),
            "html" | "htm" => "html".to_string(),
            "md" | "markdown" => "markdown".to_string(),
            other => other.to_string(),
        }
    }
}

impl CodeParser for SimpleCodeParser {
    fn parse_file(&self, file_path: &str, content: &str) -> Result<Vec<CodeBlock>, IndexError> {
        let ext = std::path::Path::new(file_path)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let language = Self::language_from_extension(ext);
        let lines: Vec<&str> = content.lines().collect();

        if lines.len() < self.min_lines_per_block {
            return Ok(vec![]);
        }

        let mut blocks = Vec::new();
        let mut start = 0;

        while start < lines.len() {
            let end = std::cmp::min(start + self.max_lines_per_block, lines.len());
            let block_lines = end - start;

            if block_lines >= self.min_lines_per_block {
                let block_content: String = lines[start..end].join("\n");
                blocks.push(CodeBlock {
                    file_path: file_path.to_string(),
                    content: block_content,
                    start_line: start + 1, // 1-based
                    end_line: end,         // 1-based inclusive
                    language: language.clone(),
                });
            }

            start = end;
        }

        Ok(blocks)
    }
}

/// Processes files for indexing: parses, computes hashes, and creates embeddings.
pub struct FileProcessor {
    parser: Box<dyn CodeParser>,
}

impl FileProcessor {
    /// Creates a new file processor with the given parser.
    pub fn new(parser: Box<dyn CodeParser>) -> Self {
        Self { parser }
    }

    /// Creates a new file processor with the default simple parser.
    pub fn with_default_parser() -> Self {
        Self {
            parser: Box::new(SimpleCodeParser::default()),
        }
    }

    /// Processes a single file.
    pub fn process_file(
        &self,
        file_path: &str,
        content: &str,
    ) -> Result<FileProcessingResult, IndexError> {
        let content_hash = CacheManager::compute_hash(content.as_bytes());
        let blocks = self.parser.parse_file(file_path, content)?;

        Ok(FileProcessingResult {
            file_path: file_path.to_string(),
            blocks,
            content_hash,
        })
    }

    /// Processes multiple files in batch.
    pub fn process_batch(
        &self,
        files: &[(&str, &str)],
    ) -> BatchProcessingSummary {
        let mut summary = BatchProcessingSummary::default();

        for (path, content) in files {
            match self.process_file(path, content) {
                Ok(result) => summary.processed_files.push(result),
                Err(e) => summary.errors.push(format!("{}: {}", path, e)),
            }
        }

        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_code_parser_rust() {
        let parser = SimpleCodeParser::default();
        let content = "fn main() {\n    println!(\"hello\");\n}\n\nfn foo() {\n    bar();\n}\n";
        let blocks = parser.parse_file("test.rs", content).unwrap();

        assert!(!blocks.is_empty());
        assert_eq!(blocks[0].language, "rust");
        assert_eq!(blocks[0].start_line, 1);
    }

    #[test]
    fn test_simple_code_parser_too_short() {
        let parser = SimpleCodeParser::default();
        let content = "fn main() {}";
        let blocks = parser.parse_file("test.rs", content).unwrap();
        assert!(blocks.is_empty()); // Only 1 line, below min of 3
    }

    #[test]
    fn test_simple_code_parser_chunking() {
        let parser = SimpleCodeParser::new(5, 2);
        let content = (0..12).map(|i| format!("line {}", i)).collect::<Vec<_>>().join("\n");
        let blocks = parser.parse_file("test.rs", &content).unwrap();

        // 12 lines with max 5 per block = 3 blocks (5+5+2)
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].start_line, 1);
        assert_eq!(blocks[0].end_line, 5);
        assert_eq!(blocks[2].start_line, 11);
        assert_eq!(blocks[2].end_line, 12);
    }

    #[test]
    fn test_language_from_extension() {
        assert_eq!(SimpleCodeParser::language_from_extension("rs"), "rust");
        assert_eq!(SimpleCodeParser::language_from_extension("ts"), "typescript");
        assert_eq!(SimpleCodeParser::language_from_extension("py"), "python");
        assert_eq!(SimpleCodeParser::language_from_extension("xyz"), "xyz");
    }

    #[test]
    fn test_file_processor_process_file() {
        let processor = FileProcessor::with_default_parser();
        let content = "fn main() {\n    println!(\"hello\");\n    println!(\"world\");\n}\n";

        let result = processor.process_file("test.rs", content).unwrap();
        assert_eq!(result.file_path, "test.rs");
        assert!(!result.content_hash.is_empty());
        assert!(!result.blocks.is_empty());
    }

    #[test]
    fn test_file_processor_batch() {
        let processor = FileProcessor::with_default_parser();
        let files = vec![
            ("a.rs", "fn a() {\n    a1();\n    a2();\n}"),
            ("b.rs", "fn b() {\n    b1();\n    b2();\n}"),
        ];

        let summary = processor.process_batch(&files);
        assert_eq!(summary.processed_files.len(), 2);
        assert!(summary.errors.is_empty());
    }

    #[test]
    fn test_code_block_fields() {
        let parser = SimpleCodeParser::new(100, 3);
        let content = "line1\nline2\nline3\nline4\nline5";
        let blocks = parser.parse_file("test.py", content).unwrap();

        assert_eq!(blocks[0].file_path, "test.py");
        assert_eq!(blocks[0].language, "python");
        assert_eq!(blocks[0].start_line, 1);
        assert_eq!(blocks[0].end_line, 5);
        assert!(blocks[0].content.contains("line1"));
    }
}

/// Line counting utilities for files.
/// Mirrors src/integrations/misc/line-counter.ts

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Count the number of lines in a file.
pub fn count_file_lines(file_path: &Path) -> std::io::Result<usize> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);
    Ok(reader.lines().count())
}

/// Result of counting lines and estimating tokens.
#[derive(Clone, Debug)]
pub struct LineAndTokenCountResult {
    /// Total number of lines counted.
    pub line_count: usize,
    /// Estimated token count (rough: ~4 chars per token).
    pub token_estimate: usize,
    /// Whether the full file was scanned (false if early exit occurred).
    pub complete: bool,
}

/// Options for counting lines and tokens.
#[derive(Clone, Debug)]
pub struct LineAndTokenCountOptions {
    /// Maximum tokens allowed before early exit. If None, scans entire file.
    pub budget_tokens: Option<usize>,
    /// Number of lines to buffer before running token estimation (default: 256).
    pub chunk_lines: usize,
}

impl Default for LineAndTokenCountOptions {
    fn default() -> Self {
        Self {
            budget_tokens: None,
            chunk_lines: 256,
        }
    }
}

/// Count lines and estimate tokens in a file.
/// Processes file in chunks to avoid memory issues and can early-exit when budget is exceeded.
pub fn count_file_lines_and_tokens(
    file_path: &Path,
    options: LineAndTokenCountOptions,
) -> std::io::Result<LineAndTokenCountResult> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);

    let mut line_count = 0usize;
    let mut char_count = 0usize;
    let mut complete = true;

    for line_result in reader.lines() {
        let line = line_result?;
        let line_len = line.len();
        char_count += line_len + 1; // +1 for newline
        line_count += 1;

        // Check budget every chunk_lines
        if line_count % options.chunk_lines == 0 {
            if let Some(budget) = options.budget_tokens {
                let token_estimate = char_count / 4; // rough: ~4 chars per token
                if token_estimate > budget {
                    complete = false;
                    break;
                }
            }
        }
    }

    let token_estimate = char_count / 4;

    Ok(LineAndTokenCountResult {
        line_count,
        token_estimate,
        complete,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_file(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", content).unwrap();
        file
    }

    #[test]
    fn test_count_file_lines() {
        let file = create_test_file("line1\nline2\nline3\n");
        let count = count_file_lines(file.path()).unwrap();
        assert_eq!(3, count);
    }

    #[test]
    fn test_count_file_lines_empty() {
        let file = create_test_file("");
        let count = count_file_lines(file.path()).unwrap();
        assert_eq!(0, count);
    }

    #[test]
    fn test_count_file_lines_single() {
        let file = create_test_file("single line");
        let count = count_file_lines(file.path()).unwrap();
        assert_eq!(1, count);
    }

    #[test]
    fn test_count_lines_and_tokens() {
        let file = create_test_file("hello world\nthis is a test\n");
        let result = count_file_lines_and_tokens(file.path(), LineAndTokenCountOptions::default()).unwrap();
        assert_eq!(2, result.line_count);
        assert!(result.complete);
    }

    #[test]
    fn test_count_lines_and_tokens_with_budget() {
        let content = "a".repeat(1000) + "\n";
        let repeated = content.repeat(100);
        let file = create_test_file(&repeated);

        let options = LineAndTokenCountOptions {
            budget_tokens: Some(10), // Very small budget
            chunk_lines: 10,
        };

        let result = count_file_lines_and_tokens(file.path(), options).unwrap();
        assert!(!result.complete);
    }

    #[test]
    fn test_count_file_lines_nonexistent() {
        let result = count_file_lines(Path::new("/nonexistent/file.txt"));
        assert!(result.is_err());
    }
}

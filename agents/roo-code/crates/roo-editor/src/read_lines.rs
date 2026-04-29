/// Read specific lines from a file.
/// Mirrors src/integrations/misc/read-lines.ts

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Read a range of lines from a file (1-indexed, inclusive).
/// Returns the lines as a single String with newlines.
pub fn read_lines(file_path: &Path, start_line: usize, end_line: usize) -> std::io::Result<String> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);

    let mut result = Vec::new();
    for (idx, line_result) in reader.lines().enumerate() {
        let line_num = idx + 1; // 1-indexed
        if line_num > end_line {
            break;
        }
        if line_num >= start_line {
            let line = line_result?;
            result.push(line);
        }
    }

    Ok(result.join("\n"))
}

/// Read the entire content of a file as lines.
pub fn read_all_lines(file_path: &Path) -> std::io::Result<Vec<String>> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);
    reader.lines().collect()
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
    fn test_read_lines_range() {
        let file = create_test_file("line1\nline2\nline3\nline4\nline5\n");
        let result = read_lines(file.path(), 2, 4).unwrap();
        assert_eq!("line2\nline3\nline4", result);
    }

    #[test]
    fn test_read_lines_single() {
        let file = create_test_file("line1\nline2\nline3\n");
        let result = read_lines(file.path(), 2, 2).unwrap();
        assert_eq!("line2", result);
    }

    #[test]
    fn test_read_lines_all() {
        let file = create_test_file("line1\nline2\nline3\n");
        let result = read_lines(file.path(), 1, 3).unwrap();
        assert_eq!("line1\nline2\nline3", result);
    }

    #[test]
    fn test_read_lines_beyond_end() {
        let file = create_test_file("line1\nline2\n");
        let result = read_lines(file.path(), 1, 100).unwrap();
        assert_eq!("line1\nline2", result);
    }

    #[test]
    fn test_read_all_lines() {
        let file = create_test_file("a\nb\nc\n");
        let lines = read_all_lines(file.path()).unwrap();
        assert_eq!(vec!["a", "b", "c"], lines);
    }
}

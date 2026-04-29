/// Adds line numbers to content, starting from `start_line`.
///
/// Port of `addLineNumbers` from `extract-text.ts`.
pub fn add_line_numbers(content: &str, start_line: usize) -> String {
    // If content is empty, return empty string - empty files should not have line numbers
    // If content is empty but startLine > 1, return "startLine | " because we know the file is not empty
    // but the content is empty at that line offset
    if content.is_empty() {
        return if start_line == 1 {
            String::new()
        } else {
            format!("{} | \n", start_line)
        };
    }

    // Split into lines and handle trailing line feeds (\n)
    let mut lines: Vec<&str> = content.split('\n').collect();
    let last_line_empty = lines.last() == Some(&"");
    if last_line_empty {
        lines.pop();
    }

    let max_line_number_width = format!("{}", start_line + lines.len() - 1).len();
    let numbered_content: Vec<String> = lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let line_number = format!("{}", start_line + index);
            let padded = format!("{:>width$}", line_number, width = max_line_number_width);
            format!("{} | {}", padded, line)
        })
        .collect();

    format!("{}\n", numbered_content.join("\n"))
}

/// Checks if every line in the content has line numbers prefixed (e.g., "1 | content" or "123 | content").
/// Line numbers must be followed by a single pipe character (not double pipes).
///
/// Port of `everyLineHasLineNumbers` from `extract-text.ts`.
pub fn every_line_has_line_numbers(content: &str) -> bool {
    let lines: Vec<&str> = content.split("\r\n").flat_map(|s| s.split('\n')).collect();
    !lines.is_empty()
        && lines.iter().all(|line| {
            // Match: optional whitespace, digits, whitespace, single pipe (not double)
            let bytes = line.as_bytes();
            let mut i = 0;
            // Skip whitespace
            while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
                i += 1;
            }
            // Must have at least one digit
            if i >= bytes.len() || !bytes[i].is_ascii_digit() {
                return false;
            }
            while i < bytes.len() && bytes[i].is_ascii_digit() {
                i += 1;
            }
            // Skip whitespace
            while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
                i += 1;
            }
            // Must have exactly one pipe (not double)
            if i >= bytes.len() || bytes[i] != b'|' {
                return false;
            }
            // Must not be followed by another pipe
            i + 1 >= bytes.len() || bytes[i + 1] != b'|'
        })
}

/// Strips line numbers from content while preserving the actual content.
///
/// When `aggressive` is false (default): Only strips lines with clear number patterns like "123 | content"
/// When `aggressive` is true: Uses a more lenient pattern that also matches lines with just a pipe character.
///
/// Port of `stripLineNumbers` from `extract-text.ts`.
pub fn strip_line_numbers(content: &str, aggressive: bool) -> String {
    // Split into lines to handle each line individually
    let lines: Vec<&str> = content.split("\r\n").flat_map(|s| s.split('\n')).collect();

    // Process each line
    let processed_lines: Vec<String> = lines
        .iter()
        .map(|line| {
            if aggressive {
                // Aggressive pattern: optional digits, optional space, pipe, space, then content
                strip_line_aggressive(line)
            } else {
                // Standard pattern: digits, whitespace, single pipe (not double), optional space, then content
                strip_line_standard(line)
            }
        })
        .collect();

    // Join back with original line endings
    let line_ending = if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut result = processed_lines.join(line_ending);

    // Preserve trailing newline if present in original content
    if content.ends_with(line_ending) && !result.ends_with(line_ending) {
        result.push_str(line_ending);
    }

    result
}

fn strip_line_standard(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut i = 0;

    // Skip leading whitespace
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }

    // Must have digits
    let digits_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == digits_start {
        return line.to_string();
    }

    // Skip whitespace
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }

    // Must have pipe (not double pipe)
    if i >= bytes.len() || bytes[i] != b'|' {
        return line.to_string();
    }
    if i + 1 < bytes.len() && bytes[i + 1] == b'|' {
        return line.to_string();
    }
    i += 1; // skip pipe

    // Skip optional single space
    if i < bytes.len() && bytes[i] == b' ' {
        i += 1;
    }

    line[i..].to_string()
}

fn strip_line_aggressive(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut i = 0;

    // Skip leading whitespace
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }

    // Optional digits
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }

    // Optional whitespace
    while i < bytes.len() && (bytes[i] == b' ' || bytes[i] == b'\t') {
        i += 1;
    }

    // Must have pipe
    if i >= bytes.len() || bytes[i] != b'|' {
        return line.to_string();
    }
    i += 1; // skip pipe

    // Must have space
    if i >= bytes.len() || bytes[i] != b' ' {
        return line.to_string();
    }
    i += 1; // skip space

    line[i..].to_string()
}

/// Normalizes a string by replacing smart quotes, typographic characters,
/// collapsing whitespace, and trimming.
///
/// Port of `normalizeString` from `text-normalization.ts`.
pub fn normalize_string(s: &str) -> String {
    let mut normalized = s.to_string();

    // Replace smart quotes
    normalized = normalized.replace('\u{201C}', "\""); // Left double quote
    normalized = normalized.replace('\u{201D}', "\""); // Right double quote
    normalized = normalized.replace('\u{2018}', "'"); // Left single quote
    normalized = normalized.replace('\u{2019}', "'"); // Right single quote

    // Replace typographic characters
    normalized = normalized.replace('\u{2026}', "..."); // Ellipsis
    normalized = normalized.replace('\u{2014}', "-"); // Em dash
    normalized = normalized.replace('\u{2013}', "-"); // En dash
    normalized = normalized.replace('\u{00A0}', " "); // Non-breaking space

    // Normalize whitespace - collapse multiple whitespace to single space
    let mut result = String::with_capacity(normalized.len());
    let mut last_was_space = false;
    for ch in normalized.chars() {
        if ch.is_whitespace() {
            if !last_was_space {
                result.push(' ');
                last_was_space = true;
            }
        } else {
            result.push(ch);
            last_was_space = false;
        }
    }

    // Trim
    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_line_numbers_basic() {
        let content = "line 1\nline 2\nline 3";
        let result = add_line_numbers(content, 1);
        assert_eq!(result, "1 | line 1\n2 | line 2\n3 | line 3\n");
    }

    #[test]
    fn test_add_line_numbers_with_start() {
        let content = "line 1\nline 2";
        let result = add_line_numbers(content, 5);
        assert_eq!(result, "5 | line 1\n6 | line 2\n");
    }

    #[test]
    fn test_add_line_numbers_empty() {
        let result = add_line_numbers("", 1);
        assert_eq!(result, "");
    }

    #[test]
    fn test_add_line_numbers_empty_with_start() {
        let result = add_line_numbers("", 5);
        assert_eq!(result, "5 | \n");
    }

    #[test]
    fn test_every_line_has_line_numbers_true() {
        let content = "1 | hello\n2 | world";
        assert!(every_line_has_line_numbers(content));
    }

    #[test]
    fn test_every_line_has_line_numbers_false() {
        let content = "hello\nworld";
        assert!(!every_line_has_line_numbers(content));
    }

    #[test]
    fn test_every_line_has_line_numbers_double_pipe() {
        let content = "1 || hello";
        assert!(!every_line_has_line_numbers(content));
    }

    #[test]
    fn test_strip_line_numbers_basic() {
        let content = "1 | hello\n2 | world";
        assert_eq!(strip_line_numbers(content, false), "hello\nworld");
    }

    #[test]
    fn test_strip_line_numbers_preserves_content() {
        let content = "hello\nworld";
        assert_eq!(strip_line_numbers(content, false), "hello\nworld");
    }

    #[test]
    fn test_strip_line_numbers_aggressive() {
        let content = "| hello\n| world";
        assert_eq!(strip_line_numbers(content, true), "hello\nworld");
    }

    #[test]
    fn test_normalize_string_smart_quotes() {
        let s = "\u{201C}hello\u{201D} \u{2018}world\u{2019}";
        assert_eq!(normalize_string(s), "\"hello\" 'world'");
    }

    #[test]
    fn test_normalize_string_whitespace() {
        let s = "  hello   world  ";
        assert_eq!(normalize_string(s), "hello world");
    }

    #[test]
    fn test_normalize_string_typographic() {
        let s = "hello\u{2026}world\u{2014}test\u{2013}end";
        assert_eq!(normalize_string(s), "hello...world-test-end");
    }
}

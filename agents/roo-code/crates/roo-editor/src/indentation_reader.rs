/// Indentation reader for extracting code blocks based on indentation.
/// Mirrors src/integrations/misc/indentation-reader.ts

/// Represents a code block extracted by indentation level.
#[derive(Clone, Debug)]
pub struct IndentationBlock {
    pub start_line: usize,
    pub end_line: usize,
    pub content: String,
    pub indent_level: usize,
}

/// Extract indentation-based blocks from source code.
/// Returns blocks where the indentation level changes.
pub fn extract_blocks(content: &str) -> Vec<IndentationBlock> {
    let lines: Vec<&str> = content.lines().collect();
    let mut blocks = Vec::new();
    let mut current_block_start: Option<usize> = None;
    let mut block_indent: usize = 0;
    let mut seen_deeper: bool = false;
    let mut current_content = String::new();

    for (idx, line) in lines.iter().enumerate() {
        let line_num = idx + 1;
        let indent = count_leading_whitespace(line);

        // Skip empty lines for block detection
        if line.trim().is_empty() {
            if !current_content.is_empty() {
                current_content.push('\n');
            }
            continue;
        }

        if current_block_start.is_none() {
            current_block_start = Some(line_num);
            block_indent = indent;
            seen_deeper = false;
            current_content = line.to_string();
        } else if indent > block_indent {
            // Deeper indentation — continue current block
            seen_deeper = true;
            if !current_content.is_empty() {
                current_content.push('\n');
            }
            current_content.push_str(line);
        } else if indent < block_indent {
            // Dedented below block start — end current block, start new
            blocks.push(IndentationBlock {
                start_line: current_block_start.unwrap(),
                end_line: line_num - 1,
                content: current_content.clone(),
                indent_level: block_indent,
            });
            current_block_start = Some(line_num);
            block_indent = indent;
            seen_deeper = false;
            current_content = line.to_string();
        } else {
            // indent == block_indent
            if seen_deeper {
                // Returned to block indent after deeper content — new sibling block
                blocks.push(IndentationBlock {
                    start_line: current_block_start.unwrap(),
                    end_line: line_num - 1,
                    content: current_content.clone(),
                    indent_level: block_indent,
                });
                current_block_start = Some(line_num);
                seen_deeper = false;
                current_content = line.to_string();
            } else {
                // Same indent, no deeper seen yet — continue current block
                if !current_content.is_empty() {
                    current_content.push('\n');
                }
                current_content.push_str(line);
            }
        }
    }

    // Don't forget the last block
    if !current_content.is_empty() {
        blocks.push(IndentationBlock {
            start_line: current_block_start.unwrap_or(1),
            end_line: lines.len(),
            content: current_content,
            indent_level: block_indent,
        });
    }

    blocks
}

/// Count the number of leading whitespace characters (spaces or tabs).
/// Tabs are counted as the equivalent of `tab_size` spaces.
pub fn count_leading_whitespace(line: &str) -> usize {
    let mut count = 0;
    for ch in line.chars() {
        match ch {
            ' ' => count += 1,
            '\t' => count += 4, // Default tab size
            _ => break,
        }
    }
    count
}

/// Detect the indentation style used in the content.
/// Returns (indent_char, indent_size) where indent_char is ' ' or '\t'.
pub fn detect_indent_style(content: &str) -> (char, usize) {
    let mut spaces = 0usize;
    let mut tabs = 0usize;
    let mut space_indents = Vec::new();

    for line in content.lines() {
        if line.is_empty() {
            continue;
        }
        let leading = count_leading_whitespace(line);
        if leading == 0 {
            continue;
        }

        if line.starts_with('\t') {
            tabs += 1;
        } else if line.starts_with(' ') {
            spaces += 1;
            // Track common space counts
            if !space_indents.contains(&leading) {
                space_indents.push(leading);
            }
        }
    }

    if tabs > spaces {
        ('\t', 1)
    } else {
        let size = find_gcd(&space_indents).max(2);
        (' ', size)
    }
}

fn find_gcd(numbers: &[usize]) -> usize {
    if numbers.is_empty() {
        return 4;
    }
    let mut result = numbers[0];
    for &num in numbers.iter().skip(1) {
        result = gcd(result, num);
    }
    result
}

fn gcd(a: usize, b: usize) -> usize {
    if b == 0 { a } else { gcd(b, a % b) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_leading_whitespace_spaces() {
        assert_eq!(0, count_leading_whitespace("hello"));
        assert_eq!(2, count_leading_whitespace("  hello"));
        assert_eq!(4, count_leading_whitespace("    hello"));
    }

    #[test]
    fn test_count_leading_whitespace_tabs() {
        assert_eq!(4, count_leading_whitespace("\thello"));
        assert_eq!(8, count_leading_whitespace("\t\thello"));
    }

    #[test]
    fn test_count_leading_whitespace_mixed() {
        assert_eq!(5, count_leading_whitespace(" \thello"));
    }

    #[test]
    fn test_extract_blocks_simple() {
        let content = "fn main() {\n    println!(\"hello\");\n}\n";
        let blocks = extract_blocks(content);
        assert!(!blocks.is_empty());
    }

    #[test]
    fn test_extract_blocks_nested() {
        // Two top-level blocks at indent 0, each with deeper children
        let content = "class A:\n    pass\nclass B:\n    pass\n";
        let blocks = extract_blocks(content);
        assert!(blocks.len() >= 2);
    }

    #[test]
    fn test_detect_indent_style_spaces() {
        let content = "if true:\n    do_something()\n    more_stuff()\n";
        let (ch, size) = detect_indent_style(content);
        assert_eq!(' ', ch);
        assert_eq!(4, size);
    }

    #[test]
    fn test_detect_indent_style_tabs() {
        let content = "if true:\n\tdo_something()\n\tmore_stuff()\n";
        let (ch, _size) = detect_indent_style(content);
        assert_eq!('\t', ch);
    }
}

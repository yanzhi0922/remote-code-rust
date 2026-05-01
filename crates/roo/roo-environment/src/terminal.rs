//! Terminal details formatting.
//!
//! Ported from `src/core/environment/getEnvironmentDetails.ts` (terminal sections).

use crate::types::{InactiveTerminalInfo, TerminalInfo};

/// Format the "Actively Running Terminals" section.
///
/// Produces output like:
/// ```text
///
/// # Actively Running Terminals
/// ## Terminal {id} (Active)
/// ### Working Directory: `{cwd}`
/// ### Original command: `{command}`
/// ### New Output
/// {output}
/// ```
pub fn format_active_terminals(terminals: &[TerminalInfo]) -> String {
    if terminals.is_empty() {
        return String::new();
    }

    let mut details = String::from("\n\n# Actively Running Terminals");

    for terminal in terminals {
        details.push_str(&format!(
            "\n## Terminal {} (Active)",
            terminal.id
        ));
        details.push_str(&format!(
            "\n### Working Directory: `{}`",
            terminal.working_directory
        ));
        details.push_str(&format!(
            "\n### Original command: `{}`",
            terminal.last_command
        ));

        if let Some(ref output) = terminal.new_output {
            if !output.is_empty() {
                // Stub: in the TS version this goes through
                // Terminal.compressTerminalOutput. We keep the output as-is
                // (compression belongs to the terminal crate).
                let compressed = compress_terminal_output(output);
                details.push_str(&format!("\n### New Output\n{}", compressed));
            }
        }
    }

    details
}

/// Format the "Inactive Terminals with Completed Process Output" section.
///
/// Produces output like:
/// ```text
///
/// # Inactive Terminals with Completed Process Output
/// ## Terminal {id} (Inactive)
/// ### Working Directory: `{cwd}`
/// ### New Output
/// Command: `{command}`
/// {output}
/// ```
pub fn format_inactive_terminals(terminals: &[InactiveTerminalInfo]) -> String {
    // Only include terminals that actually have completed processes with output.
    let terminals_with_output: Vec<&InactiveTerminalInfo> = terminals
        .iter()
        .filter(|t| {
            t.completed_processes
                .iter()
                .any(|p| !p.output.is_empty())
        })
        .collect();

    if terminals_with_output.is_empty() {
        return String::new();
    }

    let mut details = String::from("\n\n# Inactive Terminals with Completed Process Output");

    for terminal in &terminals_with_output {
        let mut terminal_outputs: Vec<String> = Vec::new();

        for process in &terminal.completed_processes {
            if !process.output.is_empty() {
                let compressed = compress_terminal_output(&process.output);
                terminal_outputs.push(format!(
                    "Command: `{}`\n{}",
                    process.command, compressed
                ));
            }
        }

        if !terminal_outputs.is_empty() {
            details.push_str(&format!(
                "\n## Terminal {} (Inactive)",
                terminal.id
            ));
            details.push_str(&format!(
                "\n### Working Directory: `{}`",
                terminal.working_directory
            ));

            for output in &terminal_outputs {
                details.push_str(&format!("\n### New Output\n{}", output));
            }
        }
    }

    details
}

/// Compress terminal output by removing ANSI escape sequences, collapsing
/// consecutive blank lines, applying run-length encoding for repeated lines,
/// and truncating to line/character limits.
///
/// Source: `src/integrations/terminal/BaseTerminal.ts` — `compressTerminalOutput`
/// which calls `applyRunLengthEncoding` then `truncateOutput`.
fn compress_terminal_output(output: &str) -> String {
    compress_terminal_output_with_limits(output, LINE_LIMIT, CHARACTER_LIMIT)
}

/// Hardcoded UI display limits matching the TypeScript implementation.
const LINE_LIMIT: usize = 500;
const CHARACTER_LIMIT: usize = 50_000;

/// Compress terminal output with configurable limits (for testing).
fn compress_terminal_output_with_limits(
    output: &str,
    line_limit: usize,
    character_limit: usize,
) -> String {
    // Step 1: Strip ANSI escape sequences
    let stripped = strip_ansi_escapes(output);

    // Step 2: Collapse consecutive blank lines into a single blank line
    let collapsed = collapse_blank_lines(&stripped);

    // Step 3: Apply run-length encoding for repeated lines
    let rle = apply_run_length_encoding(&collapsed);

    // Step 4: Truncate to limits
    truncate_output(&rle, line_limit, character_limit)
}

/// Strip ANSI escape sequences from terminal output.
fn strip_ansi_escapes(input: &str) -> String {
    use regex::Regex;
    // Match ANSI escape sequences: ESC [ ... letter, or ESC ] ... BEL/ST
    let re = Regex::new(r"\x1b\[[0-9;]*[A-Za-z]|\x1b\][^\x07]*\x07|\x1b\][^\x1b]*\x1b\\|\x1b[^\[\]].?").unwrap();
    re.replace_all(input, "").to_string()
}

/// Collapse consecutive blank lines into a single blank line.
fn collapse_blank_lines(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut prev_blank = false;

    for line in input.lines() {
        let is_blank = line.trim().is_empty();
        if is_blank && prev_blank {
            continue; // skip consecutive blank lines
        }
        if !result.is_empty() {
            result.push('\n');
        }
        result.push_str(line);
        prev_blank = is_blank;
    }

    result
}

/// Apply run-length encoding to compress repeated consecutive lines.
///
/// Source: `src/integrations/misc/extract-text.ts` — `applyRunLengthEncoding`
fn apply_run_length_encoding(content: &str) -> String {
    if content.is_empty() {
        return content.to_string();
    }

    let mut result = String::with_capacity(content.len());
    let mut prev_line: Option<&str> = None;
    let mut repeat_count: usize = 0;

    for line in content.lines() {
        match prev_line {
            None => {
                prev_line = Some(line);
            }
            Some(prev) if line == prev => {
                repeat_count += 1;
            }
            Some(prev) => {
                flush_repeated(&mut result, prev, repeat_count);
                prev_line = Some(line);
                repeat_count = 0;
            }
        }
    }

    // Flush the last group
    if let Some(prev) = prev_line {
        flush_repeated(&mut result, prev, repeat_count);
    }

    result
}

/// Flush a repeated line group, using compression description when beneficial.
fn flush_repeated(result: &mut String, line: &str, repeat_count: usize) {
    if repeat_count > 0 {
        let compression_desc = format!("<previous line repeated {} additional times>\n", repeat_count);
        // Only compress if the description is shorter than the repeated content
        if compression_desc.len() < line.len() * (repeat_count + 1) {
            result.push_str(line);
            result.push('\n');
            result.push_str(&compression_desc);
        } else {
            for _ in 0..=repeat_count {
                result.push_str(line);
                result.push('\n');
            }
        }
    } else {
        result.push_str(line);
        result.push('\n');
    }
}

/// Truncate output to line and character limits.
///
/// When the character limit is exceeded, keeps 20% from the start and 80% from
/// the end. When the line limit is exceeded (but character limit is not), keeps
/// 20% of lines from the start and 80% from the end.
///
/// Source: `src/integrations/misc/extract-text.ts` — `truncateOutput`
fn truncate_output(content: &str, line_limit: usize, character_limit: usize) -> String {
    // Character limit takes priority
    if content.len() > character_limit {
        let before_limit = (character_limit as f64 * 0.2) as usize;
        let after_limit = character_limit - before_limit;

        let start_section = &content[..before_limit.min(content.len())];
        let end_start = content.len().saturating_sub(after_limit);
        let end_section = &content[end_start..];
        let omitted_chars = content.len() - character_limit;

        return format!(
            "{}\n[...{} characters omitted...]\n{}",
            start_section, omitted_chars, end_section
        );
    }

    // Check line limit
    let total_lines = content.lines().count();
    if total_lines <= line_limit {
        return content.to_string();
    }

    let before_limit = (line_limit as f64 * 0.2) as usize;
    let after_limit = line_limit - before_limit;

    let lines: Vec<&str> = content.lines().collect();
    let start_section: Vec<&str> = lines.iter().take(before_limit).copied().collect();
    let end_section: Vec<&str> = lines.iter().rev().take(after_limit).copied().collect::<Vec<_>>();
    let omitted_lines = total_lines - line_limit;

    let mut result = start_section.join("\n");
    result.push_str(&format!("\n[...{} lines omitted...]\n\n", omitted_lines));
    // Reverse the end section back to original order
    let mut end_rev: Vec<&str> = end_section;
    end_rev.reverse();
    result.push_str(&end_rev.join("\n"));

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CompletedProcess;

    #[test]
    fn test_active_terminals_empty() {
        let result = format_active_terminals(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_active_terminal_with_output() {
        let terminals = vec![TerminalInfo {
            id: "terminal-1".to_string(),
            working_directory: "/test/path/src".to_string(),
            last_command: "npm test".to_string(),
            new_output: Some("Test output".to_string()),
        }];
        let result = format_active_terminals(&terminals);
        assert!(result.contains("# Actively Running Terminals"));
        assert!(result.contains("## Terminal terminal-1 (Active)"));
        assert!(result.contains("### Working Directory: `/test/path/src`"));
        assert!(result.contains("### Original command: `npm test`"));
        assert!(result.contains("### New Output\nTest output"));
    }

    #[test]
    fn test_active_terminal_without_output() {
        let terminals = vec![TerminalInfo {
            id: "t1".to_string(),
            working_directory: "/home".to_string(),
            last_command: "ls".to_string(),
            new_output: None,
        }];
        let result = format_active_terminals(&terminals);
        assert!(result.contains("## Terminal t1 (Active)"));
        assert!(!result.contains("### New Output"));
    }

    #[test]
    fn test_active_terminal_with_empty_output() {
        let terminals = vec![TerminalInfo {
            id: "t1".to_string(),
            working_directory: "/home".to_string(),
            last_command: "ls".to_string(),
            new_output: Some(String::new()),
        }];
        let result = format_active_terminals(&terminals);
        assert!(!result.contains("### New Output"));
    }

    #[test]
    fn test_multiple_active_terminals() {
        let terminals = vec![
            TerminalInfo {
                id: "t1".to_string(),
                working_directory: "/a".to_string(),
                last_command: "cmd1".to_string(),
                new_output: Some("out1".to_string()),
            },
            TerminalInfo {
                id: "t2".to_string(),
                working_directory: "/b".to_string(),
                last_command: "cmd2".to_string(),
                new_output: Some("out2".to_string()),
            },
        ];
        let result = format_active_terminals(&terminals);
        assert!(result.contains("## Terminal t1 (Active)"));
        assert!(result.contains("## Terminal t2 (Active)"));
    }

    #[test]
    fn test_inactive_terminals_empty() {
        let result = format_inactive_terminals(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_inactive_terminal_with_completed_process() {
        let terminals = vec![InactiveTerminalInfo {
            id: "terminal-2".to_string(),
            working_directory: "/test/path/build".to_string(),
            completed_processes: vec![CompletedProcess {
                command: "npm build".to_string(),
                output: "Build output".to_string(),
            }],
        }];
        let result = format_inactive_terminals(&terminals);
        assert!(result.contains("# Inactive Terminals with Completed Process Output"));
        assert!(result.contains("## Terminal terminal-2 (Inactive)"));
        assert!(result.contains("### Working Directory: `/test/path/build`"));
        assert!(result.contains("Command: `npm build`"));
        assert!(result.contains("Build output"));
    }

    #[test]
    fn test_inactive_terminal_no_completed_processes() {
        let terminals = vec![InactiveTerminalInfo {
            id: "t1".to_string(),
            working_directory: "/home".to_string(),
            completed_processes: vec![],
        }];
        let result = format_inactive_terminals(&terminals);
        assert!(result.is_empty());
    }

    #[test]
    fn test_inactive_terminal_empty_output() {
        let terminals = vec![InactiveTerminalInfo {
            id: "t1".to_string(),
            working_directory: "/home".to_string(),
            completed_processes: vec![CompletedProcess {
                command: "echo".to_string(),
                output: String::new(),
            }],
        }];
        let result = format_inactive_terminals(&terminals);
        assert!(result.is_empty());
    }

    #[test]
    fn test_inactive_terminal_multiple_processes() {
        let terminals = vec![InactiveTerminalInfo {
            id: "t1".to_string(),
            working_directory: "/project".to_string(),
            completed_processes: vec![
                CompletedProcess {
                    command: "npm build".to_string(),
                    output: "build ok".to_string(),
                },
                CompletedProcess {
                    command: "npm test".to_string(),
                    output: "tests passed".to_string(),
                },
            ],
        }];
        let result = format_inactive_terminals(&terminals);
        assert!(result.contains("Command: `npm build`\nbuild ok"));
        assert!(result.contains("Command: `npm test`\ntests passed"));
        // Two "### New Output" sections
        assert_eq!(result.matches("### New Output").count(), 2);
    }

    // --- Compression tests ---

    #[test]
    fn test_strip_ansi_escapes() {
        let input = "\x1b[32mHello\x1b[0m \x1b[1;33mWorld\x1b[0m";
        let result = strip_ansi_escapes(input);
        assert_eq!(result, "Hello World");
    }

    #[test]
    fn test_strip_ansi_escapes_empty() {
        assert_eq!(strip_ansi_escapes(""), "");
    }

    #[test]
    fn test_strip_ansi_escapes_no_escapes() {
        assert_eq!(strip_ansi_escapes("plain text"), "plain text");
    }

    #[test]
    fn test_collapse_blank_lines() {
        let input = "line1\n\n\n\nline2\n\nline3";
        let result = collapse_blank_lines(input);
        assert_eq!(result, "line1\n\nline2\n\nline3");
    }

    #[test]
    fn test_collapse_blank_lines_no_blanks() {
        let input = "line1\nline2\nline3";
        let result = collapse_blank_lines(input);
        assert_eq!(result, "line1\nline2\nline3");
    }

    #[test]
    fn test_collapse_blank_lines_all_blanks() {
        let input = "\n\n\n";
        let result = collapse_blank_lines(input);
        assert_eq!(result, "");
    }

    #[test]
    fn test_run_length_encoding_repeated_lines() {
        let input = "longerline\nlongerline\nlongerline\nlongerline\nlongerline\n";
        let result = apply_run_length_encoding(input);
        assert!(result.contains("<previous line repeated 4 additional times>"));
    }

    #[test]
    fn test_run_length_encoding_no_repetition() {
        let input = "line1\nline2\nline3\n";
        let result = apply_run_length_encoding(input);
        assert_eq!(result, "line1\nline2\nline3\n");
    }

    #[test]
    fn test_run_length_encoding_empty() {
        assert_eq!(apply_run_length_encoding(""), "");
    }

    #[test]
    fn test_run_length_encoding_short_lines_not_compressed() {
        // Short lines where compression description is longer than the content
        let input = "y\ny\ny\ny\ny\n";
        let result = apply_run_length_encoding(input);
        // Should not compress because description is longer than repeated content
        assert!(!result.contains("<previous line repeated"));
    }

    #[test]
    fn test_truncate_output_within_limits() {
        let input = "line1\nline2\nline3";
        let result = truncate_output(input, 100, 10000);
        assert_eq!(result, input);
    }

    #[test]
    fn test_truncate_output_exceeds_character_limit() {
        let input = "a".repeat(200);
        let result = truncate_output(&input, 1000, 100);
        assert!(result.contains("characters omitted"));
    }

    #[test]
    fn test_truncate_output_exceeds_line_limit() {
        let input: Vec<String> = (0..100).map(|i| format!("line {}", i)).collect();
        let input = input.join("\n");
        let result = truncate_output(&input, 10, 100000);
        assert!(result.contains("lines omitted"));
    }

    #[test]
    fn test_compress_terminal_output_integration() {
        // Integration test with ANSI codes, blank lines, and repeated lines
        let long_line = "This is a sufficiently long line that should be compressed by run-length encoding when repeated many times";
        let input = format!(
            "\x1b[32m{}\x1b[0m\n\n\n\n{}\n{}\n{}\n{}\n{}\nDone!",
            long_line, long_line, long_line, long_line, long_line, long_line
        );
        let result = compress_terminal_output_with_limits(&input, 500, 50000);
        // Should have stripped ANSI codes
        assert!(!result.contains("\x1b["));
        // Should have collapsed blank lines
        assert!(!result.contains("\n\n\n"));
        // Should have compressed repeated lines
        assert!(result.contains("<previous line repeated"));
        assert!(result.contains("Done!"));
    }

    #[test]
    fn test_compress_preserves_simple_output() {
        // Simple output should pass through largely unchanged
        let input = "Test output";
        let result = compress_terminal_output_with_limits(input, 500, 50000);
        assert!(result.contains("Test output"));
    }
}

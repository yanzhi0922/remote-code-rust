//! Simple YAML frontmatter parser for command `.md` files.
//!
//! Extracts `description`, `argument-hint`, and `mode` from a `---`-delimited
//! YAML header. A hand-written line parser is used instead of a full YAML
//! library to keep the dependency footprint small.
//!
//! Maps to TypeScript source: `src/services/command/commands.ts` (frontmatter parsing with gray-matter)

/// Parsed frontmatter fields from a command `.md` file.
#[derive(Debug, Clone, Default)]
pub struct CommandFrontMatter {
    pub description: Option<String>,
    pub argument_hint: Option<String>,
    pub mode: Option<String>,
}

/// Result of splitting a file's content into optional frontmatter and body.
pub struct ParsedCommand {
    pub frontmatter: CommandFrontMatter,
    pub body: String,
}

/// Find the closing `---` delimiter in frontmatter text.
///
/// Returns `Some((position, delimiter_byte_length))` where `position` is the
/// start of the closing delimiter in `rest` and `delimiter_byte_length` is
/// the number of bytes to skip past the delimiter.
fn find_closing_delimiter(rest: &str) -> Option<(usize, usize)> {
    // Case 1: closing --- preceded by a newline (most common)
    if let Some(pos) = rest.find("\n---") {
        // Make sure it's exactly "---" (not "----")
        let after = &rest[pos + 4..];
        let next_line = after.find('\n').unwrap_or(after.len());
        let delim_line = &after[..next_line].trim_end_matches('\r');
        if delim_line.is_empty() {
            return Some((pos, 4)); // skip "\n---"
        }
    }

    // Case 2: closing --- at the very start of rest (empty frontmatter)
    if rest.starts_with("---") {
        let after = &rest[3..];
        // Must be followed by newline or end-of-string
        if after.is_empty() || after.starts_with('\n') || after.starts_with("\r\n") {
            let skip = if after.starts_with("\r\n") {
                5 // "---\r\n"
            } else if after.starts_with('\n') {
                4 // "---\n"
            } else {
                3 // "---" (end of string)
            };
            return Some((0, skip));
        }
    }

    // Case 3: closing --- at the end without trailing newline
    if rest.ends_with("---") && rest.len() > 3 {
        return Some((rest.len() - 3, 3));
    }

    None
}

/// Parse the content of a command `.md` file.
///
/// If the content starts with `---`, the text between the opening and closing
/// `---` markers is treated as YAML frontmatter. Otherwise the entire content
/// is returned as the body with no frontmatter fields.
pub fn parse_command_content(content: &str) -> ParsedCommand {
    let trimmed = content.trim_start();

    if !trimmed.starts_with("---") {
        return ParsedCommand {
            frontmatter: CommandFrontMatter::default(),
            body: content.trim().to_string(),
        };
    }

    // Skip the opening ---
    let after_opening = &trimmed[3..];
    let rest = after_opening.trim_start_matches(['\r', '\n']);

    // Find the closing ---
    let closing_pos = find_closing_delimiter(rest);

    let (frontmatter_str, body) = match closing_pos {
        Some((pos, delimiter_len)) => {
            let fm = &rest[..pos];
            // Body starts after the closing --- and any whitespace
            let after_closing = &rest[pos + delimiter_len..];
            let body = after_closing.trim_start_matches(['\r', '\n']);
            (fm, body.to_string())
        }
        None => {
            // No closing --- found; treat entire content as body
            return ParsedCommand {
                frontmatter: CommandFrontMatter::default(),
                body: content.trim().to_string(),
            };
        }
    };

    let frontmatter = parse_frontmatter_str(frontmatter_str);

    ParsedCommand {
        frontmatter,
        body,
    }
}

/// Parse a frontmatter string into a [`CommandFrontMatter`].
fn parse_frontmatter_str(s: &str) -> CommandFrontMatter {
    let mut fm = CommandFrontMatter::default();

    for line in s.lines() {
        let trimmed_line = line.trim();

        if trimmed_line.is_empty() {
            continue;
        }

        // Only handle simple "key: value" lines
        if let Some((key, value)) = split_kv(trimmed_line) {
            match key {
                "description" => {
                    let v = value.trim().to_string();
                    if !v.is_empty() {
                        fm.description = Some(v);
                    }
                }
                "argument-hint" => {
                    let v = value.trim().to_string();
                    if !v.is_empty() {
                        fm.argument_hint = Some(v);
                    }
                }
                "mode" => {
                    let v = value.trim().to_string();
                    if !v.is_empty() {
                        fm.mode = Some(v);
                    }
                }
                _ => {}
            }
        }
    }

    fm
}

/// Split a `key: value` line, returning `(key, value)`.
fn split_kv(line: &str) -> Option<(&str, &str)> {
    let colon_pos = line.find(':')?;
    let key = &line[..colon_pos];
    let value = &line[colon_pos + 1..];
    Some((key.trim(), value.trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- parse_command_content ----

    #[test]
    fn test_no_frontmatter() {
        let content = "Hello, this is the command body.";
        let parsed = parse_command_content(content);
        assert!(parsed.frontmatter.description.is_none());
        assert!(parsed.frontmatter.argument_hint.is_none());
        assert!(parsed.frontmatter.mode.is_none());
        assert_eq!(parsed.body, "Hello, this is the command body.");
    }

    #[test]
    fn test_empty_frontmatter() {
        let content = "---\n---\nBody content here.";
        let parsed = parse_command_content(content);
        assert!(parsed.frontmatter.description.is_none());
        assert!(parsed.frontmatter.argument_hint.is_none());
        assert!(parsed.frontmatter.mode.is_none());
        assert_eq!(parsed.body, "Body content here.");
    }

    #[test]
    fn test_description_only() {
        let content = "---\ndescription: My command description\n---\nBody content.";
        let parsed = parse_command_content(content);
        assert_eq!(parsed.frontmatter.description.as_deref(), Some("My command description"));
        assert!(parsed.frontmatter.argument_hint.is_none());
        assert!(parsed.frontmatter.mode.is_none());
        assert_eq!(parsed.body, "Body content.");
    }

    #[test]
    fn test_all_fields() {
        let content = "---\ndescription: A test command\nargument-hint: <file-path>\nmode: code\n---\nDo something useful.";
        let parsed = parse_command_content(content);
        assert_eq!(parsed.frontmatter.description.as_deref(), Some("A test command"));
        assert_eq!(parsed.frontmatter.argument_hint.as_deref(), Some("<file-path>"));
        assert_eq!(parsed.frontmatter.mode.as_deref(), Some("code"));
        assert_eq!(parsed.body, "Do something useful.");
    }

    #[test]
    fn test_mode_only() {
        let content = "---\nmode: architect\n---\nDesign the system.";
        let parsed = parse_command_content(content);
        assert!(parsed.frontmatter.description.is_none());
        assert!(parsed.frontmatter.argument_hint.is_none());
        assert_eq!(parsed.frontmatter.mode.as_deref(), Some("architect"));
        assert_eq!(parsed.body, "Design the system.");
    }

    #[test]
    fn test_argument_hint_only() {
        let content = "---\nargument-hint: <url>\n---\nFetch the URL.";
        let parsed = parse_command_content(content);
        assert!(parsed.frontmatter.description.is_none());
        assert_eq!(parsed.frontmatter.argument_hint.as_deref(), Some("<url>"));
        assert!(parsed.frontmatter.mode.is_none());
        assert_eq!(parsed.body, "Fetch the URL.");
    }

    #[test]
    fn test_empty_values_ignored() {
        let content = "---\ndescription: \nmode:   \n---\nBody.";
        let parsed = parse_command_content(content);
        assert!(parsed.frontmatter.description.is_none());
        assert!(parsed.frontmatter.mode.is_none());
        assert_eq!(parsed.body, "Body.");
    }

    #[test]
    fn test_unknown_fields_ignored() {
        let content = "---\nunknown-key: value\ndescription: Hello\n---\nBody.";
        let parsed = parse_command_content(content);
        assert_eq!(parsed.frontmatter.description.as_deref(), Some("Hello"));
        assert!(parsed.frontmatter.argument_hint.is_none());
        assert!(parsed.frontmatter.mode.is_none());
    }

    #[test]
    fn test_no_closing_delimiter() {
        let content = "---\ndescription: No closing\nBody here.";
        let parsed = parse_command_content(content);
        // Without closing ---, entire content is treated as body
        assert!(parsed.frontmatter.description.is_none());
        assert_eq!(parsed.body, "---\ndescription: No closing\nBody here.");
    }

    #[test]
    fn test_crlf_line_endings() {
        let content = "---\r\ndescription: Hello\r\n---\r\nBody content.";
        let parsed = parse_command_content(content);
        assert_eq!(parsed.frontmatter.description.as_deref(), Some("Hello"));
        assert_eq!(parsed.body, "Body content.");
    }

    #[test]
    fn test_multiline_body() {
        let content = "---\ndescription: Multi\n---\nLine 1\nLine 2\nLine 3";
        let parsed = parse_command_content(content);
        assert_eq!(parsed.body, "Line 1\nLine 2\nLine 3");
    }

    #[test]
    fn test_description_with_colons() {
        let content = "---\ndescription: Use this: do that\n---\nBody.";
        let parsed = parse_command_content(content);
        assert_eq!(
            parsed.frontmatter.description.as_deref(),
            Some("Use this: do that")
        );
    }

    #[test]
    fn test_whitespace_around_values() {
        let content = "---\ndescription:   spaced   \n---\nBody.";
        let parsed = parse_command_content(content);
        assert_eq!(parsed.frontmatter.description.as_deref(), Some("spaced"));
    }
}

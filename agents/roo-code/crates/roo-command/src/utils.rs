//! Utility functions for command file handling.
//!
//! Maps to TypeScript source: `src/services/command/commands.ts` (getCommandNameFromFile, isMarkdownFile)

/// Extract the command name from a filename by stripping the `.md` extension.
///
/// The comparison is case-insensitive to handle `.MD`, `.Md`, etc.
///
/// # Examples
///
/// ```
/// use roo_command::utils::get_command_name_from_file;
///
/// assert_eq!(get_command_name_from_file("hello.md"), "hello");
/// assert_eq!(get_command_name_from_file("hello.MD"), "hello");
/// assert_eq!(get_command_name_from_file("hello.txt"), "hello.txt");
/// assert_eq!(get_command_name_from_file("my-command.md"), "my-command");
/// ```
pub fn get_command_name_from_file(filename: &str) -> String {
    if filename.to_ascii_lowercase().ends_with(".md") {
        filename[..filename.len() - 3].to_string()
    } else {
        filename.to_string()
    }
}

/// Check whether a filename has a Markdown extension.
///
/// Case-insensitive: `.md`, `.MD`, `.Md` all return `true`.
///
/// # Examples
///
/// ```
/// use roo_command::utils::is_markdown_file;
///
/// assert!(is_markdown_file("readme.md"));
/// assert!(is_markdown_file("readme.MD"));
/// assert!(is_markdown_file("readme.Md"));
/// assert!(!is_markdown_file("readme.txt"));
/// assert!(!is_markdown_file("readme"));
/// ```
pub fn is_markdown_file(filename: &str) -> bool {
    filename.to_ascii_lowercase().ends_with(".md")
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- get_command_name_from_file ----

    #[test]
    fn test_get_command_name_simple_md() {
        assert_eq!(get_command_name_from_file("hello.md"), "hello");
    }

    #[test]
    fn test_get_command_name_uppercase_md() {
        assert_eq!(get_command_name_from_file("hello.MD"), "hello");
    }

    #[test]
    fn test_get_command_name_mixed_case_md() {
        assert_eq!(get_command_name_from_file("hello.Md"), "hello");
    }

    #[test]
    fn test_get_command_name_non_md() {
        assert_eq!(get_command_name_from_file("hello.txt"), "hello.txt");
    }

    #[test]
    fn test_get_command_name_no_extension() {
        assert_eq!(get_command_name_from_file("hello"), "hello");
    }

    #[test]
    fn test_get_command_name_with_dashes() {
        assert_eq!(get_command_name_from_file("my-command.md"), "my-command");
    }

    #[test]
    fn test_get_command_name_with_dots() {
        assert_eq!(get_command_name_from_file("my.command.md"), "my.command");
    }

    #[test]
    fn test_get_command_name_empty() {
        assert_eq!(get_command_name_from_file(""), "");
    }

    // ---- is_markdown_file ----

    #[test]
    fn test_is_markdown_file_lowercase() {
        assert!(is_markdown_file("readme.md"));
    }

    #[test]
    fn test_is_markdown_file_uppercase() {
        assert!(is_markdown_file("readme.MD"));
    }

    #[test]
    fn test_is_markdown_file_mixed_case() {
        assert!(is_markdown_file("readme.Md"));
    }

    #[test]
    fn test_is_markdown_file_txt() {
        assert!(!is_markdown_file("readme.txt"));
    }

    #[test]
    fn test_is_markdown_file_no_extension() {
        assert!(!is_markdown_file("readme"));
    }

    #[test]
    fn test_is_markdown_file_md_in_middle() {
        // "readme.md.bak" does NOT end with .md
        assert!(!is_markdown_file("readme.md.bak"));
    }

    #[test]
    fn test_is_markdown_file_empty() {
        assert!(!is_markdown_file(""));
    }

    #[test]
    fn test_is_markdown_file_only_extension() {
        assert!(is_markdown_file(".md"));
    }
}

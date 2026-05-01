//! Regular expressions for @ mention and slash command parsing.
//!
//! Maps to TypeScript source: `src/shared/context-mentions.ts`
//! (mentionRegexGlobal, commandRegexGlobal, unescapeSpaces)

use std::sync::LazyLock;

use regex::Regex;

/// Regex that matches @ mentions in text.
///
/// Matches `@` followed by one of:
/// - A file/folder path starting with `/` (e.g., `@/src/main.rs`, `@/src/`)
/// - The keyword `problems`
/// - The keyword `git-changes`
/// - A hex string of 7-40 characters (git commit hash)
/// - The keyword `terminal`
/// - An HTTP(S) URL
///
/// The regex uses a global (replacing all matches) approach.
pub fn mention_regex() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"@(/?[^\s@]+|problems|git-changes|[a-f0-9]{7,40}|terminal|https?://[^\s]+)")
            .expect("Failed to compile mention regex")
    });
    &RE
}

/// Regex that matches slash commands in text.
///
/// Matches `/` followed by a command name (alphanumeric, hyphens, underscores).
pub fn command_regex() -> &'static Regex {
    static RE: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"/([a-zA-Z0-9_-]+)")
            .expect("Failed to compile command regex")
    });
    &RE
}

/// Unescape spaces that were escaped with backslash in a mention path.
///
/// In mention paths, spaces can be escaped as `\ ` to prevent the regex
/// from treating them as delimiters. This function converts them back.
pub fn unescape_spaces(path: &str) -> String {
    path.replace("\\ ", " ")
}

/// Check if a string looks like a git commit hash (7-40 hex characters).
pub fn is_git_hash(s: &str) -> bool {
    let len = s.len();
    if len < 7 || len > 40 {
        return false;
    }
    s.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    // === mention_regex tests ===

    #[test]
    fn test_mention_regex_file_path() {
        let re = mention_regex();
        let caps: Vec<_> = re.captures_iter("@/src/main.rs").collect();
        assert_eq!(caps.len(), 1);
        assert_eq!(&caps[0][1], "/src/main.rs");
    }

    #[test]
    fn test_mention_regex_folder_path() {
        let re = mention_regex();
        let caps: Vec<_> = re.captures_iter("@/src/").collect();
        assert_eq!(caps.len(), 1);
        assert_eq!(&caps[0][1], "/src/");
    }

    #[test]
    fn test_mention_regex_problems() {
        let re = mention_regex();
        let text = "check @problems for issues";
        let caps: Vec<_> = re.captures_iter(text).collect();
        assert_eq!(caps.len(), 1);
        assert_eq!(&caps[0][1], "problems");
    }

    #[test]
    fn test_mention_regex_git_changes() {
        let re = mention_regex();
        let text = "see @git-changes";
        let caps: Vec<_> = re.captures_iter(text).collect();
        assert_eq!(caps.len(), 1);
        assert_eq!(&caps[0][1], "git-changes");
    }

    #[test]
    fn test_mention_regex_git_hash_7_chars() {
        let re = mention_regex();
        let text = "commit @abc1234";
        let caps: Vec<_> = re.captures_iter(text).collect();
        assert_eq!(caps.len(), 1);
        assert_eq!(&caps[0][1], "abc1234");
    }

    #[test]
    fn test_mention_regex_git_hash_40_chars() {
        let re = mention_regex();
        let hash = "a".repeat(40);
        let text = format!("commit @{}", hash);
        let caps: Vec<_> = re.captures_iter(&text).collect();
        assert_eq!(caps.len(), 1);
        assert_eq!(&caps[0][1], hash);
    }

    #[test]
    fn test_mention_regex_git_hash_too_short() {
        let re = mention_regex();
        let text = "commit @abc123";
        let caps: Vec<_> = re.captures_iter(text).collect();
        // 6 chars should not match as git hash — but it might match as a path
        // since /?[^\s@]+ would match "abc123" as a path-like thing
        // Actually, the regex matches `abc123` via the `[^\s@]+` branch
        // This is expected behavior in the TS source too
        assert_eq!(caps.len(), 1);
    }

    #[test]
    fn test_mention_regex_terminal() {
        let re = mention_regex();
        let text = "check @terminal";
        let caps: Vec<_> = re.captures_iter(text).collect();
        assert_eq!(caps.len(), 1);
        assert_eq!(&caps[0][1], "terminal");
    }

    #[test]
    fn test_mention_regex_url() {
        let re = mention_regex();
        let text = "see @https://example.com/page";
        let caps: Vec<_> = re.captures_iter(text).collect();
        assert_eq!(caps.len(), 1);
        assert_eq!(&caps[0][1], "https://example.com/page");
    }

    #[test]
    fn test_mention_regex_http_url() {
        let re = mention_regex();
        let text = "see @http://example.com";
        let caps: Vec<_> = re.captures_iter(text).collect();
        assert_eq!(caps.len(), 1);
        assert_eq!(&caps[0][1], "http://example.com");
    }

    #[test]
    fn test_mention_regex_multiple_mentions() {
        let re = mention_regex();
        let text = "@/file1.rs and @problems and @terminal";
        let caps: Vec<_> = re.captures_iter(text).collect();
        assert_eq!(caps.len(), 3);
        assert_eq!(&caps[0][1], "/file1.rs");
        assert_eq!(&caps[1][1], "problems");
        assert_eq!(&caps[2][1], "terminal");
    }

    #[test]
    fn test_mention_regex_no_match() {
        let re = mention_regex();
        let text = "no mentions here";
        let caps: Vec<_> = re.captures_iter(text).collect();
        assert!(caps.is_empty());
    }

    #[test]
    fn test_mention_regex_bare_at_sign() {
        let re = mention_regex();
        let text = "email@example.com";
        // The regex requires @ followed by a non-whitespace, non-@ character
        // "email@example.com" — @e matches, capturing "example.com" via the path branch
        let caps: Vec<_> = re.captures_iter(text).collect();
        // This matches because @e starts a mention and "example.com" is captured
        assert_eq!(caps.len(), 1);
        assert_eq!(&caps[0][1], "example.com");
    }

    #[test]
    fn test_mention_regex_relative_path() {
        let re = mention_regex();
        let text = "@/relative/path/to/file.txt";
        let caps: Vec<_> = re.captures_iter(text).collect();
        assert_eq!(caps.len(), 1);
        assert_eq!(&caps[0][1], "/relative/path/to/file.txt");
    }

    // === command_regex tests ===

    #[test]
    fn test_command_regex_simple() {
        let re = command_regex();
        let caps: Vec<_> = re.captures_iter("/help").collect();
        assert_eq!(caps.len(), 1);
        assert_eq!(&caps[0][1], "help");
    }

    #[test]
    fn test_command_regex_with_hyphen() {
        let re = command_regex();
        let caps: Vec<_> = re.captures_iter("/my-command").collect();
        assert_eq!(caps.len(), 1);
        assert_eq!(&caps[0][1], "my-command");
    }

    #[test]
    fn test_command_regex_with_underscore() {
        let re = command_regex();
        let caps: Vec<_> = re.captures_iter("/my_command").collect();
        assert_eq!(caps.len(), 1);
        assert_eq!(&caps[0][1], "my_command");
    }

    #[test]
    fn test_command_regex_with_numbers() {
        let re = command_regex();
        let caps: Vec<_> = re.captures_iter("/cmd123").collect();
        assert_eq!(caps.len(), 1);
        assert_eq!(&caps[0][1], "cmd123");
    }

    #[test]
    fn test_command_regex_multiple_commands() {
        let re = command_regex();
        let text = "/help and /test-cmd";
        let caps: Vec<_> = re.captures_iter(text).collect();
        assert_eq!(caps.len(), 2);
        assert_eq!(&caps[0][1], "help");
        assert_eq!(&caps[1][1], "test-cmd");
    }

    #[test]
    fn test_command_regex_in_path() {
        let re = command_regex();
        let text = "/usr/bin/file";
        let caps: Vec<_> = re.captures_iter(text).collect();
        // The regex matches every /word segment — it doesn't distinguish between
        // slash commands and file paths. The TS source handles this by checking
        // if the command actually exists before replacing.
        assert_eq!(caps.len(), 3);
        assert_eq!(&caps[0][1], "usr");
        assert_eq!(&caps[1][1], "bin");
        assert_eq!(&caps[2][1], "file");
    }

    // === unescape_spaces tests ===

    #[test]
    fn test_unescape_spaces_basic() {
        assert_eq!(unescape_spaces("path\\ with\\ spaces"), "path with spaces");
    }

    #[test]
    fn test_unescape_spaces_no_escapes() {
        assert_eq!(unescape_spaces("normal/path"), "normal/path");
    }

    #[test]
    fn test_unescape_spaces_single_escape() {
        assert_eq!(unescape_spaces("my\\ file.rs"), "my file.rs");
    }

    #[test]
    fn test_unescape_spaces_multiple_escapes() {
        assert_eq!(
            unescape_spaces("path\\ with\\ many\\ spaces"),
            "path with many spaces"
        );
    }

    #[test]
    fn test_unescape_spaces_empty() {
        assert_eq!(unescape_spaces(""), "");
    }

    #[test]
    fn test_unescape_spaces_trailing_backslash() {
        // A trailing backslash followed by nothing — no space to unescape
        assert_eq!(unescape_spaces("path\\"), "path\\");
    }

    // === is_git_hash tests ===

    #[test]
    fn test_is_git_hash_valid_7() {
        assert!(is_git_hash("abc1234"));
    }

    #[test]
    fn test_is_git_hash_valid_40() {
        let hash = "a".repeat(40);
        assert!(is_git_hash(&hash));
    }

    #[test]
    fn test_is_git_hash_valid_20() {
        assert!(is_git_hash("deadbeefdeadbeefdead"));
    }

    #[test]
    fn test_is_git_hash_too_short() {
        assert!(!is_git_hash("abc123"));
    }

    #[test]
    fn test_is_git_hash_too_long() {
        let hash = "a".repeat(41);
        assert!(!is_git_hash(&hash));
    }

    #[test]
    fn test_is_git_hash_uppercase() {
        assert!(!is_git_hash("ABC1234"));
    }

    #[test]
    fn test_is_git_hash_mixed_case() {
        assert!(!is_git_hash("aBc1234"));
    }

    #[test]
    fn test_is_git_hash_non_hex() {
        assert!(!is_git_hash("ghijklm"));
    }
}

//! Markdown formatting section.
//!
//! Source: `src/core/prompts/sections/markdown-formatting.ts`

/// Returns the markdown formatting rules section.
///
/// Source: `src/core/prompts/sections/markdown-formatting.ts` — `markdownFormattingSection`
pub fn markdown_formatting_section() -> &'static str {
    r#"====

MARKDOWN RULES

ALL responses MUST show ANY `language construct` OR filename reference as clickable, exactly as [`filename OR language.declaration()`](relative/file/path.ext:line); line is required for `syntax` and optional for filename links. This applies to ALL markdown responses and ALSO those in attempt_completion"#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_markdown_formatting_section() {
        let result = markdown_formatting_section();
        assert!(result.starts_with("====\n\nMARKDOWN RULES"));
        assert!(result.contains("language construct"));
        assert!(result.contains("filename reference as clickable"));
        assert!(result.contains("line is required"));
        assert!(result.contains("syntax"));
        assert!(result.contains("optional for filename links"));
        assert!(result.contains("attempt_completion"));
    }

    #[test]
    fn test_markdown_formatting_section_link_format() {
        let result = markdown_formatting_section();
        // Verify the link format example is present
        assert!(result.contains("[`filename OR language.declaration()`](relative/file/path.ext:line)"));
    }
}

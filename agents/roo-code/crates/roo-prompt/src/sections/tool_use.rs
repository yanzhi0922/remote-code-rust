//! Tool use section.
//!
//! Source: `src/core/prompts/sections/tool-use.ts`

/// Returns the shared tool use section.
///
/// Source: `src/core/prompts/sections/tool-use.ts` — `getSharedToolUseSection`
pub fn get_shared_tool_use_section() -> &'static str {
    r#"====

TOOL USE

You have access to a set of tools that are executed upon the user's approval. Use the provider-native tool-calling mechanism. Do not include XML markup or examples. You must call at least one tool per assistant response. Prefer calling as many tools as are reasonably needed in a single response to reduce back-and-forth and complete tasks faster."#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_shared_tool_use_section() {
        let result = get_shared_tool_use_section();
        assert!(result.starts_with("====\n\nTOOL USE"));
        assert!(result.contains("executed upon the user's approval"));
        assert!(result.contains("provider-native tool-calling mechanism"));
        assert!(result.contains("Do not include XML markup or examples"));
        assert!(result.contains("at least one tool per assistant response"));
        assert!(result.contains("reduce back-and-forth"));
    }
}

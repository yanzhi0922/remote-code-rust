//! Tool use guidelines section.
//!
//! Source: `src/core/prompts/sections/tool-use-guidelines.ts`

/// Returns the tool use guidelines section.
///
/// Source: `src/core/prompts/sections/tool-use-guidelines.ts` — `getToolUseGuidelinesSection`
pub fn get_tool_use_guidelines_section() -> &'static str {
    r#"# Tool Use Guidelines

1. Assess what information you already have and what information you need to proceed with the task.
2. Choose the most appropriate tool based on the task and the tool descriptions provided. Assess if you need additional information to proceed, and which of the available tools would be most effective for gathering this information. For example using the list_files tool is more effective than running a command like `ls` in the terminal. It's critical that you think about each available tool and use the one that best fits the current step in the task.
3. If multiple actions are needed, you may use multiple tools in a single message when appropriate, or use tools iteratively across messages. Each tool use should be informed by the results of previous tool uses. Do not assume the outcome of any tool use. Each step must be informed by the previous step's result.

By carefully considering the user's response after tool executions, you can react accordingly and make informed decisions about how to proceed with the task. This iterative process helps ensure the overall success and accuracy of your work."#
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_tool_use_guidelines_section() {
        let result = get_tool_use_guidelines_section();
        assert!(result.starts_with("# Tool Use Guidelines"));
        assert!(result.contains("1. Assess what information you already have"));
        assert!(result.contains("2. Choose the most appropriate tool"));
        assert!(result.contains("3. If multiple actions are needed"));
        assert!(result.contains("list_files tool is more effective"));
        assert!(result.contains("iterative process"));
    }

    #[test]
    fn test_get_tool_use_guidelines_section_complete_text() {
        let result = get_tool_use_guidelines_section();
        // Verify the complete text matches TS source
        assert!(result.contains("Do not assume the outcome of any tool use"));
        assert!(result.contains("Each step must be informed by the previous step's result"));
    }
}

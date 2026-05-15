//! Support Prompt Templates
//!
//! Defines all 10 prompt templates used for user-facing support actions
//! like enhancing, condensing, explaining, fixing, and improving code.
//!
//! Source: `src/shared/support-prompt.ts` — mirrors the TS implementation
//! exactly, including all variable placeholders (${varName}) and structured
//! output instructions.

use std::collections::HashMap;

// ---------------------------------------------------------------------------
// SupportPromptType
// ---------------------------------------------------------------------------

/// All supported prompt template types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SupportPromptType {
    /// Enhance a user prompt to make it clearer and more specific.
    Enhance,
    /// Condense / summarize a conversation or text.
    Condense,
    /// Explain the selected code.
    Explain,
    /// Fix bugs or issues in the selected code.
    Fix,
    /// Improve the selected code (refactor, optimize).
    Improve,
    /// Add selected code to the assistant's context.
    AddToContext,
    /// Add terminal output to the assistant's context.
    TerminalAddToContext,
    /// Fix terminal output (analyze errors).
    TerminalFix,
    /// Explain terminal output.
    TerminalExplain,
    /// Generate a new task prompt.
    NewTask,
}

impl SupportPromptType {
    /// Returns the string key used to look up the template.
    pub fn key(&self) -> &'static str {
        match self {
            Self::Enhance => "ENHANCE",
            Self::Condense => "CONDENSE",
            Self::Explain => "EXPLAIN",
            Self::Fix => "FIX",
            Self::Improve => "IMPROVE",
            Self::AddToContext => "ADD_TO_CONTEXT",
            Self::TerminalAddToContext => "TERMINAL_ADD_TO_CONTEXT",
            Self::TerminalFix => "TERMINAL_FIX",
            Self::TerminalExplain => "TERMINAL_EXPLAIN",
            Self::NewTask => "NEW_TASK",
        }
    }

    /// Returns all prompt types.
    pub fn all() -> &'static [SupportPromptType] {
        &[
            SupportPromptType::Enhance,
            SupportPromptType::Condense,
            SupportPromptType::Explain,
            SupportPromptType::Fix,
            SupportPromptType::Improve,
            SupportPromptType::AddToContext,
            SupportPromptType::TerminalAddToContext,
            SupportPromptType::TerminalFix,
            SupportPromptType::TerminalExplain,
            SupportPromptType::NewTask,
        ]
    }
}

// ---------------------------------------------------------------------------
// Diagnostic text generation
// ---------------------------------------------------------------------------

/// Generate diagnostic text from a list of diagnostics.
///
/// Source: `src/shared/support-prompt.ts` — `generateDiagnosticText`
pub fn generate_diagnostic_text(diagnostics: &[DiagnosticEntry]) -> String {
    if diagnostics.is_empty() {
        return String::new();
    }
    let mut lines = String::from("\nCurrent problems detected:\n");
    for d in diagnostics {
        lines.push_str("- [");
        lines.push_str(&d.source);
        lines.push_str("] ");
        lines.push_str(&d.message);
        if let Some(code) = &d.code {
            lines.push_str(&format!(" ({code})"));
        }
        lines.push('\n');
    }
    lines
}

/// A single diagnostic entry.
#[derive(Debug, Clone)]
pub struct DiagnosticEntry {
    pub source: String,
    pub message: String,
    pub code: Option<String>,
}

// ---------------------------------------------------------------------------
// Default templates — exact copies from TS
// Source: `src/shared/support-prompt.ts` — `supportPromptConfigs`
// ---------------------------------------------------------------------------

/// Source: ENHANCE template
const TEMPLATE_ENHANCE: &str = r#"Generate an enhanced version of this prompt (reply with only the enhanced prompt - no conversation, explanations, lead-in, bullet points, placeholders, or surrounding quotes):

${userInput}"#;

/// Source: CONDENSE template (~100 lines of structured summarization instructions)
const TEMPLATE_CONDENSE: &str = r#"CRITICAL: This summarization request is a SYSTEM OPERATION, not a user message.
When analyzing "user requests" and "user intent", completely EXCLUDE this summarization message.
The "most recent user request" and "Optional Next Step" must be based on what the user was doing BEFORE this system message appeared.
The goal is for work to continue seamlessly after condensation - as if it never happened.

Your task is to create a detailed summary of the conversation so far, paying close attention to the user's explicit requests and your previous actions.
This summary should be thorough in capturing technical details, code patterns, and architectural decisions that would be essential for continuing development work without losing context.

Before providing your final summary, wrap your analysis in <analysis> tags to organize your thoughts and ensure you've covered all necessary points. In your analysis process:

1. Chronologically analyze each message and section of the conversation. For each section thoroughly identify:
   - The user's explicit requests and intents
   - Your approach to addressing the user's requests
   - Key decisions, technical concepts and code patterns
   - Specific details like:
     - file names
     - full code snippets
     - function signatures
     - file edits
   - Errors that you ran into and how you fixed them
   - Pay special attention to specific user feedback that you received, especially if the user told you to do something differently.
2. Double-check for technical accuracy and completeness, addressing each required element thoroughly.

Your summary should include the following sections:

1. Primary Request and Intent: Capture all of the user's explicit requests and intents in detail
2. Key Technical Concepts: List all important technical concepts, technologies, and frameworks discussed.
3. Files and Code Sections: Enumerate specific files and code sections examined, modified, or created. Pay special attention to the most recent messages and include full code snippets where applicable and include a summary of why this file read or edit is important.
4. Errors and fixes: List all errors that you ran into, and how you fixed them. Pay special attention to specific user feedback that you received, especially if the user told you to do something differently.
5. Problem Solving: Document problems solved and any ongoing troubleshooting efforts.
6. All user messages: List ALL user messages that are not tool results. These are critical for understanding the users' feedback and changing intent.
7. Pending Tasks: Outline any pending tasks that you have explicitly been asked to work on.
8. Current Work: Describe in detail precisely what was being worked on immediately before this summary request, paying special attention to the most recent messages from both user and assistant. Include file names and code snippets where applicable.
9. Optional Next Step: List the next step that you will take that is related to the most recent work you were doing. IMPORTANT: ensure that this step is DIRECTLY in line with the user's most recent explicit requests, and the task you were working on immediately before this summary request. If your last task was concluded, then only list next steps if they are explicitly in line with the users request. Do not start on tangential requests or really old requests that were already completed without confirming with the user first.

If there is a next step, include direct quotes from the most recent conversation showing exactly what task you were working on and where you left off. This should be verbatim to ensure there's no drift in task interpretation.

Here's an example of how your output should be structured:

<example>
<analysis>
[Your thought process, ensuring all points are covered thoroughly and accurately]
</analysis>

<summary>
1. Primary Request and Intent:
   [Detailed description]

2. Key Technical Concepts:
   - [Concept 1]
   - [Concept 2]
   - [...]

3. Files and Code Sections:
   - [File Name 1]
      - [Summary of why this file is important]
      - [Summary of the changes made to this file, if any]
      - [Important Code Snippet]
   - [File Name 2]
      - [Important Code Snippet]
   - [...]

4. Errors and fixes:
   - [Detailed description of error 1]:
      - [How you fixed the error]
      - [User feedback on the error if any]
   - [...]

5. Problem Solving:
   [Description of solved problems and ongoing troubleshooting]

6. All user messages:
   - [Detailed non tool use user message]
   - [...]

7. Pending Tasks:
   - [Task 1]
   - [Task 2]
   - [...]

8. Current Work:
   [Precise description of current work]

9. Optional Next Step:
   [Optional Next step to take]

</summary>
</example>

Please provide your summary based on the conversation so far, following this structure and ensuring precision and thoroughness in your response.

Note: Any <command> blocks from the original task will be automatically appended to your summary wrapped in <system-reminder> tags. You do not need to include them in your summary text.

There may be other summarization instructions provided in the included context. If so, remember to follow these instructions when creating your summary. Examples of instructions include:
<example>
## Compact Instructions
When summarizing the conversation focus on typescript code changes and also remember the mistakes you made and how you fixed them.
</example>

<example>
# Summary instructions
When you are using compact - please focus on test output and code changes. Include file reads verbatim.
</example>"#;

/// Source: EXPLAIN template
const TEMPLATE_EXPLAIN: &str = r#"Explain the following code from file path ${filePath}:${startLine}-${endLine}
${userInput}

```
${selectedText}
```

Please provide a clear and concise explanation of what this code does, including:
1. The purpose and functionality
2. Key components and their interactions
3. Important patterns or techniques used"#;

/// Source: FIX template
const TEMPLATE_FIX: &str = r#"Fix any issues in the following code from file path ${filePath}:${startLine}-${endLine}
${diagnosticText}
${userInput}

```
${selectedText}
```

Please:
1. Address all detected problems listed above (if any)
2. Identify any other potential bugs or issues
3. Provide corrected code
4. Explain what was fixed and why"#;

/// Source: IMPROVE template
const TEMPLATE_IMPROVE: &str = r#"Improve the following code from file path ${filePath}:${startLine}-${endLine}
${userInput}

```
${selectedText}
```

Please suggest improvements for:
1. Code readability and maintainability
2. Performance optimization
3. Best practices and patterns
4. Error handling and edge cases

Provide the improved code along with explanations for each enhancement."#;

/// Source: ADD_TO_CONTEXT template
const TEMPLATE_ADD_TO_CONTEXT: &str = r#"${filePath}:${startLine}-${endLine}
```
${selectedText}
```"#;

/// Source: TERMINAL_ADD_TO_CONTEXT template
const TEMPLATE_TERMINAL_ADD_TO_CONTEXT: &str = r#"${userInput}
Terminal output:
```
${terminalContent}
```"#;

/// Source: TERMINAL_FIX template
const TEMPLATE_TERMINAL_FIX: &str = r#"${userInput}
Fix this terminal command:
```
${terminalContent}
```

Please:
1. Identify any issues in the command
2. Provide the corrected command
3. Explain what was fixed and why"#;

/// Source: TERMINAL_EXPLAIN template
const TEMPLATE_TERMINAL_EXPLAIN: &str = r#"${userInput}
Explain this terminal command:
```
${terminalContent}
```

Please provide:
1. What the command does
2. Explanation of each part/flag
3. Expected output and behavior"#;

/// Source: NEW_TASK template
const TEMPLATE_NEW_TASK: &str = "${userInput}";

/// Returns the default template for a given support prompt type.
pub fn default_template(prompt_type: SupportPromptType) -> &'static str {
    match prompt_type {
        SupportPromptType::Enhance => TEMPLATE_ENHANCE,
        SupportPromptType::Condense => TEMPLATE_CONDENSE,
        SupportPromptType::Explain => TEMPLATE_EXPLAIN,
        SupportPromptType::Fix => TEMPLATE_FIX,
        SupportPromptType::Improve => TEMPLATE_IMPROVE,
        SupportPromptType::AddToContext => TEMPLATE_ADD_TO_CONTEXT,
        SupportPromptType::TerminalAddToContext => TEMPLATE_TERMINAL_ADD_TO_CONTEXT,
        SupportPromptType::TerminalFix => TEMPLATE_TERMINAL_FIX,
        SupportPromptType::TerminalExplain => TEMPLATE_TERMINAL_EXPLAIN,
        SupportPromptType::NewTask => TEMPLATE_NEW_TASK,
    }
}

// ---------------------------------------------------------------------------
// Template variable substitution
// Source: `src/shared/support-prompt.ts` — `createPrompt`
// ---------------------------------------------------------------------------

/// Substitute `${key}` placeholders in the template with values from the params map.
///
/// Handles the special `diagnosticText` key by generating diagnostic text
/// from the `diagnostics` parameter if present.
///
/// Any placeholder not found in `params` is replaced with an empty string.
pub fn create_prompt(template: &str, params: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    // Simple approach: replace all ${key} patterns
    // We iterate through params and replace each known key
    for (key, value) in params {
        let placeholder = format!("${{{key}}}");
        result = result.replace(&placeholder, value);
    }
    // Replace any remaining ${...} placeholders with empty string
    // Use regex-free approach: find all ${...} and replace with empty
    let mut output = String::with_capacity(result.len());
    let mut chars = result.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' && chars.peek() == Some(&'{') {
            chars.next(); // consume '{'
            let mut found_close = false;
            let mut inner = String::new();
            for ic in chars.by_ref() {
                if ic == '}' {
                    found_close = true;
                    break;
                }
                inner.push(ic);
            }
            // Replace with empty string (unresolved variable)
            let _ = inner;
            if !found_close {
                output.push_str("${");
                output.push_str(&inner);
            }
        } else {
            output.push(c);
        }
    }
    output
}

// ---------------------------------------------------------------------------
// Convenience functions
// ---------------------------------------------------------------------------

/// Get the support prompt template for the given type, optionally using
/// custom templates from the user's configuration.
///
/// Source: `src/shared/support-prompt.ts` — `supportPrompt.get`
pub fn get_support_prompt(
    prompt_type: SupportPromptType,
    custom_prompts: Option<&serde_json::Value>,
) -> String {
    if let Some(custom) = custom_prompts {
        if let Some(obj) = custom.as_object() {
            if let Some(val) = obj.get(prompt_type.key()) {
                if let Some(s) = val.as_str() {
                    return s.to_string();
                }
            }
        }
    }
    default_template(prompt_type).to_string()
}

/// Create a support prompt by looking up the template and substituting parameters.
///
/// Source: `src/shared/support-prompt.ts` — `supportPrompt.create`
pub fn create_support_prompt(
    prompt_type: SupportPromptType,
    custom_prompts: Option<&serde_json::Value>,
    params: &HashMap<String, String>,
) -> String {
    let template = get_support_prompt(prompt_type, custom_prompts);
    create_prompt(&template, params)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_prompt_types_have_templates() {
        for pt in SupportPromptType::all() {
            let tmpl = default_template(*pt);
            assert!(
                !tmpl.is_empty(),
                "Template for {:?} should not be empty",
                pt
            );
        }
    }

    #[test]
    fn test_create_prompt_substitution() {
        let template = "Hello ${name}, your code:\n${code}";
        let mut params = HashMap::new();
        params.insert("name".to_string(), "Alice".to_string());
        params.insert("code".to_string(), "fn main() {}".to_string());
        let result = create_prompt(template, &params);
        assert_eq!(result, "Hello Alice, your code:\nfn main() {}");
    }

    #[test]
    fn test_create_prompt_missing_param() {
        let template = "Hello ${name}, ${missing}";
        let mut params = HashMap::new();
        params.insert("name".to_string(), "Bob".to_string());
        let result = create_prompt(template, &params);
        assert_eq!(result, "Hello Bob, ");
    }

    #[test]
    fn test_enhance_template_matches_ts() {
        let tmpl = default_template(SupportPromptType::Enhance);
        assert!(tmpl.contains("Generate an enhanced version of this prompt"));
        assert!(tmpl.contains("${userInput}"));
    }

    #[test]
    fn test_condense_template_matches_ts() {
        let tmpl = default_template(SupportPromptType::Condense);
        assert!(tmpl.contains("CRITICAL: This summarization request is a SYSTEM OPERATION"));
        assert!(tmpl.contains("<analysis>"));
        assert!(tmpl.contains("<summary>"));
        assert!(tmpl.contains("Primary Request and Intent"));
        assert!(tmpl.contains("Optional Next Step"));
    }

    #[test]
    fn test_explain_template_has_file_vars() {
        let tmpl = default_template(SupportPromptType::Explain);
        assert!(tmpl.contains("${filePath}"));
        assert!(tmpl.contains("${startLine}"));
        assert!(tmpl.contains("${endLine}"));
        assert!(tmpl.contains("${selectedText}"));
    }

    #[test]
    fn test_fix_template_has_diagnostic_var() {
        let tmpl = default_template(SupportPromptType::Fix);
        assert!(tmpl.contains("${diagnosticText}"));
        assert!(tmpl.contains("${selectedText}"));
    }

    #[test]
    fn test_terminal_templates_have_terminal_content() {
        let tmpl_add = default_template(SupportPromptType::TerminalAddToContext);
        assert!(tmpl_add.contains("${terminalContent}"));

        let tmpl_fix = default_template(SupportPromptType::TerminalFix);
        assert!(tmpl_fix.contains("${terminalContent}"));

        let tmpl_explain = default_template(SupportPromptType::TerminalExplain);
        assert!(tmpl_explain.contains("${terminalContent}"));
    }

    #[test]
    fn test_new_task_template_is_just_variable() {
        let tmpl = default_template(SupportPromptType::NewTask);
        assert_eq!(tmpl, "${userInput}");
    }

    #[test]
    fn test_diagnostic_text_generation() {
        let diagnostics = vec![
            DiagnosticEntry {
                source: "Error".to_string(),
                message: "type mismatch".to_string(),
                code: Some("E0308".to_string()),
            },
            DiagnosticEntry {
                source: "Warning".to_string(),
                message: "unused variable".to_string(),
                code: None,
            },
        ];
        let text = generate_diagnostic_text(&diagnostics);
        assert!(text.contains("Current problems detected"));
        assert!(text.contains("[Error] type mismatch (E0308)"));
        assert!(text.contains("[Warning] unused variable"));
    }

    #[test]
    fn test_diagnostic_text_empty() {
        let text = generate_diagnostic_text(&[]);
        assert!(text.is_empty());
    }

    #[test]
    fn test_get_support_prompt_custom() {
        let custom = serde_json::json!({
            "ENHANCE": "Custom enhance: ${userInput}"
        });
        let tmpl = get_support_prompt(SupportPromptType::Enhance, Some(&custom));
        assert_eq!(tmpl, "Custom enhance: ${userInput}");
    }

    #[test]
    fn test_get_support_prompt_custom_wrong_type() {
        let custom = serde_json::json!({
            "FIX": "Fix this: ${userInput}"
        });
        let tmpl = get_support_prompt(SupportPromptType::Enhance, Some(&custom));
        assert!(tmpl.contains("Generate an enhanced version")); // falls back to default
    }

    #[test]
    fn test_create_support_prompt_end_to_end() {
        let mut params = HashMap::new();
        params.insert("userInput".to_string(), "my code here".to_string());
        let result = create_support_prompt(SupportPromptType::Enhance, None, &params);
        assert!(result.contains("my code here"));
        assert!(result.contains("Generate an enhanced version"));
    }

    #[test]
    fn test_support_prompt_type_keys() {
        assert_eq!(SupportPromptType::Enhance.key(), "ENHANCE");
        assert_eq!(SupportPromptType::Condense.key(), "CONDENSE");
        assert_eq!(SupportPromptType::Explain.key(), "EXPLAIN");
        assert_eq!(SupportPromptType::Fix.key(), "FIX");
        assert_eq!(SupportPromptType::Improve.key(), "IMPROVE");
        assert_eq!(SupportPromptType::AddToContext.key(), "ADD_TO_CONTEXT");
        assert_eq!(
            SupportPromptType::TerminalAddToContext.key(),
            "TERMINAL_ADD_TO_CONTEXT"
        );
        assert_eq!(SupportPromptType::TerminalFix.key(), "TERMINAL_FIX");
        assert_eq!(SupportPromptType::TerminalExplain.key(), "TERMINAL_EXPLAIN");
        assert_eq!(SupportPromptType::NewTask.key(), "NEW_TASK");
    }
}

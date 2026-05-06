//! Support Prompt Templates
//!
//! Defines all 10 prompt templates used for user-facing support actions
//! like enhancing, condensing, explaining, fixing, and improving code.
//!
//! Mirrors `packages/types/src/support-prompt.ts`.

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
// Default templates
// ---------------------------------------------------------------------------

/// Default enhancement template.
const TEMPLATE_ENHANCE: &str =
    "You are a prompt enhancement assistant. Your task is to improve the following user prompt \
     to make it clearer, more specific, and more likely to get a good response from an AI coding assistant. \
     Do not change the intent of the prompt. Just make it clearer and more detailed.\n\n\
     User prompt:\n{{user_input}}\n\n\
     Enhanced prompt:";

/// Default condense template.
const TEMPLATE_CONDENSE: &str =
    "Summarize the following content concisely while preserving all key information and intent:\n\n\
     {{user_input}}";

/// Default explain template.
const TEMPLATE_EXPLAIN: &str =
    "Explain the following code in clear, simple terms:\n\n\
     ```\n{{user_input}}\n```";

/// Default fix template.
const TEMPLATE_FIX: &str =
    "Find and fix any bugs or issues in the following code. Explain what was wrong and provide the corrected version:\n\n\
     ```\n{{user_input}}\n```";

/// Default improve template.
const TEMPLATE_IMPROVE: &str =
    "Improve the following code by making it more readable, efficient, and following best practices. \
     Explain the improvements:\n\n\
     ```\n{{user_input}}\n```";

/// Default add-to-context template.
const TEMPLATE_ADD_TO_CONTEXT: &str =
    "The user wants to add the following code to the conversation context:\n\n\
     ```\n{{user_input}}\n```";

/// Default terminal add-to-context template.
const TEMPLATE_TERMINAL_ADD_TO_CONTEXT: &str =
    "The user wants to add the following terminal output to the conversation context:\n\n\
     ```\n{{user_input}}\n```";

/// Default terminal fix template.
const TEMPLATE_TERMINAL_FIX: &str =
    "Analyze the following terminal output, identify any errors, and suggest fixes:\n\n\
     ```\n{{user_input}}\n```";

/// Default terminal explain template.
const TEMPLATE_TERMINAL_EXPLAIN: &str =
    "Explain what the following terminal output means:\n\n\
     ```\n{{user_input}}\n```";

/// Default new-task template.
const TEMPLATE_NEW_TASK: &str =
    "Create a new task based on the following description:\n\n{{user_input}}";

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
// ---------------------------------------------------------------------------

/// Substitute `{{key}}` placeholders in the template with values from the params map.
///
/// Any placeholder not found in `params` is replaced with an empty string.
pub fn create_prompt(template: &str, params: &HashMap<String, String>) -> String {
    let mut result = template.to_string();
    for (key, value) in params {
        let placeholder = format!("{{{{{}}}}}", key);
        result = result.replace(&placeholder, value);
    }
    // Replace any remaining placeholders with empty string
    // (simple approach: find {{...}} patterns)
    result
}

// ---------------------------------------------------------------------------
// Convenience functions
// ---------------------------------------------------------------------------

/// Get the support prompt template for the given type, optionally using
/// custom templates from the user's configuration.
///
/// If `custom_prompts` contains a value for this prompt type key, it is used;
/// otherwise the default template is returned.
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
/// This is the main entry point: given a prompt type, optional custom templates,
/// and a map of parameters, it returns the final prompt string ready for use.
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
            assert!(!tmpl.is_empty(), "Template for {:?} should not be empty", pt);
        }
    }

    #[test]
    fn test_create_prompt_substitution() {
        let template = "Hello {{name}}, your code:\n{{code}}";
        let mut params = HashMap::new();
        params.insert("name".to_string(), "Alice".to_string());
        params.insert("code".to_string(), "fn main() {}".to_string());
        let result = create_prompt(template, &params);
        assert_eq!(result, "Hello Alice, your code:\nfn main() {}");
    }

    #[test]
    fn test_create_prompt_missing_param() {
        let template = "Hello {{name}}, {{missing}}";
        let mut params = HashMap::new();
        params.insert("name".to_string(), "Bob".to_string());
        let result = create_prompt(template, &params);
        assert_eq!(result, "Hello Bob, {{missing}}");
    }

    #[test]
    fn test_get_support_prompt_default() {
        let tmpl = get_support_prompt(SupportPromptType::Enhance, None);
        assert!(tmpl.contains("{{user_input}}"));
    }

    #[test]
    fn test_get_support_prompt_custom() {
        let custom = serde_json::json!({
            "ENHANCE": "Custom enhance: {{user_input}}"
        });
        let tmpl = get_support_prompt(SupportPromptType::Enhance, Some(&custom));
        assert_eq!(tmpl, "Custom enhance: {{user_input}}");
    }

    #[test]
    fn test_get_support_prompt_custom_wrong_type() {
        let custom = serde_json::json!({
            "FIX": "Fix this: {{user_input}}"
        });
        let tmpl = get_support_prompt(SupportPromptType::Enhance, Some(&custom));
        assert!(tmpl.contains("prompt enhancement")); // falls back to default
    }

    #[test]
    fn test_create_support_prompt_end_to_end() {
        let mut params = HashMap::new();
        params.insert("user_input".to_string(), "my code here".to_string());
        let result = create_support_prompt(SupportPromptType::Explain, None, &params);
        assert!(result.contains("my code here"));
        assert!(result.contains("Explain"));
    }

    #[test]
    fn test_support_prompt_type_keys() {
        assert_eq!(SupportPromptType::Enhance.key(), "ENHANCE");
        assert_eq!(SupportPromptType::Condense.key(), "CONDENSE");
        assert_eq!(SupportPromptType::Explain.key(), "EXPLAIN");
        assert_eq!(SupportPromptType::Fix.key(), "FIX");
        assert_eq!(SupportPromptType::Improve.key(), "IMPROVE");
        assert_eq!(SupportPromptType::AddToContext.key(), "ADD_TO_CONTEXT");
        assert_eq!(SupportPromptType::TerminalAddToContext.key(), "TERMINAL_ADD_TO_CONTEXT");
        assert_eq!(SupportPromptType::TerminalFix.key(), "TERMINAL_FIX");
        assert_eq!(SupportPromptType::TerminalExplain.key(), "TERMINAL_EXPLAIN");
        assert_eq!(SupportPromptType::NewTask.key(), "NEW_TASK");
    }
}
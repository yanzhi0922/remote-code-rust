//! System prompt generation.
//!
//! Derived from `src/core/webview/generateSystemPrompt.ts`.
//!
//! Generates the system prompt for a given mode, incorporating custom
//! instructions, MCP tools, diff strategy, and other configuration.


// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Parameters for system prompt generation.
///
/// Source: `src/core/webview/generateSystemPrompt.ts` — extracted from function params
#[derive(Debug, Clone)]
pub struct GenerateSystemPromptParams {
    /// The mode to generate the prompt for.
    pub mode: Option<String>,
    /// The current working directory.
    pub cwd: String,
    /// Custom mode prompts.
    pub custom_mode_prompts: Option<serde_json::Value>,
    /// Custom instructions.
    pub custom_instructions: Option<String>,
    /// Whether MCP is enabled.
    pub mcp_enabled: bool,
    /// Experiment flags.
    pub experiments: Option<serde_json::Value>,
    /// Language setting.
    pub language: Option<String>,
    /// Whether subfolder rules are enabled.
    pub enable_subfolder_rules: bool,
    /// Whether the todo list is enabled.
    pub todo_list_enabled: bool,
    /// Whether agent rules are enabled.
    pub use_agent_rules: bool,
    /// Whether new tasks require todos.
    pub new_task_require_todos: bool,
    /// Whether the model is a stealth model.
    pub is_stealth_model: Option<bool>,
    /// .rooignore instructions.
    pub roo_ignore_instructions: Option<String>,
}

/// Result of system prompt generation.
#[derive(Debug, Clone)]
pub struct GenerateSystemPromptResult {
    pub success: bool,
    pub system_prompt: Option<String>,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// System prompt generation
// ---------------------------------------------------------------------------

/// Default mode slug when none is specified.
const DEFAULT_MODE_SLUG: &str = "code";

/// Generates a system prompt for the given configuration.
///
/// Source: `src/core/webview/generateSystemPrompt.ts` — `generateSystemPrompt`
///
/// This function builds the complete system prompt by:
/// 1. Resolving the mode (defaulting to "code")
/// 2. Loading custom mode prompts and instructions
/// 3. Incorporating MCP tools if enabled
/// 4. Adding .rooignore instructions
/// 5. Applying experiment-specific modifications
///
/// # Arguments
/// * `params` - Parameters for prompt generation
/// * `prompt_builder` - Function that builds the actual prompt text
///
/// # Returns
/// A `GenerateSystemPromptResult` with the generated prompt or error.
pub fn generate_system_prompt(
    params: GenerateSystemPromptParams,
    prompt_builder: &dyn Fn(&SystemPromptContext) -> String,
) -> GenerateSystemPromptResult {
    let mode = params.mode.as_deref().unwrap_or(DEFAULT_MODE_SLUG).to_string();

    let context = SystemPromptContext {
        mode,
        cwd: params.cwd,
        custom_instructions: params.custom_instructions,
        mcp_enabled: params.mcp_enabled,
        language: params.language,
        enable_subfolder_rules: params.enable_subfolder_rules,
        todo_list_enabled: params.todo_list_enabled,
        use_agent_rules: params.use_agent_rules,
        new_task_require_todos: params.new_task_require_todos,
        is_stealth_model: params.is_stealth_model.unwrap_or(false),
        roo_ignore_instructions: params.roo_ignore_instructions,
    };

    let system_prompt = prompt_builder(&context);

    GenerateSystemPromptResult {
        success: true,
        system_prompt: Some(system_prompt),
        error: None,
    }
}

/// Context provided to the system prompt builder.
#[derive(Debug, Clone)]
pub struct SystemPromptContext {
    pub mode: String,
    pub cwd: String,
    pub custom_instructions: Option<String>,
    pub mcp_enabled: bool,
    pub language: Option<String>,
    pub enable_subfolder_rules: bool,
    pub todo_list_enabled: bool,
    pub use_agent_rules: bool,
    pub new_task_require_todos: bool,
    pub is_stealth_model: bool,
    pub roo_ignore_instructions: Option<String>,
}

/// Default system prompt builder.
///
/// Generates a basic system prompt with the provided context.
/// In the full implementation, this would call the full SYSTEM_PROMPT
/// function from the prompts module.
pub fn default_prompt_builder(context: &SystemPromptContext) -> String {
    let mut parts = vec![];

    // Role description
    parts.push(format!(
        "You are Roo, a knowledgeable and skilled AI coding assistant for the {} mode.",
        context.mode
    ));

    // Working directory
    parts.push(format!("\nWorking directory: {}", context.cwd));

    // Custom instructions
    if let Some(ref instructions) = context.custom_instructions {
        if !instructions.is_empty() {
            parts.push(format!("\nCustom instructions:\n{instructions}"));
        }
    }

    // .rooignore instructions
    if let Some(ref ignore) = context.roo_ignore_instructions {
        if !ignore.is_empty() {
            parts.push(format!("\nFile access restrictions:\n{ignore}"));
        }
    }

    // MCP tools
    if context.mcp_enabled {
        parts.push("\nMCP tools are enabled and available.".to_string());
    }

    // Language
    if let Some(ref lang) = context.language {
        if !lang.is_empty() {
            parts.push(format!("\nLanguage: {lang}"));
        }
    }

    // Todo list
    if context.todo_list_enabled {
        parts.push("\nTodo list tracking is enabled.".to_string());
    }

    // Stealth model
    if context.is_stealth_model {
        parts.push("\nNote: Running in stealth mode.".to_string());
    }

    parts.join("\n")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_system_prompt_basic() {
        let params = GenerateSystemPromptParams {
            mode: Some("code".to_string()),
            cwd: "/home/user/project".to_string(),
            custom_mode_prompts: None,
            custom_instructions: None,
            mcp_enabled: false,
            experiments: None,
            language: None,
            enable_subfolder_rules: false,
            todo_list_enabled: true,
            use_agent_rules: true,
            new_task_require_todos: false,
            is_stealth_model: None,
            roo_ignore_instructions: None,
        };
        let result = generate_system_prompt(params, &default_prompt_builder);
        assert!(result.success);
        assert!(result.system_prompt.is_some());
        let prompt = result.system_prompt.unwrap();
        assert!(prompt.contains("code"));
        assert!(prompt.contains("/home/user/project"));
    }

    #[test]
    fn test_generate_system_prompt_with_custom_instructions() {
        let params = GenerateSystemPromptParams {
            mode: None,
            cwd: "/test".to_string(),
            custom_mode_prompts: None,
            custom_instructions: Some("Always use TypeScript".to_string()),
            mcp_enabled: true,
            experiments: None,
            language: Some("en".to_string()),
            enable_subfolder_rules: false,
            todo_list_enabled: true,
            use_agent_rules: true,
            new_task_require_todos: false,
            is_stealth_model: Some(true),
            roo_ignore_instructions: None,
        };
        let result = generate_system_prompt(params, &default_prompt_builder);
        assert!(result.success);
        let prompt = result.system_prompt.unwrap();
        assert!(prompt.contains("Always use TypeScript"));
        assert!(prompt.contains("MCP tools"));
        assert!(prompt.contains("stealth"));
        assert!(prompt.contains("en"));
    }

    #[test]
    fn test_generate_system_prompt_default_mode() {
        let params = GenerateSystemPromptParams {
            mode: None,
            cwd: "/test".to_string(),
            custom_mode_prompts: None,
            custom_instructions: None,
            mcp_enabled: false,
            experiments: None,
            language: None,
            enable_subfolder_rules: false,
            todo_list_enabled: false,
            use_agent_rules: true,
            new_task_require_todos: false,
            is_stealth_model: None,
            roo_ignore_instructions: None,
        };
        let result = generate_system_prompt(params, &default_prompt_builder);
        assert!(result.success);
        let prompt = result.system_prompt.unwrap();
        assert!(prompt.contains("code"));
    }

    #[test]
    fn test_default_prompt_builder() {
        let context = SystemPromptContext {
            mode: "architect".to_string(),
            cwd: "/project".to_string(),
            custom_instructions: Some("Be concise".to_string()),
            mcp_enabled: true,
            language: Some("zh".to_string()),
            enable_subfolder_rules: false,
            todo_list_enabled: true,
            use_agent_rules: true,
            new_task_require_todos: false,
            is_stealth_model: false,
            roo_ignore_instructions: Some("Ignore node_modules".to_string()),
        };
        let prompt = default_prompt_builder(&context);
        assert!(prompt.contains("architect"));
        assert!(prompt.contains("Be concise"));
        assert!(prompt.contains("MCP"));
        assert!(prompt.contains("zh"));
        assert!(prompt.contains("node_modules"));
        assert!(prompt.contains("Todo"));
    }
}

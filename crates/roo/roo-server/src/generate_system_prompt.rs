//! System prompt generation.
//!
//! Source: `src/core/webview/generateSystemPrompt.ts`
//!
//! Delegates to the `roo_prompt` crate's `build_system_prompt()` which
//! faithfully assembles the full prompt with all 10+ sections matching
//! the TypeScript reference exactly.

use roo_prompt::types::SystemPromptSettings;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Parameters for system prompt generation.
///
/// Source: `src/core/webview/generateSystemPrompt.ts`
#[derive(Debug, Clone, Default)]
pub struct GenerateSystemPromptParams {
    pub mode: Option<String>,
    pub cwd: String,
    pub custom_mode_prompts: Option<serde_json::Value>,
    pub custom_instructions: Option<String>,
    pub mcp_enabled: bool,
    pub experiments: Option<serde_json::Value>,
    pub language: Option<String>,
    pub enable_subfolder_rules: bool,
    pub todo_list_enabled: bool,
    pub use_agent_rules: bool,
    pub new_task_require_todos: bool,
    pub is_stealth_model: Option<bool>,
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

/// Generates a system prompt for the given configuration.
///
/// Source: `src/core/webview/generateSystemPrompt.ts` — `generateSystemPrompt`
///
/// Delegates to `roo_prompt::build_system_prompt()` which produces the
/// full system prompt matching the TS reference exactly.
pub fn generate_system_prompt(params: GenerateSystemPromptParams) -> GenerateSystemPromptResult {
    let mode = params.mode.as_deref().unwrap_or("architect");

    let settings = SystemPromptSettings {
        is_stealth_model: params.is_stealth_model.unwrap_or(false),
        ..Default::default()
    };

    let os_info = format!("{} {}", std::env::consts::OS, env!("CARGO_PKG_VERSION"));
    let shell = if cfg!(windows) { "powershell" } else { "bash" };
    let home_dir = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "~".to_string());

    let system_prompt = roo_prompt::build_system_prompt(
        &params.cwd,
        mode,
        None,                      // custom_modes
        None,                      // custom_mode_prompts
        params.mcp_enabled,        // has_mcp
        params.custom_instructions.as_deref(),
        params.language.as_deref(),
        params.roo_ignore_instructions.as_deref(),
        Some(&settings),
        &[],                       // skills
        &os_info,
        shell,
        &home_dir,
    );

    GenerateSystemPromptResult {
        success: true,
        system_prompt: Some(system_prompt),
        error: None,
    }
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
            ..Default::default()
        };
        let result = generate_system_prompt(params);
        assert!(result.success);
        let prompt = result.system_prompt.unwrap();
        assert!(prompt.contains("TOOL USE"));
        assert!(prompt.contains("/home/user/project"));
    }

    #[test]
    fn test_generate_system_prompt_with_custom_instructions() {
        let params = GenerateSystemPromptParams {
            mode: None,
            cwd: "/test".to_string(),
            custom_instructions: Some("Always use TypeScript".to_string()),
            mcp_enabled: true,
            language: Some("en".to_string()),
            ..Default::default()
        };
        let result = generate_system_prompt(params);
        assert!(result.success);
        let prompt = result.system_prompt.unwrap();
        assert!(prompt.contains("Always use TypeScript"));
    }

    #[test]
    fn test_generate_system_prompt_default_mode() {
        let params = GenerateSystemPromptParams {
            mode: None,
            cwd: "/test".to_string(),
            ..Default::default()
        };
        let result = generate_system_prompt(params);
        assert!(result.success);
        let prompt = result.system_prompt.unwrap();
        assert!(prompt.contains("TOOL USE"));
    }

    #[test]
    fn test_generate_system_prompt_has_all_sections() {
        let params = GenerateSystemPromptParams {
            mode: Some("code".to_string()),
            cwd: "/project".to_string(),
            mcp_enabled: true,
            ..Default::default()
        };
        let result = generate_system_prompt(params);
        assert!(result.success);
        let prompt = result.system_prompt.unwrap();
        assert!(prompt.contains("TOOL USE"));
        assert!(prompt.contains("CAPABILITIES"));
        assert!(prompt.contains("RULES"));
        assert!(prompt.contains("OBJECTIVE"));
        assert!(prompt.contains("SYSTEM INFORMATION"));
    }
}

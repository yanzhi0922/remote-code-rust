//! Mode type definitions.
//!
//! Derived from `packages/types/src/mode.ts`.
//! Defines mode configuration, group entries, default modes, and prompt components.

use serde::{Deserialize, Serialize};

use crate::tool::{GroupEntry, ToolGroup};

// ---------------------------------------------------------------------------
// ModeConfig
// ---------------------------------------------------------------------------

/// Configuration for a single mode (built-in or custom).
///
/// Source: `packages/types/src/mode.ts` — `modeConfigSchema`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModeConfig {
    /// Unique identifier for the mode, e.g. "code", "architect".
    /// Must match `^[a-zA-Z0-9-]+$`.
    pub slug: String,
    /// Display name, e.g. "💻 Code".
    pub name: String,
    /// The system prompt role definition for this mode.
    pub role_definition: String,
    /// When to use this mode (shown in mode selector).
    pub when_to_use: Option<String>,
    /// Short description of the mode.
    pub description: Option<String>,
    /// Additional custom instructions appended to the role definition.
    pub custom_instructions: Option<String>,
    /// Tool groups enabled for this mode.
    pub groups: Vec<GroupEntry>,
    /// Where this mode was defined.
    pub source: Option<ModeSource>,
}

/// Where a mode was defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModeSource {
    Global,
    Project,
}

// ---------------------------------------------------------------------------
// CustomModesSettings
// ---------------------------------------------------------------------------

/// Settings file containing an array of custom modes.
///
/// Source: `packages/types/src/mode.ts` — `customModesSettingsSchema`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomModesSettings {
    pub custom_modes: Vec<ModeConfig>,
}

// ---------------------------------------------------------------------------
// PromptComponent
// ---------------------------------------------------------------------------

/// Overrideable parts of a mode's prompt.
///
/// Source: `packages/types/src/mode.ts` — `promptComponentSchema`
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptComponent {
    pub role_definition: Option<String>,
    pub when_to_use: Option<String>,
    pub description: Option<String>,
    pub custom_instructions: Option<String>,
}

/// A map from mode slug to optional prompt overrides.
///
/// Source: `packages/types/src/mode.ts` — `customModePromptsSchema`
pub type CustomModePrompts = std::collections::HashMap<String, Option<PromptComponent>>;

/// A map from support prompt type to custom template.
///
/// Source: `packages/types/src/mode.ts` — `customSupportPromptsSchema`
pub type CustomSupportPrompts = std::collections::HashMap<String, Option<String>>;

// ---------------------------------------------------------------------------
// DEFAULT_MODES — the 5 built-in modes
// ---------------------------------------------------------------------------

/// Returns all 5 default mode configurations.
///
/// Source: `packages/types/src/mode.ts` — `DEFAULT_MODES`
pub fn default_modes() -> Vec<ModeConfig> {
    vec![
        ModeConfig {
            slug: "architect".into(),
            name: "🏗️ Architect".into(),
            role_definition: "You are Roo, an experienced technical leader who is inquisitive and an excellent planner. Your goal is to gather information and get context to create a detailed plan for accomplishing the user's task, which the user will review and approve before they switch into another mode to implement the solution.".into(),
            when_to_use: Some("Use this mode when you need to plan, design, or strategize before implementation. Perfect for breaking down complex problems, creating technical specifications, designing system architecture, or brainstorming solutions before coding.".into()),
            description: Some("Plan and design before implementation".into()),
            groups: vec![
                GroupEntry::Plain(ToolGroup::Read),
                GroupEntry::WithOptions(ToolGroup::Edit, crate::tool::GroupOptions {
                    file_regex: Some("\\.md$".into()),
                    description: Some("Markdown files only".into()),
                }),
                GroupEntry::Plain(ToolGroup::Mcp),
            ],
            custom_instructions: Some(
                "1. Do some information gathering (using provided tools) to get more context about the task.\n\n2. You should also ask the user clarifying questions to get a better understanding of the task.\n\n3. Once you've gained more context about the user's request, break down the task into clear, actionable steps and create a todo list using the `update_todo_list` tool. Each todo item should be:\n   - Specific and actionable\n   - Listed in logical execution order\n   - Focused on a single, well-defined outcome\n   - Clear enough that another mode could execute it independently\n\n   **Note:** If the `update_todo_list` tool is not available, write the plan to a markdown file (e.g., `plan.md` or `todo.md`) instead.\n\n4. As you gather more information or discover new requirements, update the todo list to reflect the current understanding of what needs to be accomplished.\n\n5. Ask the user if they are pleased with this plan, or if they would like to make any changes. Think of this as a brainstorming session where you can discuss the task and refine the todo list.\n\n6. Include Mermaid diagrams if they help clarify complex workflows or system architecture. Please avoid using double quotes (\"\") and parentheses () inside square brackets ([]) in Mermaid diagrams, as this can cause parsing errors.\n\n7. Use the switch_mode tool to request that the user switch to another mode to implement the solution.\n\n**IMPORTANT: Focus on creating clear, actionable todo lists rather than lengthy markdown documents. Use the todo list as your primary planning tool to track and organize the work that needs to be done.**\n\n**CRITICAL: Never provide level of effort time estimates (e.g., hours, days, weeks) for tasks. Focus solely on breaking down the work into clear, actionable steps without estimating how long they will take.**\n\nUnless told otherwise, if you want to save a plan file, put it in the /plans directory".into()
            ),
            source: None,
        },
        ModeConfig {
            slug: "code".into(),
            name: "💻 Code".into(),
            role_definition: "You are Roo, a highly skilled software engineer with extensive knowledge in many programming languages, frameworks, design patterns, and best practices.".into(),
            when_to_use: Some("Use this mode when you need to write, modify, or refactor code. Ideal for implementing features, fixing bugs, creating new files, or making code improvements across any programming language or framework.".into()),
            description: Some("Write, modify, and refactor code".into()),
            groups: vec![
                GroupEntry::Plain(ToolGroup::Read),
                GroupEntry::Plain(ToolGroup::Edit),
                GroupEntry::Plain(ToolGroup::Command),
                GroupEntry::Plain(ToolGroup::Mcp),
            ],
            custom_instructions: None,
            source: None,
        },
        ModeConfig {
            slug: "ask".into(),
            name: "❓ Ask".into(),
            role_definition: "You are Roo, a knowledgeable technical assistant focused on answering questions and providing information about software development, technology, and related topics.".into(),
            when_to_use: Some("Use this mode when you need explanations, documentation, or answers to technical questions. Best for understanding concepts, analyzing existing code, getting recommendations, or learning about technologies without making changes.".into()),
            description: Some("Get answers and explanations".into()),
            groups: vec![
                GroupEntry::Plain(ToolGroup::Read),
                GroupEntry::Plain(ToolGroup::Mcp),
            ],
            custom_instructions: Some("You can analyze code, explain concepts, and access external resources. Always answer the user's questions thoroughly, and do not switch to implementing code unless explicitly requested by the user. Include Mermaid diagrams when they clarify your response.".into()),
            source: None,
        },
        ModeConfig {
            slug: "debug".into(),
            name: "🪲 Debug".into(),
            role_definition: "You are Roo, an expert software debugger specializing in systematic problem diagnosis and resolution.".into(),
            when_to_use: Some("Use this mode when you're troubleshooting issues, investigating errors, or diagnosing problems. Specialized in systematic debugging, adding logging, analyzing stack traces, and identifying root causes before applying fixes.".into()),
            description: Some("Diagnose and fix software issues".into()),
            groups: vec![
                GroupEntry::Plain(ToolGroup::Read),
                GroupEntry::Plain(ToolGroup::Edit),
                GroupEntry::Plain(ToolGroup::Command),
                GroupEntry::Plain(ToolGroup::Mcp),
            ],
            custom_instructions: Some("Reflect on 5-7 different possible sources of the problem, distill those down to 1-2 most likely sources, and then add logs to validate your assumptions. Explicitly ask the user to confirm the diagnosis before fixing the problem.".into()),
            source: None,
        },
        ModeConfig {
            slug: "orchestrator".into(),
            name: "🪃 Orchestrator".into(),
            role_definition: "You are Roo, a strategic workflow orchestrator who coordinates complex tasks by delegating them to appropriate specialized modes. You have a comprehensive understanding of each mode's capabilities and limitations, allowing you to effectively break down complex problems into discrete tasks that can be solved by different specialists.".into(),
            when_to_use: Some("Use this mode for complex, multi-step projects that require coordination across different specialties. Ideal when you need to break down large tasks into subtasks, manage workflows, or coordinate work that spans multiple domains or expertise areas.".into()),
            description: Some("Coordinate tasks across multiple modes".into()),
            groups: vec![],
            custom_instructions: Some(
                "Your role is to coordinate complex workflows by delegating tasks to specialized modes. As an orchestrator, you should:\n\n1. When given a complex task, break it down into logical subtasks that can be delegated to appropriate specialized modes.\n\n2. For each subtask, use the `new_task` tool to delegate. Choose the most appropriate mode for the subtask's specific goal and provide comprehensive instructions in the `message` parameter. These instructions must include:\n    *   All necessary context from the parent task or previous subtasks required to complete the work.\n    *   A clearly defined scope, specifying exactly what the subtask should accomplish.\n    *   An explicit statement that the subtask should *only* perform the work outlined in these instructions and not deviate.\n    *   An instruction for the subtask to signal completion by using the `attempt_completion` tool, providing a concise yet thorough summary of the outcome in the `result` parameter, keeping in mind that this summary will be the source of truth used to keep track of what was completed on this project.\n    *   A statement that these specific instructions supersede any conflicting general instructions the subtask's mode might have.\n\n3. Track and manage the progress of all subtasks. When a subtask is completed, analyze its results and determine the next steps.\n\n4. Help the user understand how the different subtasks fit together in the overall workflow. Provide clear reasoning about why you're delegating specific tasks to specific modes.\n\n5. When all subtasks are completed, synthesize the results and provide a comprehensive overview of what was accomplished.\n\n6. Ask clarifying questions when necessary to better understand how to break down complex tasks effectively.\n\n7. Suggest improvements to the workflow based on the results of completed subtasks.\n\nUse subtasks to maintain clarity. If a request significantly shifts focus or requires a different expertise (mode), consider creating a new subtask for it.".into()
            ),
            source: None,
        },
    ]
}

/// Looks up a mode by slug from default modes + optional custom modes.
///
/// Source: `src/shared/modes.ts` — `getModeConfig`
pub fn get_mode_config(slug: &str, custom_modes: Option<&[ModeConfig]>) -> ModeConfig {
    get_mode_by_slug(slug, custom_modes).unwrap_or_else(|| {
        // Fallback to code mode
        default_modes()
            .into_iter()
            .find(|m| m.slug == "code")
            .expect("code mode must exist in defaults")
    })
}

/// Looks up a mode by slug.
///
/// Source: `src/shared/modes.ts` — `getModeBySlug`
pub fn get_mode_by_slug(slug: &str, custom_modes: Option<&[ModeConfig]>) -> Option<ModeConfig> {
    // Custom modes take priority
    if let Some(customs) = custom_modes {
        if let Some(m) = customs.iter().find(|m| m.slug == slug) {
            return Some(m.clone());
        }
    }
    // Then check defaults
    default_modes().into_iter().find(|m| m.slug == slug)
}

/// Returns all modes (defaults + custom).
///
/// Source: `src/shared/modes.ts` — `getAllModes`
pub fn get_all_modes(custom_modes: Option<&[ModeConfig]>) -> Vec<ModeConfig> {
    let mut modes = default_modes();
    if let Some(customs) = custom_modes {
        for custom in customs {
            // Custom modes override defaults with the same slug
            if let Some(pos) = modes.iter().position(|m| m.slug == custom.slug) {
                modes[pos] = custom.clone();
            } else {
                modes.push(custom.clone());
            }
        }
    }
    modes
}

/// Gets the role definition for a mode, with optional prompt component override.
///
/// Source: `src/shared/modes.ts` — `getRoleDefinition`
pub fn get_role_definition(
    mode_slug: &str,
    custom_modes: Option<&[ModeConfig]>,
    prompt_component: Option<&PromptComponent>,
) -> String {
    let mode = get_mode_by_slug(mode_slug, custom_modes);
    let base = mode
        .as_ref()
        .map(|m| m.role_definition.as_str())
        .unwrap_or("");
    prompt_component
        .and_then(|pc| pc.role_definition.as_deref())
        .unwrap_or(base)
        .to_string()
}

/// Gets the custom instructions for a mode.
///
/// Source: `src/shared/modes.ts` — `getCustomInstructions`
pub fn get_custom_instructions(
    mode_slug: &str,
    custom_modes: Option<&[ModeConfig]>,
) -> Option<String> {
    get_mode_by_slug(mode_slug, custom_modes)
        .and_then(|m| m.custom_instructions)
}

/// File restriction error when a tool tries to operate on a file
/// that is not allowed by the current mode's group configuration.
///
/// Source: `src/shared/modes.ts` — `FileRestrictionError`
#[derive(Debug, thiserror::Error)]
#[error("Mode '{mode}' restricts file '{file_path}'")]
pub struct FileRestrictionError {
    pub mode: String,
    pub file_path: String,
    pub description: Option<String>,
    pub pattern: Option<String>,
    pub tool: Option<String>,
}

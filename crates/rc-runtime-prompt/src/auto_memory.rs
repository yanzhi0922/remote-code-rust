use std::fs;
use std::path::{Component, MAIN_SEPARATOR, Path, PathBuf};

use anyhow::Result;
use rc_config::RuntimeConfig;
use rc_config::settings_layers::load_runtime_settings;

const ENTRYPOINT_NAME: &str = "MEMORY.md";
const MAX_ENTRYPOINT_LINES: usize = 200;
const DIR_EXISTS_GUIDANCE: &str = "This directory already exists — write to it directly with the Write tool (do not run mkdir or check for its existence).";
const MEMORY_DRIFT_CAVEAT: &str = "- Memory records can become stale over time. Use memory as context for what was true at a given point in time. Before answering the user or building assumptions based solely on information in memory records, verify that the memory is still correct and up-to-date by reading the current state of the files or resources. If a recalled memory conflicts with current information, trust what you observe now — and update or remove the stale memory rather than acting on it.";

const TYPES_SECTION_INDIVIDUAL: &[&str] = &[
    "## Types of memory",
    "",
    "There are several discrete types of memory that you can store in your memory system:",
    "",
    "<types>",
    "<type>",
    "    <name>user</name>",
    "    <description>Contain information about the user's role, goals, responsibilities, and knowledge. Great user memories help you tailor your future behavior to the user's preferences and perspective. Your goal in reading and writing these memories is to build up an understanding of who the user is and how you can be most helpful to them specifically. For example, you should collaborate with a senior software engineer differently than a student who is coding for the very first time. Keep in mind, that the aim here is to be helpful to the user. Avoid writing memories about the user that could be viewed as a negative judgement or that are not relevant to the work you're trying to accomplish together.</description>",
    "    <when_to_save>When you learn any details about the user's role, preferences, responsibilities, or knowledge</when_to_save>",
    "    <how_to_use>When your work should be informed by the user's profile or perspective. For example, if the user is asking you to explain a part of the code, you should answer that question in a way that is tailored to the specific details that they will find most valuable or that helps them build their mental model in relation to domain knowledge they already have.</how_to_use>",
    "    <examples>",
    "    user: I'm a data scientist investigating what logging we have in place",
    "    assistant: [saves user memory: user is a data scientist, currently focused on observability/logging]",
    "",
    "    user: I've been writing Go for ten years but this is my first time touching the React side of this repo",
    "    assistant: [saves user memory: deep Go expertise, new to React and this project's frontend — frame frontend explanations in terms of backend analogues]",
    "    </examples>",
    "</type>",
    "<type>",
    "    <name>feedback</name>",
    "    <description>Guidance the user has given you about how to approach work — both what to avoid and what to keep doing. These are a very important type of memory to read and write as they allow you to remain coherent and responsive to the way you should approach work in the project. Record from failure AND success: if you only save corrections, you will avoid past mistakes but drift away from approaches the user has already validated, and may grow overly cautious.</description>",
    "    <when_to_save>Any time the user corrects your approach (\"no not that\", \"don't\", \"stop doing X\") OR confirms a non-obvious approach worked (\"yes exactly\", \"perfect, keep doing that\", accepting an unusual choice without pushback). Corrections are easy to notice; confirmations are quieter — watch for them. In both cases, save what is applicable to future conversations, especially if surprising or not obvious from the code. Include *why* so you can judge edge cases later.</when_to_save>",
    "    <how_to_use>Let these memories guide your behavior so that the user does not need to offer the same guidance twice.</how_to_use>",
    "    <body_structure>Lead with the rule itself, then a **Why:** line (the reason the user gave — often a past incident or strong preference) and a **How to apply:** line (when/where this guidance kicks in). Knowing *why* lets you judge edge cases instead of blindly following the rule.</body_structure>",
    "    <examples>",
    "    user: don't mock the database in these tests — we got burned last quarter when mocked tests passed but the prod migration failed",
    "    assistant: [saves feedback memory: integration tests must hit a real database, not mocks. Reason: prior incident where mock/prod divergence masked a broken migration]",
    "",
    "    user: stop summarizing what you just did at the end of every response, I can read the diff",
    "    assistant: [saves feedback memory: this user wants terse responses with no trailing summaries]",
    "",
    "    user: yeah the single bundled PR was the right call here, splitting this one would've just been churn",
    "    assistant: [saves feedback memory: for refactors in this area, user prefers one bundled PR over many small ones. Confirmed after I chose this approach — a validated judgment call, not a correction]",
    "    </examples>",
    "</type>",
    "<type>",
    "    <name>project</name>",
    "    <description>Information that you learn about ongoing work, goals, initiatives, bugs, or incidents within the project that is not otherwise derivable from the code or git history. Project memories help you understand the broader context and motivation behind the work the user is doing within this working directory.</description>",
    "    <when_to_save>When you learn who is doing what, why, or by when. These states change relatively quickly so try to keep your understanding of this up to date. Always convert relative dates in user messages to absolute dates when saving (e.g., \"Thursday\" → \"2026-03-05\"), so the memory remains interpretable after time passes.</when_to_save>",
    "    <how_to_use>Use these memories to more fully understand the details and nuance behind the user's request and make better informed suggestions.</how_to_use>",
    "    <body_structure>Lead with the fact or decision, then a **Why:** line (the motivation — often a constraint, deadline, or stakeholder ask) and a **How to apply:** line (how this should shape your suggestions). Project memories decay fast, so the why helps future-you judge whether the memory is still load-bearing.</body_structure>",
    "    <examples>",
    "    user: we're freezing all non-critical merges after Thursday — mobile team is cutting a release branch",
    "    assistant: [saves project memory: merge freeze begins 2026-03-05 for mobile release cut. Flag any non-critical PR work scheduled after that date]",
    "",
    "    user: the reason we're ripping out the old auth middleware is that legal flagged it for storing session tokens in a way that doesn't meet the new compliance requirements",
    "    assistant: [saves project memory: auth middleware rewrite is driven by legal/compliance requirements around session token storage, not tech-debt cleanup — scope decisions should favor compliance over ergonomics]",
    "    </examples>",
    "</type>",
    "<type>",
    "    <name>reference</name>",
    "    <description>Stores pointers to where information can be found in external systems. These memories allow you to remember where to look to find up-to-date information outside of the project directory.</description>",
    "    <when_to_save>When you learn about resources in external systems and their purpose. For example, that bugs are tracked in a specific project in Linear or that feedback can be found in a specific Slack channel.</when_to_save>",
    "    <how_to_use>When the user references an external system or information that may be in an external system.</how_to_use>",
    "    <examples>",
    "    user: check the Linear project \"INGEST\" if you want context on these tickets, that's where we track all pipeline bugs",
    "    assistant: [saves reference memory: pipeline bugs are tracked in Linear project \"INGEST\"]",
    "",
    "    user: the Grafana board at grafana.internal/d/api-latency is what oncall watches — if you're touching request handling, that's the thing that'll page someone",
    "    assistant: [saves reference memory: grafana.internal/d/api-latency is the oncall latency dashboard — check it when editing request-path code]",
    "    </examples>",
    "</type>",
    "</types>",
    "",
];

const WHAT_NOT_TO_SAVE_SECTION: &[&str] = &[
    "## What NOT to save in memory",
    "",
    "- Code patterns, conventions, architecture, file paths, or project structure — these can be derived by reading the current project state.",
    "- Git history, recent changes, or who-changed-what — `git log` / `git blame` are authoritative.",
    "- Debugging solutions or fix recipes — the fix is in the code; the commit message has the context.",
    "- Anything already documented in CLAUDE.md files.",
    "- Ephemeral task details: in-progress work, temporary state, current conversation context.",
    "",
    "These exclusions apply even when the user explicitly asks you to save. If they ask you to save a PR list or activity summary, ask what was *surprising* or *non-obvious* about it — that is the part worth keeping.",
];

const WHEN_TO_ACCESS_SECTION: &[&str] = &[
    "## When to access memories",
    "- When memories seem relevant, or the user references prior-conversation work.",
    "- You MUST access memory when the user explicitly asks you to check, recall, or remember.",
    "- If the user says to *ignore* or *not use* memory: proceed as if MEMORY.md were empty. Do not apply remembered facts, cite, compare against, or mention memory content.",
    MEMORY_DRIFT_CAVEAT,
];

const TRUSTING_RECALL_SECTION: &[&str] = &[
    "## Before recommending from memory",
    "",
    "A memory that names a specific function, file, or flag is a claim that it existed *when the memory was written*. It may have been renamed, removed, or never merged. Before recommending it:",
    "",
    "- If the memory names a file path: check the file exists.",
    "- If the memory names a function or flag: grep for it.",
    "- If the user is about to act on your recommendation (not just asking about history), verify first.",
    "",
    "\"The memory says X exists\" is not the same as \"X exists now.\"",
    "",
    "A memory that summarizes repo state (activity logs, architecture snapshots) is frozen in time. If the user asks about *recent* or *current* state, prefer `git log` or reading the code over recalling the snapshot.",
];

const MEMORY_FRONTMATTER_EXAMPLE: &[&str] = &[
    "```markdown",
    "---",
    "name: {{memory name}}",
    "description: {{one-line description — used to decide relevance in future conversations, so be specific}}",
    "type: {{user, feedback, project, reference}}",
    "---",
    "",
    "{{memory content — for feedback/project types, structure as: rule/fact, then **Why:** and **How to apply:** lines}}",
    "```",
];

#[must_use]
pub fn has_valid_cowork_memory_path_override() -> bool {
    validated_cowork_memory_path_override_from(
        std::env::var("CLAUDE_COWORK_MEMORY_PATH_OVERRIDE").ok(),
    )
    .is_some()
}

pub fn load_cowork_memory_mechanics_prompt(config: &RuntimeConfig) -> Result<Option<String>> {
    load_cowork_memory_mechanics_prompt_with(config, &AutoMemoryInputs::from_process_env(config)?)
}

fn load_cowork_memory_mechanics_prompt_with(
    _config: &RuntimeConfig,
    inputs: &AutoMemoryInputs,
) -> Result<Option<String>> {
    if !inputs.auto_memory_enabled {
        return Ok(None);
    }
    let Some(memory_dir) =
        validated_cowork_memory_path_override_from(inputs.cowork_memory_path_override.clone())
    else {
        return Ok(None);
    };
    fs::create_dir_all(Path::new(&memory_dir))?;

    Ok(Some(
        build_memory_lines(
            "auto memory",
            &memory_dir,
            inputs.cowork_memory_extra_guidelines.clone(),
        )
        .join("\n"),
    ))
}

#[derive(Debug, Clone, Default)]
struct AutoMemoryInputs {
    auto_memory_enabled: bool,
    cowork_memory_path_override: Option<String>,
    cowork_memory_extra_guidelines: Option<String>,
}

impl AutoMemoryInputs {
    fn from_process_env(config: &RuntimeConfig) -> Result<Self> {
        Ok(Self {
            auto_memory_enabled: resolve_auto_memory_enabled(
                config,
                std::env::var("CLAUDE_CODE_DISABLE_AUTO_MEMORY").ok(),
                std::env::var("CLAUDE_CODE_SIMPLE").ok(),
                std::env::var("CLAUDE_CODE_REMOTE").ok(),
                std::env::var_os("CLAUDE_CODE_REMOTE_MEMORY_DIR"),
            )?,
            cowork_memory_path_override: std::env::var("CLAUDE_COWORK_MEMORY_PATH_OVERRIDE").ok(),
            cowork_memory_extra_guidelines: std::env::var("CLAUDE_COWORK_MEMORY_EXTRA_GUIDELINES")
                .ok()
                .filter(|value| !value.trim().is_empty()),
        })
    }
}

fn resolve_auto_memory_enabled(
    config: &RuntimeConfig,
    disable_auto_memory_env: Option<String>,
    simple_env: Option<String>,
    remote_env: Option<String>,
    remote_memory_dir: Option<std::ffi::OsString>,
) -> Result<bool> {
    if env_truthy(disable_auto_memory_env.as_deref()) {
        return Ok(false);
    }
    if env_defined_falsy(disable_auto_memory_env.as_deref()) {
        return Ok(true);
    }
    if env_truthy(simple_env.as_deref()) {
        return Ok(false);
    }
    if env_truthy(remote_env.as_deref()) && remote_memory_dir.is_none() {
        return Ok(false);
    }
    let settings = load_runtime_settings(&config.settings_files)?;
    if let Some(auto_memory_enabled) = settings.auto_memory_enabled {
        return Ok(auto_memory_enabled);
    }
    Ok(true)
}

fn env_truthy(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

fn env_defined_falsy(value: Option<&str>) -> bool {
    value.is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "" | "0" | "false" | "no" | "off"
        )
    })
}

fn validated_cowork_memory_path_override_from(raw: Option<String>) -> Option<String> {
    validate_memory_path(raw.as_deref())
}

fn validate_memory_path(raw: Option<&str>) -> Option<String> {
    let raw = raw?;
    if raw.is_empty() || raw.contains('\0') || raw.starts_with("\\\\") || raw.starts_with("//") {
        return None;
    }

    let normalized = normalize_memory_path(raw);
    let stripped = normalized.trim_end_matches(['/', '\\']);
    if stripped.is_empty()
        || !Path::new(stripped).is_absolute()
        || stripped.len() < 3
        || is_windows_drive_root(stripped)
    {
        return None;
    }

    Some(format!("{stripped}{MAIN_SEPARATOR}"))
}

fn normalize_memory_path(raw: &str) -> String {
    let mut normalized = PathBuf::new();
    for component in Path::new(raw).components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized.to_string_lossy().into_owned()
}

fn is_windows_drive_root(path: &str) -> bool {
    let bytes = path.as_bytes();
    bytes.len() == 2 && bytes[1] == b':' && bytes[0].is_ascii_alphabetic()
}

fn build_memory_lines(
    display_name: &str,
    memory_dir: &str,
    extra_guideline: Option<String>,
) -> Vec<String> {
    let mut how_to_save = vec![
        "## How to save memories".to_owned(),
        String::new(),
        "Saving a memory is a two-step process:".to_owned(),
        String::new(),
        "**Step 1** — write the memory to its own file (e.g., `user_role.md`, `feedback_testing.md`) using this frontmatter format:".to_owned(),
        String::new(),
    ];
    how_to_save.extend(
        MEMORY_FRONTMATTER_EXAMPLE
            .iter()
            .map(|line| (*line).to_owned()),
    );
    how_to_save.extend([
        String::new(),
        format!(
            "**Step 2** — add a pointer to that file in `{ENTRYPOINT_NAME}`. `{ENTRYPOINT_NAME}` is an index, not a memory — each entry should be one line, under ~150 characters: `- [Title](file.md) — one-line hook`. It has no frontmatter. Never write memory content directly into `{ENTRYPOINT_NAME}`."
        ),
        String::new(),
        format!(
            "- `{ENTRYPOINT_NAME}` is always loaded into your conversation context — lines after {MAX_ENTRYPOINT_LINES} will be truncated, so keep the index concise"
        ),
        "- Keep the name, description, and type fields in memory files up-to-date with the content"
            .to_owned(),
        "- Organize memory semantically by topic, not chronologically".to_owned(),
        "- Update or remove memories that turn out to be wrong or outdated".to_owned(),
        "- Do not write duplicate memories. First check if there is an existing memory you can update before writing a new one.".to_owned(),
    ]);

    let mut lines = vec![
        format!("# {display_name}"),
        String::new(),
        format!(
            "You have a persistent, file-based memory system at `{memory_dir}`. {DIR_EXISTS_GUIDANCE}"
        ),
        String::new(),
        "You should build up this memory system over time so that future conversations can have a complete picture of who the user is, how they'd like to collaborate with you, what behaviors to avoid or repeat, and the context behind the work the user gives you.".to_owned(),
        String::new(),
        "If the user explicitly asks you to remember something, save it immediately as whichever type fits best. If they ask you to forget something, find and remove the relevant entry.".to_owned(),
        String::new(),
    ];
    lines.extend(
        TYPES_SECTION_INDIVIDUAL
            .iter()
            .map(|line| (*line).to_owned()),
    );
    lines.extend(
        WHAT_NOT_TO_SAVE_SECTION
            .iter()
            .map(|line| (*line).to_owned()),
    );
    lines.push(String::new());
    lines.extend(how_to_save);
    lines.push(String::new());
    lines.extend(WHEN_TO_ACCESS_SECTION.iter().map(|line| (*line).to_owned()));
    lines.push(String::new());
    lines.extend(
        TRUSTING_RECALL_SECTION
            .iter()
            .map(|line| (*line).to_owned()),
    );
    lines.extend([
        String::new(),
        "## Memory and other forms of persistence".to_owned(),
        "Memory is one of several persistence mechanisms available to you as you assist the user in a given conversation. The distinction is often that memory can be recalled in future conversations and should not be used for persisting information that is only useful within the scope of the current conversation.".to_owned(),
        "- When to use or update a plan instead of memory: If you are about to start a non-trivial implementation task and would like to reach alignment with the user on your approach you should use a Plan rather than saving this information to memory. Similarly, if you already have a plan within the conversation and you have changed your approach persist that change by updating the plan rather than saving a memory.".to_owned(),
        "- When to use or update tasks instead of memory: When you need to break your work in current conversation into discrete steps or keep track of your progress use tasks instead of saving to memory. Tasks are great for persisting information about the work that needs to be done in the current conversation, but memory should be reserved for information that will be useful in future conversations.".to_owned(),
        String::new(),
    ]);
    if let Some(extra_guideline) = extra_guideline {
        lines.push(extra_guideline);
    }
    lines.push(String::new());

    lines
}

#[cfg(test)]
mod tests {
    use super::{
        AutoMemoryInputs, build_memory_lines, load_cowork_memory_mechanics_prompt_with,
        resolve_auto_memory_enabled, validate_memory_path,
    };
    use rc_config::settings_layers::RuntimeOverrides;
    use rc_config::{ProviderOverrides, RuntimeConfig, load_runtime_config};
    use rc_core::{InputFormat, OutputFormat, PermissionMode};
    use std::ffi::OsString;
    use std::fs;
    use std::path::MAIN_SEPARATOR;
    use tempfile::tempdir;

    #[test]
    fn validate_memory_path_accepts_absolute_paths_and_enforces_single_trailing_separator() {
        let raw = if cfg!(windows) {
            r"C:\Users\Yanzh\memory\"
        } else {
            "/tmp/memory/"
        };

        let normalized = validate_memory_path(Some(raw)).expect("normalized path");
        assert!(normalized.ends_with(MAIN_SEPARATOR));
        assert!(!normalized.ends_with(&format!("{MAIN_SEPARATOR}{MAIN_SEPARATOR}")));
    }

    #[test]
    fn validate_memory_path_rejects_relative_unc_and_drive_roots() {
        assert!(validate_memory_path(Some("../memory")).is_none());
        assert!(validate_memory_path(Some("//server/share")).is_none());
        if cfg!(windows) {
            assert!(validate_memory_path(Some(r"C:\")).is_none());
        }
    }

    #[test]
    fn build_memory_lines_preserves_source_sections() {
        let prompt = build_memory_lines(
            "auto memory",
            "/tmp/auto-memory/",
            Some("Custom extra guideline".to_owned()),
        )
        .join("\n");

        assert!(prompt.contains("# auto memory"));
        assert!(prompt.contains("## How to save memories"));
        assert!(prompt.contains("## Before recommending from memory"));
        assert!(prompt.contains("type: {{user, feedback, project, reference}}"));
        assert!(prompt.contains("Custom extra guideline"));
    }

    #[test]
    fn load_cowork_memory_mechanics_prompt_includes_source_backed_sections() {
        let tempdir = tempdir().expect("tempdir");
        let config = test_runtime_config(tempdir.path(), None);
        let memory_dir = tempdir.path().join("cowork-memory");
        let prompt = load_cowork_memory_mechanics_prompt_with(
            &config,
            &AutoMemoryInputs {
                auto_memory_enabled: true,
                cowork_memory_path_override: Some(memory_dir.display().to_string()),
                cowork_memory_extra_guidelines: Some("Cowork extra guideline".to_owned()),
            },
        )
        .expect("prompt")
        .expect("memory prompt");

        assert!(prompt.contains("# auto memory"));
        assert!(prompt.contains("## How to save memories"));
        assert!(prompt.contains("## Before recommending from memory"));
        assert!(prompt.contains("Cowork extra guideline"));
        assert!(prompt.contains(&format!("{}{}", memory_dir.display(), MAIN_SEPARATOR)));
        assert!(memory_dir.is_dir());
    }

    #[test]
    fn load_cowork_memory_mechanics_prompt_respects_disabled_state() {
        let tempdir = tempdir().expect("tempdir");
        let config = test_runtime_config(tempdir.path(), None);
        let prompt = load_cowork_memory_mechanics_prompt_with(
            &config,
            &AutoMemoryInputs {
                auto_memory_enabled: false,
                cowork_memory_path_override: Some(
                    tempdir.path().join("cowork-memory").display().to_string(),
                ),
                cowork_memory_extra_guidelines: None,
            },
        )
        .expect("prompt");

        assert!(prompt.is_none());
    }

    #[test]
    fn resolve_auto_memory_enabled_respects_settings_opt_out() {
        let tempdir = tempdir().expect("tempdir");
        let settings_dir = tempdir.path().join("workspace").join(".remote-code");
        fs::create_dir_all(&settings_dir).expect("settings dir");
        let settings_path = settings_dir.join("settings.json");
        fs::write(&settings_path, r#"{"autoMemoryEnabled": false}"#).expect("settings");
        let config = test_runtime_config(tempdir.path(), Some(settings_path));

        let enabled = resolve_auto_memory_enabled(&config, None, None, None, None::<OsString>)
            .expect("enabled");

        assert!(!enabled);
    }

    #[test]
    fn resolve_auto_memory_enabled_env_priority_matches_source_truth() {
        let tempdir = tempdir().expect("tempdir");
        let config = test_runtime_config(tempdir.path(), None);

        let enabled = resolve_auto_memory_enabled(
            &config,
            Some("0".to_owned()),
            Some("1".to_owned()),
            Some("1".to_owned()),
            None::<OsString>,
        )
        .expect("enabled");
        assert!(enabled);

        let disabled = resolve_auto_memory_enabled(
            &config,
            Some("true".to_owned()),
            None,
            None,
            None::<OsString>,
        )
        .expect("disabled");
        assert!(!disabled);

        let remote_disabled = resolve_auto_memory_enabled(
            &config,
            None,
            None,
            Some("1".to_owned()),
            None::<OsString>,
        )
        .expect("remote disabled");
        assert!(!remote_disabled);
    }

    fn test_runtime_config(
        base: &std::path::Path,
        explicit_settings: Option<std::path::PathBuf>,
    ) -> RuntimeConfig {
        let cwd = base.join("workspace");
        let profile = base.join(".remote-code-rust");
        fs::create_dir_all(&cwd).expect("cwd");
        fs::create_dir_all(&profile).expect("profile");

        load_runtime_config(
            Some(cwd),
            Some(profile),
            None,
            PermissionMode::BypassPermissions,
            InputFormat::Text,
            OutputFormat::Text,
            false,
            false,
            false,
            false,
            8,
            ProviderOverrides::default(),
            RuntimeOverrides {
                settings_files: explicit_settings.into_iter().collect(),
                ..RuntimeOverrides::default()
            },
        )
        .expect("runtime config")
    }
}

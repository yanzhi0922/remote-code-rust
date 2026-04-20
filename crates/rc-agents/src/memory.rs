//! Agent memory management matching Claude Code's `AgentTool/agentMemory.ts`.
//!
//! Persistent agent memory allows agents to store facts and learnings across
//! sessions. Memory is scoped to user, project, or local contexts and stored
//! as markdown files in the agent memory directory.

use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::Result;
use chrono::{DateTime, Utc};
use directories::BaseDirs;
use serde::{Deserialize, Serialize};

use crate::definition::AgentMemoryScope;

const ENTRYPOINT_NAME: &str = "MEMORY.md";
const MAX_ENTRYPOINT_LINES: usize = 200;
const MAX_ENTRYPOINT_BYTES: usize = 25_000;
const MAX_SANITIZED_LENGTH: usize = 200;
const DIR_EXISTS_GUIDANCE: &str = "This directory already exists — write to it directly with the Write tool (do not run mkdir or check for its existence).";
const MEMORY_DRIFT_CAVEAT: &str = "- Memory records can become stale over time. Use memory as context for what was true at a given point in time. Before answering the user or building assumptions based solely on information in memory records, verify that the memory is still correct and up-to-date by reading the current state of the files or resources. If a recalled memory conflicts with current information, trust what you observe now — and update or remove the stale memory rather than acting on it.";
const REMOTE_MEMORY_PROJECTS_DIRNAME: &str = "projects";

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

/// In-memory representation of an agent's persistent memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMemory {
    /// The agent type this memory belongs to.
    pub agent_type: String,
    /// Collected facts/learnings.
    pub facts: Vec<String>,
    /// When this memory was last updated.
    pub last_updated: DateTime<Utc>,
}

impl AgentMemory {
    /// Create a new empty memory for the given agent type.
    #[must_use]
    pub fn new(agent_type: impl Into<String>) -> Self {
        Self {
            agent_type: agent_type.into(),
            facts: Vec::new(),
            last_updated: Utc::now(),
        }
    }

    /// Load agent memory from the given directory.
    ///
    /// Looks for a `MEMORY.md` file in the directory. Returns `Ok(None)` if
    /// no memory file exists.
    pub fn load(agent_type: &str, dir: &Path) -> Result<Option<Self>> {
        let memory_file = dir.join("MEMORY.md");
        if !memory_file.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&memory_file)?;
        let facts = parse_memory_content(&content);
        Ok(Some(Self {
            agent_type: agent_type.to_owned(),
            facts,
            last_updated: fs::metadata(&memory_file)
                .and_then(|m| m.modified())
                .map(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .map(|d| DateTime::from_timestamp(d.as_secs() as i64, 0))
                        .ok()
                        .flatten()
                })
                .ok()
                .flatten()
                .unwrap_or_else(Utc::now),
        }))
    }

    /// Save agent memory to the given directory.
    ///
    /// Creates the directory if it doesn't exist. Writes a `MEMORY.md` file
    /// with the current facts.
    pub fn save(&self, dir: &Path) -> Result<()> {
        fs::create_dir_all(dir)?;
        let content = self.render_markdown();
        fs::write(dir.join("MEMORY.md"), content)?;
        Ok(())
    }

    /// Add a new fact, deduplicating against existing entries.
    pub fn add_fact(&mut self, fact: impl Into<String>) {
        let fact = fact.into();
        if !self.facts.contains(&fact) {
            self.facts.push(fact);
            self.last_updated = Utc::now();
        }
    }

    /// Remove facts matching a predicate.
    pub fn remove_facts(&mut self, predicate: impl Fn(&str) -> bool) {
        let before = self.facts.len();
        self.facts.retain(|f| !predicate(f));
        if self.facts.len() != before {
            self.last_updated = Utc::now();
        }
    }

    /// Render the memory as a prompt section for injection into agent context.
    pub fn to_prompt_section(&self) -> String {
        if self.facts.is_empty() {
            return String::new();
        }
        let mut out = String::from("# Persistent Agent Memory\n\n");
        out.push_str("The following facts were learned in previous sessions:\n\n");
        for fact in &self.facts {
            out.push_str("- ");
            out.push_str(fact);
            out.push('\n');
        }
        out
    }

    /// Render the memory as a markdown document.
    fn render_markdown(&self) -> String {
        let mut out = String::from("# Agent Memory\n\n");
        out.push_str(&format!(
            "_Last updated: {}_\n\n",
            self.last_updated.format("%Y-%m-%d %H:%M:%S UTC")
        ));
        for fact in &self.facts {
            out.push_str("- ");
            out.push_str(fact);
            out.push('\n');
        }
        out
    }
}

/// Parse facts from a markdown memory file.
fn parse_memory_content(content: &str) -> Vec<String> {
    content
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            trimmed.strip_prefix("- ").map(|s| s.to_owned())
        })
        .collect()
}

/// Sanitize an agent type name for use as a directory name.
/// Replaces colons (invalid on Windows) with dashes.
pub fn sanitize_agent_type_for_path(agent_type: &str) -> String {
    agent_type.replace(':', "-")
}

fn default_claude_config_home() -> PathBuf {
    BaseDirs::new()
        .map(|base| base.home_dir().join(".claude"))
        .unwrap_or_else(|| PathBuf::from(".claude"))
}

fn remote_memory_base_dir() -> Option<PathBuf> {
    std::env::var_os("CLAUDE_CODE_REMOTE_MEMORY_DIR").map(PathBuf::from)
}

/// Get the agent memory directory for a given agent type and scope.
///
/// - `Project` scope: `<base>/.claude/agent-memory/<agent_type>/`
/// - `User` scope: `<config_home>/agent-memory/<agent_type>/`
/// - `Local` scope: `<base>/.claude/agent-memory-local/<agent_type>/`
pub fn get_agent_memory_dir(
    agent_type: &str,
    scope: AgentMemoryScope,
    base: &Path,
    config_home: &Path,
) -> std::path::PathBuf {
    let dir_name = sanitize_agent_type_for_path(agent_type);
    let memory_base = remote_memory_base_dir().unwrap_or_else(|| config_home.to_path_buf());
    match scope {
        AgentMemoryScope::Project => base.join(".claude").join("agent-memory").join(dir_name),
        AgentMemoryScope::User => memory_base.join("agent-memory").join(dir_name),
        AgentMemoryScope::Local => {
            if let Some(remote_base) = remote_memory_base_dir() {
                remote_base
                    .join(REMOTE_MEMORY_PROJECTS_DIRNAME)
                    .join(sanitize_path_component(
                        &canonical_project_root(base).to_string_lossy(),
                    ))
                    .join("agent-memory-local")
                    .join(dir_name)
            } else {
                base.join(".claude")
                    .join("agent-memory-local")
                    .join(dir_name)
            }
        }
    }
}

/// Check if a path is within an agent memory directory (any scope).
///
/// The path should be absolute or relative to the current working directory.
/// Does not resolve symlinks or canonicalize the path.
pub fn is_agent_memory_path(path: &Path, base: &Path, config_home: &Path) -> bool {
    let normalized = normalize_path(path);

    // User scope
    let user_memory = remote_memory_base_dir()
        .unwrap_or_else(|| config_home.to_path_buf())
        .join("agent-memory");
    if normalized.starts_with(&user_memory) {
        return true;
    }

    // Project scope
    let project_memory = base.join(".claude").join("agent-memory");
    if normalized.starts_with(&project_memory) {
        return true;
    }

    // Local scope
    let local_memory = get_agent_memory_dir("...", AgentMemoryScope::Local, base, config_home)
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| base.join(".claude").join("agent-memory-local"));
    if normalized.starts_with(&local_memory) {
        return true;
    }

    false
}

/// Build the memory prompt for an agent with memory enabled.
///
/// Creates the memory directory if needed and returns a prompt section
/// describing how to use persistent memory.
pub fn build_memory_prompt(
    agent_type: &str,
    scope: AgentMemoryScope,
    base: &Path,
    config_home: &Path,
) -> String {
    let scope_note = match scope {
        AgentMemoryScope::User => {
            "Since this memory is user-scope, keep learnings general since they apply across all projects"
        }
        AgentMemoryScope::Project => {
            "Since this memory is project-scope and shared with your team via version control, tailor your memories to this project"
        }
        AgentMemoryScope::Local => {
            "Since this memory is local-scope (not checked into version control), tailor your memories to this project and machine"
        }
    };

    let memory_dir = get_agent_memory_dir(agent_type, scope, base, config_home);
    let _ = fs::create_dir_all(&memory_dir);
    let memory_dir_str = with_trailing_separator(memory_dir.clone());
    let mut lines = build_memory_lines(
        "Persistent Agent Memory",
        &memory_dir_str,
        cowork_memory_extra_guideline(scope_note),
    );

    let entrypoint = memory_dir.join(ENTRYPOINT_NAME);
    let entrypoint_content = fs::read_to_string(entrypoint).unwrap_or_default();
    if entrypoint_content.trim().is_empty() {
        lines.extend([
            format!("## {ENTRYPOINT_NAME}"),
            String::new(),
            format!(
                "Your {ENTRYPOINT_NAME} is currently empty. When you save new memories, they will appear here."
            ),
        ]);
    } else {
        lines.extend([
            format!("## {ENTRYPOINT_NAME}"),
            String::new(),
            truncate_entrypoint_content(&entrypoint_content),
        ]);
    }

    lines.join("\n")
}

/// Get a display string for the memory scope.
pub fn get_memory_scope_display(scope: &Option<AgentMemoryScope>, _base: &Path) -> &'static str {
    match scope {
        Some(AgentMemoryScope::User) => "User (~/.claude/agent-memory/)",
        Some(AgentMemoryScope::Project) => "Project (.claude/agent-memory/)",
        Some(AgentMemoryScope::Local) => "Local (.claude/agent-memory-local/)",
        None => "None",
    }
}

pub fn append_memory_prompt_to_system_prompt(
    base_prompt: &str,
    agent_type: &str,
    scope: AgentMemoryScope,
    base: &Path,
    config_home: Option<&Path>,
) -> String {
    let default_home;
    let config_home = match config_home {
        Some(config_home) => config_home,
        None => {
            default_home = default_claude_config_home();
            default_home.as_path()
        }
    };
    format!(
        "{base_prompt}\n\n{}",
        build_memory_prompt(agent_type, scope, base, config_home)
    )
}

fn cowork_memory_extra_guideline(scope_note: &str) -> Option<String> {
    let extra = std::env::var("CLAUDE_COWORK_MEMORY_EXTRA_GUIDELINES")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let mut guidelines = vec![format!("- {scope_note}")];
    if let Some(extra) = extra {
        guidelines.push(extra);
    }
    Some(guidelines.join("\n"))
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
        lines.push(String::new());
    }
    lines
}

fn truncate_entrypoint_content(raw: &str) -> String {
    let trimmed = raw.trim();
    let content_lines = trimmed.lines().collect::<Vec<_>>();
    let line_count = content_lines.len();
    let byte_count = trimmed.len();
    let was_line_truncated = line_count > MAX_ENTRYPOINT_LINES;
    let was_byte_truncated = byte_count > MAX_ENTRYPOINT_BYTES;

    if !was_line_truncated && !was_byte_truncated {
        return trimmed.to_owned();
    }

    let mut truncated = if was_line_truncated {
        content_lines[..MAX_ENTRYPOINT_LINES].join("\n")
    } else {
        trimmed.to_owned()
    };

    if truncated.len() > MAX_ENTRYPOINT_BYTES {
        let boundary = floor_char_boundary(&truncated, MAX_ENTRYPOINT_BYTES);
        let slice = &truncated[..boundary];
        let cut_at = slice.rfind('\n').unwrap_or(boundary);
        truncated = slice[..cut_at].to_owned();
    }

    let reason = match (was_line_truncated, was_byte_truncated) {
        (true, true) => format!("{line_count} lines and {}", format_file_size(byte_count)),
        (true, false) => format!("{line_count} lines (limit: {MAX_ENTRYPOINT_LINES})"),
        (false, true) => format!(
            "{} (limit: {}) — index entries are too long",
            format_file_size(byte_count),
            format_file_size(MAX_ENTRYPOINT_BYTES)
        ),
        (false, false) => String::new(),
    };

    format!(
        "{truncated}\n\n> WARNING: {ENTRYPOINT_NAME} is {reason}. Only part of it was loaded. Keep index entries to one line under ~200 chars; move detail into topic files."
    )
}

fn floor_char_boundary(text: &str, max_bytes: usize) -> usize {
    if max_bytes >= text.len() {
        return text.len();
    }
    let mut boundary = max_bytes;
    while boundary > 0 && !text.is_char_boundary(boundary) {
        boundary -= 1;
    }
    boundary
}

fn format_file_size(size_in_bytes: usize) -> String {
    let kb = size_in_bytes as f64 / 1024.0;
    if kb < 1.0 {
        return format!("{size_in_bytes} bytes");
    }
    if kb < 1024.0 {
        return format_decimal(kb, "KB");
    }
    let mb = kb / 1024.0;
    if mb < 1024.0 {
        return format_decimal(mb, "MB");
    }
    let gb = mb / 1024.0;
    format_decimal(gb, "GB")
}

fn format_decimal(value: f64, unit: &str) -> String {
    let rendered = format!("{value:.1}");
    let rendered = rendered.strip_suffix(".0").unwrap_or(&rendered);
    format!("{rendered}{unit}")
}

fn with_trailing_separator(path: PathBuf) -> String {
    let mut rendered = path.to_string_lossy().into_owned();
    if !rendered.ends_with(std::path::MAIN_SEPARATOR) {
        rendered.push(std::path::MAIN_SEPARATOR);
    }
    rendered
}

fn canonical_project_root(cwd: &Path) -> PathBuf {
    find_canonical_git_root(cwd)
        .or_else(|| fs::canonicalize(cwd).ok())
        .unwrap_or_else(|| cwd.to_path_buf())
}

fn find_canonical_git_root(cwd: &Path) -> Option<PathBuf> {
    let common_dir = git_absolute_path(
        cwd,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )
    .or_else(|| git_absolute_path(cwd, &["rev-parse", "--git-common-dir"]))?;
    let common_dir = fs::canonicalize(&common_dir).unwrap_or(common_dir);
    if common_dir
        .file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value == ".git")
    {
        return common_dir.parent().map(Path::to_path_buf);
    }
    Some(common_dir)
}

fn git_absolute_path(cwd: &Path, args: &[&str]) -> Option<PathBuf> {
    let output = std::process::Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!value.is_empty()).then(|| PathBuf::from(value))
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

fn sanitize_path_component(raw: &str) -> String {
    let sanitized = raw
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '-' })
        .collect::<String>();
    if sanitized.len() <= MAX_SANITIZED_LENGTH {
        return sanitized;
    }
    format!(
        "{}-{}",
        &sanitized[..MAX_SANITIZED_LENGTH],
        simple_hash(raw)
    )
}

fn simple_hash(raw: &str) -> String {
    let mut hash: i32 = 0;
    for ch in raw.chars() {
        hash = hash
            .wrapping_shl(5)
            .wrapping_sub(hash)
            .wrapping_add(ch as i32);
    }
    to_base36(i64::from(hash).unsigned_abs())
}

fn to_base36(mut value: u64) -> String {
    if value == 0 {
        return "0".to_owned();
    }
    let mut digits = Vec::new();
    while value > 0 {
        let rem = (value % 36) as u8;
        let digit = match rem {
            0..=9 => char::from(b'0' + rem),
            _ => char::from(b'a' + (rem - 10)),
        };
        digits.push(digit);
        value /= 36;
    }
    digits.into_iter().rev().collect()
}

// ── Memory snapshot support ───────────────────────────────────────────────

/// Base directory name for agent memory snapshots.
const SNAPSHOT_BASE: &str = "agent-memory-snapshots";

/// File name for the snapshot metadata.
const SNAPSHOT_JSON: &str = "snapshot.json";

/// A snapshot of an agent's memory at a point in time.
///
/// Snapshots allow agent memory to be saved, shared, and restored across
/// sessions and team members. They capture the full set of facts along
/// with metadata about when the snapshot was taken.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMemorySnapshot {
    /// The agent type this snapshot belongs to.
    pub agent_type: String,
    /// Facts captured in the snapshot.
    pub facts: Vec<String>,
    /// When the snapshot was created.
    pub created_at: DateTime<Utc>,
    /// When the snapshot was last updated.
    pub updated_at: DateTime<Utc>,
}

impl AgentMemorySnapshot {
    /// Create a new empty snapshot for the given agent type.
    #[must_use]
    pub fn new(agent_type: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            agent_type: agent_type.into(),
            facts: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }

    /// Create a snapshot from an existing [`AgentMemory`].
    pub fn from_memory(memory: &AgentMemory) -> Self {
        Self {
            agent_type: memory.agent_type.clone(),
            facts: memory.facts.clone(),
            created_at: memory.last_updated,
            updated_at: Utc::now(),
        }
    }

    /// Number of facts in the snapshot.
    pub fn fact_count(&self) -> usize {
        self.facts.len()
    }

    /// Check if the snapshot is empty.
    pub fn is_empty(&self) -> bool {
        self.facts.is_empty()
    }
}

/// Diff between two memory snapshots.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySnapshotDiff {
    /// Facts added in the new snapshot.
    pub added: Vec<String>,
    /// Facts removed from the old snapshot.
    pub removed: Vec<String>,
    /// Facts present in both snapshots.
    pub unchanged: Vec<String>,
}

/// Get the snapshot directory for an agent type within a project.
///
/// Returns `<base>/.claude/agent-memory-snapshots/<agent_type>/`.
pub fn get_snapshot_dir(base: &Path, agent_type: &str) -> std::path::PathBuf {
    let dir_name = sanitize_agent_type_for_path(agent_type);
    base.join(".claude").join(SNAPSHOT_BASE).join(dir_name)
}

/// Get the snapshot file path.
pub fn get_snapshot_path(base: &Path, agent_type: &str) -> std::path::PathBuf {
    get_snapshot_dir(base, agent_type).join(SNAPSHOT_JSON)
}

/// Take a memory snapshot and save it to disk.
///
/// Creates the snapshot directory if it doesn't exist.
pub fn take_memory_snapshot(base: &Path, memory: &AgentMemory) -> Result<()> {
    let snapshot = AgentMemorySnapshot::from_memory(memory);
    let dir = get_snapshot_dir(base, &memory.agent_type);
    fs::create_dir_all(&dir)?;

    let path = dir.join(SNAPSHOT_JSON);
    let json = serde_json::to_string_pretty(&snapshot)?;
    fs::write(path, json)?;

    Ok(())
}

/// Load a memory snapshot from disk.
///
/// Returns `Ok(None)` if no snapshot exists for the given agent type.
pub fn load_memory_snapshot(base: &Path, agent_type: &str) -> Result<Option<AgentMemorySnapshot>> {
    let path = get_snapshot_path(base, agent_type);
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&path)?;
    let snapshot: AgentMemorySnapshot = serde_json::from_str(&content)?;
    Ok(Some(snapshot))
}

/// Restore agent memory from a snapshot.
///
/// Creates a new [`AgentMemory`] populated with the snapshot's facts.
pub fn restore_from_snapshot(snapshot: &AgentMemorySnapshot) -> AgentMemory {
    let mut memory = AgentMemory::new(&snapshot.agent_type);
    for fact in &snapshot.facts {
        memory.add_fact(fact);
    }
    memory
}

/// Compute the diff between two memory snapshots.
///
/// Returns the facts added, removed, and unchanged.
pub fn diff_snapshots(old: &AgentMemorySnapshot, new: &AgentMemorySnapshot) -> MemorySnapshotDiff {
    use std::collections::BTreeSet;

    let old_set: BTreeSet<&str> = old.facts.iter().map(|s| s.as_str()).collect();
    let new_set: BTreeSet<&str> = new.facts.iter().map(|s| s.as_str()).collect();

    let added: Vec<String> = new_set
        .difference(&old_set)
        .map(|s| (*s).to_owned())
        .collect();
    let removed: Vec<String> = old_set
        .difference(&new_set)
        .map(|s| (*s).to_owned())
        .collect();
    let unchanged: Vec<String> = old_set
        .intersection(&new_set)
        .map(|s| (*s).to_owned())
        .collect();

    MemorySnapshotDiff {
        added,
        removed,
        unchanged,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::path::PathBuf;

    #[test]
    fn new_memory_is_empty() {
        let mem = AgentMemory::new("test-agent");
        assert_eq!(mem.agent_type, "test-agent");
        assert!(mem.facts.is_empty());
    }

    #[test]
    fn add_fact_deduplicates() {
        let mut mem = AgentMemory::new("test");
        mem.add_fact("The project uses Rust");
        mem.add_fact("The project uses Rust");
        assert_eq!(mem.facts.len(), 1);
    }

    #[test]
    fn add_fact_different_entries() {
        let mut mem = AgentMemory::new("test");
        mem.add_fact("Uses Rust");
        mem.add_fact("Uses Tokio");
        assert_eq!(mem.facts.len(), 2);
    }

    #[test]
    fn remove_facts_by_predicate() {
        let mut mem = AgentMemory::new("test");
        mem.add_fact("keep this");
        mem.add_fact("remove this");
        mem.add_fact("also keep");
        mem.remove_facts(|f| f.contains("remove"));
        assert_eq!(mem.facts.len(), 2);
        assert!(mem.facts.contains(&"keep this".to_owned()));
        assert!(mem.facts.contains(&"also keep".to_owned()));
    }

    #[test]
    fn prompt_section_empty() {
        let mem = AgentMemory::new("test");
        assert!(mem.to_prompt_section().is_empty());
    }

    #[test]
    fn prompt_section_with_facts() {
        let mut mem = AgentMemory::new("test");
        mem.add_fact("fact one");
        let section = mem.to_prompt_section();
        assert!(section.contains("# Persistent Agent Memory"));
        assert!(section.contains("fact one"));
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut mem = AgentMemory::new("test-agent");
        mem.add_fact("fact 1");
        mem.add_fact("fact 2");

        mem.save(dir.path()).expect("save");
        let loaded = AgentMemory::load("test-agent", dir.path())
            .expect("load")
            .expect("some");

        let loaded_facts: HashSet<_> = loaded.facts.into_iter().collect();
        let expected_facts: HashSet<_> = vec!["fact 1".to_owned(), "fact 2".to_owned()]
            .into_iter()
            .collect();
        assert_eq!(loaded_facts, expected_facts);
    }

    #[test]
    fn load_nonexistent_returns_none() {
        let dir = tempfile::tempdir().expect("tempdir");
        let result = AgentMemory::load("nonexistent", dir.path()).expect("load");
        assert!(result.is_none());
    }

    #[test]
    fn sanitize_agent_type() {
        assert_eq!(
            sanitize_agent_type_for_path("my-plugin:my-agent"),
            "my-plugin-my-agent"
        );
        assert_eq!(sanitize_agent_type_for_path("simple"), "simple");
    }

    #[test]
    fn memory_dir_project_scope() {
        let base = PathBuf::from("/project");
        let config = PathBuf::from("/home/.config");
        let dir = get_agent_memory_dir("test", AgentMemoryScope::Project, &base, &config);
        assert_eq!(dir, PathBuf::from("/project/.claude/agent-memory/test"));
    }

    #[test]
    fn memory_dir_user_scope() {
        let base = PathBuf::from("/project");
        let config = PathBuf::from("/home/.config");
        let dir = get_agent_memory_dir("test", AgentMemoryScope::User, &base, &config);
        assert_eq!(dir, PathBuf::from("/home/.config/agent-memory/test"));
    }

    #[test]
    fn is_agent_memory_path_check() {
        let base = PathBuf::from("/project");
        let config = PathBuf::from("/home/.config");
        let project_mem = PathBuf::from("/project/.claude/agent-memory/test/MEMORY.md");
        assert!(is_agent_memory_path(&project_mem, &base, &config));

        let user_mem = PathBuf::from("/home/.config/agent-memory/test/MEMORY.md");
        assert!(is_agent_memory_path(&user_mem, &base, &config));

        let random = PathBuf::from("/tmp/other.md");
        assert!(!is_agent_memory_path(&random, &base, &config));
    }

    #[test]
    fn build_memory_prompt_includes_scope_guidance_and_memory_contents() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path().join("project");
        let config = dir.path().join("config");
        let memory_dir =
            get_agent_memory_dir("review:agent", AgentMemoryScope::Project, &base, &config);
        fs::create_dir_all(&memory_dir).expect("memory dir");
        fs::write(
            memory_dir.join(ENTRYPOINT_NAME),
            "- [Review preference](review.md) — prefer focused diffs\n",
        )
        .expect("memory entrypoint");

        let prompt = build_memory_prompt("review:agent", AgentMemoryScope::Project, &base, &config);

        assert!(prompt.contains("# Persistent Agent Memory"));
        assert!(prompt.contains("Since this memory is project-scope"));
        assert!(prompt.contains("## MEMORY.md"));
        assert!(prompt.contains("prefer focused diffs"));
        assert!(prompt.contains(&with_trailing_separator(memory_dir)));
    }

    #[test]
    fn build_memory_prompt_creates_directory_and_reports_empty_entrypoint() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path().join("project");
        let config = dir.path().join("config");
        let memory_dir = get_agent_memory_dir("planner", AgentMemoryScope::Local, &base, &config);

        let prompt = build_memory_prompt("planner", AgentMemoryScope::Local, &base, &config);

        assert!(memory_dir.is_dir());
        assert!(prompt.contains("Your MEMORY.md is currently empty"));
        assert!(prompt.contains("Since this memory is local-scope"));
    }

    #[test]
    fn append_memory_prompt_to_system_prompt_uses_default_config_home_without_lifetime_bug() {
        let dir = tempfile::tempdir().expect("tempdir");
        let base = dir.path().join("project");

        let prompt = append_memory_prompt_to_system_prompt(
            "Base prompt",
            "planner",
            AgentMemoryScope::User,
            &base,
            None,
        );

        assert!(prompt.starts_with("Base prompt\n\n# Persistent Agent Memory"));
    }

    #[test]
    fn parse_memory_content_extracts_facts() {
        let content = "# Agent Memory\n\n- fact one\n- fact two\n\nSome other text\n";
        let facts = parse_memory_content(content);
        assert_eq!(facts, vec!["fact one", "fact two"]);
    }

    // ── Snapshot tests ──────────────────────────────────────────────────

    #[test]
    fn snapshot_new_is_empty() {
        let snap = AgentMemorySnapshot::new("test-agent");
        assert_eq!(snap.agent_type, "test-agent");
        assert!(snap.facts.is_empty());
        assert!(snap.is_empty());
        assert_eq!(snap.fact_count(), 0);
    }

    #[test]
    fn snapshot_from_memory() {
        let mut mem = AgentMemory::new("test-agent");
        mem.add_fact("fact 1");
        mem.add_fact("fact 2");
        let snap = AgentMemorySnapshot::from_memory(&mem);
        assert_eq!(snap.agent_type, "test-agent");
        assert_eq!(snap.facts, vec!["fact 1", "fact 2"]);
        assert_eq!(snap.fact_count(), 2);
    }

    #[test]
    fn snapshot_serde_roundtrip() {
        let mut mem = AgentMemory::new("serde-test");
        mem.add_fact("fact");
        let snap = AgentMemorySnapshot::from_memory(&mem);
        let json = serde_json::to_string_pretty(&snap).expect("serialize");
        let parsed: AgentMemorySnapshot = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.agent_type, "serde-test");
        assert_eq!(parsed.facts, vec!["fact"]);
    }

    #[test]
    fn get_snapshot_dir_path() {
        let base = PathBuf::from("/project");
        let dir = get_snapshot_dir(&base, "test-agent");
        assert_eq!(
            dir,
            PathBuf::from("/project/.claude/agent-memory-snapshots/test-agent")
        );
    }

    #[test]
    fn get_snapshot_dir_sanitizes_colons() {
        let base = PathBuf::from("/project");
        let dir = get_snapshot_dir(&base, "plugin:agent");
        assert_eq!(
            dir,
            PathBuf::from("/project/.claude/agent-memory-snapshots/plugin-agent")
        );
    }

    #[test]
    fn take_and_load_snapshot() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut mem = AgentMemory::new("snap-test");
        mem.add_fact("fact 1");
        mem.add_fact("fact 2");

        take_memory_snapshot(dir.path(), &mem).expect("take snapshot");
        let loaded = load_memory_snapshot(dir.path(), "snap-test")
            .expect("load snapshot")
            .expect("some");

        assert_eq!(loaded.agent_type, "snap-test");
        let facts: HashSet<_> = loaded.facts.into_iter().collect();
        assert!(facts.contains("fact 1"));
        assert!(facts.contains("fact 2"));
    }

    #[test]
    fn load_snapshot_nonexistent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let result = load_memory_snapshot(dir.path(), "nonexistent").expect("load");
        assert!(result.is_none());
    }

    #[test]
    fn restore_from_snapshot_creates_memory() {
        let snap = AgentMemorySnapshot {
            agent_type: "test".to_owned(),
            facts: vec!["a".to_owned(), "b".to_owned()],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let mem = restore_from_snapshot(&snap);
        assert_eq!(mem.agent_type, "test");
        assert_eq!(mem.facts.len(), 2);
        assert!(mem.facts.contains(&"a".to_owned()));
        assert!(mem.facts.contains(&"b".to_owned()));
    }

    #[test]
    fn diff_snapshots_added() {
        let old = AgentMemorySnapshot::new("test");
        let mut new = AgentMemorySnapshot::new("test");
        new.facts.push("new fact".to_owned());
        let diff = diff_snapshots(&old, &new);
        assert_eq!(diff.added, vec!["new fact"]);
        assert!(diff.removed.is_empty());
        assert!(diff.unchanged.is_empty());
    }

    #[test]
    fn diff_snapshots_removed() {
        let mut old = AgentMemorySnapshot::new("test");
        old.facts.push("old fact".to_owned());
        let new = AgentMemorySnapshot::new("test");
        let diff = diff_snapshots(&old, &new);
        assert!(diff.added.is_empty());
        assert_eq!(diff.removed, vec!["old fact"]);
        assert!(diff.unchanged.is_empty());
    }

    #[test]
    fn diff_snapshots_unchanged() {
        let mut old = AgentMemorySnapshot::new("test");
        old.facts.push("shared".to_owned());
        let mut new = AgentMemorySnapshot::new("test");
        new.facts.push("shared".to_owned());
        let diff = diff_snapshots(&old, &new);
        assert!(diff.added.is_empty());
        assert!(diff.removed.is_empty());
        assert_eq!(diff.unchanged, vec!["shared"]);
    }

    #[test]
    fn diff_snapshots_complex() {
        let mut old = AgentMemorySnapshot::new("test");
        old.facts = vec!["kept".to_owned(), "removed".to_owned()];
        let mut new = AgentMemorySnapshot::new("test");
        new.facts = vec!["kept".to_owned(), "added".to_owned()];
        let diff = diff_snapshots(&old, &new);
        assert_eq!(diff.added, vec!["added"]);
        assert_eq!(diff.removed, vec!["removed"]);
        assert_eq!(diff.unchanged, vec!["kept"]);
    }
}

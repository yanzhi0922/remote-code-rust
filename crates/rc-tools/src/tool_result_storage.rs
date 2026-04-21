use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use once_cell::sync::Lazy;
use rc_core::{ConversationEntry, ConversationRole};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const DEFAULT_MAX_RESULT_SIZE_CHARS: usize = 50_000;
pub const MAX_TOOL_RESULTS_PER_MESSAGE_CHARS: usize = 200_000;
pub const TOOL_RESULTS_SUBDIR: &str = "tool-results";
pub const PERSISTED_OUTPUT_TAG: &str = "<persisted-output>";
pub const PERSISTED_OUTPUT_CLOSING_TAG: &str = "</persisted-output>";
pub const PREVIEW_SIZE_BYTES: usize = 2_000;

static NON_TEXT_CONTENT_ERROR: Lazy<String> =
    Lazy::new(|| "Cannot persist tool results containing non-text content".to_owned());

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedToolResult {
    pub filepath: PathBuf,
    pub original_size: usize,
    pub is_json: bool,
    pub preview: String,
    pub has_more: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContentReplacementState {
    pub seen_ids: std::collections::HashSet<String>,
    pub replacements: std::collections::HashMap<String, String>,
}

impl ContentReplacementState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ContentReplacementKind {
    ToolResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ContentReplacementRecord {
    pub kind: ContentReplacementKind,
    #[serde(rename = "toolUseId")]
    pub tool_use_id: String,
    pub replacement: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ToolResultBudgetOutcome {
    pub newly_replaced: Vec<ContentReplacementRecord>,
}

#[derive(Debug, Clone)]
struct ToolResultCandidate {
    tool_use_id: String,
    tool_name: String,
    content: CandidateContent,
    size: usize,
}

#[derive(Debug, Clone)]
enum CandidateContent {
    Text(String),
    Blocks(Vec<Value>),
}

impl CandidateContent {
    fn persist(&self, tool_use_id: &str, tool_results_dir: &Path) -> Result<PersistedToolResult> {
        match self {
            Self::Text(content) => persist_tool_result_text(content, tool_use_id, tool_results_dir),
            Self::Blocks(content_blocks) => {
                persist_tool_result_blocks(content_blocks, tool_use_id, tool_results_dir)
            }
        }
    }
}

pub fn session_tool_results_dir(session_dir: &Path) -> PathBuf {
    session_dir.join(TOOL_RESULTS_SUBDIR)
}

pub fn get_tool_result_path(tool_results_dir: &Path, id: &str, is_json: bool) -> PathBuf {
    tool_results_dir.join(format!("{id}.{}", if is_json { "json" } else { "txt" }))
}

pub fn ensure_tool_results_dir(tool_results_dir: &Path) -> Result<()> {
    fs::create_dir_all(tool_results_dir)
        .with_context(|| format!("failed to create {}", tool_results_dir.display()))
}

pub fn process_tool_result_text(
    content: &str,
    tool_use_id: &str,
    tool_results_dir: Option<&Path>,
    threshold: Option<usize>,
) -> Result<String> {
    let limit = threshold.unwrap_or(DEFAULT_MAX_RESULT_SIZE_CHARS);
    if content.chars().count() <= limit {
        return Ok(content.to_owned());
    }
    let Some(tool_results_dir) = tool_results_dir else {
        return Ok(content.to_owned());
    };

    let persisted = persist_tool_result_text(content, tool_use_id, tool_results_dir)?;
    Ok(build_large_tool_result_message(&persisted))
}

pub fn persist_tool_result_text(
    content: &str,
    tool_use_id: &str,
    tool_results_dir: &Path,
) -> Result<PersistedToolResult> {
    ensure_tool_results_dir(tool_results_dir)?;
    let filepath = get_tool_result_path(tool_results_dir, tool_use_id, false);

    match fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&filepath)
    {
        Ok(mut file) => {
            file.write_all(content.as_bytes())
                .with_context(|| format!("failed to write {}", filepath.display()))?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => {
            return Err(anyhow!(
                "{}",
                filesystem_error_message(&filepath, error.kind(), &error.to_string())
            ));
        }
    }

    let (preview, has_more) = generate_preview(content, PREVIEW_SIZE_BYTES);
    Ok(PersistedToolResult {
        filepath,
        original_size: content.len(),
        is_json: false,
        preview,
        has_more,
    })
}

pub fn persist_tool_result_blocks(
    content_blocks: &[Value],
    tool_use_id: &str,
    tool_results_dir: &Path,
) -> Result<PersistedToolResult> {
    if content_blocks.iter().any(|block| {
        block.get("type").and_then(Value::as_str) != Some("text")
            || block.get("text").and_then(Value::as_str).is_none()
    }) {
        return Err(anyhow!(NON_TEXT_CONTENT_ERROR.clone()));
    }

    ensure_tool_results_dir(tool_results_dir)?;
    let filepath = get_tool_result_path(tool_results_dir, tool_use_id, true);
    let content = serde_json::to_string_pretty(content_blocks).context("serialize tool content")?;

    match fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&filepath)
    {
        Ok(mut file) => {
            file.write_all(content.as_bytes())
                .with_context(|| format!("failed to write {}", filepath.display()))?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => {
            return Err(anyhow!(
                "{}",
                filesystem_error_message(&filepath, error.kind(), &error.to_string())
            ));
        }
    }

    let (preview, has_more) = generate_preview(&content, PREVIEW_SIZE_BYTES);
    Ok(PersistedToolResult {
        filepath,
        original_size: content.len(),
        is_json: true,
        preview,
        has_more,
    })
}

pub fn build_large_tool_result_message(result: &PersistedToolResult) -> String {
    let mut message = String::new();
    message.push_str(PERSISTED_OUTPUT_TAG);
    message.push('\n');
    message.push_str(&format!(
        "Output too large ({}). Full output saved to: {}\n\n",
        format_file_size(result.original_size),
        result.filepath.display()
    ));
    message.push_str(&format!(
        "Preview (first {}):\n",
        format_file_size(PREVIEW_SIZE_BYTES)
    ));
    message.push_str(&result.preview);
    if result.has_more {
        message.push_str("\n...\n");
    } else {
        message.push('\n');
    }
    message.push_str(PERSISTED_OUTPUT_CLOSING_TAG);
    message
}

#[must_use]
pub fn reconstruct_content_replacement_state(
    conversation: &[ConversationEntry],
    records: &[ContentReplacementRecord],
    inherited_replacements: Option<&std::collections::HashMap<String, String>>,
) -> ContentReplacementState {
    let mut state = ContentReplacementState::new();
    let candidate_ids = collect_candidates_by_message(conversation)
        .into_iter()
        .flatten()
        .map(|candidate| candidate.tool_use_id)
        .collect::<std::collections::HashSet<_>>();

    state.seen_ids.extend(candidate_ids.iter().cloned());
    for record in records {
        if record.kind == ContentReplacementKind::ToolResult
            && candidate_ids.contains(&record.tool_use_id)
        {
            state
                .replacements
                .insert(record.tool_use_id.clone(), record.replacement.clone());
        }
    }
    if let Some(inherited_replacements) = inherited_replacements {
        for (id, replacement) in inherited_replacements {
            if candidate_ids.contains(id) && !state.replacements.contains_key(id) {
                state.replacements.insert(id.clone(), replacement.clone());
            }
        }
    }
    state
}

pub fn apply_tool_result_budget_to_conversation(
    conversation: &mut [ConversationEntry],
    state: &mut ContentReplacementState,
    tool_results_dir: &Path,
    skip_tool_names: &std::collections::HashSet<String>,
) -> Result<ToolResultBudgetOutcome> {
    let candidates_by_message = collect_candidates_by_message(conversation);
    let mut replacement_map = std::collections::HashMap::<String, String>::new();
    let mut to_persist = Vec::new();

    for candidates in candidates_by_message {
        let mut fresh = Vec::new();
        let mut frozen_size = 0usize;

        for candidate in candidates {
            if let Some(replacement) = state.replacements.get(&candidate.tool_use_id) {
                replacement_map.insert(candidate.tool_use_id.clone(), replacement.clone());
            } else if state.seen_ids.contains(&candidate.tool_use_id) {
                frozen_size = frozen_size.saturating_add(candidate.size);
            } else {
                fresh.push(candidate);
            }
        }

        if fresh.is_empty() {
            continue;
        }

        let mut eligible = Vec::new();
        let mut skipped = Vec::new();
        for candidate in fresh {
            if skip_tool_names.contains(&candidate.tool_name) {
                skipped.push(candidate);
            } else {
                eligible.push(candidate);
            }
        }
        state
            .seen_ids
            .extend(skipped.into_iter().map(|candidate| candidate.tool_use_id));

        let fresh_size = eligible
            .iter()
            .fold(0usize, |sum, candidate| sum.saturating_add(candidate.size));
        let selected =
            if frozen_size.saturating_add(fresh_size) > MAX_TOOL_RESULTS_PER_MESSAGE_CHARS {
                select_fresh_to_replace(eligible.as_slice(), frozen_size)
            } else {
                Vec::new()
            };
        let selected_ids = selected
            .iter()
            .map(|candidate| candidate.tool_use_id.clone())
            .collect::<std::collections::HashSet<_>>();

        state.seen_ids.extend(
            eligible
                .into_iter()
                .filter(|candidate| !selected_ids.contains(&candidate.tool_use_id))
                .map(|candidate| candidate.tool_use_id),
        );
        to_persist.extend(selected);
    }

    if replacement_map.is_empty() && to_persist.is_empty() {
        return Ok(ToolResultBudgetOutcome::default());
    }

    let mut newly_replaced = Vec::new();
    for candidate in to_persist {
        state.seen_ids.insert(candidate.tool_use_id.clone());
        let persisted = match candidate
            .content
            .persist(&candidate.tool_use_id, tool_results_dir)
        {
            Ok(persisted) => persisted,
            Err(_) => continue,
        };
        let replacement = build_large_tool_result_message(&persisted);
        replacement_map.insert(candidate.tool_use_id.clone(), replacement.clone());
        state
            .replacements
            .insert(candidate.tool_use_id.clone(), replacement.clone());
        newly_replaced.push(ContentReplacementRecord {
            kind: ContentReplacementKind::ToolResult,
            tool_use_id: candidate.tool_use_id,
            replacement,
        });
    }

    if !replacement_map.is_empty() {
        replace_tool_result_contents(conversation, &replacement_map);
    }

    Ok(ToolResultBudgetOutcome { newly_replaced })
}

fn collect_candidates_by_message(
    conversation: &[ConversationEntry],
) -> Vec<Vec<ToolResultCandidate>> {
    let mut groups = Vec::new();
    let mut current = Vec::new();

    let flush = |groups: &mut Vec<Vec<ToolResultCandidate>>,
                 current: &mut Vec<ToolResultCandidate>| {
        if !current.is_empty() {
            groups.push(std::mem::take(current));
        }
    };

    for entry in conversation {
        match entry.role {
            ConversationRole::Assistant => flush(&mut groups, &mut current),
            ConversationRole::Tool | ConversationRole::User => {
                current.extend(collect_candidates_from_entry(entry));
            }
            ConversationRole::System => {}
        }
    }
    flush(&mut groups, &mut current);
    groups
}

fn collect_candidates_from_entry(entry: &ConversationEntry) -> Vec<ToolResultCandidate> {
    if entry.role != ConversationRole::Tool {
        return Vec::new();
    }
    let Some(tool_use_id) = entry.tool_call_id.clone() else {
        return Vec::new();
    };
    let tool_name = entry.name.clone().unwrap_or_default();

    if !entry.content_blocks.is_empty() {
        if content_blocks_have_image(&entry.content_blocks) {
            return Vec::new();
        }
        let size = content_blocks_size(&entry.content_blocks);
        return vec![ToolResultCandidate {
            tool_use_id,
            tool_name,
            content: CandidateContent::Blocks(entry.content_blocks.clone()),
            size,
        }];
    }

    if entry.text.is_empty() || is_content_already_compacted(&entry.text) {
        return Vec::new();
    }
    vec![ToolResultCandidate {
        tool_use_id,
        tool_name,
        size: entry.text.len(),
        content: CandidateContent::Text(entry.text.clone()),
    }]
}

fn select_fresh_to_replace(
    fresh: &[ToolResultCandidate],
    frozen_size: usize,
) -> Vec<ToolResultCandidate> {
    let mut sorted = fresh.to_vec();
    sorted.sort_by(|a, b| b.size.cmp(&a.size));
    let mut selected = Vec::new();
    let mut remaining = frozen_size.saturating_add(
        fresh
            .iter()
            .fold(0usize, |sum, candidate| sum.saturating_add(candidate.size)),
    );
    for candidate in sorted {
        if remaining <= MAX_TOOL_RESULTS_PER_MESSAGE_CHARS {
            break;
        }
        remaining = remaining.saturating_sub(candidate.size);
        selected.push(candidate);
    }
    selected
}

fn replace_tool_result_contents(
    conversation: &mut [ConversationEntry],
    replacement_map: &std::collections::HashMap<String, String>,
) {
    for entry in conversation {
        if entry.role != ConversationRole::Tool {
            continue;
        }
        let Some(tool_use_id) = entry.tool_call_id.as_deref() else {
            continue;
        };
        let Some(replacement) = replacement_map.get(tool_use_id) else {
            continue;
        };
        entry.text.clone_from(replacement);
        entry.content_blocks.clear();
        entry.history_text = None;
    }
}

fn is_content_already_compacted(content: &str) -> bool {
    content.starts_with(PERSISTED_OUTPUT_TAG)
}

fn content_blocks_have_image(content_blocks: &[Value]) -> bool {
    content_blocks
        .iter()
        .any(|block| block.get("type").and_then(Value::as_str) == Some("image"))
}

fn content_blocks_size(content_blocks: &[Value]) -> usize {
    content_blocks
        .iter()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|block| block.get("text").and_then(Value::as_str))
        .map(str::len)
        .sum()
}

pub fn generate_preview(content: &str, max_bytes: usize) -> (String, bool) {
    if content.len() <= max_bytes {
        return (content.to_owned(), false);
    }
    let truncated = char_boundary_prefix(content, max_bytes);
    let last_newline = truncated.rfind('\n');
    let cut_point = last_newline
        .filter(|idx| *idx > max_bytes / 2)
        .unwrap_or(max_bytes);
    let preview = char_boundary_prefix(content, cut_point);
    (preview.to_owned(), true)
}

fn format_file_size(size_in_bytes: usize) -> String {
    let kb = size_in_bytes as f64 / 1024.0;
    if kb < 1.0 {
        return format!("{size_in_bytes} bytes");
    }
    if kb < 1024.0 {
        return trim_trailing_zero(kb, "KB");
    }
    let mb = kb / 1024.0;
    if mb < 1024.0 {
        return trim_trailing_zero(mb, "MB");
    }
    trim_trailing_zero(mb / 1024.0, "GB")
}

fn trim_trailing_zero(value: f64, suffix: &str) -> String {
    format!("{value:.1}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
        + suffix
}

fn filesystem_error_message(path: &Path, kind: std::io::ErrorKind, fallback: &str) -> String {
    match kind {
        std::io::ErrorKind::NotFound => format!("Directory not found: {}", path.display()),
        std::io::ErrorKind::PermissionDenied => format!("Permission denied: {}", path.display()),
        _ => fallback.to_owned(),
    }
}

fn char_boundary_prefix(content: &str, max_bytes: usize) -> &str {
    if content.len() <= max_bytes {
        return content;
    }
    let mut boundary = max_bytes;
    while boundary > 0 && !content.is_char_boundary(boundary) {
        boundary -= 1;
    }
    &content[..boundary]
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use rc_core::ConversationEntry;
    use serde_json::json;
    use tempfile::tempdir;

    use super::{
        PERSISTED_OUTPUT_CLOSING_TAG, PERSISTED_OUTPUT_TAG, PREVIEW_SIZE_BYTES,
        apply_tool_result_budget_to_conversation, build_large_tool_result_message,
        generate_preview, persist_tool_result_blocks, persist_tool_result_text,
        process_tool_result_text, reconstruct_content_replacement_state, session_tool_results_dir,
    };

    #[test]
    fn process_tool_result_text_persists_large_output() {
        let temp = tempdir().expect("tempdir");
        let tool_results_dir = session_tool_results_dir(temp.path());
        let content = "x".repeat(60_000);
        let processed = process_tool_result_text(&content, "call-1", Some(&tool_results_dir), None)
            .expect("process");
        assert!(processed.starts_with(PERSISTED_OUTPUT_TAG));
        assert!(processed.ends_with(PERSISTED_OUTPUT_CLOSING_TAG));
        assert!(tool_results_dir.join("call-1.txt").exists());
    }

    #[test]
    fn generate_preview_prefers_newline_boundary() {
        let content = "line1\nline2\nline3";
        let (preview, has_more) = generate_preview(content, 8);
        assert!(has_more);
        assert_eq!(preview, "line1");
    }

    #[test]
    fn persist_tool_result_blocks_rejects_non_text_blocks() {
        let temp = tempdir().expect("tempdir");
        let result = persist_tool_result_blocks(
            &[serde_json::json!({"type":"image","source":"x"})],
            "call-1",
            temp.path(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn build_large_tool_result_message_includes_preview() {
        let temp = tempdir().expect("tempdir");
        let persisted =
            persist_tool_result_text(&"a".repeat(PREVIEW_SIZE_BYTES + 10), "call-2", temp.path())
                .expect("persist");
        let message = build_large_tool_result_message(&persisted);
        assert!(message.contains("Full output saved to:"));
        assert!(message.contains("Preview (first"));
    }

    #[test]
    fn per_message_budget_persists_largest_fresh_tool_result() {
        let temp = tempdir().expect("tempdir");
        let mut conversation = vec![
            ConversationEntry::assistant(""),
            ConversationEntry::tool("small", "bash_command", "s".repeat(10_000), false),
            ConversationEntry::tool("large", "bash_command", "x".repeat(210_000), false),
        ];
        let mut state = super::ContentReplacementState::new();

        let outcome = apply_tool_result_budget_to_conversation(
            &mut conversation,
            &mut state,
            temp.path(),
            &HashSet::new(),
        )
        .expect("budget");

        assert_eq!(outcome.newly_replaced.len(), 1);
        assert_eq!(outcome.newly_replaced[0].tool_use_id, "large");
        assert!(conversation[2].text.starts_with(PERSISTED_OUTPUT_TAG));
        assert!(temp.path().join("large.txt").exists());
        assert!(state.seen_ids.contains("small"));
        assert!(state.seen_ids.contains("large"));
        assert!(state.replacements.contains_key("large"));
    }

    #[test]
    fn per_message_budget_reapplies_stored_replacement_without_repersisting() {
        let temp = tempdir().expect("tempdir");
        let mut conversation = vec![
            ConversationEntry::assistant(""),
            ConversationEntry::tool("large", "bash_command", "x".repeat(210_000), false),
        ];
        let mut state = super::ContentReplacementState::new();
        let first = apply_tool_result_budget_to_conversation(
            &mut conversation,
            &mut state,
            temp.path(),
            &HashSet::new(),
        )
        .expect("first budget");
        let replacement = first.newly_replaced[0].replacement.clone();

        conversation[1].text = "x".repeat(210_000);
        let second = apply_tool_result_budget_to_conversation(
            &mut conversation,
            &mut state,
            temp.path(),
            &HashSet::new(),
        )
        .expect("second budget");

        assert!(second.newly_replaced.is_empty());
        assert_eq!(conversation[1].text, replacement);
    }

    #[test]
    fn per_message_budget_freezes_seen_unreplaced_results() {
        let temp = tempdir().expect("tempdir");
        let mut conversation = vec![
            ConversationEntry::assistant(""),
            ConversationEntry::tool("medium", "bash_command", "m".repeat(150_000), false),
        ];
        let mut state = super::ContentReplacementState::new();
        let first = apply_tool_result_budget_to_conversation(
            &mut conversation,
            &mut state,
            temp.path(),
            &HashSet::new(),
        )
        .expect("first budget");
        assert!(first.newly_replaced.is_empty());
        assert!(state.seen_ids.contains("medium"));

        conversation.push(ConversationEntry::tool(
            "fresh",
            "bash_command",
            "f".repeat(80_000),
            false,
        ));
        let second = apply_tool_result_budget_to_conversation(
            &mut conversation,
            &mut state,
            temp.path(),
            &HashSet::new(),
        )
        .expect("second budget");

        assert_eq!(second.newly_replaced.len(), 1);
        assert_eq!(second.newly_replaced[0].tool_use_id, "fresh");
        assert_eq!(conversation[1].text, "m".repeat(150_000));
    }

    #[test]
    fn reconstruct_state_freezes_candidates_and_restores_records() {
        let mut conversation = vec![
            ConversationEntry::assistant(""),
            ConversationEntry::tool("replaced", "bash_command", "x".repeat(10), false),
            ConversationEntry::tool("plain", "bash_command", "y".repeat(10), false),
        ];
        conversation[1].content_blocks = vec![json!({"type": "text", "text": "block text"})];
        let records = vec![super::ContentReplacementRecord {
            kind: super::ContentReplacementKind::ToolResult,
            tool_use_id: "replaced".to_owned(),
            replacement: "cached replacement".to_owned(),
        }];

        let state = reconstruct_content_replacement_state(&conversation, &records, None);

        assert!(state.seen_ids.contains("replaced"));
        assert!(state.seen_ids.contains("plain"));
        assert_eq!(
            state.replacements.get("replaced").map(String::as_str),
            Some("cached replacement")
        );
        assert!(!state.replacements.contains_key("plain"));
    }
}

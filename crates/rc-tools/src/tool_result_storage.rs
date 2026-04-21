use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use once_cell::sync::Lazy;
use serde_json::Value;

pub const DEFAULT_MAX_RESULT_SIZE_CHARS: usize = 50_000;
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
    use tempfile::tempdir;

    use super::{
        PERSISTED_OUTPUT_CLOSING_TAG, PERSISTED_OUTPUT_TAG, PREVIEW_SIZE_BYTES,
        build_large_tool_result_message, generate_preview, persist_tool_result_blocks,
        persist_tool_result_text, process_tool_result_text, session_tool_results_dir,
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
}

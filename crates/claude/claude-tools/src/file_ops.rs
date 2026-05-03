//! File operation tools: list_directory, read_file, search_text, write_file,
//! replace_in_file, edit_file, glob_files, grep_files.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result, anyhow};
use globset::GlobBuilder;
use ignore::WalkBuilder;
use claude_permissions::{FilesystemOperation, assess_filesystem_access};
use regex::Regex;
use serde_json::Value;
use walkdir::WalkDir;

use super::{FileState, IGNORED_DIRS, ToolExecutionContext};

const FILE_UNCHANGED_STUB: &str = "File unchanged since last read. The content from the earlier Read tool_result in this conversation is still current — refer to that instead of re-reading.";
const FILE_UNEXPECTEDLY_MODIFIED_ERROR: &str =
    "File has been unexpectedly modified. Read it again before attempting to write it.";

const BLOCKED_DEVICE_PATHS: &[&str] = &[
    "/dev/zero",
    "/dev/random",
    "/dev/urandom",
    "/dev/full",
    "/dev/stdin",
    "/dev/tty",
    "/dev/console",
    "/dev/stdout",
    "/dev/stderr",
    "/dev/fd/0",
    "/dev/fd/1",
    "/dev/fd/2",
];

fn is_blocked_device_path(path: &str) -> bool {
    if BLOCKED_DEVICE_PATHS.contains(&path) {
        return true;
    }
    // /proc/self/fd/0-2 and /proc/<pid>/fd/0-2 are Linux aliases for stdio
    if path.starts_with("/proc/")
        && (path.ends_with("/fd/0")
            || path.ends_with("/fd/1")
            || path.ends_with("/fd/2"))
    {
        return true;
    }
    false
}

tokio::task_local! {
    static TOOL_FILESYSTEM_PERMISSION_CONFIRMED: bool;
}

pub(crate) async fn with_filesystem_permission_confirmed<F, T>(confirmed: bool, future: F) -> T
where
    F: std::future::Future<Output = T>,
{
    TOOL_FILESYSTEM_PERMISSION_CONFIRMED
        .scope(confirmed, future)
        .await
}

fn filesystem_permission_confirmed_for_dispatch() -> bool {
    TOOL_FILESYSTEM_PERMISSION_CONFIRMED
        .try_with(|confirmed| *confirmed)
        .unwrap_or(false)
}

fn normalize_quotes(s: &str) -> String {
    s.replace('\u{201C}', "\"")
     .replace('\u{201D}', "\"")
     .replace('\u{2018}', "'")
     .replace('\u{2019}', "'")
}

fn find_actual_string(file_content: &str, search_string: &str) -> Option<String> {
    // First try exact match
    if file_content.contains(search_string) {
        return Some(search_string.to_owned());
    }
    // Try with normalized quotes — both sides get curly→straight normalization
    let normalized_search = normalize_quotes(search_string);
    let normalized_file = normalize_quotes(file_content);
    if let Some(index) = normalized_file.find(&normalized_search) {
        // Extract the actual substring from the original file content at the same position
        let end = index + search_string.len();
        if end <= file_content.len() {
            return Some(file_content[index..end].to_owned());
        }
    }
    None
}

fn normalize_for_comparison(path: PathBuf) -> PathBuf {
    let rendered = path.to_string_lossy();
    if let Some(stripped) = rendered.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        path
    }
}

fn file_mtime_ms(path: &Path) -> Result<u128> {
    let modified = std::fs::metadata(path)?.modified()?;
    Ok(modified
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis())
}

fn ensure_current_read_state(
    context: &ToolExecutionContext,
    target: &Path,
    current_content: &str,
) -> Result<()> {
    let Some(read_state) = context.read_file_state.get(target) else {
        return Err(anyhow!(
            "File has not been read yet. Read it first before writing to it."
        ));
    };
    if read_state.is_partial_view {
        return Err(anyhow!(
            "File has not been read yet. Read it first before writing to it."
        ));
    }

    let last_write_time = file_mtime_ms(target)?;
    if last_write_time > read_state.timestamp {
        let content_unchanged = read_state.offset.is_none()
            && read_state.limit.is_none()
            && current_content == read_state.content;
        if !content_unchanged {
            return Err(anyhow!(
                "File has been modified since read, either by the user or by a linter. Read it again before attempting to write it."
            ));
        }
    }
    Ok(())
}

fn ensure_current_read_state_before_atomic_write(
    context: &ToolExecutionContext,
    target: &Path,
    current_content: &str,
) -> Result<()> {
    let Some(read_state) = context.read_file_state.get(target) else {
        return Err(anyhow!(FILE_UNEXPECTEDLY_MODIFIED_ERROR));
    };
    let last_write_time = file_mtime_ms(target)?;
    if last_write_time > read_state.timestamp {
        let content_unchanged = read_state.offset.is_none()
            && read_state.limit.is_none()
            && current_content == read_state.content;
        if !content_unchanged {
            return Err(anyhow!(FILE_UNEXPECTEDLY_MODIFIED_ERROR));
        }
    }
    Ok(())
}

fn update_post_write_state(context: &ToolExecutionContext, target: &Path, content: String) {
    if let Ok(timestamp) = file_mtime_ms(target) {
        context
            .read_file_state
            .set(target, FileState::post_write(content, timestamp));
    }
}

pub(crate) fn file_path_input(input: &Value) -> Option<&str> {
    input
        .get("file_path")
        .or_else(|| input.get("path"))
        .and_then(Value::as_str)
        .filter(|path| !path.trim().is_empty())
}

pub(crate) fn resolve_workspace_path_for_operation(
    context: &ToolExecutionContext,
    maybe_relative: Option<&str>,
    operation: FilesystemOperation,
) -> Result<PathBuf> {
    let raw_path = maybe_relative.unwrap_or(".");
    let options = crate::filesystem_access_options();
    let check = assess_filesystem_access(raw_path, &context.cwd, &options, operation);

    if check.allowed
        || (check.requires_confirmation && filesystem_permission_confirmed_for_dispatch())
    {
        return Ok(check.normalized_path);
    }

    Err(anyhow!(
        "{}: {}",
        check
            .reason
            .unwrap_or_else(|| "Path is not allowed".to_owned()),
        check.normalized_path.display()
    ))
}

fn canonical_plan_file_path(plan_file_path: &Path) -> PathBuf {
    if plan_file_path.exists() {
        normalize_for_comparison(
            plan_file_path
                .canonicalize()
                .unwrap_or_else(|_| plan_file_path.to_path_buf()),
        )
    } else {
        let parent = plan_file_path.parent().unwrap_or_else(|| Path::new("."));
        let file_name = plan_file_path.file_name().unwrap_or_default();
        let canonical_parent = normalize_for_comparison(
            parent
                .canonicalize()
                .unwrap_or_else(|_| parent.to_path_buf()),
        );
        canonical_parent.join(file_name)
    }
}

fn maybe_persist_plan_snapshot(target: &Path) {
    let target_path = canonical_plan_file_path(target);
    let is_active_plan_file = current_plan_file_path()
        .as_ref()
        .map(|plan_path| canonical_plan_file_path(plan_path))
        .is_some_and(|plan_path| plan_path == target_path);
    if is_active_plan_file {
        let _ = crate::plan_mode::persist_plan_snapshot_if_active();
    }
}

pub(crate) fn list_directory(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let target = resolve_workspace_path_for_operation(
        context,
        input.get("path").and_then(Value::as_str),
        FilesystemOperation::Read,
    )?;
    let recursive = input
        .get("recursive")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let max_entries = input
        .get("max_entries")
        .and_then(Value::as_u64)
        .unwrap_or(200) as usize;
    let mut builder = WalkBuilder::new(&target);
    builder.hidden(false);
    if !recursive {
        builder.max_depth(Some(1));
    }
    let mut lines = Vec::new();
    for entry in builder.build().take(max_entries) {
        let entry = entry?;
        let path = entry.path();
        if path == target {
            continue;
        }
        if path.components().any(|component| {
            IGNORED_DIRS.contains(&component.as_os_str().to_string_lossy().as_ref())
        }) {
            continue;
        }
        let relative = path.strip_prefix(&context.cwd).unwrap_or(path);
        let marker = if entry
            .file_type()
            .is_some_and(|file_type| file_type.is_dir())
        {
            "dir"
        } else {
            "file"
        };
        lines.push(format!("[{marker}] {}", relative.display()));
    }
    if lines.is_empty() {
        Ok("No files matched.".to_owned())
    } else {
        Ok(lines.join("\n"))
    }
}

pub fn read_file(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let path = file_path_input(input).ok_or_else(|| anyhow!("read_file requires file_path"))?;
    if is_blocked_device_path(path) {
        return Err(anyhow!(
            "Cannot read '{}': this device file would block or produce infinite output.",
            path
        ));
    }
    let target =
        resolve_workspace_path_for_operation(context, Some(path), FilesystemOperation::Read)?;
    let start_line = input
        .get("offset")
        .or_else(|| input.get("start_line"))
        .and_then(Value::as_u64)
        .unwrap_or(1) as usize;
    let limit = input
        .get("limit")
        .and_then(Value::as_u64)
        .map(|value| value as usize);
    let end_line = limit
        .map(|limit| start_line.saturating_add(limit).saturating_sub(1))
        .or_else(|| {
            input
                .get("end_line")
                .and_then(Value::as_u64)
                .map(|value| value as usize)
        })
        .unwrap_or(usize::MAX);
    let max_chars = input
        .get("max_chars")
        .and_then(Value::as_u64)
        .unwrap_or(50_000) as usize;

    if let Some(read_state) = context.read_file_state.get(&target)
        && !read_state.is_partial_view
        && read_state.offset == Some(start_line)
        && read_state.limit == limit
        && let Ok(mtime) = file_mtime_ms(&target)
        && mtime <= read_state.timestamp
    {
        return Ok(FILE_UNCHANGED_STUB.to_owned());
    }

    let ext = target.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg") {
        let data = std::fs::read(&target)?;
        let mime = match ext.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "svg" => "image/svg+xml",
            _ => "application/octet-stream",
        };
        let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &data);
        return Ok(format!("Image file: {}\nMIME type: {}\nSize: {} bytes\nBase64 data: data:{};base64,{}",
            target.display(), mime, data.len(), mime, &b64[..b64.len().min(50000)]));
    }

    if ext == "pdf" {
        let file_size = std::fs::metadata(&target).map(|m| m.len()).unwrap_or(0);
        let pages_param = input
            .get("pages")
            .and_then(Value::as_str);

        // Try pdftotext (poppler-utils) for text extraction
        if let Ok(text) = extract_pdf_via_pdftotext(&target, pages_param, file_size) {
            if let Ok(timestamp) = file_mtime_ms(&target) {
                context.read_file_state.set(
                    &target,
                    FileState::read(text.clone(), timestamp, start_line, limit),
                );
            }
            return Ok(text);
        }

        // Try pdftotext without page range as fallback
        if pages_param.is_some() {
            if let Ok(text) = extract_pdf_via_pdftotext(&target, None, file_size) {
                if let Ok(timestamp) = file_mtime_ms(&target) {
                    context.read_file_state.set(
                        &target,
                        FileState::read(text.clone(), timestamp, start_line, limit),
                    );
                }
                return Ok(text);
            }
        }

        return Err(anyhow!(
            "PDF reading requires `pdftotext` from poppler-utils. \
             Install with: `brew install poppler` (macOS) or `apt-get install poppler-utils` (Debian/Ubuntu).\n\
             File: {} ({} bytes)",
            target.display(),
            file_size
        ));
    }

    // Notebook (.ipynb) files
    if ext == "ipynb" {
        let contents = std::fs::read_to_string(&target)
            .with_context(|| format!("failed to read {}", target.display()))?;
        let rendered = render_notebook_cells(&contents, start_line, end_line, max_chars)?;
        if let Ok(timestamp) = file_mtime_ms(&target) {
            let raw_cells = render_notebook_cells(&contents, 1, usize::MAX, usize::MAX)
                .unwrap_or_default();
            context.read_file_state.set(
                &target,
                FileState::read(raw_cells, timestamp, start_line, limit),
            );
        }
        return Ok(rendered);
    }

    let contents = std::fs::read_to_string(&target)
        .with_context(|| format!("failed to read {}", target.display()))?;
    let raw_selected = contents
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let line_number = index + 1;
            if line_number < start_line || line_number > end_line {
                None
            } else {
                Some(line.to_owned())
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let selected = contents
        .lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let line_number = index + 1;
            if line_number < start_line || line_number > end_line {
                None
            } else {
                Some(format!("{line_number:>4} {line}"))
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    let selected = selected.chars().take(max_chars).collect::<String>();
    if let Ok(timestamp) = file_mtime_ms(&target) {
        context.read_file_state.set(
            &target,
            FileState::read(raw_selected, timestamp, start_line, limit),
        );
    }
    Ok(selected)
}

pub(crate) fn search_text(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let pattern = input
        .get("pattern")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("search_text requires a pattern"))?;
    let target = resolve_workspace_path_for_operation(
        context,
        input.get("path").and_then(Value::as_str),
        FilesystemOperation::Read,
    )?;
    let regex = Regex::new(pattern).or_else(|_| Regex::new(&regex::escape(pattern)))?;
    let max_matches = input
        .get("max_matches")
        .and_then(Value::as_u64)
        .unwrap_or(50) as usize;
    let mut matches = Vec::new();
    for entry in WalkDir::new(&target).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().components().any(|component| {
            IGNORED_DIRS.contains(&component.as_os_str().to_string_lossy().as_ref())
        }) {
            continue;
        }
        let Ok(contents) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        for (index, line) in contents.lines().enumerate() {
            if regex.is_match(line) {
                let relative = entry
                    .path()
                    .strip_prefix(&context.cwd)
                    .unwrap_or(entry.path());
                matches.push(format!(
                    "{}:{}:{}",
                    relative.display(),
                    index + 1,
                    line.trim()
                ));
                if matches.len() >= max_matches {
                    return Ok(matches.join("\n"));
                }
            }
        }
    }
    if matches.is_empty() {
        Ok("No matches found.".to_owned())
    } else {
        Ok(matches.join("\n"))
    }
}

pub(crate) fn write_file(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let path = file_path_input(input).ok_or_else(|| anyhow!("write_file requires file_path"))?;
    let content = input
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("write_file requires content"))?;
    let append = input
        .get("append")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let target =
        resolve_workspace_path_for_operation(context, Some(path), FilesystemOperation::Create)?;
    let existing = match std::fs::read_to_string(&target) {
        Ok(existing) => Some(existing),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    if let Some(existing) = existing.as_deref() {
        ensure_current_read_state(context, &target, existing)?;
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if append {
        let existing = existing.unwrap_or_default();
        ensure_current_read_state_before_atomic_write(context, &target, &existing).or_else(
            |error| {
                if target.exists() { Err(error) } else { Ok(()) }
            },
        )?;
        use std::io::Write as _;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&target)?;
        file.write_all(content.as_bytes())?;
    } else {
        if let Some(existing) = existing.as_deref() {
            ensure_current_read_state_before_atomic_write(context, &target, existing)?;
        }
        std::fs::write(&target, content)?;
    }
    let final_content = if append {
        std::fs::read_to_string(&target).unwrap_or_else(|_| content.to_owned())
    } else {
        content.to_owned()
    };
    update_post_write_state(context, &target, final_content);
    maybe_persist_plan_snapshot(&target);
    Ok(format!("Wrote {}", target.display()))
}

pub(crate) fn replace_in_file(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let path =
        file_path_input(input).ok_or_else(|| anyhow!("replace_in_file requires file_path"))?;
    let search = input
        .get("search")
        .or_else(|| input.get("old_string"))
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("replace_in_file requires search text"))?;
    let replace = input
        .get("replace")
        .or_else(|| input.get("new_string"))
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("replace_in_file requires replacement text"))?;
    let replace_all = input
        .get("all")
        .or_else(|| input.get("replace_all"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let target =
        resolve_workspace_path_for_operation(context, Some(path), FilesystemOperation::Write)?;
    let original = std::fs::read_to_string(&target)?;
    ensure_current_read_state(context, &target, &original)?;
    if search == replace {
        return Err(anyhow!(
            "No changes to make: old_string and new_string are exactly the same."
        ));
    }
    let actual_search = match find_actual_string(&original, search) {
        Some(s) => s,
        None => {
            return Err(anyhow!(
                "String to replace not found in file.\nString: {search}"
            ));
        }
    };
    let match_count = original.matches(&*actual_search).count();
    if match_count > 1 && !replace_all {
        return Err(anyhow!("Found {match_count} occurrences of the search string. Use 'all: true' to replace all, or provide a more specific search string."));
    }
    let updated = if replace_all {
        original.replace(&*actual_search, replace)
    } else {
        original.replacen(&*actual_search, replace, 1)
    };
    ensure_current_read_state_before_atomic_write(context, &target, &original)?;
    std::fs::write(&target, updated)?;
    let updated = std::fs::read_to_string(&target).unwrap_or_default();
    update_post_write_state(context, &target, updated);
    maybe_persist_plan_snapshot(&target);
    Ok(format!("Updated {}", target.display()))
}

pub(crate) fn edit_file(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let path = file_path_input(input).ok_or_else(|| anyhow!("edit_file requires file_path"))?;
    let target =
        resolve_workspace_path_for_operation(context, Some(path), FilesystemOperation::Write)?;
    let legacy_edits;
    let edits = if let Some(edits) = input.get("edits").and_then(Value::as_array) {
        edits
    } else {
        let old_string = input
            .get("old_string")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("edit_file requires old_string"))?;
        let new_string = input
            .get("new_string")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("edit_file requires new_string"))?;
        if old_string == new_string {
            return Err(anyhow!(
                "No changes to make: old_string and new_string are exactly the same."
            ));
        }
        legacy_edits = vec![serde_json::json!({
            "search": old_string,
            "replace": new_string,
            "all": input
                .get("replace_all")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        })];
        &legacy_edits
    };
    let create_if_missing = input
        .get("create_if_missing")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut content = if target.exists() {
        let content = std::fs::read_to_string(&target)?;
        ensure_current_read_state(context, &target, &content)?;
        content
    } else if create_if_missing
        || edits
            .first()
            .and_then(|edit| edit.get("search").and_then(Value::as_str))
            == Some("")
    {
        String::new()
    } else {
        return Err(anyhow!("{} does not exist", target.display()));
    };
    let original_content = content.clone();
    for edit in edits {
        let search = edit
            .get("search")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("edit is missing search"))?;
        let replace = edit
            .get("replace")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("edit is missing replace"))?;
        let search = normalize_quotes(search);
        let replace = normalize_quotes(replace);
        let replace_all = edit.get("all").and_then(Value::as_bool).unwrap_or(false);
        if search.is_empty() {
            if content.is_empty() {
                content = replace.to_owned();
                continue;
            }
            return Err(anyhow!("Cannot create new file - file already exists."));
        }
        if search == replace {
            return Err(anyhow!(
                "No changes to make: old_string and new_string are exactly the same."
            ));
        }
        let actual_search = match find_actual_string(&content, &search) {
            Some(s) => s,
            None => {
                return Err(anyhow!(
                    "String to replace not found in file.\nString: {search}"
                ));
            }
        };
        let matches = content.matches(&*actual_search).count();
        if matches > 1 && !replace_all {
            return Err(anyhow!(
                "Found {matches} matches of the string to replace, but replace_all is false. To replace all occurrences, set replace_all to true. To replace only one occurrence, please provide more context to uniquely identify the instance.\nString: {search}"
            ));
        }
        content = if replace_all {
            content.replace(&*actual_search, &*replace)
        } else {
            content.replacen(&*actual_search, &*replace, 1)
        };
    }
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if target.exists() {
        ensure_current_read_state_before_atomic_write(context, &target, &original_content)?;
    }
    std::fs::write(&target, content)?;
    let updated = std::fs::read_to_string(&target).unwrap_or_default();
    update_post_write_state(context, &target, updated);
    maybe_persist_plan_snapshot(&target);
    Ok(format!(
        "Applied {} edits to {}",
        edits.len(),
        target.display()
    ))
}

pub(crate) fn glob_files(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let pattern = input
        .get("pattern")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("glob requires a pattern"))?;
    let base = resolve_workspace_path_for_operation(
        context,
        input.get("path").and_then(Value::as_str),
        FilesystemOperation::Read,
    )?;
    let full_pattern = format!("{}/{}", base.display(), pattern).replace('\\', "/");
    let mut results = Vec::new();
    let entries = glob::glob(&full_pattern).context("invalid glob pattern")?;
    for entry in entries {
        let path = match entry {
            Ok(p) => p,
            Err(_) => continue,
        };
        if path.is_dir() {
            continue;
        }
        let canonical_path = path.canonicalize().unwrap_or_else(|_| path.clone());
        let canonical_cwd = context
            .cwd
            .canonicalize()
            .unwrap_or_else(|_| context.cwd.clone());
        if !canonical_path.starts_with(&canonical_cwd) {
            continue;
        }
        let relative = path.strip_prefix(&context.cwd).unwrap_or(&path);
        results.push(relative.display().to_string());
    }
    if results.is_empty() {
        Ok("No files matched.".to_owned())
    } else {
        Ok(results.join("\n"))
    }
}

pub(crate) fn grep_files(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let pattern = input
        .get("pattern")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("grep requires a pattern"))?;
    let target = resolve_workspace_path_for_operation(
        context,
        input.get("path").and_then(Value::as_str),
        FilesystemOperation::Read,
    )?;
    // Accept both "glob" (TS-compatible) and "include" (legacy) field names
    let glob_pattern = input
        .get("glob")
        .or_else(|| input.get("include"))
        .and_then(Value::as_str);
    let output_mode = input
        .get("output_mode")
        .and_then(Value::as_str)
        .unwrap_or("files_with_matches");
    let head_limit = input
        .get("head_limit")
        .and_then(Value::as_u64)
        .unwrap_or(250) as usize;
    let offset = input
        .get("offset")
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;

    // Context lines: -C overrides -A and -B
    let context_before = input
        .get("-B")
        .or_else(|| input.get("-C"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;
    let context_after = input
        .get("-A")
        .or_else(|| input.get("-C"))
        .and_then(Value::as_u64)
        .unwrap_or(0) as usize;

    // Use default 1-line context when no explicit context is given (for content mode)
    let (ctx_before, ctx_after) = if context_before == 0 && context_after == 0 {
        (1, 1)
    } else {
        (context_before, context_after)
    };

    if !["content", "files_with_matches", "count"].contains(&output_mode) {
        return Err(anyhow!(
            "output_mode must be 'content', 'files_with_matches', or 'count'"
        ));
    }

    // -i flag for explicit case sensitivity control; auto-detect if not set
    let explicit_case_insensitive = input.get("-i").and_then(Value::as_bool);
    let case_insensitive = explicit_case_insensitive.unwrap_or_else(|| {
        pattern
            .chars()
            .all(|c| c.is_ascii_lowercase() || !c.is_alphabetic())
    });

    let re = regex::RegexBuilder::new(pattern)
        .case_insensitive(case_insensitive)
        .build()
        .or_else(|_| regex::RegexBuilder::new(&regex::escape(pattern)).build())?;

    let file_matcher: Option<globset::GlobMatcher> = match glob_pattern {
        Some(fp) => Some(
            GlobBuilder::new(fp)
                .literal_separator(true)
                .build()
                .context("invalid glob pattern")?
                .compile_matcher(),
        ),
        None => None,
    };

    let mut walker = WalkBuilder::new(&target);
    walker.hidden(false).git_ignore(true).git_exclude(true);

    if let Some(matcher) = file_matcher {
        walker.filter_entry(move |entry| {
            if entry.file_type().map_or(false, |ft| ft.is_dir()) {
                return true;
            }
            entry
                .path()
                .file_name()
                .is_some_and(|name| matcher.is_match(name))
        });
    }

    let mut files_with_matches: Vec<(PathBuf, u128)> = Vec::new();
    let mut count_per_file: Vec<(PathBuf, usize)> = Vec::new();
    let mut content_matches: Vec<String> = Vec::new();
    let mut total_content_matches = 0usize;

    for entry in walker.build().filter_map(|e| e.ok()) {
        if !entry.file_type().map_or(false, |ft| ft.is_file()) {
            continue;
        }
        let path = entry.path().to_path_buf();
        let contents = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let relative = path
            .strip_prefix(&context.cwd)
            .unwrap_or(&path)
            .to_path_buf();
        let lines: Vec<&str> = contents.lines().collect();

        match output_mode {
            "files_with_matches" => {
                if lines.iter().any(|line| re.is_match(line)) {
                    let mtime = file_mtime_ms(&path).unwrap_or(0);
                    files_with_matches.push((relative, mtime));
                }
            }
            "count" => {
                let count = lines.iter().filter(|l| re.is_match(l)).count();
                if count > 0 {
                    count_per_file.push((relative, count));
                }
            }
            _ => {
                for (index, line) in lines.iter().enumerate() {
                    if re.is_match(line) {
                        total_content_matches += 1;
                        if total_content_matches <= head_limit {
                            let start = index.saturating_sub(ctx_before);
                            let end = (index + ctx_after + 1).min(lines.len());
                            for (offset, context_line) in lines[start..end].iter().enumerate() {
                                let line_idx = start + offset;
                                let prefix = if line_idx == index { ">" } else { " " };
                                content_matches.push(format!(
                                    "{}:{}{} {}",
                                    relative.display(),
                                    line_idx + 1,
                                    prefix,
                                    context_line.trim_end()
                                ));
                            }
                            content_matches.push(String::new());
                        }
                    }
                }
            }
        }

        if output_mode == "content" && total_content_matches >= head_limit {
            break;
        }
    }

    match output_mode {
        "files_with_matches" => {
            if files_with_matches.is_empty() {
                return Ok("No files matched.".to_owned());
            }
            files_with_matches.sort_by(|a, b| b.1.cmp(&a.1));
            let skip = offset.min(files_with_matches.len());
            files_with_matches.drain(..skip);
            let truncated = files_with_matches.len() > head_limit;
            files_with_matches.truncate(head_limit);
            let mut out: Vec<String> = files_with_matches
                .iter()
                .map(|(p, _)| format!("{}", p.display()))
                .collect();
            if truncated {
                out.push(
                    "\nFiles still truncated. Consider using a more specific path or pattern."
                        .to_owned(),
                );
            }
            Ok(out.join("\n"))
        }
        "count" => {
            if count_per_file.is_empty() {
                return Ok("No files matched.".to_owned());
            }
            let lines: Vec<String> = count_per_file
                .iter()
                .map(|(path, count)| format!("{}:{}", path.display(), count))
                .collect();
            Ok(lines.join("\n"))
        }
        _ => {
            if content_matches.is_empty() {
                return Ok("No files matched.".to_owned());
            }
            let truncated = total_content_matches > head_limit;
            if truncated {
                content_matches.push(format!(
                    "\n[Showing first {} of {} results. Use a more specific pattern to narrow results.]",
                    head_limit.min(total_content_matches),
                    total_content_matches
                ));
            }
            Ok(content_matches.join("\n").trim_end().to_owned())
        }
    }
}

fn current_plan_file_path() -> Option<PathBuf> {
    crate::plan_mode::current_plan_file_path()
}

fn extract_pdf_via_pdftotext(
    path: &Path,
    pages: Option<&str>,
    file_size: u64,
) -> Result<String> {
    let pdftotext = which_pdftotext()?;
    let mut cmd = std::process::Command::new(&pdftotext);
    cmd.arg("-layout");

    if let Some(page_range) = pages {
        let (start, end) = parse_pdf_page_range(page_range)?;
        if end - start + 1 > 20 {
            return Err(anyhow!(
                "Maximum 20 pages per read request. Requested {} pages.",
                end - start + 1
            ));
        }
        cmd.arg("-f").arg(start.to_string());
        cmd.arg("-l").arg(end.to_string());
    } else if file_size > 5_000_000 {
        cmd.arg("-f").arg("1").arg("-l").arg("20");
    }

    let output = cmd.arg(path).arg("-").output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("pdftotext failed: {}", stderr.trim()));
    }

    let text = String::from_utf8_lossy(&output.stdout);
    let truncated: String = text.chars().take(50_000).collect();
    let header = format!(
        "PDF file: {} ({} bytes)\n{}\n",
        path.display(),
        file_size,
        if pages.is_some() {
            format!("Pages: {}", pages.unwrap())
        } else {
            "Full document".to_owned()
        }
    );
    Ok(format!("{}{}", header, truncated))
}

fn which_pdftotext() -> Result<PathBuf> {
    #[cfg(windows)]
    let resolver = "where";
    #[cfg(not(windows))]
    let resolver = "which";

    let output = std::process::Command::new(resolver)
        .arg("pdftotext")
        .output()?;

    if !output.status.success() {
        return Err(anyhow!("pdftotext not found in PATH"));
    }

    let path = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("pdftotext not found"))?;

    if path.exists() {
        Ok(path)
    } else {
        Err(anyhow!("pdftotext not found at {}", path.display()))
    }
}

fn parse_pdf_page_range(range: &str) -> Result<(u32, u32)> {
    let range = range.trim();
    if let Some((s, e)) = range.split_once('-') {
        let start: u32 = s.trim().parse().map_err(|_| anyhow!("Invalid page number: {}", s))?;
        let end: u32 = e.trim().parse().map_err(|_| anyhow!("Invalid page number: {}", e))?;
        if start == 0 || end == 0 || start > end {
            return Err(anyhow!("Invalid page range: {}", range));
        }
        return Ok((start, end));
    }
    let pages: Vec<u32> = range
        .split(',')
        .filter_map(|p| p.trim().parse().ok())
        .filter(|&p| p > 0)
        .collect();
    if pages.is_empty() {
        return Err(anyhow!("Invalid page range: {}", range));
    }
    let start = *pages.iter().min().unwrap();
    let end = *pages.iter().max().unwrap();
    Ok((start, end))
}

fn render_notebook_cells(
    raw: &str,
    start_line: usize,
    end_line: usize,
    max_chars: usize,
) -> Result<String> {
    let notebook: serde_json::Value = serde_json::from_str(raw)
        .with_context(|| "failed to parse .ipynb JSON")?;

    let cells = notebook
        .get("cells")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("notebook has no cells array"))?;

    let mut lines = Vec::new();
    lines.push(format!("Notebook: {} cells\n", cells.len()));

    for (idx, cell) in cells.iter().enumerate() {
        let cell_type = cell.get("cell_type").and_then(Value::as_str).unwrap_or("unknown");
        let cell_id = cell.get("id").and_then(Value::as_str).unwrap_or("?");
        let source = cell
            .get("source")
            .and_then(|s| {
                if s.is_array() {
                    Some(
                        s.as_array()
                            .unwrap()
                            .iter()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join(""),
                    )
                } else {
                    s.as_str().map(|s| s.to_owned())
                }
            })
            .unwrap_or_default();

        let outputs = cell
            .get("outputs")
            .and_then(|o| o.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|out| {
                        out.get("text")
                            .or_else(|| out.get("data").and_then(|d| d.get("text/plain")))
                            .and_then(|t| {
                                if t.is_array() {
                                    Some(
                                        t.as_array()
                                            .unwrap()
                                            .iter()
                                            .filter_map(Value::as_str)
                                            .collect::<Vec<_>>()
                                            .join(""),
                                    )
                                } else {
                                    t.as_str().map(|s| s.to_owned())
                                }
                            })
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

        let cell_header = format!("--- Cell {} ({}) [id={}] ---", idx + 1, cell_type, cell_id);
        lines.push(cell_header);
        for source_line in source.lines() {
            lines.push(source_line.to_owned());
        }
        if !outputs.is_empty() {
            lines.push("Output:".to_owned());
            for output_line in outputs.lines() {
                lines.push(format!("  {}", output_line));
            }
        }
        lines.push(String::new());
    }

    let all_text = lines.join("\n");
    let rendered: String = all_text
        .lines()
        .enumerate()
        .filter_map(|(i, line)| {
            let line_num = i + 1;
            if line_num < start_line || line_num > end_line {
                None
            } else {
                Some(format!("{line_num:>4} {line}"))
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok(rendered.chars().take(max_chars).collect())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use once_cell::sync::Lazy;
    use serde_json::json;
    use tempfile::tempdir;

    use super::*;
    use crate::plan_mode::{self, ExitPlanModeInput, PlanModeRuntime, PlanModeRuntimeSnapshot};

    static FILE_OPS_TEST_MUTEX: Lazy<std::sync::Mutex<()>> =
        Lazy::new(|| std::sync::Mutex::new(()));

    #[derive(Debug)]
    struct StubPlanRuntime {
        plan_file_path: PathBuf,
    }

    impl PlanModeRuntime for StubPlanRuntime {
        fn enter_plan_mode(&self, _objective: &str) -> Result<String> {
            Ok(String::new())
        }

        fn exit_plan_mode(&self, _input: ExitPlanModeInput) -> Result<String> {
            Ok(String::new())
        }

        fn snapshot(&self) -> PlanModeRuntimeSnapshot {
            PlanModeRuntimeSnapshot {
                permission_mode: claude_core::PermissionMode::Plan,
                plan_file_path: Some(self.plan_file_path.clone()),
            }
        }

        fn persist_plan_snapshot(&self) -> Result<()> {
            // Stub: no-op for tests
            Ok(())
        }
    }

    #[test]
    fn resolve_workspace_path_allows_active_plan_file_outside_workspace() {
        let _guard = FILE_OPS_TEST_MUTEX.lock().expect("test mutex");
        let tempdir = tempdir().expect("tempdir");
        let workspace = tempdir.path().join("workspace");
        let profile = tempdir.path().join(".remote-code-rust");
        let plan_file = profile.join("plans").join("plan.md");
        std::fs::create_dir_all(&workspace).expect("workspace");
        std::fs::create_dir_all(plan_file.parent().expect("plan dir")).expect("plans");

        plan_mode::configure_plan_mode_runtime(Some(Arc::new(StubPlanRuntime {
            plan_file_path: plan_file.clone(),
        })))
        .expect("install plan runtime");

        let context = ToolExecutionContext {
            cwd: workspace.clone(),
            ..ToolExecutionContext::default()
        };
        let result = write_file(
            &json!({
                "path": plan_file.to_string_lossy().to_string(),
                "content": "# plan"
            }),
            &context,
        )
        .expect("plan file write");

        assert!(result.contains("plan.md"));
        assert_eq!(
            std::fs::read_to_string(plan_file).expect("plan file"),
            "# plan"
        );

        plan_mode::configure_plan_mode_runtime(None).expect("clear plan runtime");
    }
}

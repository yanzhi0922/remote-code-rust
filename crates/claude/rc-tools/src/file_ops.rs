//! File operation tools: list_directory, read_file, search_text, write_file,
//! replace_in_file, edit_file, glob_files, grep_files.

use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use anyhow::{Context, Result, anyhow};
use globset::GlobBuilder;
use ignore::WalkBuilder;
use rc_permissions::{FilesystemOperation, assess_filesystem_access};
use regex::Regex;
use serde_json::Value;
use walkdir::WalkDir;

use super::{FileState, IGNORED_DIRS, ToolExecutionContext};

const FILE_UNCHANGED_STUB: &str = "File unchanged since last read. The content from the earlier Read tool_result in this conversation is still current — refer to that instead of re-reading.";
const FILE_UNEXPECTEDLY_MODIFIED_ERROR: &str =
    "File has been unexpectedly modified. Read it again before attempting to write it.";

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
    if !original.contains(search) {
        return Err(anyhow!(
            "String to replace not found in file.\nString: {search}"
        ));
    }
    let updated = if replace_all {
        original.replace(search, replace)
    } else {
        original.replacen(search, replace, 1)
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
        let matches = content.matches(search).count();
        if matches == 0 {
            return Err(anyhow!(
                "String to replace not found in file.\nString: {search}"
            ));
        }
        if matches > 1 && !replace_all {
            return Err(anyhow!(
                "Found {matches} matches of the string to replace, but replace_all is false. To replace all occurrences, set replace_all to true. To replace only one occurrence, please provide more context to uniquely identify the instance.\nString: {search}"
            ));
        }
        content = if replace_all {
            content.replace(search, replace)
        } else {
            content.replacen(search, replace, 1)
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
    let include = input.get("include").and_then(Value::as_str);
    let output_mode = input
        .get("output_mode")
        .and_then(Value::as_str)
        .unwrap_or("content");
    if !["content", "files_with_matches", "count"].contains(&output_mode) {
        return Err(anyhow!(
            "output_mode must be 'content', 'files_with_matches', or 'count'"
        ));
    }
    let regex = Regex::new(pattern).or_else(|_| Regex::new(&regex::escape(pattern)))?;
    let file_matcher: Option<globset::GlobMatcher> = match include {
        Some(fp) => Some(
            GlobBuilder::new(fp)
                .literal_separator(true)
                .build()
                .context("invalid include pattern")?
                .compile_matcher(),
        ),
        None => None,
    };
    let mut content_matches = Vec::new();
    let mut files_with_matches: Vec<String> = Vec::new();
    let mut count_per_file: Vec<(String, usize)> = Vec::new();
    let mut match_count = 0usize;
    for entry in WalkDir::new(&target).into_iter().filter_map(Result::ok) {
        if !entry.file_type().is_file() {
            continue;
        }
        if entry.path().components().any(|component| {
            IGNORED_DIRS.contains(&component.as_os_str().to_string_lossy().as_ref())
        }) {
            continue;
        }
        if let Some(ref matcher) = file_matcher {
            let file_name = entry.file_name().to_string_lossy();
            if !matcher.is_match(file_name.as_ref()) {
                continue;
            }
        }
        let Ok(contents) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        let lines: Vec<&str> = contents.lines().collect();
        let mut file_match_count = 0usize;
        for (index, line) in lines.iter().enumerate() {
            if regex.is_match(line) {
                file_match_count += 1;
                if output_mode == "content" {
                    let relative = entry
                        .path()
                        .strip_prefix(&context.cwd)
                        .unwrap_or(entry.path());
                    let start = if index > 0 { index - 1 } else { 0 };
                    let end = (index + 2).min(lines.len());
                    for (offset, context_line) in lines[start..end].iter().enumerate() {
                        let line_idx = start + offset;
                        let prefix = if line_idx == index { ">" } else { " " };
                        content_matches.push(format!(
                            "{}:{}{} {}",
                            relative.display(),
                            line_idx + 1,
                            prefix,
                            context_line.trim()
                        ));
                    }
                    content_matches.push(String::new());
                    match_count += 1;
                    if match_count >= 50 {
                        break;
                    }
                }
            }
        }
        if file_match_count > 0 {
            let relative = entry
                .path()
                .strip_prefix(&context.cwd)
                .unwrap_or(entry.path())
                .to_string_lossy()
                .to_string();
            if output_mode == "files_with_matches" {
                files_with_matches.push(relative);
            } else if output_mode == "count" {
                count_per_file.push((relative, file_match_count));
            }
        }
        if output_mode == "content" && match_count >= 50 {
            break;
        }
    }
    match output_mode {
        "files_with_matches" => {
            if files_with_matches.is_empty() {
                Ok("No matches found.".to_owned())
            } else {
                Ok(files_with_matches.join("\n"))
            }
        }
        "count" => {
            if count_per_file.is_empty() {
                Ok("No matches found.".to_owned())
            } else {
                let lines: Vec<String> = count_per_file
                    .iter()
                    .map(|(path, count)| format!("{}: {}", path, count))
                    .collect();
                Ok(lines.join("\n"))
            }
        }
        _ => {
            if content_matches.is_empty() {
                Ok("No matches found.".to_owned())
            } else {
                Ok(content_matches.join("\n").trim_end().to_owned())
            }
        }
    }
}

fn current_plan_file_path() -> Option<PathBuf> {
    crate::plan_mode::current_plan_file_path()
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
                permission_mode: rc_core::PermissionMode::Plan,
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

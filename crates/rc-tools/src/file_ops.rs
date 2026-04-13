//! File operation tools: list_directory, read_file, search_text, write_file,
//! replace_in_file, edit_file, glob_files, grep_files.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use globset::GlobBuilder;
use ignore::WalkBuilder;
use regex::Regex;
use serde_json::Value;
use walkdir::WalkDir;

use super::{IGNORED_DIRS, ToolExecutionContext};

pub(crate) fn resolve_workspace_path(cwd: &Path, maybe_relative: Option<&str>) -> Result<PathBuf> {
    let candidate = match maybe_relative {
        Some(path) if !path.trim().is_empty() => cwd.join(path),
        _ => cwd.to_path_buf(),
    };
    let canonical_cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());
    let canonical_candidate = candidate.canonicalize().unwrap_or(candidate.clone());
    if !canonical_candidate.starts_with(&canonical_cwd) {
        return Err(anyhow!(
            "path {} escapes the workspace {}",
            candidate.display(),
            cwd.display()
        ));
    }
    Ok(candidate)
}

pub(crate) fn list_directory(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let target = resolve_workspace_path(&context.cwd, input.get("path").and_then(Value::as_str))?;
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

pub(crate) fn read_file(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let path = input
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("read_file requires a path"))?;
    let target = resolve_workspace_path(&context.cwd, Some(path))?;
    let contents = std::fs::read_to_string(&target)
        .with_context(|| format!("failed to read {}", target.display()))?;
    let start_line = input.get("start_line").and_then(Value::as_u64).unwrap_or(1) as usize;
    let end_line = input
        .get("end_line")
        .and_then(Value::as_u64)
        .unwrap_or(usize::MAX as u64) as usize;
    let max_chars = input
        .get("max_chars")
        .and_then(Value::as_u64)
        .unwrap_or(50_000) as usize;
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
    Ok(selected.chars().take(max_chars).collect())
}

pub(crate) fn search_text(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let pattern = input
        .get("pattern")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("search_text requires a pattern"))?;
    let target = resolve_workspace_path(&context.cwd, input.get("path").and_then(Value::as_str))?;
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
    let path = input
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("write_file requires a path"))?;
    let content = input
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("write_file requires content"))?;
    let append = input
        .get("append")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let target = resolve_workspace_path(&context.cwd, Some(path))?;
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if append {
        use std::io::Write as _;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&target)?;
        file.write_all(content.as_bytes())?;
    } else {
        std::fs::write(&target, content)?;
    }
    Ok(format!("Wrote {}", target.display()))
}

pub(crate) fn replace_in_file(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let path = input
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("replace_in_file requires a path"))?;
    let search = input
        .get("search")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("replace_in_file requires search text"))?;
    let replace = input
        .get("replace")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("replace_in_file requires replacement text"))?;
    let replace_all = input.get("all").and_then(Value::as_bool).unwrap_or(false);
    let target = resolve_workspace_path(&context.cwd, Some(path))?;
    let original = std::fs::read_to_string(&target)?;
    let updated = if replace_all {
        original.replace(search, replace)
    } else {
        original.replacen(search, replace, 1)
    };
    std::fs::write(&target, updated)?;
    Ok(format!("Updated {}", target.display()))
}

pub(crate) fn edit_file(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let path = input
        .get("path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("edit_file requires a path"))?;
    let target = resolve_workspace_path(&context.cwd, Some(path))?;
    let edits = input
        .get("edits")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("edit_file requires edits"))?;
    let create_if_missing = input
        .get("create_if_missing")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut content = if target.exists() {
        std::fs::read_to_string(&target)?
    } else if create_if_missing {
        String::new()
    } else {
        return Err(anyhow!("{} does not exist", target.display()));
    };
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
        if search.is_empty() && create_if_missing && content.is_empty() {
            content = replace.to_owned();
            continue;
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
    std::fs::write(&target, content)?;
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
    let base = resolve_workspace_path(&context.cwd, input.get("path").and_then(Value::as_str))?;
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
    let target = resolve_workspace_path(&context.cwd, input.get("path").and_then(Value::as_str))?;
    let file_pattern = input.get("file_pattern").and_then(Value::as_str);
    let max_matches = input
        .get("max_matches")
        .and_then(Value::as_u64)
        .unwrap_or(50) as usize;
    let regex = Regex::new(pattern).or_else(|_| Regex::new(&regex::escape(pattern)))?;
    let file_matcher: Option<globset::GlobMatcher> = match file_pattern {
        Some(fp) => Some(
            GlobBuilder::new(fp)
                .literal_separator(true)
                .build()
                .context("invalid file_pattern")?
                .compile_matcher(),
        ),
        None => None,
    };
    let mut matches = Vec::new();
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
        for (index, line) in lines.iter().enumerate() {
            if regex.is_match(line) {
                let relative = entry
                    .path()
                    .strip_prefix(&context.cwd)
                    .unwrap_or(entry.path());
                let start = if index > 0 { index - 1 } else { 0 };
                let end = (index + 2).min(lines.len());
                for (offset, context_line) in lines[start..end].iter().enumerate() {
                    let line_idx = start + offset;
                    let prefix = if line_idx == index { ">" } else { " " };
                    matches.push(format!(
                        "{}:{}{} {}",
                        relative.display(),
                        line_idx + 1,
                        prefix,
                        context_line.trim()
                    ));
                }
                matches.push(String::new());
                match_count += 1;
                if match_count >= max_matches {
                    return Ok(matches.join("\n").trim_end().to_owned());
                }
            }
        }
    }
    if matches.is_empty() {
        Ok("No matches found.".to_owned())
    } else {
        Ok(matches.join("\n").trim_end().to_owned())
    }
}

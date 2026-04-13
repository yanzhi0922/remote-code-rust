//! Memory read/write tools for persistent agent memory (RC.md).

use std::path::PathBuf;

use anyhow::{Result, anyhow};
use serde_json::Value;

use super::ToolExecutionContext;

pub(crate) fn memory_read_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let scope = input.get("scope").and_then(Value::as_str).unwrap_or("all");
    let home = dirs_home()?;
    let mgr = rc_session::memory::MemoryManager::new(&home, Some(&context.cwd));
    let content = match scope {
        "global" => mgr.read_global()?,
        "project" => mgr.read_project()?,
        _ => mgr.read_all()?,
    };
    if content.is_empty() {
        Ok("No memory content found.".to_owned())
    } else {
        Ok(content)
    }
}

pub(crate) fn memory_write_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let scope = input
        .get("scope")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("memory_write requires a scope (global or project)"))?;
    let content = input
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("memory_write requires content"))?;
    let mode = input
        .get("mode")
        .and_then(Value::as_str)
        .unwrap_or("append");
    let home = dirs_home()?;
    let mgr = rc_session::memory::MemoryManager::new(&home, Some(&context.cwd));
    match mode {
        "overwrite" => match scope {
            "global" => mgr.write_global(content)?,
            "project" => mgr.write_project(content)?,
            _ => return Err(anyhow!("scope must be 'global' or 'project'")),
        },
        _ => match scope {
            "global" => mgr.append_global(content)?,
            "project" => mgr.append_project(content)?,
            _ => return Err(anyhow!("scope must be 'global' or 'project'")),
        },
    }
    Ok(format!("Memory updated ({scope}, {mode})."))
}

/// Resolve the user's home directory.
pub(crate) fn dirs_home() -> Result<PathBuf> {
    directories::BaseDirs::new()
        .map(|bd| bd.home_dir().to_path_buf())
        .ok_or_else(|| anyhow!("could not determine home directory"))
}

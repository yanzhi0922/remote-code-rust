//! Memory read/write tools for persistent agent memory (RC.md).
//!
//! Provides both the legacy memory manager tools and the new
//! [`MemoryStore`](rc_utils::memory_store::MemoryStore)-backed tools
//! for structured memory operations.

use std::path::PathBuf;

use anyhow::{Result, anyhow};
use serde_json::Value;

use rc_utils::memory_store::MemoryStore;
use rc_utils::memory_types::MemoryType;

use super::ToolExecutionContext;

// ---------------------------------------------------------------------------
// Legacy tools (existing)
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// New MemoryStore-backed tools
// ---------------------------------------------------------------------------

/// Save a structured memory entry using the new MemoryStore.
#[allow(dead_code)]
///
/// Input JSON should contain:
/// - `key` (string, required): unique key for the memory entry
/// - `content` (string, required): memory content
/// - `scope` (string, optional): "project", "user", or "agent" (default: "project")
/// - `tags` (array of strings, optional): tags for categorization
pub(crate) fn memory_save_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let key = input
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("memory_save requires a key"))?;
    let content = input
        .get("content")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("memory_save requires content"))?;
    let scope_str = input
        .get("scope")
        .and_then(Value::as_str)
        .unwrap_or("project");
    let scope = parse_scope(scope_str)?;
    let tags = input
        .get("tags")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let store = MemoryStore::new(&context.cwd);
    let stored = store.save_memory(key, content, scope, tags)?;

    Ok(format!(
        "Memory saved: key={}, scope={}, tags={}",
        stored.key,
        scope,
        stored.entry.tags.join(", ")
    ))
}

/// Load a structured memory entry using the new MemoryStore.
#[allow(dead_code)]
///
/// Input JSON should contain:
/// - `key` (string, required): key of the memory entry to load
/// - `scope` (string, optional): "project", "user", or "agent" (default: "project")
pub(crate) fn memory_load_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let key = input
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("memory_load requires a key"))?;
    let scope_str = input
        .get("scope")
        .and_then(Value::as_str)
        .unwrap_or("project");
    let scope = parse_scope(scope_str)?;

    let store = MemoryStore::new(&context.cwd);
    let stored = store.load_memory(key, scope)?;

    Ok(format!(
        "Memory loaded: key={}\nscope={}\ntimestamp={}\ntags={}\n\n{}",
        stored.key,
        stored.entry.scope,
        stored.entry.timestamp,
        stored.entry.tags.join(", "),
        stored.entry.content
    ))
}

/// Search memory entries using the new MemoryStore.
#[allow(dead_code)]
///
/// Input JSON should contain:
/// - `query` (string, required): search query
pub(crate) fn memory_search_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let query = input
        .get("query")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("memory_search requires a query"))?;

    let store = MemoryStore::new(&context.cwd);
    let results = store.search_memory(query)?;

    if results.is_empty() {
        Ok(format!("No memories found matching '{query}'."))
    } else {
        let mut output = format!(
            "Found {} memory entries matching '{}':\n",
            results.len(),
            query
        );
        for stored in &results {
            output.push_str(&format!(
                "  [{}] {} (scope: {}, tags: {})\n",
                stored.key,
                stored.entry.content.chars().take(80).collect::<String>(),
                stored.entry.scope,
                stored.entry.tags.join(", ")
            ));
        }
        Ok(output)
    }
}

/// List memory entries using the new MemoryStore.
#[allow(dead_code)]
///
/// Input JSON should contain:
/// - `scope` (string, optional): "project", "user", or "agent" (default: lists all)
pub(crate) fn memory_list_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let store = MemoryStore::new(&context.cwd);

    let scope_str = input.get("scope").and_then(Value::as_str);

    if let Some(scope_str) = scope_str {
        let scope = parse_scope(scope_str)?;
        let entries = store.list_memories(scope)?;
        if entries.is_empty() {
            return Ok(format!("No memories in {scope} scope."));
        }
        let mut output = format!("Memories in {scope} scope ({}):\n", entries.len());
        for stored in &entries {
            output.push_str(&format!(
                "  [{}] {} (tags: {})\n",
                stored.key,
                stored.entry.content.chars().take(60).collect::<String>(),
                stored.entry.tags.join(", ")
            ));
        }
        Ok(output)
    } else {
        let all = store.list_all_memories()?;
        let total: usize = all.values().map(|v| v.len()).sum();
        if total == 0 {
            return Ok("No memories found in any scope.".to_owned());
        }
        let mut output = format!("All memories ({total}):\n");
        for (scope, entries) in &all {
            if entries.is_empty() {
                continue;
            }
            output.push_str(&format!("  [{scope}] ({} entries):\n", entries.len()));
            for stored in entries {
                output.push_str(&format!(
                    "    [{}] {}\n",
                    stored.key,
                    stored.entry.content.chars().take(50).collect::<String>()
                ));
            }
        }
        Ok(output)
    }
}

/// Delete a memory entry using the new MemoryStore.
#[allow(dead_code)]
///
/// Input JSON should contain:
/// - `key` (string, required): key of the memory entry to delete
/// - `scope` (string, optional): "project", "user", or "agent" (default: "project")
pub(crate) fn memory_delete_tool(input: &Value, context: &ToolExecutionContext) -> Result<String> {
    let key = input
        .get("key")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("memory_delete requires a key"))?;
    let scope_str = input
        .get("scope")
        .and_then(Value::as_str)
        .unwrap_or("project");
    let scope = parse_scope(scope_str)?;

    let store = MemoryStore::new(&context.cwd);
    store.delete_memory(key, scope)?;

    Ok(format!("Memory deleted: key={key}, scope={scope}"))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse a scope string into a [`MemoryType`].
fn parse_scope(s: &str) -> Result<MemoryType> {
    MemoryType::from_str_opt(s)
        .ok_or_else(|| anyhow!("invalid scope '{s}'. Must be 'project', 'user', or 'agent'"))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn build_test_context() -> (ToolExecutionContext, PathBuf) {
        let temp = tempdir().expect("tempdir should work");
        let root = temp.path().to_path_buf();
        let context = ToolExecutionContext {
            cwd: root.clone(),
            ..ToolExecutionContext::default()
        };
        (context, root)
    }

    fn ok<T, E: std::fmt::Display>(result: Result<T, E>) -> T {
        match result {
            Ok(value) => value,
            Err(error) => panic!("unexpected error: {error}"),
        }
    }

    // --- parse_scope ---

    #[test]
    fn parse_scope_project() {
        assert_eq!(parse_scope("project").expect("parse"), MemoryType::Project);
    }

    #[test]
    fn parse_scope_user() {
        assert_eq!(parse_scope("user").expect("parse"), MemoryType::User);
    }

    #[test]
    fn parse_scope_agent() {
        assert_eq!(parse_scope("agent").expect("parse"), MemoryType::Agent);
    }

    #[test]
    fn parse_scope_invalid() {
        assert!(parse_scope("invalid").is_err());
    }

    #[test]
    fn parse_scope_empty() {
        assert!(parse_scope("").is_err());
    }

    // --- memory_save_tool ---

    #[test]
    fn memory_save_tool_basic() {
        let (context, _temp) = build_test_context();
        let input = serde_json::json!({
            "key": "test-note",
            "content": "This is a test note",
            "scope": "project",
            "tags": ["test", "note"]
        });
        let result = ok(memory_save_tool(&input, &context));
        assert!(result.contains("test-note"));
        assert!(result.contains("project"));
    }

    #[test]
    fn memory_save_tool_missing_key() {
        let (context, _temp) = build_test_context();
        let input = serde_json::json!({
            "content": "no key"
        });
        assert!(memory_save_tool(&input, &context).is_err());
    }

    #[test]
    fn memory_save_tool_missing_content() {
        let (context, _temp) = build_test_context();
        let input = serde_json::json!({
            "key": "no-content"
        });
        assert!(memory_save_tool(&input, &context).is_err());
    }

    // --- memory_load_tool ---

    #[test]
    fn memory_load_tool_roundtrip() {
        let (context, _temp) = build_test_context();
        let save_input = serde_json::json!({
            "key": "roundtrip",
            "content": "Round trip content",
            "scope": "project"
        });
        ok(memory_save_tool(&save_input, &context));

        let load_input = serde_json::json!({
            "key": "roundtrip",
            "scope": "project"
        });
        let result = ok(memory_load_tool(&load_input, &context));
        assert!(result.contains("Round trip content"));
    }

    #[test]
    fn memory_load_tool_missing_key() {
        let (context, _temp) = build_test_context();
        let input = serde_json::json!({});
        assert!(memory_load_tool(&input, &context).is_err());
    }

    #[test]
    fn memory_load_tool_nonexistent() {
        let (context, _temp) = build_test_context();
        let input = serde_json::json!({
            "key": "nonexistent",
            "scope": "project"
        });
        assert!(memory_load_tool(&input, &context).is_err());
    }

    // --- memory_search_tool ---

    #[test]
    fn memory_search_tool_finds_match() {
        let (context, _temp) = build_test_context();
        let save_input = serde_json::json!({
            "key": "rust-tips",
            "content": "Use cargo clippy for linting",
            "scope": "project"
        });
        ok(memory_save_tool(&save_input, &context));

        let search_input = serde_json::json!({
            "query": "clippy"
        });
        let result = ok(memory_search_tool(&search_input, &context));
        assert!(result.contains("rust-tips"));
    }

    #[test]
    fn memory_search_tool_no_results() {
        let (context, _temp) = build_test_context();
        let search_input = serde_json::json!({
            "query": "nonexistent-query-xyz"
        });
        let result = ok(memory_search_tool(&search_input, &context));
        assert!(result.contains("No memories found"));
    }

    #[test]
    fn memory_search_tool_missing_query() {
        let (context, _temp) = build_test_context();
        let input = serde_json::json!({});
        assert!(memory_search_tool(&input, &context).is_err());
    }

    // --- memory_list_tool ---

    #[test]
    fn memory_list_tool_empty() {
        let (context, _temp) = build_test_context();
        let input = serde_json::json!({});
        let result = ok(memory_list_tool(&input, &context));
        assert!(result.contains("No memories"));
    }

    #[test]
    fn memory_list_tool_with_entries() {
        let (context, _temp) = build_test_context();
        let save1 = serde_json::json!({
            "key": "alpha",
            "content": "Alpha content",
            "scope": "project"
        });
        let save2 = serde_json::json!({
            "key": "beta",
            "content": "Beta content",
            "scope": "project"
        });
        ok(memory_save_tool(&save1, &context));
        ok(memory_save_tool(&save2, &context));

        let list_input = serde_json::json!({
            "scope": "project"
        });
        let result = ok(memory_list_tool(&list_input, &context));
        assert!(result.contains("alpha"));
        assert!(result.contains("beta"));
    }

    // --- memory_delete_tool ---

    #[test]
    fn memory_delete_tool_basic() {
        let (context, _temp) = build_test_context();
        let save_input = serde_json::json!({
            "key": "to-delete",
            "content": "Delete me",
            "scope": "project"
        });
        ok(memory_save_tool(&save_input, &context));

        let delete_input = serde_json::json!({
            "key": "to-delete",
            "scope": "project"
        });
        let result = ok(memory_delete_tool(&delete_input, &context));
        assert!(result.contains("to-delete"));
    }

    #[test]
    fn memory_delete_tool_nonexistent() {
        let (context, _temp) = build_test_context();
        let input = serde_json::json!({
            "key": "ghost",
            "scope": "project"
        });
        assert!(memory_delete_tool(&input, &context).is_err());
    }
}

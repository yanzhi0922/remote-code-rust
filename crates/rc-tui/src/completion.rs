//! Tab completion for slash commands, tool names, and file paths.

/// Complete a partial slash command input.
pub fn complete_slash_command(partial: &str) -> Vec<String> {
    let all_commands = [
        "/help", "/status", "/cost", "/compact", "/compact!",
        "/clear", "/sessions", "/tools", "/doctor", "/theme",
        "/quit", "/exit",
    ];
    all_commands
        .iter()
        .filter(|cmd| cmd.starts_with(partial))
        .map(|cmd| cmd.to_string())
        .collect::<Vec<_>>()
}

/// Get tool name completions matching a prefix.
pub fn get_tool_completions(prefix: &str) -> Vec<String> {
    let specs = rc_tools::builtin_tool_specs();
    specs
        .iter()
        .filter(|s| s.name.starts_with(prefix))
        .map(|s| s.name.clone())
        .collect()
}

/// Get file path completions for a partial path.
pub fn get_file_completions(partial: &str, cwd: &std::path::Path) -> Vec<String> {
    if partial.is_empty() {
        return Vec::new();
    }
    let path = std::path::Path::new(partial);
    let (dir, file_prefix) = if partial.ends_with('/') || partial.ends_with('\\') {
        (cwd.join(partial), "")
    } else if let Some(parent) = path.parent() {
        if parent.as_os_str().is_empty() {
            (cwd.to_path_buf(), path.file_name().and_then(|n| n.to_str()).unwrap_or(""))
        } else {
            (cwd.join(parent), path.file_name().and_then(|n| n.to_str()).unwrap_or(""))
        }
    } else {
        (cwd.to_path_buf(), partial)
    };

    let mut results = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(file_prefix) {
                let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                let suffix = if is_dir { "/" } else { "" };
                // Reconstruct the path relative to cwd.
                let base = if partial.contains('/') || partial.contains('\\') {
                    let parent_str = if let Some(p) = path.parent() {
                        p.to_string_lossy().to_string()
                    } else {
                        String::new()
                    };
                    if parent_str.is_empty() {
                        format!("{name}{suffix}")
                    } else {
                        format!("{parent_str}/{name}{suffix}")
                    }
                } else {
                    format!("{name}{suffix}")
                };
                results.push(base);
            }
        }
    }
    results.sort();
    results.truncate(20); // Limit results.
    results
}

/// Update search results based on the current query.
pub fn update_search_results(history: &[String], query: &str, results: &mut Vec<usize>) {
    results.clear();
    if query.is_empty() {
        return;
    }
    // Search from newest to oldest.
    for (i, entry) in history.iter().enumerate().rev() {
        if entry.contains(query) {
            results.push(i);
        }
    }
}

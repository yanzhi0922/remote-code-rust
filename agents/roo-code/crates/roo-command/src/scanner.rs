//! Directory scanning and symlink resolution for command files.
//!
//! Maps to TypeScript source: `src/services/command/commands.ts`
//! (scanCommandDirectory, resolveCommandSymLink, resolveCommandDirectoryEntry)

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::frontmatter::parse_command_content;
use crate::types::{Command, CommandFileInfo, CommandSource};
use crate::utils::{get_command_name_from_file, is_markdown_file};

/// Maximum recursion depth for symlink resolution.
const MAX_DEPTH: usize = 5;

/// Scan a command directory and insert discovered commands into the map.
///
/// Commands are keyed by name. If `source` is `Project`, it always overrides
/// existing entries. Otherwise it only inserts if the key is absent (so that
/// higher-priority sources are preserved).
///
/// Errors (missing directory, unreadable files) are silently ignored.
pub async fn scan_command_directory(
    dir_path: &Path,
    source: CommandSource,
    commands: &mut HashMap<String, Command>,
) {
    // Check directory exists
    let metadata = match tokio::fs::metadata(dir_path).await {
        Ok(m) => m,
        Err(_) => return,
    };
    if !metadata.is_dir() {
        return;
    }

    let mut read_dir = match tokio::fs::read_dir(dir_path).await {
        Ok(rd) => rd,
        Err(_) => return,
    };

    // Collect all command file infos (including symlink-resolved ones)
    let mut file_infos: Vec<CommandFileInfo> = Vec::new();

    while let Some(entry) = read_dir.next_entry().await.unwrap_or(None) {
        let file_name = entry.file_name();
        let name_str = file_name.to_string_lossy().to_string();
        let full_path = entry.path();

        let file_type = match entry.file_type().await {
            Ok(ft) => ft,
            Err(_) => continue,
        };

        if file_type.is_file() {
            if is_markdown_file(&name_str) {
                file_infos.push(CommandFileInfo {
                    original_path: full_path.clone(),
                    resolved_path: full_path,
                });
            }
        } else if file_type.is_symlink() {
            resolve_command_sym_link(&full_path, &mut file_infos, 0).await;
        }
    }

    // Process each collected file
    for info in file_infos {
        let command_name = get_command_name_from_file(
            info.original_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .as_ref(),
        );

        let content = match tokio::fs::read_to_string(&info.resolved_path).await {
            Ok(c) => c,
            Err(_) => continue,
        };

        let parsed = parse_command_content(&content);

        // Project commands always override; others only insert if absent
        if source == CommandSource::Project || !commands.contains_key(&command_name) {
            commands.insert(
                command_name,
                Command {
                    name: get_command_name_from_file(
                        info.original_path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .as_ref(),
                    ),
                    content: parsed.body,
                    source: source.clone(),
                    file_path: info.resolved_path,
                    description: parsed.frontmatter.description,
                    argument_hint: parsed.frontmatter.argument_hint,
                    mode: parsed.frontmatter.mode,
                },
            );
        }
    }
}

/// Recursively resolve a symbolic link and collect command file infos.
///
/// Follows symlinks up to [`MAX_DEPTH`] levels to prevent cyclic loops.
/// If the target is a file, it is collected (if markdown). If the target is
/// a directory, its entries are processed. If the target is another symlink,
/// resolution continues recursively.
pub fn resolve_command_sym_link<'a>(
    symlink_path: &'a Path,
    file_infos: &'a mut Vec<CommandFileInfo>,
    depth: usize,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
    Box::pin(async move {
    if depth > MAX_DEPTH {
        return;
    }

    // Read the symlink target
    let link_target = match tokio::fs::read_link(symlink_path).await {
        Ok(t) => t,
        Err(_) => return,
    };

    // Resolve relative to the symlink's parent directory
    let resolved_target = symlink_path
        .parent()
        .unwrap_or(Path::new("."))
        .join(&link_target);

    // Use lstat to detect nested symlinks
    let stats = match tokio::fs::symlink_metadata(&resolved_target).await {
        Ok(s) => s,
        Err(_) => return,
    };

    if stats.is_file() {
        if is_markdown_file(
            resolved_target
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .as_ref(),
        ) {
            file_infos.push(CommandFileInfo {
                original_path: symlink_path.to_path_buf(),
                resolved_path: resolved_target,
            });
        }
    } else if stats.is_dir() {
        let mut entries = match tokio::fs::read_dir(&resolved_target).await {
            Ok(rd) => rd,
            Err(_) => return,
        };
        loop {
            let entry = match entries.next_entry().await {
                Ok(Some(e)) => e,
                Ok(None) => break,
                Err(_) => continue,
            };
            resolve_command_directory_entry(&entry, &resolved_target, file_infos, depth + 1).await;
        }
    } else if stats.is_symlink() {
        // Nested symlink
        resolve_command_sym_link(&resolved_target, file_infos, depth + 1).await;
    }
    })
}

/// Resolve a single directory entry (file or symlink).
pub fn resolve_command_directory_entry<'a>(
    entry: &'a tokio::fs::DirEntry,
    dir_path: &'a Path,
    file_infos: &'a mut Vec<CommandFileInfo>,
    depth: usize,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
    Box::pin(async move {
    if depth > MAX_DEPTH {
        return;
    }

    let file_name = entry.file_name();
    let name_str = file_name.to_string_lossy().to_string();
    let full_path = dir_path.join(&file_name);

    let file_type = match entry.file_type().await {
        Ok(ft) => ft,
        Err(_) => return,
    };

    if file_type.is_file() {
        if is_markdown_file(&name_str) {
            file_infos.push(CommandFileInfo {
                original_path: full_path.clone(),
                resolved_path: full_path,
            });
        }
    } else if file_type.is_symlink() {
        resolve_command_sym_link(&full_path, file_infos, depth + 1).await;
    }
    })
}

/// Try to resolve a symlinked command file to its real path.
///
/// Returns `Some(resolved_path)` if `file_path` is a symlink pointing to a
/// regular file, or `None` otherwise.
pub async fn try_resolve_symlinked_command(file_path: &Path) -> Option<PathBuf> {
    let lstat = tokio::fs::symlink_metadata(file_path).await.ok()?;
    if !lstat.is_symlink() {
        return None;
    }

    let link_target = tokio::fs::read_link(file_path).await.ok()?;
    let resolved_target = file_path
        .parent()
        .unwrap_or(Path::new("."))
        .join(&link_target);

    let stat = tokio::fs::metadata(&resolved_target).await.ok()?;
    if stat.is_file() {
        Some(resolved_target)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper: create a `.md` file with the given content in `dir`.
    fn create_md_file(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, content).unwrap();
        path
    }

    #[tokio::test]
    async fn test_scan_empty_directory() {
        let tmp = TempDir::new().unwrap();
        let commands_dir = tmp.path().join("commands");
        fs::create_dir_all(&commands_dir).unwrap();

        let mut commands = HashMap::new();
        scan_command_directory(&commands_dir, CommandSource::Global, &mut commands).await;
        assert!(commands.is_empty());
    }

    #[tokio::test]
    async fn test_scan_nonexistent_directory() {
        let mut commands = HashMap::new();
        scan_command_directory(
            Path::new("/nonexistent/path/commands"),
            CommandSource::Global,
            &mut commands,
        )
        .await;
        assert!(commands.is_empty());
    }

    #[tokio::test]
    async fn test_scan_with_md_files() {
        let tmp = TempDir::new().unwrap();
        let commands_dir = tmp.path().join("commands");
        fs::create_dir_all(&commands_dir).unwrap();

        create_md_file(&commands_dir, "hello.md", "Hello command body");
        create_md_file(&commands_dir, "world.md", "---\ndescription: World command\n---\nWorld body");

        let mut commands = HashMap::new();
        scan_command_directory(&commands_dir, CommandSource::Global, &mut commands).await;

        assert_eq!(commands.len(), 2);
        assert!(commands.contains_key("hello"));
        assert!(commands.contains_key("world"));
        assert_eq!(commands["hello"].content, "Hello command body");
        assert_eq!(commands["world"].content, "World body");
        assert_eq!(
            commands["world"].description.as_deref(),
            Some("World command")
        );
    }

    #[tokio::test]
    async fn test_scan_ignores_non_md_files() {
        let tmp = TempDir::new().unwrap();
        let commands_dir = tmp.path().join("commands");
        fs::create_dir_all(&commands_dir).unwrap();

        create_md_file(&commands_dir, "valid.md", "Valid");
        create_md_file(&commands_dir, "ignore.txt", "Ignored");
        create_md_file(&commands_dir, "also_ignore.json", "{}");

        let mut commands = HashMap::new();
        scan_command_directory(&commands_dir, CommandSource::Global, &mut commands).await;

        assert_eq!(commands.len(), 1);
        assert!(commands.contains_key("valid"));
    }

    #[tokio::test]
    async fn test_scan_project_overrides_global() {
        let tmp = TempDir::new().unwrap();

        // Global commands
        let global_dir = tmp.path().join("global");
        fs::create_dir_all(&global_dir).unwrap();
        create_md_file(&global_dir, "shared.md", "Global version");

        // Project commands
        let project_dir = tmp.path().join("project");
        fs::create_dir_all(&project_dir).unwrap();
        create_md_file(&project_dir, "shared.md", "Project version");
        create_md_file(&project_dir, "only-project.md", "Project only");

        let mut commands = HashMap::new();

        // Scan global first
        scan_command_directory(&global_dir, CommandSource::Global, &mut commands).await;
        assert_eq!(commands["shared"].content, "Global version");

        // Scan project — should override "shared"
        scan_command_directory(&project_dir, CommandSource::Project, &mut commands).await;

        assert_eq!(commands["shared"].content, "Project version");
        assert_eq!(commands["shared"].source, CommandSource::Project);
        assert!(commands.contains_key("only-project"));
    }

    #[tokio::test]
    async fn test_scan_global_does_not_override_existing() {
        let tmp = TempDir::new().unwrap();

        let dir1 = tmp.path().join("first");
        fs::create_dir_all(&dir1).unwrap();
        create_md_file(&dir1, "cmd.md", "First version");

        let dir2 = tmp.path().join("second");
        fs::create_dir_all(&dir2).unwrap();
        create_md_file(&dir2, "cmd.md", "Second version");

        let mut commands = HashMap::new();

        // First scan (Global) — inserts
        scan_command_directory(&dir1, CommandSource::Global, &mut commands).await;
        assert_eq!(commands["cmd"].content, "First version");

        // Second scan (Global) — should NOT override
        scan_command_directory(&dir2, CommandSource::Global, &mut commands).await;
        assert_eq!(commands["cmd"].content, "First version");
    }

    #[tokio::test]
    async fn test_try_resolve_symlinked_command_regular_file() {
        let tmp = TempDir::new().unwrap();
        let path = create_md_file(tmp.path(), "regular.md", "content");

        let result = try_resolve_symlinked_command(&path).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_try_resolve_symlinked_command_nonexistent() {
        let result =
            try_resolve_symlinked_command(Path::new("/nonexistent/file.md")).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_scan_file_path_is_set() {
        let tmp = TempDir::new().unwrap();
        let commands_dir = tmp.path().join("commands");
        fs::create_dir_all(&commands_dir).unwrap();

        let file_path = create_md_file(&commands_dir, "test.md", "Test body");

        let mut commands = HashMap::new();
        scan_command_directory(&commands_dir, CommandSource::Global, &mut commands).await;

        assert_eq!(commands["test"].file_path, file_path);
    }

    #[tokio::test]
    async fn test_scan_source_is_set_correctly() {
        let tmp = TempDir::new().unwrap();
        let commands_dir = tmp.path().join("commands");
        fs::create_dir_all(&commands_dir).unwrap();
        create_md_file(&commands_dir, "cmd.md", "Body");

        let mut commands = HashMap::new();
        scan_command_directory(&commands_dir, CommandSource::Project, &mut commands).await;
        assert_eq!(commands["cmd"].source, CommandSource::Project);

        let mut commands2 = HashMap::new();
        scan_command_directory(&commands_dir, CommandSource::Global, &mut commands2).await;
        assert_eq!(commands2["cmd"].source, CommandSource::Global);
    }
}

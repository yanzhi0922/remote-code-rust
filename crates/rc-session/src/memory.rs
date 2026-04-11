//! CLAUDE.md-style memory system.
//!
//! Provides persistent memory across sessions via Markdown files:
//! - **Global memory**: `~/.remote-code-rust/CLAUDE.md`
//! - **Project memory**: `<project>/.remote-code-rust/CLAUDE.md`

use std::fs;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

const MEMORY_FILENAME: &str = "CLAUDE.md";
const PROFILE_DIR: &str = ".remote-code-rust";

/// Manages persistent memory files (global and project-scoped).
#[derive(Debug, Clone)]
pub struct MemoryManager {
    /// Global memory path (`~/.remote-code-rust/CLAUDE.md`).
    global_path: PathBuf,
    /// Project memory path (`<project>/.remote-code-rust/CLAUDE.md`).
    project_path: Option<PathBuf>,
}

impl MemoryManager {
    /// Create a new memory manager.
    ///
    /// - `home_dir`: the user's home directory (for global memory).
    /// - `project_dir`: optional project root directory (for project memory).
    pub fn new(home_dir: &Path, project_dir: Option<&Path>) -> Self {
        let global_path = home_dir.join(PROFILE_DIR).join(MEMORY_FILENAME);
        let project_path = project_dir.map(|dir| dir.join(PROFILE_DIR).join(MEMORY_FILENAME));
        Self {
            global_path,
            project_path,
        }
    }

    /// Read all memory content (global + project), separated by a header.
    pub fn read_all(&self) -> Result<String> {
        let global = self.read_global().unwrap_or_default();
        let project = self
            .project_path
            .as_ref()
            .and_then(|p| read_file_if_exists(p).ok())
            .unwrap_or_default();

        let mut parts = Vec::new();
        if !global.is_empty() {
            parts.push(format!("## Global Memory\n\n{global}"));
        }
        if !project.is_empty() {
            parts.push(format!("## Project Memory\n\n{project}"));
        }
        Ok(parts.join("\n\n---\n\n"))
    }

    /// Read the global memory file.
    pub fn read_global(&self) -> Result<String> {
        read_file_if_exists(&self.global_path)
    }

    /// Read the project memory file.
    pub fn read_project(&self) -> Result<String> {
        match &self.project_path {
            Some(path) => read_file_if_exists(path),
            None => Ok(String::new()),
        }
    }

    /// Append content to the global memory file.
    pub fn append_global(&self, content: &str) -> Result<()> {
        self.ensure_parent(&self.global_path)?;
        append_to_file(&self.global_path, content)
    }

    /// Append content to the project memory file.
    pub fn append_project(&self, content: &str) -> Result<()> {
        match &self.project_path {
            Some(path) => {
                self.ensure_parent(path)?;
                append_to_file(path, content)
            }
            None => Err(anyhow::anyhow!("No project directory configured")),
        }
    }

    /// Overwrite the global memory file.
    pub fn write_global(&self, content: &str) -> Result<()> {
        self.ensure_parent(&self.global_path)?;
        fs::write(&self.global_path, content)
            .with_context(|| format!("failed to write {}", self.global_path.display()))
    }

    /// Overwrite the project memory file.
    pub fn write_project(&self, content: &str) -> Result<()> {
        match &self.project_path {
            Some(path) => {
                self.ensure_parent(path)?;
                fs::write(path, content)
                    .with_context(|| format!("failed to write {}", path.display()))
            }
            None => Err(anyhow::anyhow!("No project directory configured")),
        }
    }

    /// Get the global memory file path.
    #[must_use]
    pub fn global_memory_path(&self) -> &Path {
        &self.global_path
    }

    /// Get the project memory file path, if configured.
    #[must_use]
    pub fn project_memory_path(&self) -> Option<&Path> {
        self.project_path.as_deref()
    }

    fn ensure_parent(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        Ok(())
    }
}

/// Read a file if it exists; return empty string otherwise.
fn read_file_if_exists(path: &Path) -> Result<String> {
    if path.exists() {
        fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))
    } else {
        Ok(String::new())
    }
}

/// Append content to a file (creating it if necessary).
fn append_to_file(path: &Path, content: &str) -> Result<()> {
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .with_context(|| format!("failed to open {} for append", path.display()))?;
    // Ensure trailing newline before appending
    if file.metadata().is_ok_and(|m| m.len() > 0) && !content.starts_with('\n') {
        file.write_all(b"\n")?;
    }
    file.write_all(content.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn read_all_returns_empty_when_no_files() {
        let home = tempdir().expect("tempdir");
        let mgr = MemoryManager::new(home.path(), None);
        let content = mgr.read_all().expect("read_all");
        assert!(content.is_empty());
    }

    #[test]
    fn write_and_read_global() {
        let home = tempdir().expect("tempdir");
        let mgr = MemoryManager::new(home.path(), None);
        mgr.write_global("hello global").expect("write");
        let content = mgr.read_global().expect("read");
        assert_eq!(content, "hello global");
    }

    #[test]
    fn write_and_read_project() {
        let home = tempdir().expect("tempdir");
        let project = tempdir().expect("tempdir");
        let mgr = MemoryManager::new(home.path(), Some(project.path()));
        mgr.write_project("hello project").expect("write");
        let content = mgr.read_project().expect("read");
        assert_eq!(content, "hello project");
    }

    #[test]
    fn append_adds_content() {
        let home = tempdir().expect("tempdir");
        let mgr = MemoryManager::new(home.path(), None);
        mgr.write_global("first").expect("write");
        mgr.append_global("second").expect("append");
        let content = mgr.read_global().expect("read");
        assert_eq!(content, "first\nsecond");
    }

    #[test]
    fn read_all_combines_both() {
        let home = tempdir().expect("tempdir");
        let project = tempdir().expect("tempdir");
        let mgr = MemoryManager::new(home.path(), Some(project.path()));
        mgr.write_global("global content").expect("write global");
        mgr.write_project("project content").expect("write project");
        let all = mgr.read_all().expect("read_all");
        assert!(all.contains("global content"));
        assert!(all.contains("project content"));
    }
}

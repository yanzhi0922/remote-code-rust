//! Filesystem permission helpers for tool path authorization.
//!
//! This is the Rust-side analogue of Claude Code's path permission pipeline:
//! raw input validation, symlink-aware destination checking, dangerous edit
//! detection, and working-directory/additional-directory allowlists.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::path_validation::{
    PathValidation, clean_path_input, path_requires_manual_approval, validate_path,
};

/// Filesystem operation kind used for permission checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesystemOperation {
    Read,
    Write,
    Create,
}

/// Result of a filesystem permission check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesystemCheckResult {
    /// Whether the operation can proceed without an explicit permission prompt.
    pub allowed: bool,
    /// Whether the caller should route through the interactive permission flow.
    pub requires_confirmation: bool,
    /// Reason for a prompt or denial.
    pub reason: Option<String>,
    /// Normalized absolute path used for the decision.
    pub normalized_path: PathBuf,
    /// All path forms considered during the decision (original, symlinks, real path).
    pub checked_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPath {
    pub resolved_path: PathBuf,
    pub is_symlink: bool,
    pub is_canonical: bool,
}

const DANGEROUS_FILES: &[&str] = &[
    ".gitconfig",
    ".gitmodules",
    ".bashrc",
    ".bash_profile",
    ".zshrc",
    ".zprofile",
    ".profile",
    ".ripgreprc",
    ".mcp.json",
    ".claude.json",
];

const DANGEROUS_DIRECTORIES: &[&str] = &[".git", ".vscode", ".idea", ".claude"];

/// Backward-compatible wrapper that checks whether a path sits inside the
/// current working directory or an explicitly allowed extra directory.
#[must_use]
pub fn check_filesystem_permission(
    path: &str,
    cwd: &str,
    additional_dirs: &[String],
) -> FilesystemCheckResult {
    let cwd = PathBuf::from(cwd);
    let additional_dirs = additional_dirs
        .iter()
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    assess_filesystem_access(
        path,
        &cwd,
        &additional_dirs,
        FilesystemOperation::Read,
        None,
    )
}

/// Main path permission entry point used by the tool runtime.
#[must_use]
pub fn assess_filesystem_access(
    path: &str,
    cwd: &Path,
    additional_dirs: &[PathBuf],
    operation: FilesystemOperation,
    plan_file: Option<&Path>,
) -> FilesystemCheckResult {
    let cleaned = clean_path_input(path);
    let normalized_path = resolve_candidate_path(cwd, Some(cleaned.as_str()));

    match validate_path(&cleaned) {
        PathValidation::Valid => {}
        PathValidation::Invalid(reason) => {
            return deny(normalized_path, reason);
        }
    }

    let checked_paths = get_paths_for_permission_check(&normalized_path);

    if is_plan_file_path(&checked_paths, plan_file) {
        return allow(normalized_path, checked_paths);
    }

    if let Some(reason) =
        path_requires_manual_approval(&cleaned, !matches!(operation, FilesystemOperation::Read))
    {
        return ask(normalized_path, checked_paths, reason);
    }

    if matches!(
        operation,
        FilesystemOperation::Write | FilesystemOperation::Create
    ) && let Some(reason) = check_path_safety_for_auto_edit(&checked_paths)
    {
        return ask(normalized_path, checked_paths, reason);
    }

    if path_in_allowed_working_path(&checked_paths, cwd, additional_dirs) {
        return allow(normalized_path, checked_paths);
    }

    ask(
        normalized_path,
        checked_paths,
        "Path is outside the allowed working directories.".to_owned(),
    )
}

/// Resolve a user-provided path against the current working directory.
#[must_use]
pub fn resolve_candidate_path(cwd: &Path, maybe_relative: Option<&str>) -> PathBuf {
    match maybe_relative {
        Some(path) if !path.trim().is_empty() => {
            let candidate = PathBuf::from(clean_path_input(path));
            if candidate.is_absolute() {
                candidate
            } else {
                cwd.join(candidate)
            }
        }
        _ => cwd.to_path_buf(),
    }
}

/// Normalize a path for case-insensitive comparison.
#[must_use]
pub fn normalize_for_comparison(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let stripped = raw.strip_prefix(r"\\?\").unwrap_or(&raw);
    let mut normalized = stripped.replace('\\', "/").to_ascii_lowercase();

    while normalized.len() > 1 && normalized.ends_with('/') && !is_root_like(&normalized) {
        normalized.pop();
    }

    normalized
}

/// Resolve a path without failing on non-existent targets or broken symlinks.
#[must_use]
pub fn safe_resolve_path(path: &Path) -> ResolvedPath {
    if is_unc_path(path) {
        return ResolvedPath {
            resolved_path: path.to_path_buf(),
            is_symlink: false,
            is_canonical: false,
        };
    }

    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => {
            return ResolvedPath {
                resolved_path: path.to_path_buf(),
                is_symlink: false,
                is_canonical: false,
            };
        }
    };

    #[cfg(unix)]
    {
        use std::os::unix::fs::FileTypeExt;

        let file_type = metadata.file_type();
        if file_type.is_fifo()
            || file_type.is_socket()
            || file_type.is_char_device()
            || file_type.is_block_device()
        {
            return ResolvedPath {
                resolved_path: path.to_path_buf(),
                is_symlink: false,
                is_canonical: false,
            };
        }
    }

    match std::fs::canonicalize(path) {
        Ok(resolved_path) => ResolvedPath {
            is_symlink: normalize_for_comparison(&resolved_path) != normalize_for_comparison(path),
            resolved_path,
            is_canonical: true,
        },
        Err(_) => ResolvedPath {
            resolved_path: path.to_path_buf(),
            is_symlink: metadata.file_type().is_symlink(),
            is_canonical: false,
        },
    }
}

/// Resolve the deepest existing ancestor of a path, preserving non-existent tail segments.
#[must_use]
pub fn resolve_deepest_existing_ancestor(path: &Path) -> Option<PathBuf> {
    let mut current = path.to_path_buf();
    let mut tail_segments = Vec::new();

    loop {
        let metadata = match std::fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(_) => {
                let parent = current.parent()?;
                if parent == current {
                    return None;
                }
                if let Some(name) = current.file_name() {
                    tail_segments.push(name.to_owned());
                }
                current = parent.to_path_buf();
                continue;
            }
        };

        if metadata.file_type().is_symlink() {
            if let Ok(resolved) = std::fs::canonicalize(&current) {
                return Some(rejoin_tail(resolved, &tail_segments));
            }
            if let Ok(target) = std::fs::read_link(&current) {
                let absolute_target = if target.is_absolute() {
                    target
                } else {
                    current
                        .parent()
                        .unwrap_or_else(|| Path::new("."))
                        .join(target)
                };
                return Some(rejoin_tail(absolute_target, &tail_segments));
            }
            return None;
        }

        if let Ok(resolved) = std::fs::canonicalize(&current) {
            let rejoined = rejoin_tail(resolved.clone(), &tail_segments);
            if normalize_for_comparison(&rejoined) != normalize_for_comparison(path) {
                return Some(rejoined);
            }
        }
        return None;
    }
}

/// Collect all path forms relevant to a permission check.
#[must_use]
pub fn get_paths_for_permission_check(path: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    push_unique_path(&mut paths, path.to_path_buf());

    if is_unc_path(path) {
        return paths;
    }

    let mut current = path.to_path_buf();
    let mut visited = HashSet::new();
    for _ in 0..40 {
        let current_key = normalize_for_comparison(&current);
        if !visited.insert(current_key) {
            break;
        }

        if !current.exists() {
            if normalize_for_comparison(&current) == normalize_for_comparison(path)
                && let Some(resolved) = resolve_deepest_existing_ancestor(path)
            {
                push_unique_path(&mut paths, resolved);
            }
            break;
        }

        let metadata = match std::fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(_) => break,
        };

        if !metadata.file_type().is_symlink() {
            break;
        }

        let target = match std::fs::read_link(&current) {
            Ok(target) => target,
            Err(_) => break,
        };

        let absolute_target = if target.is_absolute() {
            target
        } else {
            current
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(target)
        };

        push_unique_path(&mut paths, absolute_target.clone());
        current = absolute_target;
    }

    let resolved = safe_resolve_path(path);
    if resolved.is_symlink
        && normalize_for_comparison(&resolved.resolved_path) != normalize_for_comparison(path)
    {
        push_unique_path(&mut paths, resolved.resolved_path);
    }

    paths
}

/// Check whether a path sits inside a single allowed root.
#[must_use]
pub fn path_in_working_path(path: &Path, root: &Path) -> bool {
    let normalized_path = normalize_for_comparison(path);
    let normalized_root = normalize_for_comparison(root);

    normalized_path == normalized_root
        || normalized_path.starts_with(&(ensure_trailing_separator(&normalized_root)))
}

/// Check whether every checked path form sits inside the cwd or one of the additional roots.
#[must_use]
pub fn path_in_allowed_working_path(
    checked_paths: &[PathBuf],
    cwd: &Path,
    additional_dirs: &[PathBuf],
) -> bool {
    let mut roots = vec![cwd.to_path_buf()];
    roots.extend(additional_dirs.iter().cloned());

    let root_forms = roots
        .iter()
        .map(|root| get_paths_for_permission_check(root))
        .collect::<Vec<_>>();

    checked_paths.iter().all(|path| {
        root_forms
            .iter()
            .any(|forms| forms.iter().any(|root| path_in_working_path(path, root)))
    })
}

fn is_plan_file_path(checked_paths: &[PathBuf], plan_file: Option<&Path>) -> bool {
    let Some(plan_file) = plan_file else {
        return false;
    };
    let plan_forms = get_paths_for_permission_check(plan_file);
    checked_paths.iter().any(|checked| {
        plan_forms
            .iter()
            .any(|plan| normalize_for_comparison(checked) == normalize_for_comparison(plan))
    })
}

fn check_path_safety_for_auto_edit(checked_paths: &[PathBuf]) -> Option<String> {
    for path in checked_paths {
        if is_claude_config_file_path(path) {
            return Some("Editing Claude settings files requires manual approval.".to_owned());
        }
        if is_dangerous_file_path_to_auto_edit(path) {
            return Some(
                "Editing dangerous configuration paths requires manual approval.".to_owned(),
            );
        }
    }
    None
}

fn is_claude_config_file_path(path: &Path) -> bool {
    let normalized = normalize_for_comparison(path);
    normalized.ends_with("/.claude/settings.json")
        || normalized.ends_with("/.claude/settings.local.json")
}

fn is_dangerous_file_path_to_auto_edit(path: &Path) -> bool {
    let normalized = normalize_for_comparison(path);
    if normalized.starts_with("//") {
        return true;
    }

    let segments = normalized
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    if segments
        .iter()
        .any(|segment| DANGEROUS_DIRECTORIES.contains(segment))
    {
        return true;
    }

    segments
        .last()
        .is_some_and(|segment| DANGEROUS_FILES.contains(segment))
}

fn is_root_like(path: &str) -> bool {
    path == "/" || (path.len() == 3 && path.as_bytes()[1] == b':' && path.ends_with('/'))
}

fn ensure_trailing_separator(path: &str) -> String {
    if path.ends_with('/') {
        path.to_owned()
    } else {
        format!("{path}/")
    }
}

fn is_unc_path(path: &Path) -> bool {
    let rendered = path.to_string_lossy();
    rendered.starts_with(r"\\") || rendered.starts_with("//")
}

fn rejoin_tail(mut base: PathBuf, tail_segments: &[std::ffi::OsString]) -> PathBuf {
    for segment in tail_segments.iter().rev() {
        base.push(segment);
    }
    base
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    let normalized = normalize_for_comparison(&path);
    if paths
        .iter()
        .any(|existing| normalize_for_comparison(existing) == normalized)
    {
        return;
    }
    paths.push(path);
}

fn allow(normalized_path: PathBuf, checked_paths: Vec<PathBuf>) -> FilesystemCheckResult {
    FilesystemCheckResult {
        allowed: true,
        requires_confirmation: false,
        reason: None,
        normalized_path,
        checked_paths,
    }
}

fn ask(
    normalized_path: PathBuf,
    checked_paths: Vec<PathBuf>,
    reason: String,
) -> FilesystemCheckResult {
    FilesystemCheckResult {
        allowed: false,
        requires_confirmation: true,
        reason: Some(reason),
        normalized_path,
        checked_paths,
    }
}

fn deny(normalized_path: PathBuf, reason: String) -> FilesystemCheckResult {
    FilesystemCheckResult {
        allowed: false,
        requires_confirmation: false,
        reason: Some(reason),
        normalized_path,
        checked_paths: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[cfg(unix)]
    fn symlink_dir(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::unix::fs::symlink(target, link)
    }

    #[cfg(windows)]
    fn symlink_dir(target: &Path, link: &Path) -> std::io::Result<()> {
        std::os::windows::fs::symlink_dir(target, link)
    }

    #[test]
    fn allow_within_cwd() {
        let tempdir = tempdir().expect("tempdir");
        let result = assess_filesystem_access(
            "src/main.rs",
            tempdir.path(),
            &[],
            FilesystemOperation::Read,
            None,
        );
        assert!(result.allowed);
    }

    #[test]
    fn ask_outside_cwd() {
        let tempdir = tempdir().expect("tempdir");
        let outside = tempdir.path().parent().expect("parent").join("outside.txt");
        let result = assess_filesystem_access(
            outside.to_string_lossy().as_ref(),
            tempdir.path(),
            &[],
            FilesystemOperation::Read,
            None,
        );
        assert!(!result.allowed);
        assert!(result.requires_confirmation);
    }

    #[test]
    fn allow_additional_dir() {
        let tempdir = tempdir().expect("tempdir");
        let extra = tempdir.path().join("extra");
        std::fs::create_dir_all(&extra).expect("extra");
        let target = extra.join("file.txt");

        let result = assess_filesystem_access(
            target.to_string_lossy().as_ref(),
            tempdir.path(),
            std::slice::from_ref(&extra),
            FilesystemOperation::Read,
            None,
        );
        assert!(result.allowed);
    }

    #[test]
    fn dangerous_write_requires_confirmation() {
        let tempdir = tempdir().expect("tempdir");
        let target = tempdir.path().join(".git").join("config");
        let result = assess_filesystem_access(
            target.to_string_lossy().as_ref(),
            tempdir.path(),
            &[],
            FilesystemOperation::Write,
            None,
        );
        assert!(!result.allowed);
        assert!(result.requires_confirmation);
        assert!(result.reason.is_some());
    }

    #[test]
    fn invalid_path_is_denied() {
        let tempdir = tempdir().expect("tempdir");
        let result = assess_filesystem_access(
            "file\0.txt",
            tempdir.path(),
            &[],
            FilesystemOperation::Read,
            None,
        );
        assert!(!result.allowed);
        assert!(!result.requires_confirmation);
    }

    #[test]
    fn active_plan_file_is_allowed_outside_workspace() {
        let tempdir = tempdir().expect("tempdir");
        let workspace = tempdir.path().join("workspace");
        let profile = tempdir.path().join(".remote-code-rust");
        let plan_file = profile.join("plans").join("plan.md");
        std::fs::create_dir_all(&workspace).expect("workspace");
        std::fs::create_dir_all(plan_file.parent().expect("plan dir")).expect("plan dir");

        let result = assess_filesystem_access(
            plan_file.to_string_lossy().as_ref(),
            &workspace,
            &[],
            FilesystemOperation::Write,
            Some(&plan_file),
        );
        assert!(result.allowed);
    }

    #[test]
    fn parent_symlink_escape_requires_confirmation() {
        let tempdir = tempdir().expect("tempdir");
        let workspace = tempdir.path().join("workspace");
        let outside = tempdir.path().join("outside");
        std::fs::create_dir_all(&workspace).expect("workspace");
        std::fs::create_dir_all(&outside).expect("outside");

        let link = workspace.join("out");
        if symlink_dir(&outside, &link).is_err() {
            return;
        }

        let result = assess_filesystem_access(
            link.join("new.txt").to_string_lossy().as_ref(),
            &workspace,
            &[],
            FilesystemOperation::Create,
            None,
        );
        assert!(!result.allowed);
        assert!(result.requires_confirmation);
    }
}

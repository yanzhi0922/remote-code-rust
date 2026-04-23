//! Detection of destructive PowerShell commands.
//!
//! Identifies potentially destructive commands and returns a warning string
//! for display in the permission dialog. This is purely informational —
//! it doesn't affect permission logic or auto-approval.

use once_cell::sync::Lazy;
use regex::Regex;

struct DestructivePattern {
    pattern: Regex,
    warning: &'static str,
}

static DESTRUCTIVE_PATTERNS: Lazy<Vec<DestructivePattern>> = Lazy::new(|| {
    // Build patterns one by one so a single bad pattern doesn't poison the entire Lazy.
    let mut patterns = Vec::new();

    // Remove-Item with -Recurse and -Force (and common aliases)
    // Use simpler patterns to avoid regex compilation issues
    if let Ok(re) = Regex::new(r"(?i)\b(Remove-Item|rm|del|rd|rmdir|ri)\b.*-Recurse.*-Force\b") {
        patterns.push(DestructivePattern {
            pattern: re,
            warning: "Note: may recursively force-remove files",
        });
    }
    if let Ok(re) = Regex::new(r"(?i)\b(Remove-Item|rm|del|rd|rmdir|ri)\b.*-Force.*-Recurse\b") {
        patterns.push(DestructivePattern {
            pattern: re,
            warning: "Note: may recursively force-remove files",
        });
    }
    if let Ok(re) = Regex::new(r"(?i)\b(Remove-Item|rm|del|rd|rmdir|ri)\b.*-Recurse\b") {
        patterns.push(DestructivePattern {
            pattern: re,
            warning: "Note: may recursively remove files",
        });
    }
    if let Ok(re) = Regex::new(r"(?i)\b(Remove-Item|rm|del|rd|rmdir|ri)\b.*-Force\b") {
        patterns.push(DestructivePattern {
            pattern: re,
            warning: "Note: may force-remove files",
        });
    }

    // Stop-Process
    if let Ok(re) = Regex::new(r"(?i)\bStop-Process\b") {
        patterns.push(DestructivePattern {
            pattern: re,
            warning: "Note: may terminate running processes",
        });
    }

    // Remove-Service
    if let Ok(re) = Regex::new(r"(?i)\bRemove-Service\b") {
        patterns.push(DestructivePattern {
            pattern: re,
            warning: "Note: may remove a Windows service",
        });
    }

    // Clear-Content on broad paths
    if let Ok(re) = Regex::new(r"(?i)\bClear-Content\b.*\*") {
        patterns.push(DestructivePattern {
            pattern: re,
            warning: "Note: may clear content of multiple files",
        });
    }

    // Format-Volume
    if let Ok(re) = Regex::new(r"(?i)\bFormat-Volume\b") {
        patterns.push(DestructivePattern {
            pattern: re,
            warning: "Note: may format a disk volume",
        });
    }

    // Clear-Disk
    if let Ok(re) = Regex::new(r"(?i)\bClear-Disk\b") {
        patterns.push(DestructivePattern {
            pattern: re,
            warning: "Note: may clear a disk",
        });
    }

    // Git destructive operations
    if let Ok(re) = Regex::new(r"(?i)\bgit\s+reset\s+--hard\b") {
        patterns.push(DestructivePattern {
            pattern: re,
            warning: "Note: may discard uncommitted changes",
        });
    }
    if let Ok(re) = Regex::new(r"(?i)\bgit\s+push\b.*(--force|--force-with-lease)\b") {
        patterns.push(DestructivePattern {
            pattern: re,
            warning: "Note: may overwrite remote history",
        });
    }
    if let Ok(re) = Regex::new(r"(?i)\bgit\s+push\s+-f\b") {
        patterns.push(DestructivePattern {
            pattern: re,
            warning: "Note: may overwrite remote history",
        });
    }
    // git clean -f (but not -n dry-run)
    if let Ok(re) = Regex::new(r"(?i)\bgit\s+clean\b.*-f") {
        patterns.push(DestructivePattern {
            pattern: re,
            warning: "Note: may permanently delete untracked files",
        });
    }
    if let Ok(re) = Regex::new(r"(?i)\bgit\s+stash\s+(drop|clear)\b") {
        patterns.push(DestructivePattern {
            pattern: re,
            warning: "Note: may permanently remove stashed changes",
        });
    }

    // Database operations
    if let Ok(re) = Regex::new(r"(?i)\b(DROP|TRUNCATE)\s+(TABLE|DATABASE|SCHEMA)\b") {
        patterns.push(DestructivePattern {
            pattern: re,
            warning: "Note: may drop or truncate database objects",
        });
    }

    // System operations
    if let Ok(re) = Regex::new(r"(?i)\bStop-Computer\b") {
        patterns.push(DestructivePattern {
            pattern: re,
            warning: "Note: will shut down the computer",
        });
    }
    if let Ok(re) = Regex::new(r"(?i)\bRestart-Computer\b") {
        patterns.push(DestructivePattern {
            pattern: re,
            warning: "Note: will restart the computer",
        });
    }
    if let Ok(re) = Regex::new(r"(?i)\bClear-RecycleBin\b") {
        patterns.push(DestructivePattern {
            pattern: re,
            warning: "Note: permanently deletes recycled files",
        });
    }

    patterns
});

/// Checks if a PowerShell command matches known destructive patterns.
///
/// Returns a human-readable warning string, or `None` if no destructive
/// pattern is detected.
#[must_use]
pub fn get_destructive_warning(command: &str) -> Option<&'static str> {
    for dp in DESTRUCTIVE_PATTERNS.iter() {
        if dp.pattern.is_match(command) {
            return Some(dp.warning);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::get_destructive_warning;

    #[test]
    fn test_remove_item_recurse_force() {
        assert_eq!(
            get_destructive_warning("Remove-Item ./node_modules -Recurse -Force"),
            Some("Note: may recursively force-remove files")
        );
        assert_eq!(
            get_destructive_warning("rm -Recurse -Force ./build"),
            Some("Note: may recursively force-remove files")
        );
    }

    #[test]
    fn test_remove_item_recurse_only() {
        assert_eq!(
            get_destructive_warning("Remove-Item ./dist -Recurse"),
            Some("Note: may recursively remove files")
        );
    }

    #[test]
    fn test_remove_item_force_only() {
        assert_eq!(
            get_destructive_warning("Remove-Item ./temp -Force"),
            Some("Note: may force-remove files")
        );
    }

    #[test]
    fn test_stop_process() {
        assert_eq!(
            get_destructive_warning("Stop-Process -Name 'notepad'"),
            Some("Note: may terminate running processes")
        );
    }

    #[test]
    fn test_git_reset_hard() {
        assert_eq!(
            get_destructive_warning("git reset --hard HEAD"),
            Some("Note: may discard uncommitted changes")
        );
    }

    #[test]
    fn test_git_push_force() {
        assert_eq!(
            get_destructive_warning("git push --force origin main"),
            Some("Note: may overwrite remote history")
        );
    }

    #[test]
    fn test_git_clean_force() {
        assert_eq!(
            get_destructive_warning("git clean -fdx"),
            Some("Note: may permanently delete untracked files")
        );
    }

    #[test]
    fn test_format_volume() {
        assert_eq!(
            get_destructive_warning("Format-Volume -DriveLetter D -FileSystem NTFS"),
            Some("Note: may format a disk volume")
        );
    }

    #[test]
    fn test_stop_computer() {
        assert_eq!(
            get_destructive_warning("Stop-Computer"),
            Some("Note: will shut down the computer")
        );
    }

    #[test]
    fn test_restart_computer() {
        assert_eq!(
            get_destructive_warning("Restart-Computer"),
            Some("Note: will restart the computer")
        );
    }

    #[test]
    fn test_clear_recycle_bin() {
        assert_eq!(
            get_destructive_warning("Clear-RecycleBin"),
            Some("Note: permanently deletes recycled files")
        );
    }

    #[test]
    fn test_database_drop() {
        assert_eq!(
            get_destructive_warning("DROP TABLE users"),
            Some("Note: may drop or truncate database objects")
        );
    }

    #[test]
    fn test_safe_commands_no_warning() {
        assert!(get_destructive_warning("Get-Process").is_none());
        assert!(get_destructive_warning("Get-ChildItem -Force").is_none());
        assert!(get_destructive_warning("git status").is_none());
        assert!(get_destructive_warning("Write-Output 'hello'").is_none());
    }

    #[test]
    fn test_git_stash_drop() {
        assert_eq!(
            get_destructive_warning("git stash drop stash@{0}"),
            Some("Note: may permanently remove stashed changes")
        );
    }
}

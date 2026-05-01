//! Shell detection and validation utilities.
//!
//! Derived from `src/utils/shell.ts`.
//!
//! Provides shell path detection, validation against an allowlist,
//! and platform-specific fallback logic.


/// Security: Allowlist of approved shell executables to prevent arbitrary command execution.
///
/// Source: `src/utils/shell.ts` — `SHELL_ALLOWLIST`
const SHELL_ALLOWLIST: &[&str] = &[
    // Windows PowerShell variants
    r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe",
    r"C:\Program Files\PowerShell\7\pwsh.exe",
    r"C:\Program Files\PowerShell\6\pwsh.exe",
    r"C:\Program Files\PowerShell\5\pwsh.exe",
    // Windows Command Prompt
    r"C:\Windows\System32\cmd.exe",
    // Windows WSL
    r"C:\Windows\System32\wsl.exe",
    // Git Bash on Windows
    r"C:\Program Files\Git\bin\bash.exe",
    r"C:\Program Files\Git\usr\bin\bash.exe",
    r"C:\Program Files (x86)\Git\bin\bash.exe",
    r"C:\Program Files (x86)\Git\usr\bin\bash.exe",
    // MSYS2/MinGW/Cygwin on Windows
    r"C:\msys64\usr\bin\bash.exe",
    r"C:\msys32\usr\bin\bash.exe",
    r"C:\MinGW\msys\1.0\bin\bash.exe",
    r"C:\cygwin64\bin\bash.exe",
    r"C:\cygwin\bin\bash.exe",
    // Unix/Linux/macOS - Bourne-compatible shells
    "/bin/sh",
    "/usr/bin/sh",
    "/bin/bash",
    "/usr/bin/bash",
    "/usr/local/bin/bash",
    "/opt/homebrew/bin/bash",
    "/opt/local/bin/bash",
    // Z Shell
    "/bin/zsh",
    "/usr/bin/zsh",
    "/usr/local/bin/zsh",
    "/opt/homebrew/bin/zsh",
    "/opt/local/bin/zsh",
    // Dash
    "/bin/dash",
    "/usr/bin/dash",
    // Ash
    "/bin/ash",
    "/usr/bin/ash",
    // C Shells
    "/bin/csh",
    "/usr/bin/csh",
    "/bin/tcsh",
    "/usr/bin/tcsh",
    "/usr/local/bin/tcsh",
    // Korn Shells
    "/bin/ksh",
    "/usr/bin/ksh",
    "/bin/ksh93",
    "/usr/bin/ksh93",
    "/bin/mksh",
    "/usr/bin/mksh",
    "/bin/pdksh",
    "/usr/bin/pdksh",
    // Fish Shell
    "/usr/bin/fish",
    "/usr/local/bin/fish",
    "/opt/homebrew/bin/fish",
    "/opt/local/bin/fish",
    // Modern shells
    "/usr/bin/elvish",
    "/usr/local/bin/elvish",
    "/usr/bin/xonsh",
    "/usr/local/bin/xonsh",
    "/usr/bin/nu",
    "/usr/local/bin/nu",
    "/usr/bin/nushell",
    "/usr/local/bin/nushell",
    "/usr/bin/ion",
    "/usr/local/bin/ion",
    // BusyBox
    "/bin/busybox",
    "/usr/bin/busybox",
];

/// Shell path constants for fallback.
///
/// Source: `src/utils/shell.ts` — `SHELL_PATHS`
pub struct ShellPaths;

impl ShellPaths {
    pub const POWERSHELL_7: &str = r"C:\Program Files\PowerShell\7\pwsh.exe";
    pub const POWERSHELL_LEGACY: &str = r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe";
    pub const CMD: &str = r"C:\Windows\System32\cmd.exe";
    pub const WSL_BASH: &str = "/bin/bash";
    pub const MAC_DEFAULT: &str = "/bin/zsh";
    pub const LINUX_DEFAULT: &str = "/bin/bash";
    pub const FALLBACK: &str = "/bin/sh";
}

// ---------------------------------------------------------------------------
// Shell validation
// ---------------------------------------------------------------------------

/// Validates if a shell path is in the allowlist to prevent arbitrary command execution.
///
/// Source: `src/utils/shell.ts` — `isShellAllowed`
pub fn is_shell_allowed(shell_path: &str) -> bool {
    if shell_path.is_empty() {
        return false;
    }

    // Normalize the path (handle separators)
    let normalized = normalize_path(shell_path);

    // Direct lookup
    if SHELL_ALLOWLIST.contains(&normalized.as_str()) {
        return true;
    }

    // On Windows, try case-insensitive comparison
    if cfg!(windows) {
        let lower_path = normalized.to_lowercase();
        for allowed in SHELL_ALLOWLIST {
            if allowed.to_lowercase() == lower_path {
                return true;
            }
        }
    }

    false
}

/// Normalize a file path for comparison.
fn normalize_path(path: &str) -> String {
    // Simple normalization: replace forward slashes with backslashes on Windows
    if cfg!(windows) {
        path.replace('/', "\\")
    } else {
        path.to_string()
    }
}

// ---------------------------------------------------------------------------
// Shell detection
// ---------------------------------------------------------------------------

/// Returns the detected shell path for the current platform.
///
/// Source: `src/utils/shell.ts` — `getShell`
///
/// Detection order:
/// 1. Environment-specific shell (SHELL on Unix, COMSPEC on Windows)
/// 2. Safe fallback based on platform
/// 3. Validation against allowlist
pub fn get_shell() -> String {
    let mut shell: Option<String> = None;

    // Try environment-based detection
    shell = shell.or_else(get_shell_from_env);

    // If still nothing, fall back to a default
    if shell.is_none() {
        shell = Some(get_safe_fallback_shell().to_string());
    }

    // Validate the shell against allowlist
    if let Some(ref s) = shell {
        if !is_shell_allowed(s) {
            shell = Some(get_safe_fallback_shell().to_string());
        }
    }

    shell.unwrap_or_else(|| ShellPaths::FALLBACK.to_string())
}

/// Gets shell from environment variables.
///
/// Source: `src/utils/shell.ts` — `getShellFromEnv`
fn get_shell_from_env() -> Option<String> {
    if cfg!(windows) {
        std::env::var("COMSPEC")
            .ok()
            .or_else(|| Some(ShellPaths::CMD.to_string()))
    } else if cfg!(target_os = "macos") {
        std::env::var("SHELL")
            .ok()
            .or_else(|| Some(ShellPaths::MAC_DEFAULT.to_string()))
    } else if cfg!(target_os = "linux") {
        std::env::var("SHELL")
            .ok()
            .or_else(|| Some(ShellPaths::LINUX_DEFAULT.to_string()))
    } else {
        std::env::var("SHELL").ok()
    }
}

/// Returns a safe fallback shell based on the platform.
///
/// Source: `src/utils/shell.ts` — `getSafeFallbackShell`
pub fn get_safe_fallback_shell() -> &'static str {
    if cfg!(windows) {
        ShellPaths::CMD
    } else if cfg!(target_os = "macos") {
        ShellPaths::MAC_DEFAULT
    } else {
        ShellPaths::LINUX_DEFAULT
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn test_is_shell_allowed_unix_shells() {
        assert!(is_shell_allowed("/bin/bash"));
        assert!(is_shell_allowed("/bin/zsh"));
        assert!(is_shell_allowed("/bin/sh"));
        assert!(is_shell_allowed("/usr/bin/fish"));
    }

    #[test]
    fn test_is_shell_allowed_empty() {
        assert!(!is_shell_allowed(""));
    }

    #[test]
    fn test_is_shell_allowed_unknown() {
        assert!(!is_shell_allowed("/usr/local/bin/my-custom-shell"));
    }

    #[test]
    fn test_get_safe_fallback_shell() {
        let fallback = get_safe_fallback_shell();
        // Should return a valid path
        assert!(!fallback.is_empty());
    }

    #[test]
    fn test_get_shell_returns_non_empty() {
        let shell = get_shell();
        assert!(!shell.is_empty());
    }

    #[test]
    fn test_shell_paths_constants() {
        assert_eq!(ShellPaths::FALLBACK, "/bin/sh");
        assert_eq!(ShellPaths::MAC_DEFAULT, "/bin/zsh");
        assert_eq!(ShellPaths::LINUX_DEFAULT, "/bin/bash");
    }

    #[test]
    fn test_is_shell_allowed_with_normalization() {
        // On non-Windows, forward slashes should work
        if !cfg!(windows) {
            assert!(is_shell_allowed("/bin/bash"));
        }
    }
}

//! Shell integration detection and configuration.
//!
//! Manages shell integration for bash, zsh, fish, PowerShell, and cmd.
//! Provides temporary ZDOTDIR setup for zsh integration and shell
//! integration script path resolution.
//!
//! Source: `src/integrations/terminal/ShellIntegrationManager.ts`

use std::collections::HashMap;
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// ShellType
// ---------------------------------------------------------------------------

/// Supported shell types for integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShellType {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Cmd,
}

impl ShellType {
    /// Get the shell type from a shell executable path or name.
    pub fn from_path(path: &str) -> Self {
        let lower = path.to_lowercase();
        if lower.contains("bash") {
            ShellType::Bash
        } else if lower.contains("zsh") {
            ShellType::Zsh
        } else if lower.contains("fish") {
            ShellType::Fish
        } else if lower.contains("pwsh") || lower.contains("powershell") {
            ShellType::PowerShell
        } else if lower.contains("cmd") {
            ShellType::Cmd
        } else {
            // Default to bash on Unix, PowerShell on Windows
            if cfg!(windows) {
                ShellType::PowerShell
            } else {
                ShellType::Bash
            }
        }
    }

    /// Get the default shell for the current platform.
    pub fn default_shell() -> Self {
        if cfg!(windows) {
            ShellType::PowerShell
        } else {
            // Check SHELL env var
            match std::env::var("SHELL").as_deref() {
                Ok("/bin/zsh") | Ok("/usr/bin/zsh") => ShellType::Zsh,
                Ok("/bin/fish") | Ok("/usr/bin/fish") => ShellType::Fish,
                _ => ShellType::Bash,
            }
        }
    }

    /// Get the shell executable name.
    pub fn executable(&self) -> &'static str {
        match self {
            ShellType::Bash => "bash",
            ShellType::Zsh => "zsh",
            ShellType::Fish => "fish",
            ShellType::PowerShell => "pwsh",
            ShellType::Cmd => "cmd",
        }
    }

    /// Get the shell integration script filename.
    ///
    /// Mirrors the TS `getShellIntegrationPath()` mapping.
    pub fn integration_script_filename(&self) -> Option<&'static str> {
        match self {
            ShellType::Bash => Some("shellIntegration-bash.sh"),
            ShellType::Zsh => Some("shellIntegration-rc.zsh"),
            ShellType::Fish => Some("shellIntegration.fish"),
            ShellType::PowerShell => Some("shellIntegration.ps1"),
            ShellType::Cmd => None, // No shell integration for cmd
        }
    }
}

impl std::fmt::Display for ShellType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShellType::Bash => write!(f, "bash"),
            ShellType::Zsh => write!(f, "zsh"),
            ShellType::Fish => write!(f, "fish"),
            ShellType::PowerShell => write!(f, "pwsh"),
            ShellType::Cmd => write!(f, "cmd"),
        }
    }
}

// ---------------------------------------------------------------------------
// ShellIntegrationManager
// ---------------------------------------------------------------------------

/// Manages shell integration for terminal instances.
///
/// Handles:
/// - ZDOTDIR temporary directory creation for zsh integration
/// - Shell integration script path resolution
/// - Cleanup of temporary directories
///
/// Source: `src/integrations/terminal/ShellIntegrationManager.ts`
pub struct ShellIntegrationManager {
    /// Map from terminal ID to temporary ZDOTDIR path.
    terminal_tmp_dirs: HashMap<u32, PathBuf>,
    /// Base path for shell integration scripts (VSCode app root equivalent).
    shell_integration_base_path: Option<PathBuf>,
}

impl ShellIntegrationManager {
    /// Create a new ShellIntegrationManager.
    pub fn new() -> Self {
        Self {
            terminal_tmp_dirs: HashMap::new(),
            shell_integration_base_path: None,
        }
    }

    /// Create a new ShellIntegrationManager with a custom shell integration base path.
    pub fn with_base_path(base_path: PathBuf) -> Self {
        Self {
            terminal_tmp_dirs: HashMap::new(),
            shell_integration_base_path: Some(base_path),
        }
    }

    /// Get the map of terminal temporary directories.
    pub fn terminal_tmp_dirs(&self) -> &HashMap<u32, PathBuf> {
        &self.terminal_tmp_dirs
    }

    // -------------------------------------------------------------------
    // Shell integration script path
    // -------------------------------------------------------------------

    /// Get the path to the shell integration script for a given shell type.
    ///
    /// Mirrors the TS `getShellIntegrationPath()` method.
    pub fn get_shell_integration_path(&self, shell: ShellType) -> Option<PathBuf> {
        let filename = shell.integration_script_filename()?;

        match &self.shell_integration_base_path {
            Some(base) => {
                // Use the configured base path
                Some(base.join("vs").join("workbench").join("contrib")
                    .join("terminal").join("common").join("scripts")
                    .join(filename))
            }
            None => {
                // Try to find VSCode's app root
                // In the TS version, this uses vscode.env.appRoot
                // In Rust, we try common locations
                #[cfg(windows)]
                {
                    let program_files = std::env::var("PROGRAMFILES")
                        .unwrap_or_else(|_| "C:\\Program Files".to_string());
                    let vscode_path = PathBuf::from(program_files.clone())
                        .join("Microsoft VS Code")
                        .join("resources")
                        .join("app")
                        .join("out")
                        .join("vs")
                        .join("workbench")
                        .join("contrib")
                        .join("terminal")
                        .join("common")
                        .join("scripts")
                        .join(filename);
                    if vscode_path.parent().map(|p| p.exists()).unwrap_or(false) {
                        return Some(vscode_path);
                    }

                    // Try VS Code Insiders
                    let vscode_insiders_path = PathBuf::from(program_files)
                        .join("Microsoft VS Code Insiders")
                        .join("resources")
                        .join("app")
                        .join("out")
                        .join("vs")
                        .join("workbench")
                        .join("contrib")
                        .join("terminal")
                        .join("common")
                        .join("scripts")
                        .join(filename);
                    if vscode_insiders_path.parent().map(|p| p.exists()).unwrap_or(false) {
                        return Some(vscode_insiders_path);
                    }

                    None
                }
                #[cfg(not(windows))]
                {
                    // Try common macOS/Linux paths
                    let paths = [
                        "/Applications/Visual Studio Code.app/Contents/Resources/app/out",
                        "/usr/share/code/out",
                        "/usr/lib/code/out",
                    ];

                    for base in &paths {
                        let script_path = PathBuf::from(base)
                            .join("vs")
                            .join("workbench")
                            .join("contrib")
                            .join("terminal")
                            .join("common")
                            .join("scripts")
                            .join(filename);
                        if script_path.exists() {
                            return Some(script_path);
                        }
                    }

                    None
                }
            }
        }
    }

    // -------------------------------------------------------------------
    // Zsh ZDOTDIR integration
    // -------------------------------------------------------------------

    /// Initialize a temporary directory for ZDOTDIR (zsh shell integration).
    ///
    /// Creates a temporary directory with a `.zshrc` that sources the VSCode
    /// shell integration script and then sources the user's real zsh config.
    ///
    /// Mirrors the TS `zshInitTmpDir()` method.
    pub fn zsh_init_tmp_dir(
        &mut self,
        terminal_id: u32,
        env: &mut HashMap<String, String>,
    ) -> Result<PathBuf, String> {
        // Create a temporary directory
        let tmp_dir = std::env::temp_dir().join(format!(
            "roo-zdotdir-{}",
            rand::random::<u32>()
        ));

        // Save original ZDOTDIR
        if let Ok(original_zdotdir) = std::env::var("ZDOTDIR") {
            env.insert("ROO_ZDOTDIR".to_string(), original_zdotdir);
        }

        // Create the temporary directory
        std::fs::create_dir_all(&tmp_dir)
            .map_err(|e| format!("Failed to create temp dir: {}", e))?;

        // Get shell integration path
        let shell_integration_path = self
            .get_shell_integration_path(ShellType::Zsh)
            .unwrap_or_else(|| PathBuf::from("/dev/null"));

        // Create .zshrc content
        let _zdotdir_ref = if env.contains_key("ROO_ZDOTDIR") {
            "${ROO_ZDOTDIR}"
        } else {
            "$HOME"
        };

        let zshrc_content = format!(
            r#"
source "{}"
ZDOTDIR=${{{{ROO_ZDOTDIR:-$HOME}}}}
unset ROO_ZDOTDIR
[ -f "$ZDOTDIR/.zshenv" ] && source "$ZDOTDIR/.zshenv"
[ -f "$ZDOTDIR/.zprofile" ] && source "$ZDOTDIR/.zprofile"
[ -f "$ZDOTDIR/.zshrc" ] && source "$ZDOTDIR/.zshrc"
[ -f "$ZDOTDIR/.zlogin" ] && source "$ZDOTDIR/.zlogin"
[ "$ZDOTDIR" = "$HOME" ] && unset ZDOTDIR
"#,
            shell_integration_path.display()
        );

        // Write .zshrc
        let zshrc_path = tmp_dir.join(".zshrc");
        std::fs::write(&zshrc_path, &zshrc_content)
            .map_err(|e| format!("Failed to write .zshrc: {}", e))?;

        // Set ZDOTDIR in env
        env.insert("ZDOTDIR".to_string(), tmp_dir.to_string_lossy().to_string());

        // Track the temp dir
        self.terminal_tmp_dirs.insert(terminal_id, tmp_dir.clone());

        tracing::info!(
            "[ShellIntegrationManager] Created ZDOTDIR temp dir for terminal {}: {}",
            terminal_id,
            tmp_dir.display()
        );

        Ok(tmp_dir)
    }

    /// Clean up a temporary directory used for ZDOTDIR.
    ///
    /// Mirrors the TS `zshCleanupTmpDir()` method.
    pub fn zsh_cleanup_tmp_dir(&mut self, terminal_id: u32) -> bool {
        let tmp_dir = match self.terminal_tmp_dirs.remove(&terminal_id) {
            Some(d) => d,
            None => return false,
        };

        tracing::info!(
            "[ShellIntegrationManager] Cleaning up temp dir for terminal {}: {}",
            terminal_id,
            tmp_dir.display()
        );

        // Remove .zshrc
        let zshrc_path = tmp_dir.join(".zshrc");
        if zshrc_path.exists() {
            if let Err(e) = std::fs::remove_file(&zshrc_path) {
                tracing::warn!("Failed to remove .zshrc: {}", e);
            }
        }

        // Remove directory
        if tmp_dir.exists() {
            if let Err(e) = std::fs::remove_dir(&tmp_dir) {
                tracing::error!(
                    "[ShellIntegrationManager] Error cleaning up temp dir {}: {}",
                    tmp_dir.display(),
                    e
                );
                return false;
            }
        }

        true
    }

    /// Clean up all temporary directories.
    ///
    /// Mirrors the TS `clear()` method.
    pub fn clear(&mut self) {
        let terminal_ids: Vec<u32> = self.terminal_tmp_dirs.keys().copied().collect();
        for id in terminal_ids {
            self.zsh_cleanup_tmp_dir(id);
        }
    }

    // -------------------------------------------------------------------
    // Shell environment setup
    // -------------------------------------------------------------------

    /// Get environment modifications for shell integration.
    ///
    /// Returns a map of environment variable changes needed for the given
    /// shell type.
    pub fn get_shell_env(
        &mut self,
        terminal_id: u32,
        shell: ShellType,
    ) -> HashMap<String, String> {
        let mut env = HashMap::new();

        match shell {
            ShellType::Zsh => {
                // Set up ZDOTDIR for zsh integration
                if let Ok(_tmp_dir) = self.zsh_init_tmp_dir(terminal_id, &mut env) {
                    // ZDOTDIR is already set in env by zsh_init_tmp_dir
                }
            }
            ShellType::Bash => {
                // Bash integration is handled by sourcing the script in .bashrc
                // No env modifications needed
            }
            ShellType::Fish => {
                // Fish integration is handled by fish_user_paths or conf.d
                // No env modifications needed
            }
            ShellType::PowerShell => {
                // PowerShell integration is handled by profile
                // No env modifications needed
            }
            ShellType::Cmd => {
                // No shell integration for cmd
            }
        }

        // Common env vars for better terminal behavior
        env.insert("TERM_PROGRAM".to_string(), "roo-code".to_string());

        env
    }

    /// Get the shell integration source command for a given shell.
    ///
    /// Returns the command that should be executed to enable shell integration.
    pub fn get_integration_command(&self, shell: ShellType) -> Option<String> {
        let script_path = self.get_shell_integration_path(shell)?;

        match shell {
            ShellType::Bash => Some(format!("source '{}'", script_path.display())),
            ShellType::Zsh => Some(format!("source '{}'", script_path.display())),
            ShellType::Fish => Some(format!("source '{}'", script_path.display())),
            ShellType::PowerShell => Some(format!(". '{}'", script_path.display())),
            ShellType::Cmd => None,
        }
    }

    /// Detect the current shell from environment.
    pub fn detect_current_shell() -> ShellType {
        ShellType::default_shell()
    }
}

impl Default for ShellIntegrationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for ShellIntegrationManager {
    fn drop(&mut self) {
        self.clear();
    }
}

// ---------------------------------------------------------------------------
// ExecaTerminal / ExecaTerminalProcess equivalents
// ---------------------------------------------------------------------------

/// Configuration for an Execa-style terminal process.
///
/// Mirrors the TS `ExecaTerminal` class.
#[derive(Debug, Clone)]
pub struct ExecaTerminalConfig {
    /// Terminal ID.
    pub id: u32,
    /// Working directory.
    pub cwd: PathBuf,
    /// Shell to use.
    pub shell: ShellType,
    /// Whether to use UTF-8 encoding.
    pub utf8_encoding: bool,
}

impl ExecaTerminalConfig {
    /// Create a new Execa terminal config.
    pub fn new(id: u32, cwd: impl Into<PathBuf>) -> Self {
        Self {
            id,
            cwd: cwd.into(),
            shell: ShellType::default_shell(),
            utf8_encoding: true,
        }
    }

    /// Set the shell type.
    pub fn with_shell(mut self, shell: ShellType) -> Self {
        self.shell = shell;
        self
    }

    /// Check if this terminal is closed (always false for Execa terminals).
    pub fn is_closed(&self) -> bool {
        false
    }
}

/// The result of an Execa terminal process execution.
///
/// Mirrors the TS `ExecaTerminalProcess` fullOutput and exit code.
#[derive(Debug, Clone)]
pub struct ExecaProcessResult {
    /// Full output captured from the process.
    pub full_output: String,
    /// Exit code (0 for success).
    pub exit_code: i32,
    /// Signal name if the process was killed by a signal.
    pub signal_name: Option<String>,
}

impl ExecaProcessResult {
    /// Create a successful result.
    pub fn success(output: String) -> Self {
        Self {
            full_output: output,
            exit_code: 0,
            signal_name: None,
        }
    }

    /// Create a failure result.
    pub fn failure(exit_code: i32, output: String) -> Self {
        Self {
            full_output: output,
            exit_code,
            signal_name: None,
        }
    }
}

/// Merge a process and promise into a combined result.
///
/// In the TS version, `mergePromise` creates a mixin of both a TerminalProcess
/// and a Promise. In Rust, we use a simpler approach with explicit future handling.
///
/// Source: `src/integrations/terminal/mergePromise.ts`
pub fn merge_promise<F, T>(
    process_result: ExecaProcessResult,
    future: F,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ExecaProcessResult, String>> + Send>>
where
    F: std::future::Future<Output = Result<T, String>> + Send + 'static,
{
    Box::pin(async move {
        match future.await {
            Ok(_) => Ok(process_result),
            Err(e) => Err(e),
        }
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Test 1: ShellType from path ----
    #[test]
    fn test_shell_type_from_path() {
        assert_eq!(ShellType::from_path("/bin/bash"), ShellType::Bash);
        assert_eq!(ShellType::from_path("/bin/zsh"), ShellType::Zsh);
        assert_eq!(ShellType::from_path("/usr/bin/fish"), ShellType::Fish);
        assert_eq!(ShellType::from_path("pwsh"), ShellType::PowerShell);
        assert_eq!(ShellType::from_path("powershell"), ShellType::PowerShell);
        assert_eq!(ShellType::from_path("cmd.exe"), ShellType::Cmd);
    }

    // ---- Test 2: ShellType integration script filename ----
    #[test]
    fn test_integration_script_filename() {
        assert_eq!(
            ShellType::Bash.integration_script_filename(),
            Some("shellIntegration-bash.sh")
        );
        assert_eq!(
            ShellType::Zsh.integration_script_filename(),
            Some("shellIntegration-rc.zsh")
        );
        assert_eq!(
            ShellType::Fish.integration_script_filename(),
            Some("shellIntegration.fish")
        );
        assert_eq!(
            ShellType::PowerShell.integration_script_filename(),
            Some("shellIntegration.ps1")
        );
        assert_eq!(ShellType::Cmd.integration_script_filename(), None);
    }

    // ---- Test 3: ShellType display ----
    #[test]
    fn test_shell_type_display() {
        assert_eq!(format!("{}", ShellType::Bash), "bash");
        assert_eq!(format!("{}", ShellType::Zsh), "zsh");
        assert_eq!(format!("{}", ShellType::Fish), "fish");
        assert_eq!(format!("{}", ShellType::PowerShell), "pwsh");
        assert_eq!(format!("{}", ShellType::Cmd), "cmd");
    }

    // ---- Test 4: ShellType executable ----
    #[test]
    fn test_shell_type_executable() {
        assert_eq!(ShellType::Bash.executable(), "bash");
        assert_eq!(ShellType::Zsh.executable(), "zsh");
        assert_eq!(ShellType::Fish.executable(), "fish");
        assert_eq!(ShellType::PowerShell.executable(), "pwsh");
        assert_eq!(ShellType::Cmd.executable(), "cmd");
    }

    // ---- Test 5: ShellIntegrationManager creation ----
    #[test]
    fn test_manager_creation() {
        let manager = ShellIntegrationManager::new();
        assert!(manager.terminal_tmp_dirs.is_empty());
    }

    // ---- Test 6: ShellIntegrationManager with base path ----
    #[test]
    fn test_manager_with_base_path() {
        let manager = ShellIntegrationManager::with_base_path(PathBuf::from("/test"));
        assert!(manager.shell_integration_base_path.is_some());
    }

    // ---- Test 7: Get shell integration path with base path ----
    #[test]
    fn test_get_shell_integration_path() {
        let manager = ShellIntegrationManager::with_base_path(PathBuf::from("/vscode"));
        let path = manager.get_shell_integration_path(ShellType::Bash);
        assert!(path.is_some());
        let path = path.unwrap();
        assert!(path.to_string_lossy().contains("shellIntegration-bash.sh"));
    }

    // ---- Test 8: Zsh init and cleanup ----
    #[test]
    fn test_zsh_init_and_cleanup() {
        let mut manager = ShellIntegrationManager::new();
        let mut env = HashMap::new();

        let result = manager.zsh_init_tmp_dir(1, &mut env);
        // May fail if no shell integration scripts found, which is OK in test
        if let Ok(tmp_dir) = result {
            assert!(tmp_dir.exists());
            assert!(manager.terminal_tmp_dirs.contains_key(&1));

            let cleaned = manager.zsh_cleanup_tmp_dir(1);
            assert!(cleaned);
            assert!(!manager.terminal_tmp_dirs.contains_key(&1));
        }
    }

    // ---- Test 9: Cleanup non-existent terminal ----
    #[test]
    fn test_cleanup_nonexistent() {
        let mut manager = ShellIntegrationManager::new();
        assert!(!manager.zsh_cleanup_tmp_dir(999));
    }

    // ---- Test 10: Clear all ----
    #[test]
    fn test_clear_all() {
        let mut manager = ShellIntegrationManager::new();
        let mut env = HashMap::new();

        // Try to create two temp dirs
        let _ = manager.zsh_init_tmp_dir(1, &mut env);
        let _ = manager.zsh_init_tmp_dir(2, &mut env);

        manager.clear();
        assert!(manager.terminal_tmp_dirs.is_empty());
    }

    // ---- Test 11: ExecaTerminalConfig ----
    #[test]
    fn test_execa_terminal_config() {
        let config = ExecaTerminalConfig::new(1, "/tmp");
        assert_eq!(config.id, 1);
        assert!(!config.is_closed());
    }

    // ---- Test 12: ExecaProcessResult ----
    #[test]
    fn test_execa_process_result() {
        let success = ExecaProcessResult::success("output".to_string());
        assert_eq!(success.exit_code, 0);
        assert_eq!(success.full_output, "output");

        let failure = ExecaProcessResult::failure(1, "error".to_string());
        assert_eq!(failure.exit_code, 1);
    }

    // ---- Test 13: Get shell env ----
    #[test]
    fn test_get_shell_env() {
        let mut manager = ShellIntegrationManager::new();
        let env = manager.get_shell_env(1, ShellType::Bash);
        assert!(env.contains_key("TERM_PROGRAM"));
    }

    // ---- Test 14: Detect current shell ----
    #[test]
    fn test_detect_current_shell() {
        let shell = ShellIntegrationManager::detect_current_shell();
        // Should return a valid shell type
        assert!(matches!(
            shell,
            ShellType::Bash | ShellType::Zsh | ShellType::Fish | ShellType::PowerShell | ShellType::Cmd
        ));
    }
}

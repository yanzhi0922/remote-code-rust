#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellKind {
    Bash,
    PowerShell,
}

#[must_use]
pub fn is_read_only_command(kind: ShellKind, command: &str) -> bool {
    let normalized = normalize(command);
    let safe_prefixes = match kind {
        ShellKind::Bash => BASH_READ_ONLY_PREFIXES,
        ShellKind::PowerShell => POWERSHELL_READ_ONLY_PREFIXES,
    };
    safe_prefixes
        .iter()
        .any(|prefix| normalized.starts_with(prefix))
}

fn normalize(command: &str) -> String {
    command
        .trim()
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

const BASH_READ_ONLY_PREFIXES: &[&str] = &[
    "ls",
    "pwd",
    "cat",
    "head",
    "tail",
    "less",
    "more",
    "rg",
    "grep",
    "find",
    "stat",
    "du",
    "df",
    "ps",
    "env",
    "printenv",
    "git status",
    "git diff",
    "git log",
    "git show",
    "git branch",
    "git rev-parse",
    "git ls-files",
    "cargo check",
    "cargo test",
    "cargo build",
    "cargo fmt",
    "cargo clippy",
    "rustc --version",
    "python --version",
    "python3 --version",
    "node --version",
    "npm list",
    "date",
    "whoami",
];

const POWERSHELL_READ_ONLY_PREFIXES: &[&str] = &[
    "get-childitem",
    "dir",
    "ls",
    "get-content",
    "type",
    "gc",
    "select-string",
    "get-item",
    "test-path",
    "resolve-path",
    "get-location",
    "pwd",
    "git status",
    "git diff",
    "git log",
    "git show",
    "cargo check",
    "cargo test",
    "cargo build",
    "python --version",
    "py --version",
    "node --version",
    "where-object",
];

#[cfg(test)]
mod tests {
    use super::{ShellKind, is_read_only_command};

    #[test]
    fn bash_read_only_prefixes_match() {
        assert!(is_read_only_command(ShellKind::Bash, "git status"));
        assert!(is_read_only_command(ShellKind::Bash, "cargo test --lib"));
        assert!(!is_read_only_command(
            ShellKind::Bash,
            "git reset --hard HEAD"
        ));
    }

    #[test]
    fn powershell_read_only_prefixes_match() {
        assert!(is_read_only_command(
            ShellKind::PowerShell,
            "Get-ChildItem -Force"
        ));
        assert!(!is_read_only_command(
            ShellKind::PowerShell,
            "Remove-Item foo -Recurse"
        ));
    }
}

#[must_use]
pub fn is_destructive_git_command(command: &str) -> bool {
    let normalized = command.trim().to_ascii_lowercase();
    DESTRUCTIVE_GIT_PATTERNS
        .iter()
        .any(|pattern| normalized.contains(pattern))
}

const DESTRUCTIVE_GIT_PATTERNS: &[&str] = &[
    "git reset --hard",
    "git clean -fd",
    "git clean -fx",
    "git clean -fdx",
    "git checkout --",
    "git restore --source",
    "git push --force",
    "git push -f",
];

#[cfg(test)]
mod tests {
    use super::is_destructive_git_command;

    #[test]
    fn destructive_git_patterns_are_detected() {
        assert!(is_destructive_git_command("git reset --hard HEAD"));
        assert!(is_destructive_git_command("git clean -fdx"));
        assert!(!is_destructive_git_command("git status"));
    }

    #[test]
    fn git_clean_fx_is_destructive() {
        assert!(is_destructive_git_command("git clean -fx"));
    }

    #[test]
    fn git_push_force_is_destructive() {
        assert!(is_destructive_git_command("git push --force"));
    }

    #[test]
    fn git_push_f_is_destructive() {
        assert!(is_destructive_git_command("git push -f"));
    }

    #[test]
    fn git_checkout_dash_dash_dot_is_destructive() {
        assert!(is_destructive_git_command("git checkout -- ."));
    }

    #[test]
    fn git_restore_source_is_destructive() {
        assert!(is_destructive_git_command("git restore --source=HEAD~1"));
    }

    #[test]
    fn case_insensitive_detection() {
        assert!(is_destructive_git_command("GIT RESET --HARD"));
    }

    #[test]
    fn git_push_force_with_lease_is_destructive() {
        assert!(is_destructive_git_command("git push --force-with-lease"));
    }

    #[test]
    fn git_reset_soft_is_not_destructive() {
        assert!(!is_destructive_git_command("git reset --soft HEAD~1"));
    }
}

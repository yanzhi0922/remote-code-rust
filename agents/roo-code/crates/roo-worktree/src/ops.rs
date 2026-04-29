//! Pure logic functions for worktree management.
//!
//! Ported from the helper functions in `handlers.ts`.

// ---------------------------------------------------------------------------
// Random suffix / name generation
// ---------------------------------------------------------------------------

/// Characters used for random suffix generation (lowercase alphanumeric).
const SUFFIX_CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyz0123456789";

/// Default suffix length (matches the TS source).
const DEFAULT_SUFFIX_LENGTH: usize = 5;

/// Generate a random alphanumeric suffix of the given `length`.
///
/// Uses a simple Xorshift64 PRNG seeded from a thread-local counter so that
/// the function is pure (no `io`) yet produces varied output across calls.
pub fn generate_random_suffix(length: usize) -> String {
    use std::cell::Cell;

    thread_local! {
        static SEED: Cell<u64> = const { Cell::new(0x1234_5678_9ABC_DEF0) };
    }

    SEED.with(|cell| {
        let mut s = cell.get();
        let mut result = String::with_capacity(length);
        for _ in 0..length {
            // Xorshift64
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            let idx = (s % SUFFIX_CHARS.len() as u64) as usize;
            result.push(SUFFIX_CHARS[idx] as char);
        }
        cell.set(s);
        result
    })
}

/// Generate a default worktree name in the format `"worktree-XXXXX"`.
pub fn generate_worktree_name() -> String {
    format!("worktree-{}", generate_random_suffix(DEFAULT_SUFFIX_LENGTH))
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

/// Check whether `cwd` is a subfolder of `workspace_root`.
///
/// Returns `true` when `cwd` is strictly deeper than `workspace_root`
/// (i.e. it starts with `workspace_root` but is not equal).
pub fn is_workspace_subfolder(cwd: &str, workspace_root: &str) -> bool {
    let cwd_n = normalize_path(cwd);
    let root_n = normalize_path(&workspace_root);

    if cwd_n == root_n {
        return false;
    }

    // cwd must start with root + separator
    cwd_n.starts_with(&root_n)
        && cwd_n.as_bytes().get(root_n.len()) == Some(&b'/')
}

/// Normalise a path: replace `\` with `/`, remove trailing `/`.
fn normalize_path(p: &str) -> String {
    let mut s = p.replace('\\', "/");
    while s.ends_with('/') && s.len() > 1 {
        s.pop();
    }
    s
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- generate_random_suffix ----

    #[test]
    fn test_generate_random_suffix_length() {
        let s = generate_random_suffix(5);
        assert_eq!(s.len(), 5);
        let s = generate_random_suffix(10);
        assert_eq!(s.len(), 10);
    }

    #[test]
    fn test_generate_random_suffix_default() {
        let s = generate_random_suffix(DEFAULT_SUFFIX_LENGTH);
        assert_eq!(s.len(), 5);
    }

    #[test]
    fn test_generate_random_suffix_characters() {
        // All characters must be lowercase alphanumeric.
        for _ in 0..20 {
            let s = generate_random_suffix(32);
            for ch in s.chars() {
                assert!(
                    ch.is_ascii_lowercase() || ch.is_ascii_digit(),
                    "unexpected char: {ch}"
                );
            }
        }
    }

    #[test]
    fn test_generate_random_suffix_zero_length() {
        let s = generate_random_suffix(0);
        assert!(s.is_empty());
    }

    #[test]
    fn test_generate_random_suffix_uniqueness() {
        // Extremely unlikely to produce duplicates with a PRNG.
        let a = generate_random_suffix(8);
        let b = generate_random_suffix(8);
        // Not guaranteed, but practically always true.
        assert_ne!(a, b, "two consecutive suffixes should differ");
    }

    // ---- generate_worktree_name ----

    #[test]
    fn test_generate_worktree_name_format() {
        let name = generate_worktree_name();
        assert!(name.starts_with("worktree-"));
        assert_eq!(name.len(), "worktree-".len() + DEFAULT_SUFFIX_LENGTH);
    }

    #[test]
    fn test_generate_worktree_name_prefix() {
        let name = generate_worktree_name();
        let suffix = &name["worktree-".len()..];
        for ch in suffix.chars() {
            assert!(ch.is_ascii_lowercase() || ch.is_ascii_digit());
        }
    }

    // ---- is_workspace_subfolder ----

    #[test]
    fn test_is_workspace_subfolder_true() {
        assert!(is_workspace_subfolder(
            "/projects/repo/src",
            "/projects/repo"
        ));
        assert!(is_workspace_subfolder(
            "C:/projects/repo/sub",
            "C:/projects/repo"
        ));
    }

    #[test]
    fn test_is_workspace_subfolder_false_same() {
        assert!(!is_workspace_subfolder("/projects/repo", "/projects/repo"));
    }

    #[test]
    fn test_is_workspace_subfolder_false_different_root() {
        assert!(!is_workspace_subfolder("/other/path", "/projects/repo"));
    }

    #[test]
    fn test_is_workspace_subfolder_false_partial_match() {
        // "/projects/repo-other" should NOT match "/projects/repo"
        assert!(!is_workspace_subfolder(
            "/projects/repo-other",
            "/projects/repo"
        ));
    }

    #[test]
    fn test_is_workspace_subfolder_backslash() {
        assert!(is_workspace_subfolder(
            "C:\\projects\\repo\\sub",
            "C:\\projects\\repo"
        ));
    }

    #[test]
    fn test_is_workspace_subfolder_trailing_slash() {
        assert!(is_workspace_subfolder(
            "/projects/repo/sub/",
            "/projects/repo/"
        ));
    }

    // ---- normalize_path ----

    #[test]
    fn test_normalize_path_forward_slash() {
        assert_eq!(normalize_path("/a/b/c"), "/a/b/c");
    }

    #[test]
    fn test_normalize_path_backslash() {
        assert_eq!(normalize_path("C:\\a\\b"), "C:/a/b");
    }

    #[test]
    fn test_normalize_path_trailing_slash() {
        assert_eq!(normalize_path("/a/b/"), "/a/b");
    }
}

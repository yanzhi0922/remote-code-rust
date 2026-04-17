//! Feature flags for skill search capabilities.
//!
//! Flags are read from environment variables at runtime and can be toggled
//! without recompilation.  Programmatic overrides (used in tests) bypass
//! environment-variable reads entirely, avoiding the need for `unsafe`
//! `set_var`/`remove_var` calls.

use std::sync::atomic::{AtomicBool, Ordering};

// ---------------------------------------------------------------------------
// Environment variable names
// ---------------------------------------------------------------------------

const ENV_SKILL_SEARCH: &str = "RC_SKILL_SEARCH_ENABLED";
const ENV_REMOTE_SEARCH: &str = "RC_REMOTE_SEARCH_ENABLED";

// ---------------------------------------------------------------------------
// Override state (for programmatic / test control)
// ---------------------------------------------------------------------------

/// Whether the skill-search override is active.
static SKILL_OVERRIDE_ACTIVE: AtomicBool = AtomicBool::new(false);
/// Override value for skill search.
static SKILL_OVERRIDE_VALUE: AtomicBool = AtomicBool::new(true);

/// Whether the remote-search override is active.
static REMOTE_OVERRIDE_ACTIVE: AtomicBool = AtomicBool::new(false);
/// Override value for remote search.
static REMOTE_OVERRIDE_VALUE: AtomicBool = AtomicBool::new(false);

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Returns `true` if local skill search is enabled.
///
/// Priority:
/// 1. Programmatic override (set via [`set_skill_search_enabled`])
/// 2. Environment variable `RC_SKILL_SEARCH_ENABLED` (`1`/`true`/`yes` → on)
/// 3. Default: `true`
pub fn is_skill_search_enabled() -> bool {
    if SKILL_OVERRIDE_ACTIVE.load(Ordering::SeqCst) {
        return SKILL_OVERRIDE_VALUE.load(Ordering::SeqCst);
    }
    read_flag(ENV_SKILL_SEARCH, true)
}

/// Returns `true` if remote skill search is enabled.
///
/// Priority:
/// 1. Programmatic override (set via [`set_remote_search_enabled`])
/// 2. Environment variable `RC_REMOTE_SEARCH_ENABLED`
/// 3. Default: `false`
pub fn is_remote_search_enabled() -> bool {
    if REMOTE_OVERRIDE_ACTIVE.load(Ordering::SeqCst) {
        return REMOTE_OVERRIDE_VALUE.load(Ordering::SeqCst);
    }
    read_flag(ENV_REMOTE_SEARCH, false)
}

/// Set the skill-search flag programmatically.
///
/// This overrides the environment variable. Pass `None` to clear the override
/// and fall back to the env var / default.
pub fn set_skill_search_enabled(enabled: Option<bool>) {
    match enabled {
        Some(val) => {
            SKILL_OVERRIDE_VALUE.store(val, Ordering::SeqCst);
            SKILL_OVERRIDE_ACTIVE.store(true, Ordering::SeqCst);
        }
        None => {
            SKILL_OVERRIDE_ACTIVE.store(false, Ordering::SeqCst);
        }
    }
}

/// Set the remote-search flag programmatically.
///
/// This overrides the environment variable. Pass `None` to clear the override
/// and fall back to the env var / default.
pub fn set_remote_search_enabled(enabled: Option<bool>) {
    match enabled {
        Some(val) => {
            REMOTE_OVERRIDE_VALUE.store(val, Ordering::SeqCst);
            REMOTE_OVERRIDE_ACTIVE.store(true, Ordering::SeqCst);
        }
        None => {
            REMOTE_OVERRIDE_ACTIVE.store(false, Ordering::SeqCst);
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn read_flag(env_var: &str, default: bool) -> bool {
    match std::env::var(env_var) {
        Ok(val) => matches!(val.to_lowercase().as_str(), "1" | "true" | "yes"),
        Err(_) => default,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn reset_overrides() {
        set_skill_search_enabled(None);
        set_remote_search_enabled(None);
    }

    #[test]
    fn skill_search_default_true() {
        reset_overrides();
        assert!(is_skill_search_enabled());
    }

    #[test]
    fn remote_search_default_false() {
        reset_overrides();
        assert!(!is_remote_search_enabled());
    }

    #[test]
    fn override_skill_search_disabled() {
        set_skill_search_enabled(Some(false));
        assert!(!is_skill_search_enabled());
        reset_overrides();
    }

    #[test]
    fn override_skill_search_enabled() {
        set_skill_search_enabled(Some(true));
        assert!(is_skill_search_enabled());
        reset_overrides();
    }

    #[test]
    fn override_remote_search_enabled() {
        set_remote_search_enabled(Some(true));
        assert!(is_remote_search_enabled());
        reset_overrides();
    }

    #[test]
    fn override_remote_search_disabled() {
        set_remote_search_enabled(Some(false));
        assert!(!is_remote_search_enabled());
        reset_overrides();
    }

    #[test]
    fn override_clear_falls_back_to_default() {
        set_skill_search_enabled(Some(false));
        set_skill_search_enabled(None);
        // Falls back to env var or default (true).
        assert!(is_skill_search_enabled());
    }

    #[test]
    fn override_clear_remote_falls_back() {
        set_remote_search_enabled(Some(true));
        set_remote_search_enabled(None);
        // Falls back to env var or default (false).
        assert!(!is_remote_search_enabled());
    }
}

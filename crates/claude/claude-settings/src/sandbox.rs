//! Sandbox settings types.

use serde::{Deserialize, Serialize};

/// Sandbox settings configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SandboxSettings {
    /// Whether the sandbox is enabled.
    pub enabled: Option<bool>,
    /// Sandbox type (e.g., "docker", "landlock").
    pub sandbox_type: Option<String>,
    /// Custom sandbox image or profile.
    pub profile: Option<String>,
}

impl SandboxSettings {
    /// Check if sandbox is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled.unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_disabled() {
        let s = SandboxSettings::default();
        assert!(!s.is_enabled());
    }

    #[test]
    fn can_enable() {
        let s = SandboxSettings {
            enabled: Some(true),
            ..Default::default()
        };
        assert!(s.is_enabled());
    }
}

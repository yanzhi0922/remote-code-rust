//! Hook settings types.
//!
//! Corresponds to `src/utils/settings/types.ts` (hooks field).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Hook settings configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookSettings {
    /// Hooks for pre-tool-use events.
    #[serde(default)]
    pub pre_tool_use: Option<Vec<HookEntry>>,
    /// Hooks for post-tool-use events.
    #[serde(default)]
    pub post_tool_use: Option<Vec<HookEntry>>,
    /// Hooks for notification events.
    #[serde(default)]
    pub notification: Option<Vec<HookEntry>>,
    /// Hooks for stop events.
    #[serde(default)]
    pub stop: Option<Vec<HookEntry>>,
}

/// A single hook entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookEntry {
    /// The hook command to execute.
    pub command: String,
    /// Optional matcher for tool names.
    pub matcher: Option<String>,
    /// Optional timeout in milliseconds.
    pub timeout: Option<u64>,
    /// Whether to run the hook in the background.
    #[serde(default)]
    pub background: bool,
    /// Environment variables for the hook.
    #[serde(default)]
    pub env: HashMap<String, String>,
}

impl HookSettings {
    /// Get hooks for a specific event type.
    #[must_use]
    pub fn get_hooks(&self, event: &str) -> &[HookEntry] {
        match event {
            "pre_tool_use" => self.pre_tool_use.as_deref().unwrap_or(&[]),
            "post_tool_use" => self.post_tool_use.as_deref().unwrap_or(&[]),
            "notification" => self.notification.as_deref().unwrap_or(&[]),
            "stop" => self.stop.as_deref().unwrap_or(&[]),
            _ => &[],
        }
    }

    /// Check if any hooks are configured.
    #[must_use]
    pub fn has_hooks(&self) -> bool {
        self.pre_tool_use.as_ref().map_or(false, |h| !h.is_empty())
            || self.post_tool_use.as_ref().map_or(false, |h| !h.is_empty())
            || self.notification.as_ref().map_or(false, |h| !h.is_empty())
            || self.stop.as_ref().map_or(false, |h| !h.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_empty() {
        let h = HookSettings::default();
        assert!(!h.has_hooks());
    }

    #[test]
    fn has_hooks_with_pre_tool() {
        let h = HookSettings {
            pre_tool_use: Some(vec![HookEntry {
                command: "echo test".to_string(),
                matcher: None,
                timeout: None,
                background: false,
                env: HashMap::new(),
            }]),
            ..Default::default()
        };
        assert!(h.has_hooks());
    }

    #[test]
    fn get_hooks_by_event() {
        let h = HookSettings {
            stop: Some(vec![HookEntry {
                command: "cleanup".to_string(),
                matcher: None,
                timeout: None,
                background: false,
                env: HashMap::new(),
            }]),
            ..Default::default()
        };
        assert!(h.get_hooks("stop").len() == 1);
        assert!(h.get_hooks("pre_tool_use").is_empty());
        assert!(h.get_hooks("unknown").is_empty());
    }

    #[test]
    fn hook_entry_serialization() {
        let entry = HookEntry {
            command: "test.sh".to_string(),
            matcher: Some("Bash".to_string()),
            timeout: Some(5000),
            background: true,
            env: HashMap::new(),
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("test.sh"));
        assert!(json.contains("Bash"));
        assert!(json.contains("5000"));
    }
}

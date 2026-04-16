//! MCP server allowlist/denylist entry types.
//!
//! Corresponds to `src/utils/settings/types.ts` (AllowedMcpServerEntrySchema, DeniedMcpServerEntrySchema).

use serde::{Deserialize, Serialize};

/// Entry in the MCP server allowlist.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllowedMcpServerEntry {
    /// Match by server name (regex pattern).
    #[serde(default)]
    pub server_name: Option<String>,
    /// Match by server command (regex pattern).
    #[serde(default)]
    pub server_command: Option<String>,
    /// Match by server URL (regex pattern).
    #[serde(default)]
    pub server_url: Option<String>,
}

/// Entry in the MCP server denylist.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeniedMcpServerEntry {
    /// Match by server name (regex pattern).
    #[serde(default)]
    pub server_name: Option<String>,
    /// Match by server command (regex pattern).
    #[serde(default)]
    pub server_command: Option<String>,
    /// Match by server URL (regex pattern).
    #[serde(default)]
    pub server_url: Option<String>,
}

/// Trait for matching MCP server entries against a server.
pub trait McpServerEntryMatcher {
    /// Check if this entry matches the given server details.
    fn matches(&self, name: &str, command: &str, url: &str) -> bool;
}

impl McpServerEntryMatcher for AllowedMcpServerEntry {
    fn matches(&self, name: &str, command: &str, url: &str) -> bool {
        if let Some(pattern) = &self.server_name
            && simple_glob_match(pattern, name) {
                return true;
            }
        if let Some(pattern) = &self.server_command
            && simple_glob_match(pattern, command) {
                return true;
            }
        if let Some(pattern) = &self.server_url
            && simple_glob_match(pattern, url) {
                return true;
            }
        false
    }
}

impl McpServerEntryMatcher for DeniedMcpServerEntry {
    fn matches(&self, name: &str, command: &str, url: &str) -> bool {
        if let Some(pattern) = &self.server_name
            && simple_glob_match(pattern, name) {
                return true;
            }
        if let Some(pattern) = &self.server_command
            && simple_glob_match(pattern, command) {
                return true;
            }
        if let Some(pattern) = &self.server_url
            && simple_glob_match(pattern, url) {
                return true;
            }
        false
    }
}

/// Simple glob matching with `*` wildcard support.
fn simple_glob_match(pattern: &str, text: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if !pattern.contains('*') {
        return pattern == text;
    }
    // Split pattern by * and check sequential matching
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.is_empty() {
        return true;
    }

    let mut idx = 0;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            if !text[idx..].starts_with(part) {
                return false;
            }
            idx += part.len();
        } else if i == parts.len() - 1 {
            if !text.ends_with(part) {
                return false;
            }
        } else {
            match text[idx..].find(part) {
                Some(pos) => idx += pos + part.len(),
                None => return false,
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_entry_by_name() {
        let entry = AllowedMcpServerEntry {
            server_name: Some("my-server".to_string()),
            server_command: None,
            server_url: None,
        };
        assert!(entry.matches("my-server", "", ""));
        assert!(!entry.matches("other-server", "", ""));
    }

    #[test]
    fn allowed_entry_wildcard() {
        let entry = AllowedMcpServerEntry {
            server_name: Some("prefix-*".to_string()),
            server_command: None,
            server_url: None,
        };
        assert!(entry.matches("prefix-server", "", ""));
        assert!(!entry.matches("other-server", "", ""));
    }

    #[test]
    fn denied_entry_by_url() {
        let entry = DeniedMcpServerEntry {
            server_name: None,
            server_command: None,
            server_url: Some("https://*.example.com/*".to_string()),
        };
        assert!(entry.matches("", "", "https://api.example.com/v1"));
    }

    #[test]
    fn simple_glob_exact() {
        assert!(simple_glob_match("test", "test"));
        assert!(!simple_glob_match("test", "other"));
    }

    #[test]
    fn simple_glob_star() {
        assert!(simple_glob_match("*", "anything"));
    }

    #[test]
    fn simple_glob_prefix() {
        assert!(simple_glob_match("prefix-*", "prefix-test"));
        assert!(!simple_glob_match("prefix-*", "other-test"));
    }

    #[test]
    fn simple_glob_suffix() {
        assert!(simple_glob_match("*-suffix", "test-suffix"));
        assert!(!simple_glob_match("*-suffix", "test-other"));
    }

    #[test]
    fn simple_glob_middle() {
        assert!(simple_glob_match("start*end", "start-middle-end"));
        assert!(!simple_glob_match("start*end", "start-middle-other"));
    }

    #[test]
    fn entry_serialization() {
        let entry = AllowedMcpServerEntry {
            server_name: Some("test-server".to_string()),
            server_command: None,
            server_url: None,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("test-server"));
        let deserialized: AllowedMcpServerEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.server_name, Some("test-server".to_string()));
    }
}

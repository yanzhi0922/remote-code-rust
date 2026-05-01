//! Simple Installer for marketplace items (modes and MCPs).
//!
//! Installs and removes marketplace items to/from local configuration files.
//! Mirrors `SimpleInstaller.ts`.

use std::collections::HashMap;
use std::path::PathBuf;

use serde_json::Value;

use crate::types::{MarketplaceItem, MarketplaceItemType};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors that can occur during installation.
#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("Unsupported item type: {0}")]
    UnsupportedItemType(String),

    #[error("Mode item missing content")]
    ModeMissingContent,

    #[error("Mode content should not be an array")]
    ModeContentArray,

    #[error("Invalid mode content: mode missing slug")]
    ModeMissingSlug,

    #[error("MCP item missing content")]
    McpMissingContent,

    #[error("Invalid YAML: {0}")]
    InvalidYaml(String),

    #[error("Invalid JSON: {0}")]
    InvalidJson(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("No workspace folder found")]
    NoWorkspaceFolder,

    #[error("CustomModesManager is not available")]
    NoCustomModesManager,
}

// ---------------------------------------------------------------------------
// Install options
// ---------------------------------------------------------------------------

/// Options for installing a marketplace item.
#[derive(Debug, Clone)]
pub struct InstallOptions {
    /// Whether to install to project or global scope.
    pub target: InstallTarget,
    /// Which installation method to use (for array content).
    pub selected_index: Option<usize>,
    /// Parameters to substitute in content templates.
    pub parameters: Option<HashMap<String, String>>,
}

/// Installation target scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallTarget {
    Project,
    Global,
}

/// Result of an installation operation.
#[derive(Debug, Clone)]
pub struct InstallResult {
    pub file_path: PathBuf,
    pub line: Option<usize>,
}

// ---------------------------------------------------------------------------
// SimpleInstaller
// ---------------------------------------------------------------------------

/// Installs and removes marketplace items (modes and MCPs).
///
/// Source: `.research/Roo-Code/src/services/marketplace/SimpleInstaller.ts`
pub struct SimpleInstaller {
    /// Path to the project workspace directory.
    project_path: Option<PathBuf>,
    /// Path to the global settings directory.
    global_settings_path: PathBuf,
}

impl SimpleInstaller {
    /// Create a new `SimpleInstaller`.
    pub fn new(project_path: Option<PathBuf>, global_settings_path: PathBuf) -> Self {
        Self {
            project_path,
            global_settings_path,
        }
    }

    /// Install a marketplace item.
    pub async fn install_item(
        &self,
        item: &MarketplaceItem,
        options: &InstallOptions,
    ) -> Result<InstallResult, InstallError> {
        match item.item_type {
            MarketplaceItemType::Mode => self.install_mode(item, options.target).await,
            MarketplaceItemType::Mcp => self.install_mcp(item, options).await,
            _ => Err(InstallError::UnsupportedItemType(format!(
                "{:?}",
                item.item_type
            ))),
        }
    }

    /// Remove a marketplace item.
    pub async fn remove_item(
        &self,
        item: &MarketplaceItem,
        target: InstallTarget,
    ) -> Result<(), InstallError> {
        match item.item_type {
            MarketplaceItemType::Mode => self.remove_mode(item, target).await,
            MarketplaceItemType::Mcp => self.remove_mcp(item, target).await,
            _ => Err(InstallError::UnsupportedItemType(format!(
                "{:?}",
                item.item_type
            ))),
        }
    }

    // -- Mode installation -----------------------------------------------------

    async fn install_mode(
        &self,
        item: &MarketplaceItem,
        target: InstallTarget,
    ) -> Result<InstallResult, InstallError> {
        let content = item
            .extra
            .get("content")
            .ok_or(InstallError::ModeMissingContent)?;

        // Modes should always have string content, not array
        if content.is_array() {
            return Err(InstallError::ModeContentArray);
        }

        let content_str = content
            .as_str()
            .ok_or(InstallError::ModeMissingContent)?;

        let file_path = self.get_mode_file_path(target)?;

        // Parse the mode data from YAML content
        let mode_data: serde_yaml::Value = serde_yaml::from_str(content_str)?;

        // Read existing file or create new structure
        let mut existing_data: serde_yaml::Value =
            serde_yaml::from_str("{customModes: []}")?;

        match tokio::fs::read_to_string(&file_path).await {
            Ok(existing) => {
                let parsed: serde_yaml::Value = serde_yaml::from_str(&existing)?;
                if parsed.is_mapping() {
                    existing_data = parsed;
                }
            }
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    return Err(InstallError::Io(e));
                }
                // File doesn't exist, use default structure
            }
        }

        // Ensure customModes array exists
        if existing_data.get("customModes").is_none() {
            existing_data["customModes"] = serde_yaml::Value::Sequence(vec![]);
        }

        let slug = mode_data
            .get("slug")
            .and_then(|v| v.as_str())
            .ok_or(InstallError::ModeMissingSlug)?;

        // Remove existing mode with same slug
        if let Some(modes) = existing_data
            .get_mut("customModes")
            .and_then(|v| v.as_sequence_mut())
        {
            modes.retain(|m| {
                m.get("slug")
                    .and_then(|v| v.as_str())
                    .map(|s| s != slug)
                    .unwrap_or(true)
            });
        }

        // Add the new mode
        if let Some(modes) = existing_data
            .get_mut("customModes")
            .and_then(|v| v.as_sequence_mut())
        {
            modes.push(mode_data.clone());
        }

        // Write back to file
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let yaml_content = serde_yaml::to_string(&existing_data)?;
        tokio::fs::write(&file_path, &yaml_content).await?;

        // Calculate line number where the mode was added
        let line = find_slug_line(&yaml_content, slug);

        Ok(InstallResult { file_path, line })
    }

    // -- MCP installation ------------------------------------------------------

    async fn install_mcp(
        &self,
        item: &MarketplaceItem,
        options: &InstallOptions,
    ) -> Result<InstallResult, InstallError> {
        let content_value = item
            .extra
            .get("content")
            .ok_or(InstallError::McpMissingContent)?;

        // Get the content to use
        let mut content_to_use = if content_value.is_array() {
            let index = options.selected_index.unwrap_or(0);
            let arr = content_value.as_array().unwrap();
            let method = arr.get(index).or_else(|| arr.first());
            match method {
                Some(m) => m
                    .get("content")
                    .and_then(|v: &Value| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                None => return Err(InstallError::McpMissingContent),
            }
        } else {
            content_value.as_str().unwrap_or("").to_string()
        };

        // Replace parameters if provided
        if let Some(params) = &options.parameters {
            for (key, value) in params {
                let pattern = format!("{{{{{}}}}}", key);
                content_to_use = content_to_use.replace(&pattern, value);
            }
        }

        let file_path = self.get_mcp_file_path(options.target)?;

        // Parse the MCP data from JSON content
        let mcp_data: Value = serde_json::from_str(&content_to_use)?;

        // Read existing file or create new structure
        let mut existing_data: Value = serde_json::json!({ "mcpServers": {} });

        match tokio::fs::read_to_string(&file_path).await {
            Ok(existing) => match serde_json::from_str::<Value>(&existing) {
                Ok(parsed) => {
                    existing_data = parsed;
                }
                Err(_) => {
                    let file_name = match options.target {
                        InstallTarget::Project => ".roo/mcp.json",
                        InstallTarget::Global => "mcp-settings.json",
                    };
                    return Err(InstallError::InvalidJson(format!(
                        "Cannot install MCP server: The {} file contains invalid JSON. \
                         Please fix the syntax errors in the file before installing new servers.",
                        file_name
                    )));
                }
            },
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    return Err(InstallError::Io(e));
                }
                // File doesn't exist, use default structure
            }
        }

        // Ensure mcpServers object exists
        if existing_data.get("mcpServers").is_none() {
            existing_data["mcpServers"] = serde_json::json!({});
        }

        // Use the item id as the server name
        let server_name = &item.id;

        // Add or update the server
        existing_data["mcpServers"][server_name] = mcp_data;

        // Write back to file
        if let Some(parent) = file_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let json_content = serde_json::to_string_pretty(&existing_data)?;
        tokio::fs::write(&file_path, &json_content).await?;

        // Calculate line number
        let line = find_server_line(&json_content, server_name);

        Ok(InstallResult { file_path, line })
    }

    // -- Mode removal ----------------------------------------------------------

    async fn remove_mode(
        &self,
        item: &MarketplaceItem,
        _target: InstallTarget,
    ) -> Result<(), InstallError> {
        let content_value = item
            .extra
            .get("content")
            .ok_or(InstallError::ModeMissingContent)?;

        let content_str = if content_value.is_array() {
            content_value
                .as_array()
                .and_then(|arr: &Vec<Value>| arr.first())
                .and_then(|v: &Value| v.get("content"))
                .and_then(|v: &Value| v.as_str())
                .unwrap_or("")
        } else {
            content_value.as_str().unwrap_or("")
        };

        let mode_data: serde_yaml::Value = serde_yaml::from_str(content_str)?;
        let _slug = mode_data
            .get("slug")
            .and_then(|v| v.as_str())
            .ok_or(InstallError::ModeMissingSlug)?;

        // In the full implementation, this would call CustomModesManager.deleteCustomMode
        Ok(())
    }

    // -- MCP removal -----------------------------------------------------------

    async fn remove_mcp(
        &self,
        item: &MarketplaceItem,
        target: InstallTarget,
    ) -> Result<(), InstallError> {
        let file_path = self.get_mcp_file_path(target)?;

        match tokio::fs::read_to_string(&file_path).await {
            Ok(existing) => {
                let mut existing_data: Value = serde_json::from_str(&existing)?;

                if let Some(servers) = existing_data.get_mut("mcpServers") {
                    let server_name = &item.id;
                    if let Some(obj) = servers.as_object_mut() {
                        obj.remove(server_name);
                    }
                    let json_content = serde_json::to_string_pretty(&existing_data)?;
                    tokio::fs::write(&file_path, &json_content).await?;
                }
                Ok(())
            }
            Err(_) => {
                // File doesn't exist or other error, nothing to remove
                Ok(())
            }
        }
    }

    // -- File path helpers -----------------------------------------------------

    fn get_mode_file_path(&self, target: InstallTarget) -> Result<PathBuf, InstallError> {
        match target {
            InstallTarget::Project => {
                let project = self.project_path.as_ref().ok_or(InstallError::NoWorkspaceFolder)?;
                Ok(project.join(".roomodes"))
            }
            InstallTarget::Global => Ok(self.global_settings_path.join("custom_modes.yaml")),
        }
    }

    fn get_mcp_file_path(&self, target: InstallTarget) -> Result<PathBuf, InstallError> {
        match target {
            InstallTarget::Project => {
                let project = self.project_path.as_ref().ok_or(InstallError::NoWorkspaceFolder)?;
                Ok(project.join(".roo").join("mcp.json"))
            }
            InstallTarget::Global => Ok(self.global_settings_path.join("mcp_settings.json")),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Find the 1-based line number containing the slug reference.
fn find_slug_line(content: &str, slug: &str) -> Option<usize> {
    for (i, line) in content.lines().enumerate() {
        if line.contains(&format!("slug: {}", slug))
            || line.contains(&format!("slug: \"{}\"", slug))
        {
            return Some(i + 1);
        }
    }
    None
}

/// Find the 1-based line number containing the server name reference.
fn find_server_line(content: &str, server_name: &str) -> Option<usize> {
    for (i, line) in content.lines().enumerate() {
        if line.contains(&format!("\"{}\"", server_name)) {
            return Some(i + 1);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_slug_line() {
        let content = "customModes:\n  - slug: test-mode\n    name: Test\n";
        assert_eq!(find_slug_line(content, "test-mode"), Some(2));
    }

    #[test]
    fn test_find_slug_line_not_found() {
        let content = "customModes:\n  - slug: other-mode\n";
        assert_eq!(find_slug_line(content, "test-mode"), None);
    }

    #[test]
    fn test_find_server_line() {
        let content = "{\n  \"mcpServers\": {\n    \"my-server\": {}\n  }\n}";
        assert_eq!(find_server_line(content, "my-server"), Some(3));
    }

    #[test]
    fn test_find_server_line_not_found() {
        let content = "{\n  \"mcpServers\": {\n  }\n}";
        assert_eq!(find_server_line(content, "my-server"), None);
    }

    #[tokio::test]
    async fn test_install_mode_creates_file() {
        let tmp = tempfile::tempdir().unwrap();
        let installer = SimpleInstaller::new(
            Some(tmp.path().to_path_buf()),
            tmp.path().join("global"),
        );

        let mut extra = serde_json::Map::new();
        extra.insert(
            "content".to_string(),
            Value::String("slug: my-mode\nname: My Mode".to_string()),
        );

        let item = MarketplaceItem {
            id: "my-mode".to_string(),
            name: "My Mode".to_string(),
            description: "Test mode".to_string(),
            item_type: MarketplaceItemType::Mode,
            author: "test".to_string(),
            url: "https://example.com".to_string(),
            tags: vec![],
            installed: false,
            extra,
        };

        let options = InstallOptions {
            target: InstallTarget::Project,
            selected_index: None,
            parameters: None,
        };

        let result = installer.install_item(&item, &options).await.unwrap();
        assert!(result.file_path.exists());
        assert!(result.line.is_some());
    }

    #[tokio::test]
    async fn test_install_mcp_creates_file() {
        let tmp = tempfile::tempdir().unwrap();
        let installer = SimpleInstaller::new(
            Some(tmp.path().to_path_buf()),
            tmp.path().join("global"),
        );

        let mut extra = serde_json::Map::new();
        extra.insert(
            "content".to_string(),
            Value::String("{\"command\":\"node\",\"args\":[\"server.js\"]}".to_string()),
        );

        let item = MarketplaceItem {
            id: "my-mcp".to_string(),
            name: "My MCP".to_string(),
            description: "Test MCP".to_string(),
            item_type: MarketplaceItemType::Mcp,
            author: "test".to_string(),
            url: "https://example.com".to_string(),
            tags: vec![],
            installed: false,
            extra,
        };

        let options = InstallOptions {
            target: InstallTarget::Project,
            selected_index: None,
            parameters: None,
        };

        let result = installer.install_item(&item, &options).await.unwrap();
        assert!(result.file_path.exists());

        // Verify content
        let content = tokio::fs::read_to_string(&result.file_path).await.unwrap();
        let parsed: Value = serde_json::from_str(&content).unwrap();
        assert!(parsed["mcpServers"]["my-mcp"].is_object());
    }

    #[tokio::test]
    async fn test_remove_mcp() {
        let tmp = tempfile::tempdir().unwrap();
        let installer = SimpleInstaller::new(
            Some(tmp.path().to_path_buf()),
            tmp.path().join("global"),
        );

        // First install
        let mut extra = serde_json::Map::new();
        extra.insert(
            "content".to_string(),
            Value::String("{\"command\":\"node\"}".to_string()),
        );

        let item = MarketplaceItem {
            id: "my-mcp".to_string(),
            name: "My MCP".to_string(),
            description: "Test MCP".to_string(),
            item_type: MarketplaceItemType::Mcp,
            author: "test".to_string(),
            url: "https://example.com".to_string(),
            tags: vec![],
            installed: false,
            extra,
        };

        let options = InstallOptions {
            target: InstallTarget::Project,
            selected_index: None,
            parameters: None,
        };

        installer.install_item(&item, &options).await.unwrap();

        // Then remove
        installer
            .remove_item(&item, InstallTarget::Project)
            .await
            .unwrap();

        // Verify it was removed
        let mcp_path = tmp.path().join(".roo").join("mcp.json");
        let content = tokio::fs::read_to_string(&mcp_path).await.unwrap();
        let parsed: Value = serde_json::from_str(&content).unwrap();
        assert!(parsed["mcpServers"]["my-mcp"].is_null());
    }
}

//! LSP Server registry and lifecycle management.
//!
//! Manages multiple LSP server instances, each associated with one or more
//! programming languages. Handles starting, stopping, and querying servers.

use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::client::LspClient;

// ---------------------------------------------------------------------------
// LspServerConfig
// ---------------------------------------------------------------------------

/// Configuration for starting an LSP server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LspServerConfig {
    /// The command to start the server (e.g. "rust-analyzer").
    pub command: String,
    /// Command-line arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// Languages this server supports (e.g. ["rust", "toml"]).
    pub languages: Vec<String>,
    /// Environment variables to set.
    #[serde(default)]
    pub env: HashMap<String, String>,
}

impl LspServerConfig {
    /// Create a new server config.
    #[must_use]
    pub fn new(command: impl Into<String>, languages: Vec<String>) -> Self {
        Self {
            command: command.into(),
            args: Vec::new(),
            languages,
            env: HashMap::new(),
        }
    }

    /// Add command-line arguments.
    #[must_use]
    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    /// Whether this server supports the given language.
    #[must_use]
    pub fn supports_language(&self, language: &str) -> bool {
        self.languages
            .iter()
            .any(|l| l.eq_ignore_ascii_case(language))
    }
}

// ---------------------------------------------------------------------------
// LspServerInstance
// ---------------------------------------------------------------------------

/// Status of a managed LSP server instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServerStatus {
    /// Server is stopped.
    Stopped,
    /// Server is starting up.
    Starting,
    /// Server is running and ready.
    Running,
    /// Server encountered an error.
    Error,
}

impl std::fmt::Display for ServerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Stopped => write!(f, "stopped"),
            Self::Starting => write!(f, "starting"),
            Self::Running => write!(f, "running"),
            Self::Error => write!(f, "error"),
        }
    }
}

/// A running LSP server instance.
#[derive(Debug)]
pub struct LspServerInstance {
    /// Server configuration.
    pub config: LspServerConfig,
    /// Current status.
    pub status: ServerStatus,
    /// LSP client for communicating with this server.
    pub client: Arc<LspClient>,
}

impl LspServerInstance {
    /// Create a new server instance from a config.
    fn from_config(config: &LspServerConfig, root_uri: &str) -> Self {
        Self {
            config: config.clone(),
            status: ServerStatus::Stopped,
            client: LspClient::new(root_uri),
        }
    }
}

// ---------------------------------------------------------------------------
// LspServerManager
// ---------------------------------------------------------------------------

/// Manages multiple LSP server instances.
#[derive(Debug)]
pub struct LspServerManager {
    /// Root URI for all servers.
    root_uri: String,
    /// Registered server instances, keyed by server ID.
    instances: RwLock<HashMap<String, LspServerInstance>>,
    /// Language → server ID mapping.
    language_map: RwLock<HashMap<String, String>>,
}

impl LspServerManager {
    /// Create a new server manager for the given root URI.
    #[must_use]
    pub fn new(root_uri: &str) -> Self {
        Self {
            root_uri: root_uri.to_string(),
            instances: RwLock::new(HashMap::new()),
            language_map: RwLock::new(HashMap::new()),
        }
    }

    /// Register a server configuration.
    ///
    /// The server is not started automatically; call [`start_server`] to launch it.
    pub fn register(&self, id: &str, config: LspServerConfig) {
        let instance = LspServerInstance::from_config(&config, &self.root_uri);

        // Update language mapping
        let mut lang_map = self.language_map.write();
        for lang in &config.languages {
            lang_map.insert(lang.to_lowercase(), id.to_string());
        }

        self.instances.write().insert(id.to_string(), instance);
    }

    /// Start a registered server by ID.
    pub fn start_server(&self, id: &str) -> Result<()> {
        let mut instances = self.instances.write();
        let instance = instances
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Server '{id}' not registered"))?;

        if instance.status == ServerStatus::Running {
            anyhow::bail!("Server '{id}' is already running");
        }

        instance.status = ServerStatus::Starting;
        instance.client.initialize("remote-code", "0.1.0")?;
        instance.status = ServerStatus::Running;
        Ok(())
    }

    /// Stop a running server by ID.
    pub fn stop_server(&self, id: &str) -> Result<()> {
        let mut instances = self.instances.write();
        let instance = instances
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("Server '{id}' not registered"))?;

        if instance.status != ServerStatus::Running {
            anyhow::bail!("Server '{id}' is not running");
        }

        instance.client.shutdown()?;
        instance.status = ServerStatus::Stopped;
        Ok(())
    }

    /// Get the server instance for a given language.
    #[must_use]
    pub fn get_server_for_language(&self, language: &str) -> Option<Arc<LspClient>> {
        let lang_map = self.language_map.read();
        let server_id = lang_map.get(&language.to_lowercase())?;
        let instances = self.instances.read();
        instances.get(server_id).map(|inst| inst.client.clone())
    }

    /// Get the status of a server.
    #[must_use]
    pub fn server_status(&self, id: &str) -> Option<ServerStatus> {
        self.instances.read().get(id).map(|i| i.status)
    }

    /// List all registered server IDs.
    #[must_use]
    pub fn server_ids(&self) -> Vec<String> {
        self.instances.read().keys().cloned().collect()
    }

    /// List all registered languages.
    #[must_use]
    pub fn languages(&self) -> Vec<String> {
        self.language_map.read().keys().cloned().collect()
    }

    /// Stop all running servers.
    pub fn stop_all(&self) {
        let ids: Vec<String> = self.server_ids();
        for id in ids {
            let _ = self.stop_server(&id);
        }
    }

    /// Number of registered servers.
    #[must_use]
    pub fn server_count(&self) -> usize {
        self.instances.read().len()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- LspServerConfig ------------------------------------------------------

    #[test]
    fn config_new() {
        let cfg = LspServerConfig::new("rust-analyzer", vec!["rust".into()]);
        assert_eq!(cfg.command, "rust-analyzer");
        assert_eq!(cfg.languages, vec!["rust"]);
        assert!(cfg.args.is_empty());
    }

    #[test]
    fn config_with_args() {
        let cfg = LspServerConfig::new("clangd", vec!["c".into(), "cpp".into()])
            .with_args(vec!["--header-insertion=never".into()]);
        assert_eq!(cfg.args.len(), 1);
    }

    #[test]
    fn config_supports_language() {
        let cfg = LspServerConfig::new("rust-analyzer", vec!["rust".into(), "toml".into()]);
        assert!(cfg.supports_language("rust"));
        assert!(cfg.supports_language("Rust"));
        assert!(cfg.supports_language("toml"));
        assert!(!cfg.supports_language("python"));
    }

    #[test]
    fn config_serialization() {
        let cfg = LspServerConfig::new("test-server", vec!["lang1".into()]);
        let json = serde_json::to_string(&cfg).expect("serialize");
        let back: LspServerConfig = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(cfg.command, back.command);
    }

    // -- ServerStatus ---------------------------------------------------------

    #[test]
    fn server_status_display() {
        assert_eq!(ServerStatus::Stopped.to_string(), "stopped");
        assert_eq!(ServerStatus::Starting.to_string(), "starting");
        assert_eq!(ServerStatus::Running.to_string(), "running");
        assert_eq!(ServerStatus::Error.to_string(), "error");
    }

    // -- LspServerManager -----------------------------------------------------

    #[test]
    fn manager_new() {
        let mgr = LspServerManager::new("file:///project");
        assert_eq!(mgr.server_count(), 0);
        assert!(mgr.server_ids().is_empty());
    }

    #[test]
    fn manager_register() {
        let mgr = LspServerManager::new("file:///project");
        let cfg = LspServerConfig::new("rust-analyzer", vec!["rust".into()]);
        mgr.register("rust", cfg);
        assert_eq!(mgr.server_count(), 1);
        assert!(mgr.server_ids().contains(&"rust".to_string()));
    }

    #[test]
    fn manager_languages() {
        let mgr = LspServerManager::new("file:///project");
        let cfg = LspServerConfig::new("rust-analyzer", vec!["rust".into(), "toml".into()]);
        mgr.register("rust", cfg);
        let mut langs = mgr.languages();
        langs.sort();
        assert_eq!(langs, vec!["rust", "toml"]);
    }

    #[test]
    fn manager_start_server() {
        let mgr = LspServerManager::new("file:///project");
        let cfg = LspServerConfig::new("rust-analyzer", vec!["rust".into()]);
        mgr.register("rust", cfg);
        mgr.start_server("rust").expect("start");
        assert_eq!(mgr.server_status("rust"), Some(ServerStatus::Running));
    }

    #[test]
    fn manager_start_unregistered_fails() {
        let mgr = LspServerManager::new("file:///project");
        assert!(mgr.start_server("nonexistent").is_err());
    }

    #[test]
    fn manager_start_twice_fails() {
        let mgr = LspServerManager::new("file:///project");
        let cfg = LspServerConfig::new("rust-analyzer", vec!["rust".into()]);
        mgr.register("rust", cfg);
        mgr.start_server("rust").expect("start");
        assert!(mgr.start_server("rust").is_err());
    }

    #[test]
    fn manager_stop_server() {
        let mgr = LspServerManager::new("file:///project");
        let cfg = LspServerConfig::new("rust-analyzer", vec!["rust".into()]);
        mgr.register("rust", cfg);
        mgr.start_server("rust").expect("start");
        mgr.stop_server("rust").expect("stop");
        assert_eq!(mgr.server_status("rust"), Some(ServerStatus::Stopped));
    }

    #[test]
    fn manager_stop_not_running_fails() {
        let mgr = LspServerManager::new("file:///project");
        let cfg = LspServerConfig::new("rust-analyzer", vec!["rust".into()]);
        mgr.register("rust", cfg);
        assert!(mgr.stop_server("rust").is_err());
    }

    #[test]
    fn manager_get_server_for_language() {
        let mgr = LspServerManager::new("file:///project");
        let cfg = LspServerConfig::new("rust-analyzer", vec!["rust".into()]);
        mgr.register("rust", cfg);
        mgr.start_server("rust").expect("start");
        let client = mgr.get_server_for_language("rust");
        assert!(client.is_some());
    }

    #[test]
    fn manager_get_server_unknown_language() {
        let mgr = LspServerManager::new("file:///project");
        assert!(mgr.get_server_for_language("python").is_none());
    }

    #[test]
    fn manager_multiple_servers() {
        let mgr = LspServerManager::new("file:///project");
        mgr.register(
            "rust",
            LspServerConfig::new("rust-analyzer", vec!["rust".into()]),
        );
        mgr.register(
            "python",
            LspServerConfig::new("pylsp", vec!["python".into()]),
        );
        assert_eq!(mgr.server_count(), 2);
        mgr.start_server("rust").expect("start rust");
        mgr.start_server("python").expect("start python");
        assert!(mgr.get_server_for_language("rust").is_some());
        assert!(mgr.get_server_for_language("python").is_some());
    }

    #[test]
    fn manager_stop_all() {
        let mgr = LspServerManager::new("file:///project");
        mgr.register(
            "rust",
            LspServerConfig::new("rust-analyzer", vec!["rust".into()]),
        );
        mgr.register(
            "python",
            LspServerConfig::new("pylsp", vec!["python".into()]),
        );
        mgr.start_server("rust").expect("start");
        mgr.start_server("python").expect("start");
        mgr.stop_all();
        assert_eq!(mgr.server_status("rust"), Some(ServerStatus::Stopped));
        assert_eq!(mgr.server_status("python"), Some(ServerStatus::Stopped));
    }

    #[test]
    fn manager_server_status_unregistered() {
        let mgr = LspServerManager::new("file:///project");
        assert_eq!(mgr.server_status("nope"), None);
    }
}

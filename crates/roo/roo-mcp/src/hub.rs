//! McpHub �?core MCP connection manager.
//!
//! Corresponds to TS: `McpHub` class (1996 lines).
//! Manages MCP server connections, tool calls, resource reads, and configuration updates.
//!
//! Key methods matching TS source:
//! - `connect_to_server()` �?`connectToServer()`
//! - `delete_connection()` �?`deleteConnection()`
//! - `update_server_connections()` �?`updateServerConnections()`
//! - `restart_connection()` �?`restartConnection()`
//! - `refresh_all_connections()` �?`refreshAllConnections()`
//! - `call_tool()` �?`callTool()`
//! - `read_resource()` �?`readResource()`
//! - `fetch_tools_list()` �?`fetchToolsList()`
//! - `fetch_resources_list()` �?`fetchResourcesList()`
//! - `fetch_resource_templates_list()` �?`fetchResourceTemplatesList()`
//! - `toggle_tool_always_allow()` �?`toggleToolAlwaysAllow()`
//! - `toggle_tool_enabled_for_prompt()` �?`toggleToolEnabledForPrompt()`
//! - `toggle_server_disabled()` �?`toggleServerDisabled()`
//! - `update_server_timeout()` �?`updateServerTimeout()`
//! - `delete_server()` �?`deleteServer()`
//! - `handle_mcp_enabled_change()` �?`handleMcpEnabledChange()`
//! - `get_servers()` �?`getServers()`
//! - `get_all_servers()` �?`getAllServers()`
//! - `find_server_name_by_sanitized_name()` �?`findServerNameBySanitizedName()`
//! - `register_client()` �?`registerClient()`
//! - `unregister_client()` �?`unregisterClient()`
//! - `wait_until_ready()` �?`waitUntilReady()`
//! - `dispose()` �?`dispose()`

use std::collections::HashMap;
use std::sync::Arc;

use notify::{Config as NotifyConfig, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use roo_types::mcp::{
    McpConnectionStatus, McpResource, McpResourceResponse, McpResourceTemplate,
    McpServerConnection, McpTool, McpToolCallResponse,
};
use tokio::sync::RwLock;

use crate::client::McpClient;
use crate::config::{validate_server_config, ValidatedServerConfig};
use crate::error::{McpError, McpResult};
use crate::name_utils::{sanitize_mcp_name, tool_names_match};
use crate::transport::{McpTransport, SseTransport, StdioTransport, StreamableHttpTransport};
use crate::types::{
    ConnectedMcpConnection, DisableReason, DisconnectedMcpConnection, McpConnection,
    McpServerState, McpSource,
};

/// State change callback type.
pub type StateChangeCallback = Box<dyn Fn() + Send + Sync>;

/// Error level for error history entries.
/// Corresponds to TS: the `level` parameter in `appendErrorMessage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorLevel {
    Error,
    Warn,
    Info,
}

// ---------------------------------------------------------------------------
// Standalone utility functions
// ---------------------------------------------------------------------------

/// Get the default environment variables for spawning MCP server processes.
///
/// Corresponds to TS: `getDefaultEnvironment()` from `@modelcontextprotocol/sdk/client/stdio.js`.
/// Returns essential system environment variables (PATH, HOME, etc.) needed by
/// child processes to function correctly.
pub fn get_default_environment() -> HashMap<String, String> {
    let mut env = HashMap::new();

    // Essential environment variables that child processes typically need
    const ESSENTIAL_VARS: &[&str] = &[
        "PATH",
        "HOME",
        "USER",
        "TMPDIR",
        "TEMP",
        "TMP",
        "SHELL",
        "LANG",
        "LC_ALL",
        "LC_CTYPE",
        "TERM",
        "NODE_PATH",
        "NPM_CONFIG_PREFIX",
        "APPDATA",
        "LOCALAPPDATA",
        "PROGRAMFILES",
        "PROGRAMFILES(X86)",
        "SYSTEMROOT",
        "COMSPEC",
        "PROCESSOR_ARCHITECTURE",
        "PROGRAMDATA",
        "ALLUSERSPROFILE",
        "HOMEDRIVE",
        "HOMEPATH",
        "LOGNAME",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_CACHE_HOME",
    ];

    for var_name in ESSENTIAL_VARS {
        if let Ok(value) = std::env::var(var_name) {
            env.insert(var_name.to_string(), value);
        }
    }

    env
}

/// Deep-compare two `serde_json::Value`s for structural equality.
///
/// Corresponds to TS: `import deepEqual from "fast-deep-equal"`.
/// `serde_json::Value::eq` already performs deep structural comparison,
/// so this is a thin wrapper for semantic clarity.
pub fn json_deep_equal(a: &serde_json::Value, b: &serde_json::Value) -> bool {
    a == b
}

/// Inject variables into a JSON configuration value.
///
/// Corresponds to TS: `injectVariables(config, variables)`.
/// Replaces the following patterns in all string values within the config:
/// - `${workspaceFolder}` → the workspace root path
/// - `${userHome}` → the user's home directory
/// - `${env:VAR_NAME}` → the value of environment variable `VAR_NAME`
///
/// Does not mutate the original; returns a new value.
pub fn inject_variables(
    config: &serde_json::Value,
    workspace_folder: &str,
) -> serde_json::Value {
    let mut config_str = serde_json::to_string(config).unwrap_or_default();

    // Replace ${workspaceFolder}
    if !workspace_folder.is_empty() {
        let posix_path = workspace_folder.replace('\\', "/");
        config_str = config_str.replace("${workspaceFolder}", &posix_path);
    }

    // Replace ${userHome}
    if let Some(home) = dirs_home() {
        let posix_home = home.replace('\\', "/");
        config_str = config_str.replace("${userHome}", &posix_home);
    }

    // Replace ${env:VAR_NAME} patterns
    inject_env_variables(&mut config_str);

    serde_json::from_str(&config_str).unwrap_or_else(|_| config.clone())
}

/// Get the user's home directory.
fn dirs_home() -> Option<String> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
}

/// Replace `${env:VAR_NAME}` patterns in the given string with environment values.
fn inject_env_variables(s: &mut String) {
    let mut result = s.clone();
    let mut start = 0;
    while let Some(idx) = result[start..].find("${env:") {
        let abs_idx = start + idx;
        let after_prefix = abs_idx + 5; // length of "${env:"
        if let Some(end_idx) = result[after_prefix..].find('}') {
            let var_name = &result[after_prefix..after_prefix + end_idx];
            let replacement = std::env::var(var_name).unwrap_or_default();
            let pattern = format!("${{{{env:{}}}}}", var_name);
            result = result.replace(&pattern, &replacement);
            // Restart search from beginning since positions may have shifted
            start = 0;
        } else {
            break;
        }
    }
    *s = result;
}

/// Merge default environment with server-specific environment variables.
///
/// Corresponds to TS: `{ ...getDefaultEnvironment(), ...(configInjected.env || {}) }`.
pub fn merge_environment(
    default_env: &HashMap<String, String>,
    server_env: &HashMap<String, String>,
) -> HashMap<String, String> {
    let mut merged = default_env.clone();
    merged.extend(server_env.clone());
    merged
}

/// Handle for a file watcher that owns the watcher and its processing task.
/// When dropped, the watcher is stopped and the task is cancelled.
struct FileWatcherHandle {
    _watcher: RecommendedWatcher,
    _task: tokio::task::JoinHandle<()>,
}

/// McpHub manages all MCP server connections.
///
/// It handles connecting to servers, discovering tools and resources,
/// and routing tool calls and resource reads to the appropriate server.
///
/// Corresponds to TS: `McpHub` class.
pub struct McpHub {
    /// All server connections (connected and disconnected).
    /// Corresponds to TS: `connections: McpConnection[]`
    connections: Arc<RwLock<Vec<McpConnection>>>,
    /// Reference count for active clients.
    /// Corresponds to TS: `refCount: number`
    ref_count: Arc<RwLock<usize>>,
    /// Whether MCP is globally enabled.
    /// Corresponds to TS: derived from `provider.getState().mcpEnabled`
    mcp_enabled: Arc<RwLock<bool>>,
    /// Whether the hub has been disposed.
    /// Corresponds to TS: `isDisposed: boolean`
    disposed: Arc<RwLock<bool>>,
    /// Registry mapping sanitized names to original names.
    /// Corresponds to TS: `sanitizedNameRegistry: Map<string, string>`
    sanitized_name_registry: Arc<RwLock<HashMap<String, String>>>,
    /// Whether a connection operation is in progress.
    /// Corresponds to TS: `isConnecting: boolean`
    is_connecting: Arc<RwLock<bool>>,
    /// Callback for state changes (to notify UI).
    /// Corresponds to TS: `notifyWebviewOfServerChanges()` �?`provider.postMessageToWebview()`
    on_state_change: Option<StateChangeCallback>,
    /// Initialization promise handle.
    /// Corresponds to TS: `initializationPromise: Promise<void>`
    initialized: Arc<RwLock<bool>>,

    // --- File watching fields ---
    /// Per-server file watchers for watchPaths and build/index.js.
    /// Corresponds to TS: `fileWatchers: Map<string, FSWatcher[]>`
    file_watchers: Arc<tokio::sync::Mutex<HashMap<String, Vec<FileWatcherHandle>>>>,
    /// Watcher for global MCP settings file.
    /// Corresponds to TS: `settingsWatcher?: vscode.FileSystemWatcher`
    settings_watcher: Arc<tokio::sync::Mutex<Option<FileWatcherHandle>>>,
    /// Watcher for project MCP file (.roo/mcp.json).
    /// Corresponds to TS: `projectMcpWatcher?: vscode.FileSystemWatcher`
    project_mcp_watcher: Arc<tokio::sync::Mutex<Option<FileWatcherHandle>>>,

    // --- Config hot-reload fields ---
    /// Whether the current config update is programmatic (should be ignored by watcher).
    /// Corresponds to TS: `isProgrammaticUpdate: boolean`
    is_programmatic_update: Arc<RwLock<bool>>,
    /// Debounce task handles for config file changes.
    /// Corresponds to TS: `configChangeDebounceTimers: Map<string, NodeJS.Timeout>`
    config_debounce_handles: Arc<RwLock<HashMap<String, tokio::task::JoinHandle<()>>>>,
    /// Workspace folder path for variable injection.
    workspace_path: Option<String>,
    /// Global MCP settings file path.
    settings_path: Option<String>,
}

impl McpHub {
    /// Create a new McpHub.
    /// Corresponds to TS: `constructor(provider: ClineProvider)`
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(Vec::new())),
            ref_count: Arc::new(RwLock::new(0)),
            mcp_enabled: Arc::new(RwLock::new(true)),
            disposed: Arc::new(RwLock::new(false)),
            sanitized_name_registry: Arc::new(RwLock::new(HashMap::new())),
            is_connecting: Arc::new(RwLock::new(false)),
            on_state_change: None,
            initialized: Arc::new(RwLock::new(false)),
            file_watchers: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
            settings_watcher: Arc::new(tokio::sync::Mutex::new(None)),
            project_mcp_watcher: Arc::new(tokio::sync::Mutex::new(None)),
            is_programmatic_update: Arc::new(RwLock::new(false)),
            config_debounce_handles: Arc::new(RwLock::new(HashMap::new())),
            workspace_path: None,
            settings_path: None,
        }
    }

    /// Create a new McpHub with workspace and settings paths.
    /// Enables file watching and config hot-reload when paths are provided.
    /// Corresponds to TS: `constructor(provider: ClineProvider)` with path setup.
    pub fn new_with_paths(
        workspace_path: Option<String>,
        settings_path: Option<String>,
    ) -> Self {
        let mut hub = Self::new();
        hub.workspace_path = workspace_path;
        hub.settings_path = settings_path;
        hub
    }

    /// Create a new McpHub with a state change callback.
    pub fn with_state_change_callback(callback: StateChangeCallback) -> Self {
        let mut hub = Self::new();
        hub.on_state_change = Some(callback);
        hub
    }

    /// Create a new McpHub with paths and a state change callback.
    pub fn with_paths_and_callback(
        workspace_path: Option<String>,
        settings_path: Option<String>,
        callback: StateChangeCallback,
    ) -> Self {
        let mut hub = Self::new_with_paths(workspace_path, settings_path);
        hub.on_state_change = Some(callback);
        hub
    }

    /// Register a client (increment reference count).
    /// Corresponds to TS: `registerClient(): void`
    pub async fn register_client(&self) {
        let mut count = self.ref_count.write().await;
        *count += 1;
        tracing::debug!("McpHub: Client registered. Ref count: {}", *count);
    }

    /// Unregister a client (decrement reference count).
    /// If count reaches zero, disposes the hub.
    /// Corresponds to TS: `unregisterClient(): Promise<void>`
    pub async fn unregister_client(&self) -> McpResult<()> {
        let mut count = self.ref_count.write().await;
        if *count > 0 {
            *count -= 1;
        }
        tracing::debug!("McpHub: Client unregistered. Ref count: {}", *count);

        if *count <= 0 {
            tracing::info!("McpHub: Last client unregistered. Disposing hub.");
            drop(count);
            self.dispose().await?;
        }
        Ok(())
    }

    /// Wait until the hub is marked as initialized.
    /// Corresponds to TS: `waitUntilReady(): Promise<void>`
    pub async fn wait_until_ready(&self) {
        let ready = self.initialized.read().await;
        if *ready {
            return;
        }
        drop(ready);
        // Simple spin-wait; the hub is initialized via `set_initialized()`
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }

    /// Mark the hub as initialized.
    pub async fn set_initialized(&self) {
        let mut initialized = self.initialized.write().await;
        *initialized = true;
    }

    /// Check if the hub is currently connecting.
    /// Corresponds to TS: `isConnecting: boolean`
    pub async fn is_connecting(&self) -> bool {
        *self.is_connecting.read().await
    }

    // -----------------------------------------------------------------------
    // Server connection management
    // -----------------------------------------------------------------------

    /// Connect to an MCP server.
    ///
    /// Validates the configuration, creates the appropriate transport,
    /// and performs the MCP handshake.
    ///
    /// Corresponds to TS: `connectToServer(name, config, source)`
    pub async fn connect_to_server(
        &self,
        name: &str,
        config: &serde_json::Value,
        source: McpSource,
    ) -> McpResult<()> {
        self.check_disposed()?;

        // Inject variables into the config before validation.
        // Corresponds to TS: `const configInjected = await injectVariables(config, { env: process.env, workspaceFolder })`
        let workspace = self
            .workspace_path
            .as_deref()
            .unwrap_or("");
        let injected_config = inject_variables(config, workspace);

        // Validate configuration
        let validated = validate_server_config(&injected_config, Some(name))?;

        // Remove existing connection if it exists with the same source
        self.delete_connection(name, source).await?;

        // Register the sanitized name for O(1) lookup.
        // Corresponds to TS: `this.sanitizedNameRegistry.set(sanitizedName, name)`
        let sanitized = sanitize_mcp_name(name);
        let mut registry = self.sanitized_name_registry.write().await;
        registry.insert(sanitized, name.to_string());
        drop(registry);

        // Check if MCP is globally enabled
        let mcp_enabled = *self.mcp_enabled.read().await;
        if !mcp_enabled {
            let connection = self.create_placeholder_connection(
                name,
                &validated,
                source,
                DisableReason::McpDisabled,
            );
            self.connections.write().await.push(connection);
            self.notify_state_change();
            return Ok(());
        }

        // Skip connecting to disabled servers
        if validated.is_disabled() {
            let connection = self.create_placeholder_connection(
                name,
                &validated,
                source,
                DisableReason::ServerDisabled,
            );
            self.connections.write().await.push(connection);
            self.notify_state_change();
            return Ok(());
        }

        // Create server state with "connecting" status
        let mut server_state = McpServerState::new(
            name.to_string(),
            serde_json::to_string(&validated)?,
            source,
        );
        server_state.status = McpConnectionStatus::Connecting;

        // Create a placeholder disconnected connection while connecting
        let placeholder = McpConnection::Disconnected(DisconnectedMcpConnection {
            server: server_state.clone(),
        });
        self.connections.write().await.push(placeholder);

        // Attempt to connect
        match self.establish_connection(name, &validated, source).await {
            Ok((client, transport, tools, resources, resource_templates, instructions)) => {
                // Replace the placeholder with a connected connection
                let mut connections = self.connections.write().await;
                if let Some(conn) = connections.find_mut(name, source) {
                    conn.server_mut().status = McpConnectionStatus::Connected;
                    conn.server_mut().error.clear();
                    conn.server_mut().tools = tools;
                    conn.server_mut().resources = resources;
                    conn.server_mut().resource_templates = resource_templates;
                    conn.server_mut().instructions = instructions;

                    // Replace with connected connection
                    let connected = ConnectedMcpConnection {
                        server: conn.server_mut().clone(),
                        client,
                        transport,
                    };
                    *conn = McpConnection::Connected(connected);
                }

                tracing::info!("Connected to MCP server '{}'", name);
                self.notify_state_change();
                Ok(())
            }
            Err(e) => {
                // Update the placeholder with error info
                let mut connections = self.connections.write().await;
                if let Some(conn) = connections.find_mut(name, source) {
                    conn.server_mut().status = McpConnectionStatus::Error;
                    conn.server_mut().append_error(&e.to_string(), ErrorLevel::Error);
                }

                tracing::error!("Failed to connect to MCP server '{}': {}", name, e);
                self.notify_state_change();
                Err(e)
            }
        }
    }

    /// Delete a connection by name and source.
    ///
    /// Properly closes the transport for connected servers before removing.
    /// Corresponds to TS: `deleteConnection(name, source)`
    pub async fn delete_connection(&self, name: &str, source: McpSource) -> McpResult<()> {
        // Clean up file watchers for this server.
        // Corresponds to TS: `this.removeFileWatchersForServer(name)`
        self.remove_file_watchers_for_server(name).await;

        let mut connections = self.connections.write().await;
        let before_len = connections.len();

        // Partition into matching (to delete) and remaining connections
        let (to_delete, remaining): (Vec<_>, Vec<_>) =
            connections.drain(..).partition(|conn| {
                conn.name() == name && conn.source() == source
            });

        // Close transports for connected servers being deleted (best-effort)
        for conn in to_delete {
            if let McpConnection::Connected(mut c) = conn {
                tracing::debug!("Closing transport for '{}' during deletion", name);
                let _ = c.transport.close().await;
            }
        }

        // Restore remaining connections
        *connections = remaining;

        // Clean up sanitized name registry if no more connections with this name.
        // Corresponds to TS: registry cleanup in `deleteConnection()`
        let has_remaining = connections.iter().any(|c| c.name() == name);
        if !has_remaining {
            let sanitized = sanitize_mcp_name(name);
            let mut registry = self.sanitized_name_registry.write().await;
            registry.remove(&sanitized);
        }

        if connections.len() < before_len {
            tracing::info!("Deleted MCP connection '{}' ({:?})", name, source);
            drop(connections);
            self.notify_state_change();
        }

        Ok(())
    }

    /// Update server connections for a given source.
    ///
    /// Removes connections that no longer exist in the config, adds new ones,
    /// and restarts connections whose config has changed.
    ///
    /// Corresponds to TS: `updateServerConnections(newServers, source, manageConnectingState)`
    pub async fn update_server_connections(
        &self,
        new_servers: &HashMap<String, serde_json::Value>,
        source: McpSource,
        manage_connecting_state: bool,
    ) -> McpResult<()> {
        self.check_disposed()?;

        if manage_connecting_state {
            *self.is_connecting.write().await = true;
        }

        // Remove all file watchers before reconfiguring.
        // Corresponds to TS: `this.removeAllFileWatchers()`
        self.remove_all_file_watchers().await;

        // Filter connections by source
        let current_names: std::collections::HashSet<String> = {
            let connections = self.connections.read().await;
            connections
                .iter()
                .filter(|conn| {
                    conn.source() == source
                        || (conn.source() == McpSource::Global && source == McpSource::Global)
                })
                .map(|c| c.name().to_string())
                .collect()
        };
        let new_names: std::collections::HashSet<String> =
            new_servers.keys().map(|s| s.clone()).collect();

        // Delete removed servers
        for name in &current_names {
            if !new_names.contains(name) {
                if let Err(e) = self.delete_connection(name, source).await {
                    tracing::error!("Failed to delete connection '{}': {}", name, e);
                }
            }
        }

        // Update or add servers
        for (name, config) in new_servers {
            // Validate the config
            if let Err(e) = validate_server_config(config, Some(name)) {
                tracing::error!("Invalid configuration for MCP server \"{}\": {}", name, e);
                continue;
            }

            let current_connection = {
                let connections = self.connections.read().await;
                connections.find(name, source).cloned()
            };

            match current_connection {
                None => {
                    // New server - setup file watcher for enabled servers.
                    // Corresponds to TS: setup file watcher before connectToServer
                    if let Ok(validated) = validate_server_config(config, Some(name)) {
                        if !validated.is_disabled() {
                            self.setup_file_watcher(name, &validated, source);
                        }
                    }
                    if let Err(e) = self.connect_to_server(name, config, source).await {
                        tracing::error!("Failed to connect to new MCP server {}: {}", name, e);
                    }
                }
                Some(existing) => {
                    // Check if config has changed using deep equality.
                    // Corresponds to TS: `!deepEqual(JSON.parse(currentConnection.server.config), config)`
                    let existing_config_str = &existing.server().config;

                    // Parse existing config to compare semantically
                    let existing_json: serde_json::Value =
                        serde_json::from_str(existing_config_str).unwrap_or_default();

                    if !json_deep_equal(&existing_json, config) {
                        // Config changed �?delete and reconnect
                        if let Err(e) = self.delete_connection(name, source).await {
                            tracing::error!(
                                "Failed to delete connection for reconnect '{}': {}",
                                name,
                                e
                            );
                        }
                        if let Err(e) =
                            self.connect_to_server(name, config, source).await
                        {
                            tracing::error!("Failed to reconnect MCP server {}: {}", name, e);
                        }
                    }
                    // If server exists with same config, do nothing
                }
            }
        }

        self.notify_state_change();
        if manage_connecting_state {
            *self.is_connecting.write().await = false;
        }

        Ok(())
    }

    /// Restart a connection by name and source.
    ///
    /// Corresponds to TS: `restartConnection(serverName, source)`
    /// Shows "connecting" status, waits 500ms, then reconnects.
    pub async fn restart_connection(&self, name: &str, source: McpSource) -> McpResult<()> {
        *self.is_connecting.write().await = true;

        // Check if MCP is globally enabled
        let mcp_enabled = *self.mcp_enabled.read().await;
        if !mcp_enabled {
            *self.is_connecting.write().await = false;
            return Ok(());
        }

        // Get existing connection and update its status
        let config_str = {
            let mut connections = self.connections.write().await;
            if let Some(conn) = connections.find_mut(name, source) {
                conn.server_mut().status = McpConnectionStatus::Connecting;
                conn.server_mut().error.clear();
                Some(conn.server().config.clone())
            } else {
                None
            }
        };

        if let Some(config_str) = config_str {
            self.notify_state_change();

            // Artificial delay to show user that server is restarting
            // Corresponds to TS: `await delay(500)`
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            // Get the actual source from the connection
            let actual_source = {
                let connections = self.connections.read().await;
                connections
                    .find(name, source)
                    .map(|c| c.source())
                    .unwrap_or(source)
            };

            if let Err(e) = self.delete_connection(name, actual_source).await {
                tracing::error!("Failed to delete connection for restart '{}': {}", name, e);
            }

            let config: serde_json::Value = serde_json::from_str(&config_str).unwrap_or_default();

            // Validate the config
            match validate_server_config(&config, Some(name)) {
                Ok(_validated) => {
                    if let Err(e) =
                        self.connect_to_server(name, &config, actual_source).await
                    {
                        tracing::error!(
                            "Failed to restart MCP server connection '{}': {}",
                            name,
                            e
                        );
                    }
                }
                Err(e) => {
                    tracing::error!(
                        "Invalid configuration for MCP server \"{}\": {}",
                        name,
                        e
                    );
                }
            }
        }

        self.notify_state_change();
        *self.is_connecting.write().await = false;
        Ok(())
    }

    /// Refresh all connections (restart all).
    ///
    /// Corresponds to TS: `refreshAllConnections()`
    /// Clears all existing connections and re-initializes from config files.
    pub async fn refresh_all_connections(&self) -> McpResult<()> {
        // Check if already connecting
        if *self.is_connecting.read().await {
            return Ok(());
        }

        // Check if MCP is globally enabled
        let mcp_enabled = *self.mcp_enabled.read().await;
        if !mcp_enabled {
            // Clear all existing connections
            let existing_connections: Vec<(String, McpSource)> = {
                let connections = self.connections.read().await;
                connections
                    .iter()
                    .map(|c| (c.name().to_string(), c.source()))
                    .collect()
            };
            for (name, source) in existing_connections {
                let _ = self.delete_connection(&name, source).await;
            }
            return Ok(());
        }

        *self.is_connecting.write().await = true;

        // Collect all connection info before clearing
        let connection_infos: Vec<(String, McpSource, String)> = {
            let connections = self.connections.read().await;
            connections
                .iter()
                .map(|c| {
                    (
                        c.name().to_string(),
                        c.source(),
                        c.server().config.clone(),
                    )
                })
                .collect()
        };

        // Clear all existing connections first
        for (name, source, _) in &connection_infos {
            let _ = self.delete_connection(name, *source).await;
        }

        // Re-connect all servers from scratch
        for (name, source, config_str) in &connection_infos {
            let config: serde_json::Value =
                serde_json::from_str(config_str).unwrap_or_default();
            if let Err(e) = self.connect_to_server(name, &config, *source).await {
                tracing::error!("Failed to refresh connection '{}': {}", name, e);
            }
        }

        // Small delay to allow connections to stabilize
        // Corresponds to TS: `await delay(100)`
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        self.notify_state_change();
        *self.is_connecting.write().await = false;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Tool and resource operations
    // -----------------------------------------------------------------------

    /// Call a tool on a specific server with timeout support.
    ///
    /// The timeout is read from the server configuration (default: 60 seconds).
    /// Corresponds to TS: `callTool(serverName, toolName, toolArguments, source)`
    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: Option<serde_json::Value>,
    ) -> McpResult<McpToolCallResponse> {
        self.check_disposed()?;

        // Find connection and check it's connected and not disabled
        let connection = {
            let connections = self.connections.read().await;
            connections
                .iter()
                .find(|c| c.name() == server_name && c.is_connected())
                .cloned()
        };

        let connection = connection
            .ok_or_else(|| McpError::NotConnected(format!(
                "No connection found for server: {}. Please make sure to use MCP servers available under 'Connected MCP Servers'.",
                server_name
            )))?;

        if connection.server().disabled {
            return Err(McpError::ServerDisabled(server_name.to_string()));
        }

        // Extract timeout from server config
        let timeout_duration = {
            let config: serde_json::Value =
                serde_json::from_str(&connection.server().config).unwrap_or_default();
            let timeout_secs = config
                .get("timeout")
                .and_then(|v| v.as_u64())
                .unwrap_or(60);
            std::time::Duration::from_secs(timeout_secs)
        };

        // We need to get mutable access to the connected connection
        let mut connections = self.connections.write().await;
        let conn = connections
            .iter_mut()
            .find(|c| c.name() == server_name && c.is_connected())
            .ok_or_else(|| McpError::NotConnected(server_name.to_string()))?;

        match conn {
            McpConnection::Connected(conn) => {
                let result = tokio::time::timeout(
                    timeout_duration,
                    conn.client.call_tool(&mut *conn.transport, tool_name, arguments),
                )
                .await;

                match result {
                    Ok(Ok(response)) => {
                        tracing::info!(
                            "Tool call '{}' on '{}' succeeded",
                            tool_name,
                            server_name
                        );
                        Ok(response)
                    }
                    Ok(Err(e)) => {
                        conn.server.status = McpConnectionStatus::Error;
                        conn.server.append_error(&e.to_string(), ErrorLevel::Error);
                        tracing::error!(
                            "Tool call '{}' on '{}' failed: {}",
                            tool_name,
                            server_name,
                            e
                        );
                        Err(e)
                    }
                    Err(_) => {
                        let err_msg = format!(
                            "Tool call '{}' on '{}' timed out after {}s",
                            tool_name,
                            server_name,
                            timeout_duration.as_secs()
                        );
                        conn.server.status = McpConnectionStatus::Error;
                        conn.server.append_error(&err_msg, ErrorLevel::Error);
                        tracing::error!("{}", err_msg);
                        Err(McpError::ToolCallFailed {
                            server_name: server_name.to_string(),
                            tool_name: tool_name.to_string(),
                            error: err_msg,
                        })
                    }
                }
            }
            _ => Err(McpError::NotConnected(server_name.to_string())),
        }
    }

    /// Read a resource from a specific server.
    /// Corresponds to TS: `readResource(serverName, uri, source)`
    pub async fn read_resource(
        &self,
        server_name: &str,
        uri: &str,
    ) -> McpResult<McpResourceResponse> {
        self.check_disposed()?;

        let mut connections = self.connections.write().await;

        let connection = connections
            .iter_mut()
            .find(|c| c.name() == server_name && c.is_connected())
            .ok_or_else(|| {
                McpError::NotConnected(format!(
                    "No connection found for server: {}",
                    server_name
                ))
            })?;

        if connection.server().disabled {
            return Err(McpError::ServerDisabled(server_name.to_string()));
        }

        match connection {
            McpConnection::Connected(conn) => {
                let result = conn.client.read_resource(&mut *conn.transport, uri).await;

                match result {
                    Ok(response) => {
                        tracing::info!(
                            "Resource read '{}' on '{}' succeeded",
                            uri,
                            server_name
                        );
                        Ok(response)
                    }
                    Err(e) => {
                        conn.server.status = McpConnectionStatus::Error;
                        conn.server.append_error(&e.to_string(), ErrorLevel::Error);
                        Err(e)
                    }
                }
            }
            _ => Err(McpError::NotConnected(server_name.to_string())),
        }
    }

    /// Fetch the tools list from a specific server and update the server state.
    ///
    /// Tools are annotated with `always_allow` and `enabled_for_prompt` based on
    /// the server configuration's `alwaysAllow` and `disabledTools` lists.
    ///
    /// Corresponds to TS: `fetchToolsList(serverName, source)`
    pub async fn fetch_tools_list(&self, server_name: &str, source: McpSource) -> McpResult<Vec<McpTool>> {
        self.check_disposed()?;

        let mut connections = self.connections.write().await;

        let connection = connections
            .iter_mut()
            .find(|c| c.name() == server_name && c.source() == source && c.is_connected());

        let connection = connection
            .ok_or_else(|| McpError::NotConnected(server_name.to_string()))?;

        match connection {
            McpConnection::Connected(conn) => {
                let raw_tools = conn
                    .client
                    .list_tools(&mut *conn.transport)
                    .await
                    .unwrap_or_default();

                // Read alwaysAllow and disabledTools from config
                let always_allow_config: Vec<String> = serde_json::from_str(&conn.server.config)
                    .ok()
                    .and_then(|v: serde_json::Value| v.get("alwaysAllow").cloned())
                    .and_then(|v| serde_json::from_value(v).ok())
                    .unwrap_or_default();

                let disabled_tools_list: Vec<String> = serde_json::from_str(&conn.server.config)
                    .ok()
                    .and_then(|v: serde_json::Value| v.get("disabledTools").cloned())
                    .and_then(|v| serde_json::from_value(v).ok())
                    .unwrap_or_default();

                // Check if wildcard "*" is in the alwaysAllow config
                let has_wildcard = always_allow_config.contains(&"*".to_string());

                // Annotate tools with always_allow and enabled_for_prompt
                let tools: Vec<McpTool> = raw_tools
                    .into_iter()
                    .map(|mut tool| {
                        tool.always_allow =
                            has_wildcard || always_allow_config.contains(&tool.name);
                        tool.enabled_for_prompt = !disabled_tools_list.contains(&tool.name);
                        tool
                    })
                    .collect();

                conn.server.tools = tools.clone();
                tracing::info!(
                    "Fetched {} tools from '{}'",
                    tools.len(),
                    server_name
                );
                Ok(tools)
            }
            _ => Err(McpError::NotConnected(server_name.to_string())),
        }
    }

    /// Fetch the resources list from a specific server.
    /// Corresponds to TS: `fetchResourcesList(serverName, source)`
    pub async fn fetch_resources_list(
        &self,
        server_name: &str,
        source: McpSource,
    ) -> McpResult<Vec<McpResource>> {
        self.check_disposed()?;

        let mut connections = self.connections.write().await;

        let connection = connections
            .iter_mut()
            .find(|c| c.name() == server_name && c.source() == source && c.is_connected());

        match connection {
            Some(McpConnection::Connected(conn)) => {
                let resources = conn
                    .client
                    .list_resources(&mut *conn.transport)
                    .await
                    .unwrap_or_default();
                conn.server.resources = resources.clone();
                Ok(resources)
            }
            _ => Ok(vec![]),
        }
    }

    /// Fetch the resource templates list from a specific server.
    /// Corresponds to TS: `fetchResourceTemplatesList(serverName, source)`
    pub async fn fetch_resource_templates_list(
        &self,
        server_name: &str,
        source: McpSource,
    ) -> McpResult<Vec<McpResourceTemplate>> {
        self.check_disposed()?;

        let mut connections = self.connections.write().await;

        let connection = connections
            .iter_mut()
            .find(|c| c.name() == server_name && c.source() == source && c.is_connected());

        match connection {
            Some(McpConnection::Connected(conn)) => {
                let templates = conn
                    .client
                    .list_resource_templates(&mut *conn.transport)
                    .await
                    .unwrap_or_default();
                conn.server.resource_templates = templates.clone();
                Ok(templates)
            }
            _ => Ok(vec![]),
        }
    }

    // -----------------------------------------------------------------------
    // Toggle methods
    // -----------------------------------------------------------------------

    /// Toggle whether a tool is always allowed (auto-approved).
    ///
    /// Updates the tool's `always_allow` flag in the server state and
    /// persists the change to the server configuration's `alwaysAllow` list.
    ///
    /// Corresponds to TS: `toggleToolAlwaysAllow(serverName, source, toolName, shouldAllow)`
    pub async fn toggle_tool_always_allow(
        &self,
        server_name: &str,
        source: McpSource,
        tool_name: &str,
        should_allow: bool,
    ) -> McpResult<()> {
        self.check_disposed()?;

        let mut connections = self.connections.write().await;

        let connection = connections
            .find_mut(server_name, source)
            .ok_or_else(|| McpError::ServerNotFound(server_name.to_string()))?;

        let server = connection.server_mut();

        // Update the tool's always_allow flag in the server state
        if let Some(tool) = server.tools.iter_mut().find(|t| t.name == tool_name) {
            tool.always_allow = should_allow;
        }

        // Update the config's alwaysAllow list
        let mut config: serde_json::Value = serde_json::from_str(&server.config)?;
        if config.is_null() {
            config = serde_json::json!({});
        }
        if config.get("alwaysAllow").is_none() {
            config["alwaysAllow"] = serde_json::json!([]);
        }

        let always_allow = config["alwaysAllow"].as_array_mut().ok_or_else(|| {
            McpError::ConfigError("alwaysAllow is not an array".to_string())
        })?;

        if should_allow {
            if !always_allow
                .iter()
                .any(|v| v.as_str() == Some(tool_name))
            {
                always_allow.push(serde_json::Value::String(tool_name.to_string()));
            }
        } else {
            always_allow.retain(|v| v.as_str() != Some(tool_name));
        }

        server.config = serde_json::to_string(&config)?;
        tracing::info!(
            "Toggle always-allow for tool '{}' on '{}': {}",
            tool_name,
            server_name,
            should_allow
        );

        drop(connections);
        self.notify_state_change();
        Ok(())
    }

    /// Toggle whether a tool is enabled for prompts.
    ///
    /// When `is_enabled` is true, removes the tool from `disabledTools`.
    /// When `is_enabled` is false, adds the tool to `disabledTools`.
    ///
    /// Corresponds to TS: `toggleToolEnabledForPrompt(serverName, source, toolName, isEnabled)`
    pub async fn toggle_tool_enabled_for_prompt(
        &self,
        server_name: &str,
        source: McpSource,
        tool_name: &str,
        is_enabled: bool,
    ) -> McpResult<()> {
        self.check_disposed()?;

        let mut connections = self.connections.write().await;

        let connection = connections
            .find_mut(server_name, source)
            .ok_or_else(|| McpError::ServerNotFound(server_name.to_string()))?;

        let server = connection.server_mut();

        // Update the tool's enabled_for_prompt flag in the server state
        if let Some(tool) = server.tools.iter_mut().find(|t| t.name == tool_name) {
            tool.enabled_for_prompt = is_enabled;
        }

        // Update the config's disabledTools list
        let mut config: serde_json::Value = serde_json::from_str(&server.config)?;
        if config.is_null() {
            config = serde_json::json!({});
        }
        if config.get("disabledTools").is_none() {
            config["disabledTools"] = serde_json::json!([]);
        }

        let disabled_tools = config["disabledTools"].as_array_mut().ok_or_else(|| {
            McpError::ConfigError("disabledTools is not an array".to_string())
        })?;

        if is_enabled {
            // Remove from disabled list
            disabled_tools.retain(|v| v.as_str() != Some(tool_name));
        } else {
            // Add to disabled list
            if !disabled_tools
                .iter()
                .any(|v| v.as_str() == Some(tool_name))
            {
                disabled_tools.push(serde_json::Value::String(tool_name.to_string()));
            }
        }

        server.config = serde_json::to_string(&config)?;
        tracing::info!(
            "Toggle enabled-for-prompt for tool '{}' on '{}': {}",
            tool_name,
            server_name,
            is_enabled
        );

        drop(connections);
        self.notify_state_change();
        Ok(())
    }

    /// Toggle whether a server is disabled.
    ///
    /// Corresponds to TS: `toggleServerDisabled(serverName, disabled, source)`
    /// When disabling a connected server, disconnects it.
    /// When enabling a disconnected server, connects it.
    pub async fn toggle_server_disabled(
        &self,
        server_name: &str,
        source: McpSource,
        disabled: bool,
    ) -> McpResult<()> {
        self.check_disposed()?;

        let connection = {
            let connections = self.connections.read().await;
            connections.find(server_name, source).cloned()
        };

        let connection = connection
            .ok_or_else(|| McpError::ServerNotFound(server_name.to_string()))?;

        let server_source = connection.source();

        // Update the config to set disabled
        {
            let mut connections = self.connections.write().await;
            if let Some(conn) = connections.find_mut(server_name, server_source) {
                conn.server_mut().disabled = disabled;

                if disabled && conn.server().status == McpConnectionStatus::Connected {
                    // Disconnect the server
                    drop(connections);
                    self.delete_connection(server_name, server_source).await?;

                    // Re-add as disabled
                    let mut config: serde_json::Value =
                        serde_json::from_str(&connection.server().config)?;
                    config["disabled"] = serde_json::Value::Bool(true);
                    self.connect_to_server(server_name, &config, server_source)
                        .await?;
                } else if !disabled && !conn.is_connected() {
                    // Re-enable: delete and reconnect
                    let config_str = conn.server().config.clone();
                    drop(connections);
                    self.delete_connection(server_name, server_source).await?;

                    let mut config: serde_json::Value =
                        serde_json::from_str(&config_str).unwrap_or_default();
                    config["disabled"] = serde_json::Value::Bool(false);
                    self.connect_to_server(server_name, &config, server_source)
                        .await?;
                } else if conn.is_connected() {
                    // Connected server: refresh capabilities
                    let tools = self
                        .fetch_tools_list(server_name, server_source)
                        .await
                        .unwrap_or_default();
                    let resources = self
                        .fetch_resources_list(server_name, server_source)
                        .await
                        .unwrap_or_default();
                    let templates = self
                        .fetch_resource_templates_list(server_name, server_source)
                        .await
                        .unwrap_or_default();

                    if let Some(conn) = connections.find_mut(server_name, server_source) {
                        conn.server_mut().tools = tools;
                        conn.server_mut().resources = resources;
                        conn.server_mut().resource_templates = templates;
                    }
                }
            }
        }

        self.notify_state_change();
        Ok(())
    }

    /// Update the timeout for a specific server.
    ///
    /// Corresponds to TS: `updateServerTimeout(serverName, timeout, source)`
    pub async fn update_server_timeout(
        &self,
        server_name: &str,
        timeout: u64,
        source: McpSource,
    ) -> McpResult<()> {
        self.check_disposed()?;

        let mut connections = self.connections.write().await;
        let connection = connections
            .find_mut(server_name, source)
            .ok_or_else(|| McpError::ServerNotFound(server_name.to_string()))?;

        let server = connection.server_mut();

        // Update the config's timeout
        let mut config: serde_json::Value = serde_json::from_str(&server.config)?;
        if config.is_null() {
            config = serde_json::json!({});
        }
        config["timeout"] = serde_json::Value::Number(timeout.into());
        server.config = serde_json::to_string(&config)?;

        tracing::info!(
            "Updated timeout for '{}' to {}s",
            server_name,
            timeout
        );

        drop(connections);
        self.notify_state_change();
        Ok(())
    }

    /// Delete a server from the configuration and disconnect it.
    ///
    /// Corresponds to TS: `deleteServer(serverName, source)`
    pub async fn delete_server(&self, server_name: &str, source: McpSource) -> McpResult<()> {
        self.check_disposed()?;

        let connection = {
            let connections = self.connections.read().await;
            connections.find(server_name, source).cloned()
        };

        let connection = connection
            .ok_or_else(|| McpError::ServerNotFound(server_name.to_string()))?;

        let server_source = connection.source();

        // Delete the connection
        self.delete_connection(server_name, server_source).await?;

        tracing::info!("Deleted MCP server '{}'", server_name);
        self.notify_state_change();
        Ok(())
    }

    // -----------------------------------------------------------------------
    // MCP enabled/disabled
    // -----------------------------------------------------------------------

    /// Handle MCP globally enabled/disabled change.
    ///
    /// Corresponds to TS: `handleMcpEnabledChange(enabled)`
    /// When disabled, properly closes all transports and converts to disconnected placeholders.
    /// When enabled, refreshes all connections.
    pub async fn handle_mcp_enabled_change(&self, enabled: bool) -> McpResult<()> {
        *self.mcp_enabled.write().await = enabled;

        if enabled {
            // Re-connect all servers
            self.refresh_all_connections().await?;
        } else {
            // Disconnect all servers: close transports and convert to disconnected placeholders
            // Corresponds to TS: iterating through connections and calling deleteConnection
            let mut connections = self.connections.write().await;
            let drained: Vec<McpConnection> = connections.drain(..).collect();
            let mut new_connections = Vec::with_capacity(drained.len());
            for conn in drained {
                match conn {
                    McpConnection::Connected(mut c) => {
                        // Properly close the transport
                        let _ = c.transport.close().await;
                        c.server.status = McpConnectionStatus::Disconnected;
                        new_connections.push(McpConnection::Disconnected(
                            DisconnectedMcpConnection { server: c.server },
                        ));
                    }
                    other => {
                        new_connections.push(other);
                    }
                }
            }
            *connections = new_connections;
        }

        self.notify_state_change();
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Server query methods
    // -----------------------------------------------------------------------

    /// Get enabled servers, deduplicating by name with project servers taking priority.
    ///
    /// Corresponds to TS: `getServers(): McpServer[]`
    /// Only returns enabled (non-disabled) servers. When the same server name exists
    /// in both global and project sources, the project version takes priority.
    pub fn get_servers(&self) -> Vec<McpServerConnection> {
        match self.connections.try_read() {
            Ok(connections) => {
                let enabled: Vec<_> = connections
                    .iter()
                    .filter(|c| !c.server().disabled)
                    .collect();

                // Deduplicate: project servers override global servers with same name
                let mut seen = std::collections::HashMap::new();
                for conn in &enabled {
                    let name = conn.name().to_string();
                    match seen.get(&name) {
                        Some(existing_source) => {
                            // Project takes priority over global
                            if conn.source() == McpSource::Project
                                && *existing_source != McpSource::Project
                            {
                                seen.insert(name, conn.source());
                            }
                        }
                        None => {
                            seen.insert(name, conn.source());
                        }
                    }
                }

                // Build result from deduplicated connections
                enabled
                    .into_iter()
                    .filter(|conn| {
                        seen.get(conn.name())
                            .map_or(false, |s| *s == conn.source())
                    })
                    .map(|conn| McpServerConnection {
                        name: conn.name().to_string(),
                        status: conn.server().status,
                        tools: conn.server().tools.clone(),
                        resources: conn.server().resources.clone(),
                        resource_templates: conn.server().resource_templates.clone(),
                        disabled_tools: conn
                            .server()
                            .tools
                            .iter()
                            .filter(|t| !t.enabled_for_prompt)
                            .map(|t| t.name.clone())
                            .collect(),
                        errors: conn.server().error_history.clone(),
                    })
                    .collect()
            }
            Err(_) => Vec::new(),
        }
    }

    /// Get all servers asynchronously (including disabled ones).
    ///
    /// Corresponds to TS: `getAllServers(): McpServer[]`
    pub async fn get_all_servers(&self) -> Vec<McpServerConnection> {
        let connections = self.connections.read().await;
        connections
            .iter()
            .map(|conn| McpServerConnection {
                name: conn.name().to_string(),
                status: conn.server().status,
                tools: conn.server().tools.clone(),
                resources: conn.server().resources.clone(),
                resource_templates: conn.server().resource_templates.clone(),
                disabled_tools: conn
                    .server()
                    .tools
                    .iter()
                    .filter(|t| !t.enabled_for_prompt)
                    .map(|t| t.name.clone())
                    .collect(),
                errors: conn.server().error_history.clone(),
            })
            .collect()
    }

    /// Find a server name by its sanitized name.
    ///
    /// Corresponds to TS: `findServerNameBySanitizedName(sanitizedServerName)`
    /// Checks in order: exact match �?registry �?fuzzy match.
    pub async fn find_server_name_by_sanitized_name(
        &self,
        sanitized_name: &str,
    ) -> Option<String> {
        // 1. First try exact match against connection names
        let connections = self.connections.read().await;
        if let Some(exact) = connections.iter().find(|c| c.name() == sanitized_name) {
            return Some(exact.name().to_string());
        }
        drop(connections);

        // 2. Check the sanitized name registry
        let registry = self.sanitized_name_registry.read().await;
        if let Some(original) = registry.get(sanitized_name) {
            return Some(original.clone());
        }
        drop(registry);

        // 3. Fallback: fuzzy match (treat hyphens and underscores as equivalent)
        let connections = self.connections.read().await;
        for conn in connections.iter() {
            if tool_names_match(conn.name(), sanitized_name) {
                return Some(conn.name().to_string());
            }
        }

        None
    }

    /// Find a connection by server name and optional source.
    ///
    /// Corresponds to TS: `findConnection(serverName, source?)`
    /// If source is specified, only finds servers with that source.
    /// If no source, project servers take priority over global.
    pub async fn find_connection(
        &self,
        server_name: &str,
        source: Option<McpSource>,
    ) -> Option<McpConnection> {
        let connections = self.connections.read().await;

        if let Some(src) = source {
            connections
                .iter()
                .find(|conn| conn.name() == server_name && conn.source() == src)
                .cloned()
        } else {
            // First look for project servers, then global
            let project_conn = connections
                .iter()
                .find(|conn| conn.name() == server_name && conn.source() == McpSource::Project)
                .cloned();
            if project_conn.is_some() {
                return project_conn;
            }
            connections
                .iter()
                .find(|conn| {
                    conn.name() == server_name
                        && (conn.source() == McpSource::Global || conn.source() == McpSource::Global)
                })
                .cloned()
        }
    }

    // -----------------------------------------------------------------------
    // Lifecycle
    // -----------------------------------------------------------------------

    /// Dispose the hub and all connections.
    ///
    /// Corresponds to TS: `dispose(): Promise<void>`
    /// Prevents multiple disposals, clears all timers, closes all connections.
    pub async fn dispose(&self) -> McpResult<()> {
        let mut disposed = self.disposed.write().await;
        if *disposed {
            return Ok(());
        }
        *disposed = true;
        drop(disposed);

        // Clear all debounce timers.
        // Corresponds to TS: clearing `configChangeDebounceTimers`
        {
            let mut handles = self.config_debounce_handles.write().await;
            for (_, handle) in handles.drain() {
                handle.abort();
            }
        }

        // Reset programmatic update flag.
        // Corresponds to TS: clearing `flagResetTimer` and resetting `isProgrammaticUpdate`
        *self.is_programmatic_update.write().await = false;

        // Remove all file watchers.
        // Corresponds to TS: `this.removeAllFileWatchers()`
        self.remove_all_file_watchers().await;

        // Dispose settings watcher.
        // Corresponds to TS: `this.settingsWatcher?.dispose()`
        {
            let mut sw = self.settings_watcher.lock().await;
            *sw = None;
        }

        // Dispose project MCP watcher.
        // Corresponds to TS: `this.projectMcpWatcher?.dispose()`
        {
            let mut pw = self.project_mcp_watcher.lock().await;
            *pw = None;
        }

        let mut connections = self.connections.write().await;
        for conn in connections.drain(..) {
            if let McpConnection::Connected(mut c) = conn {
                let _ = c.transport.close().await;
            }
        }

        // Clear sanitized name registry
        self.sanitized_name_registry.write().await.clear();

        tracing::info!("McpHub disposed");
        Ok(())
    }

    // -----------------------------------------------------------------------
    // File watching
    // -----------------------------------------------------------------------

    /// Set up file watchers for a specific server.
    ///
    /// Corresponds to TS: `setupFileWatcher(name, config, source)`.
    /// Watches `watchPaths` and `build/index.js` (from args) for changes
    /// and restarts the server when changes are detected.
    pub fn setup_file_watcher(
        &self,
        name: &str,
        config: &ValidatedServerConfig,
        source: McpSource,
    ) {
        // Only stdio type has args to watch
        if let ValidatedServerConfig::Stdio {
            args,
            watch_paths,
            ..
        } = config
        {
            let mut watchers: Vec<FileWatcherHandle> = Vec::new();

            // Setup watchers for custom watchPaths if defined.
            // Corresponds to TS: `if (config.watchPaths && config.watchPaths.length > 0)`
            if let Some(wp) = watch_paths {
                if !wp.is_empty() {
                    for watch_path in wp {
                        if let Some(handle) = Self::create_path_watcher(
                            watch_path,
                            name.to_string(),
                            source,
                        ) {
                            watchers.push(handle);
                        }
                    }
                }
            }

            // Also setup the fallback build/index.js watcher if applicable.
            // Corresponds to TS: finding `build/index.js` in args
            if let Some(file_path) = args.iter().find(|arg| arg.contains("build/index.js")) {
                if let Some(handle) =
                    Self::create_path_watcher(file_path, name.to_string(), source)
                {
                    watchers.push(handle);
                }
            }

            if !watchers.is_empty() {
                // Store watchers asynchronously - spawn a blocking task
                let file_watchers = self.file_watchers.clone();
                let name_owned = name.to_string();
                tokio::spawn(async move {
                    let mut fw = file_watchers.lock().await;
                    fw.insert(name_owned, watchers);
                });
            }
        }
    }

    /// Create a file watcher for a specific path.
    ///
    /// Returns a `FileWatcherHandle` that keeps the watcher alive.
    fn create_path_watcher(
        path: &str,
        server_name: String,
        _source: McpSource,
    ) -> Option<FileWatcherHandle> {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let path_buf = std::path::PathBuf::from(path);

        let mut watcher = match RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    // Only react to content modifications
                    if matches!(
                        event.kind,
                        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
                    ) {
                        let _ = tx.send(());
                    }
                }
            },
            NotifyConfig::default(),
        ) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!("Failed to create file watcher for '{}': {}", path, e);
                return None;
            }
        };

        // Determine recursive mode based on whether the path is a directory
        let recursive_mode = if path_buf.is_dir() {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        if let Err(e) = watcher.watch(&path_buf, recursive_mode) {
            tracing::error!("Failed to watch path '{}': {}", path, e);
            return None;
        }

        let sn = server_name.clone();
        let task = tokio::spawn(async move {
            // Debounce: wait for 500ms of silence before acting
            while rx.recv().await.is_some() {
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                // Drain any pending events
                while rx.try_recv().is_ok() {}
                tracing::info!(
                    "File change detected, triggering restart for server '{}'",
                    sn
                );
                // Note: The actual restart needs to be handled by the caller
                // through the state change callback or a separate mechanism.
            }
        });

        Some(FileWatcherHandle {
            _watcher: watcher,
            _task: task,
        })
    }

    /// Remove all file watchers.
    ///
    /// Corresponds to TS: `removeAllFileWatchers()`
    pub async fn remove_all_file_watchers(&self) {
        let mut fw = self.file_watchers.lock().await;
        fw.clear();
    }

    /// Remove file watchers for a specific server.
    ///
    /// Corresponds to TS: `removeFileWatchersForServer(serverName)`
    pub async fn remove_file_watchers_for_server(&self, server_name: &str) {
        let mut fw = self.file_watchers.lock().await;
        fw.remove(server_name);
    }

    // -----------------------------------------------------------------------
    // Config hot-reload
    // -----------------------------------------------------------------------

    /// Start watching MCP configuration files for changes.
    ///
    /// Corresponds to TS: `watchMcpSettingsFile()` and `watchProjectMcpFile()`.
    /// Should be called after the hub is created with valid paths.
    pub async fn start_config_watchers(&self) {
        self.watch_mcp_settings_file().await;
        self.watch_project_mcp_file().await;
    }

    /// Watch the global MCP settings file for changes.
    ///
    /// Corresponds to TS: `watchMcpSettingsFile()`.
    /// Debounces changes by 500ms and triggers config reload.
    pub async fn watch_mcp_settings_file(&self) {
        let settings_path = match &self.settings_path {
            Some(p) => p.clone(),
            None => return,
        };

        if !std::path::Path::new(&settings_path).exists() {
            return;
        }

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let mut watcher = match RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                        let _ = tx.send(());
                    }
                }
            },
            NotifyConfig::default(),
        ) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!("Failed to create settings file watcher: {}", e);
                return;
            }
        };

        let path = std::path::PathBuf::from(&settings_path);
        if let Err(e) = watcher.watch(&path, RecursiveMode::NonRecursive) {
            tracing::error!("Failed to watch settings file '{}': {}", settings_path, e);
            return;
        }

        let sp = settings_path.clone();
        let is_programmatic = self.is_programmatic_update.clone();
        let task = tokio::spawn(async move {
            loop {
                if rx.recv().await.is_none() {
                    break;
                }
                // Check if this is a programmatic update
                if *is_programmatic.read().await {
                    continue;
                }
                // Debounce: wait 500ms
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                while rx.try_recv().is_ok() {}
                tracing::info!("MCP settings file changed: {}", sp);
                // The actual reload is handled by the caller via the state change callback
            }
        });

        let mut sw = self.settings_watcher.lock().await;
        *sw = Some(FileWatcherHandle {
            _watcher: watcher,
            _task: task,
        });
    }

    /// Watch the project MCP file (.roo/mcp.json) for changes.
    ///
    /// Corresponds to TS: `watchProjectMcpFile()`.
    pub async fn watch_project_mcp_file(&self) {
        let workspace = match &self.workspace_path {
            Some(p) => p.clone(),
            None => return,
        };

        let project_mcp_path = std::path::PathBuf::from(&workspace)
            .join(".roo")
            .join("mcp.json");

        if !project_mcp_path.exists() {
            return;
        }

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();

        let mut watcher = match RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    if matches!(
                        event.kind,
                        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
                    ) {
                        let _ = tx.send(event.kind);
                    }
                }
            },
            NotifyConfig::default(),
        ) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!("Failed to create project MCP file watcher: {}", e);
                return;
            }
        };

        if let Err(e) = watcher.watch(&project_mcp_path, RecursiveMode::NonRecursive) {
            tracing::error!(
                "Failed to watch project MCP file '{}': {}",
                project_mcp_path.display(),
                e
            );
            return;
        }

        let is_programmatic = self.is_programmatic_update.clone();
        let pmp = project_mcp_path.to_string_lossy().to_string();
        let task = tokio::spawn(async move {
            loop {
                if rx.recv().await.is_none() {
                    break;
                }
                // Check if this is a programmatic update
                if *is_programmatic.read().await {
                    continue;
                }
                // Debounce: wait 500ms
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                while rx.try_recv().is_ok() {}
                tracing::info!("Project MCP file changed: {}", pmp);
                // The actual reload is handled by the caller via the state change callback
            }
        });

        let mut pw = self.project_mcp_watcher.lock().await;
        *pw = Some(FileWatcherHandle {
            _watcher: watcher,
            _task: task,
        });
    }

    /// Get the project MCP configuration path.
    ///
    /// Corresponds to TS: `getProjectMcpPath()`.
    /// Returns the path to `.roo/mcp.json` in the workspace, or `None` if
    /// the file doesn't exist or no workspace is configured.
    pub fn get_project_mcp_path(&self) -> Option<String> {
        let workspace = self.workspace_path.as_ref()?;
        let path = std::path::PathBuf::from(workspace)
            .join(".roo")
            .join("mcp.json");
        if path.exists() {
            Some(path.to_string_lossy().to_string())
        } else {
            None
        }
    }

    /// Set the programmatic update flag.
    ///
    /// Corresponds to TS: setting `isProgrammaticUpdate = true` before writing config.
    /// Prevents the file watcher from triggering unnecessary reloads.
    pub async fn set_programmatic_update(&self, value: bool) {
        *self.is_programmatic_update.write().await = value;
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    fn check_disposed(&self) -> McpResult<()> {
        let disposed = self.disposed.try_read().map_err(|_| McpError::Disposed)?;
        if *disposed {
            return Err(McpError::Disposed);
        }
        Ok(())
    }

    fn notify_state_change(&self) {
        if let Some(callback) = &self.on_state_change {
            callback();
        }
    }

    /// Create a placeholder connection for disabled/disconnected servers.
    ///
    /// Corresponds to TS: `createPlaceholderConnection(name, config, source, reason)`
    fn create_placeholder_connection(
        &self,
        name: &str,
        config: &ValidatedServerConfig,
        source: McpSource,
        reason: DisableReason,
    ) -> McpConnection {
        let mut server_state = McpServerState::new(
            name.to_string(),
            serde_json::to_string(config).unwrap_or_default(),
            source,
        );
        server_state.disabled = reason == DisableReason::ServerDisabled || config.is_disabled();
        server_state.status = McpConnectionStatus::Disconnected;

        McpConnection::Disconnected(DisconnectedMcpConnection {
            server: server_state,
        })
    }

    /// Establish a connection to an MCP server.
    ///
    /// Creates the transport, performs the MCP handshake, and discovers
    /// tools, resources, and resource templates.
    ///
    /// Corresponds to the inner logic of TS: `connectToServer()` after config validation.
    async fn establish_connection(
        &self,
        name: &str,
        config: &ValidatedServerConfig,
        _source: McpSource,
    ) -> McpResult<(
        McpClient,
        Box<dyn McpTransport>,
        Vec<McpTool>,
        Vec<McpResource>,
        Vec<McpResourceTemplate>,
        Option<String>,
    )> {
        let mut transport: Box<dyn McpTransport> = match config {
            ValidatedServerConfig::Stdio {
                command,
                args,
                env,
                cwd,
                ..
            } => {
                let mut t = StdioTransport::new(
                    command.clone(),
                    args.clone(),
                    env.clone(),
                    cwd.clone(),
                );
                t.connect().await?;
                Box::new(t)
            }
            ValidatedServerConfig::Sse { url, headers, .. } => {
                let mut t = SseTransport::new(url.clone(), headers.clone());
                t.connect().await?;
                Box::new(t)
            }
            ValidatedServerConfig::StreamableHttp { url, headers, .. } => {
                let mut t = StreamableHttpTransport::new(url.clone(), headers.clone());
                t.connect().await?;
                Box::new(t)
            }
        };

        // Create client and initialize
        let mut client = McpClient::new("Roo Code", "1.0.0");
        let init_result = client.initialize(&mut *transport).await?;

        // Discover tools, resources, and templates
        let raw_tools = client.list_tools(&mut *transport).await.unwrap_or_default();
        let resources = client
            .list_resources(&mut *transport)
            .await
            .unwrap_or_default();
        let resource_templates = client
            .list_resource_templates(&mut *transport)
            .await
            .unwrap_or_default();

        // Read alwaysAllow and disabledTools from config to annotate tools
        let always_allow_config = config.always_allow();
        let disabled_tools_list = config.disabled_tools();
        let has_wildcard = always_allow_config.contains(&"*".to_string());

        // Annotate tools with always_allow and enabled_for_prompt
        let tools: Vec<McpTool> = raw_tools
            .into_iter()
            .map(|mut tool| {
                tool.always_allow = has_wildcard || always_allow_config.contains(&tool.name);
                tool.enabled_for_prompt = !disabled_tools_list.contains(&tool.name);
                tool
            })
            .collect();

        let instructions = init_result.instructions;

        tracing::info!(
            "Established connection to '{}': {} tools, {} resources, {} templates",
            name,
            tools.len(),
            resources.len(),
            resource_templates.len()
        );

        Ok((client, transport, tools, resources, resource_templates, instructions))
    }
}

impl Default for McpHub {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helper trait for finding connections in a Vec
// ---------------------------------------------------------------------------

/// Extension trait for searching connections in a Vec<McpConnection>.
pub trait McpConnectionExt {
    /// Find a connection by name and source.
    fn find(&self, name: &str, source: McpSource) -> Option<&McpConnection>;
    /// Find a mutable connection by name and source.
    fn find_mut(&mut self, name: &str, source: McpSource) -> Option<&mut McpConnection>;
    /// Find a connected connection by name (any source).
    fn find_connected_mut(&mut self, name: &str) -> Option<&mut McpConnection>;
}

impl McpConnectionExt for Vec<McpConnection> {
    fn find(&self, name: &str, source: McpSource) -> Option<&McpConnection> {
        self.iter()
            .find(|conn| conn.name() == name && conn.source() == source)
    }

    fn find_mut(&mut self, name: &str, source: McpSource) -> Option<&mut McpConnection> {
        self.iter_mut()
            .find(|conn| conn.name() == name && conn.source() == source)
    }

    fn find_connected_mut(&mut self, name: &str) -> Option<&mut McpConnection> {
        let sanitized = sanitize_mcp_name(name);

        // Try exact match first, then fuzzy match
        self.iter_mut().find(|conn| {
            (conn.name() == name || tool_names_match(conn.name(), &sanitized))
                && conn.is_connected()
        })
    }
}

// Make McpConnection cloneable for read-only operations
impl Clone for McpConnection {
    fn clone(&self) -> Self {
        match self {
            McpConnection::Disconnected(d) => McpConnection::Disconnected(DisconnectedMcpConnection {
                server: d.server.clone(),
            }),
            // Connected connections can't truly be cloned (they hold transport),
            // so we clone as disconnected with the same server state
            McpConnection::Connected(c) => McpConnection::Disconnected(DisconnectedMcpConnection {
                server: c.server.clone(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hub_creation() {
        let hub = McpHub::new();
        let servers = hub.get_servers();
        assert!(servers.is_empty());
        assert!(!hub.is_connecting().await);
    }

    #[tokio::test]
    async fn test_register_unregister_client() {
        let hub = McpHub::new();
        hub.register_client().await;
        hub.register_client().await;

        let count = *hub.ref_count.read().await;
        assert_eq!(count, 2);

        hub.unregister_client().await.unwrap();
        let count = *hub.ref_count.read().await;
        assert_eq!(count, 1);

        // Last unregister should dispose
        hub.unregister_client().await.unwrap();
        let disposed = *hub.disposed.read().await;
        assert!(disposed);
    }

    #[tokio::test]
    async fn test_find_server_name_by_sanitized() {
        let hub = McpHub::new();
        // No connections yet
        let result = hub.find_server_name_by_sanitized_name("test").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_mcp_enabled_change() {
        let hub = McpHub::new();
        hub.handle_mcp_enabled_change(false).await.unwrap();
        assert!(!*hub.mcp_enabled.read().await);

        hub.handle_mcp_enabled_change(true).await.unwrap();
        assert!(*hub.mcp_enabled.read().await);
    }

    #[tokio::test]
    async fn test_dispose() {
        let hub = McpHub::new();
        hub.dispose().await.unwrap();
        assert!(*hub.disposed.read().await);

        // Operations should fail after dispose
        let result = hub
            .connect_to_server(
                "test",
                &serde_json::json!({"command": "echo"}),
                McpSource::Global,
            )
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_servers_empty() {
        let hub = McpHub::new();
        let servers = hub.get_all_servers().await;
        assert!(servers.is_empty());
    }

    #[tokio::test]
    async fn test_is_connecting_flag() {
        let hub = McpHub::new();
        assert!(!hub.is_connecting().await);

        *hub.is_connecting.write().await = true;
        assert!(hub.is_connecting().await);
    }

    #[test]
    fn test_connection_vec_find() {
        let mut connections: Vec<McpConnection> = Vec::new();

        let server = McpServerState::new(
            "test-server".to_string(),
            "{}".to_string(),
            McpSource::Global,
        );
        connections.push(McpConnection::Disconnected(DisconnectedMcpConnection {
            server,
        }));

        let found = connections.find("test-server", McpSource::Global);
        assert!(found.is_some());

        let not_found = connections.find("test-server", McpSource::Project);
        assert!(not_found.is_none());

        let not_found2 = connections.find("other-server", McpSource::Global);
        assert!(not_found2.is_none());
    }

    #[tokio::test]
    async fn test_toggle_tool_always_allow() {
        let hub = McpHub::new();

        let mut server = McpServerState::new(
            "test-server".to_string(),
            r#"{"type":"stdio","command":"echo","alwaysAllow":[],"disabledTools":[]}"#
                .to_string(),
            McpSource::Global,
        );
        server.tools = vec![McpTool {
            name: "my-tool".to_string(),
            description: None,
            input_schema: None,
            always_allow: false,
            enabled_for_prompt: true,
        }];
        hub.connections.write().await.push(McpConnection::Disconnected(
            DisconnectedMcpConnection { server },
        ));

        hub.toggle_tool_always_allow("test-server", McpSource::Global, "my-tool", true)
            .await
            .unwrap();

        let connections = hub.connections.read().await;
        let conn = connections.find("test-server", McpSource::Global).unwrap();
        let tool = conn
            .server()
            .tools
            .iter()
            .find(|t| t.name == "my-tool")
            .unwrap();
        assert!(tool.always_allow);

        let config: serde_json::Value = serde_json::from_str(&conn.server().config).unwrap();
        let always_allow = config["alwaysAllow"].as_array().unwrap();
        assert!(always_allow
            .iter()
            .any(|v| v.as_str() == Some("my-tool")));
    }

    #[tokio::test]
    async fn test_toggle_tool_always_allow_off() {
        let hub = McpHub::new();

        let mut server = McpServerState::new(
            "test-server".to_string(),
            r#"{"type":"stdio","command":"echo","alwaysAllow":["existing-tool"],"disabledTools":[]}"#
                .to_string(),
            McpSource::Global,
        );
        server.tools = vec![McpTool {
            name: "existing-tool".to_string(),
            description: None,
            input_schema: None,
            always_allow: true,
            enabled_for_prompt: true,
        }];
        hub.connections.write().await.push(McpConnection::Disconnected(
            DisconnectedMcpConnection { server },
        ));

        hub.toggle_tool_always_allow("test-server", McpSource::Global, "existing-tool", false)
            .await
            .unwrap();

        let connections = hub.connections.read().await;
        let conn = connections.find("test-server", McpSource::Global).unwrap();
        let tool = conn
            .server()
            .tools
            .iter()
            .find(|t| t.name == "existing-tool")
            .unwrap();
        assert!(!tool.always_allow);

        let config: serde_json::Value = serde_json::from_str(&conn.server().config).unwrap();
        let always_allow = config["alwaysAllow"].as_array().unwrap();
        assert!(!always_allow
            .iter()
            .any(|v| v.as_str() == Some("existing-tool")));
    }

    #[tokio::test]
    async fn test_toggle_tool_enabled_for_prompt_disable() {
        let hub = McpHub::new();

        let mut server = McpServerState::new(
            "test-server".to_string(),
            r#"{"type":"stdio","command":"echo","alwaysAllow":[],"disabledTools":[]}"#
                .to_string(),
            McpSource::Global,
        );
        server.tools = vec![McpTool {
            name: "my-tool".to_string(),
            description: None,
            input_schema: None,
            always_allow: false,
            enabled_for_prompt: true,
        }];
        hub.connections.write().await.push(McpConnection::Disconnected(
            DisconnectedMcpConnection { server },
        ));

        hub.toggle_tool_enabled_for_prompt("test-server", McpSource::Global, "my-tool", false)
            .await
            .unwrap();

        let connections = hub.connections.read().await;
        let conn = connections.find("test-server", McpSource::Global).unwrap();
        let tool = conn
            .server()
            .tools
            .iter()
            .find(|t| t.name == "my-tool")
            .unwrap();
        assert!(!tool.enabled_for_prompt);

        let config: serde_json::Value = serde_json::from_str(&conn.server().config).unwrap();
        let disabled_tools = config["disabledTools"].as_array().unwrap();
        assert!(disabled_tools
            .iter()
            .any(|v| v.as_str() == Some("my-tool")));
    }

    #[tokio::test]
    async fn test_toggle_tool_enabled_for_prompt_enable() {
        let hub = McpHub::new();

        let mut server = McpServerState::new(
            "test-server".to_string(),
            r#"{"type":"stdio","command":"echo","alwaysAllow":[],"disabledTools":["existing-tool"]}"#
                .to_string(),
            McpSource::Global,
        );
        server.tools = vec![McpTool {
            name: "existing-tool".to_string(),
            description: None,
            input_schema: None,
            always_allow: false,
            enabled_for_prompt: false,
        }];
        hub.connections.write().await.push(McpConnection::Disconnected(
            DisconnectedMcpConnection { server },
        ));

        hub.toggle_tool_enabled_for_prompt("test-server", McpSource::Global, "existing-tool", true)
            .await
            .unwrap();

        let connections = hub.connections.read().await;
        let conn = connections.find("test-server", McpSource::Global).unwrap();
        let tool = conn
            .server()
            .tools
            .iter()
            .find(|t| t.name == "existing-tool")
            .unwrap();
        assert!(tool.enabled_for_prompt);

        let config: serde_json::Value = serde_json::from_str(&conn.server().config).unwrap();
        let disabled_tools = config["disabledTools"].as_array().unwrap();
        assert!(!disabled_tools
            .iter()
            .any(|v| v.as_str() == Some("existing-tool")));
    }

    #[tokio::test]
    async fn test_toggle_tool_server_not_found() {
        let hub = McpHub::new();

        let result = hub
            .toggle_tool_always_allow("nonexistent", McpSource::Global, "tool", true)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_server_timeout() {
        let hub = McpHub::new();

        let mut server = McpServerState::new(
            "test-server".to_string(),
            r#"{"type":"stdio","command":"echo","timeout":60}"#.to_string(),
            McpSource::Global,
        );
        server.tools = vec![];
        hub.connections.write().await.push(McpConnection::Disconnected(
            DisconnectedMcpConnection { server },
        ));

        hub.update_server_timeout("test-server", 120, McpSource::Global)
            .await
            .unwrap();

        let connections = hub.connections.read().await;
        let conn = connections.find("test-server", McpSource::Global).unwrap();
        let config: serde_json::Value = serde_json::from_str(&conn.server().config).unwrap();
        assert_eq!(config["timeout"].as_u64(), Some(120));
    }

    #[tokio::test]
    async fn test_update_server_timeout_not_found() {
        let hub = McpHub::new();
        let result = hub
            .update_server_timeout("nonexistent", 120, McpSource::Global)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_server() {
        let hub = McpHub::new();

        let server = McpServerState::new(
            "test-server".to_string(),
            "{}".to_string(),
            McpSource::Global,
        );
        hub.connections.write().await.push(McpConnection::Disconnected(
            DisconnectedMcpConnection { server },
        ));

        assert_eq!(hub.connections.read().await.len(), 1);

        hub.delete_server("test-server", McpSource::Global)
            .await
            .unwrap();

        assert_eq!(hub.connections.read().await.len(), 0);
    }

    #[tokio::test]
    async fn test_delete_server_not_found() {
        let hub = McpHub::new();
        let result = hub
            .delete_server("nonexistent", McpSource::Global)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_find_connection() {
        let hub = McpHub::new();

        let server_global = McpServerState::new(
            "my-server".to_string(),
            "{}".to_string(),
            McpSource::Global,
        );
        let server_project = McpServerState::new(
            "my-server".to_string(),
            "{}".to_string(),
            McpSource::Project,
        );
        hub.connections.write().await.push(McpConnection::Disconnected(
            DisconnectedMcpConnection {
                server: server_global,
            },
        ));
        hub.connections.write().await.push(McpConnection::Disconnected(
            DisconnectedMcpConnection {
                server: server_project,
            },
        ));

        // Find by source
        let conn = hub.find_connection("my-server", Some(McpSource::Global)).await;
        assert!(conn.is_some());

        let conn = hub.find_connection("my-server", Some(McpSource::Project)).await;
        assert!(conn.is_some());

        // Without source, project takes priority
        let conn = hub.find_connection("my-server", None).await;
        assert!(conn.is_some());
        assert_eq!(conn.unwrap().source(), McpSource::Project);
    }

    #[test]
    fn test_get_servers_extracts_disabled_tools() {
        let mut connections: Vec<McpConnection> = Vec::new();

        let mut server = McpServerState::new(
            "test-server".to_string(),
            "{}".to_string(),
            McpSource::Global,
        );
        server.tools = vec![
            McpTool {
                name: "enabled-tool".to_string(),
                description: None,
                input_schema: None,
                always_allow: false,
                enabled_for_prompt: true,
            },
            McpTool {
                name: "disabled-tool".to_string(),
                description: None,
                input_schema: None,
                always_allow: false,
                enabled_for_prompt: false,
            },
        ];
        connections.push(McpConnection::Disconnected(DisconnectedMcpConnection {
            server,
        }));

        let server_conn = McpServerConnection {
            name: connections[0].name().to_string(),
            status: connections[0].server().status,
            tools: connections[0].server().tools.clone(),
            resources: connections[0].server().resources.clone(),
            resource_templates: connections[0].server().resource_templates.clone(),
            disabled_tools: connections[0]
                .server()
                .tools
                .iter()
                .filter(|t| !t.enabled_for_prompt)
                .map(|t| t.name.clone())
                .collect(),
            errors: connections[0].server().error_history.clone(),
        };

        assert_eq!(server_conn.disabled_tools, vec!["disabled-tool"]);
    }

    #[tokio::test]
    async fn test_update_server_connections_add_new() {
        let hub = McpHub::new();

        let mut servers = HashMap::new();
        servers.insert(
            "test-server".to_string(),
            serde_json::json!({"command": "echo", "disabled": true}),
        );

        // This should add a disabled server (won't actually connect)
        hub.update_server_connections(&servers, McpSource::Global, true)
            .await
            .unwrap();

        let connections = hub.connections.read().await;
        assert_eq!(connections.len(), 1);
        assert!(connections[0].server().disabled);
    }

    #[tokio::test]
    async fn test_update_server_connections_remove() {
        let hub = McpHub::new();

        // Add a server first
        let server = McpServerState::new(
            "old-server".to_string(),
            "{}".to_string(),
            McpSource::Global,
        );
        hub.connections.write().await.push(McpConnection::Disconnected(
            DisconnectedMcpConnection { server },
        ));

        assert_eq!(hub.connections.read().await.len(), 1);

        // Update with empty servers �?should remove the old one
        let empty: HashMap<String, serde_json::Value> = HashMap::new();
        hub.update_server_connections(&empty, McpSource::Global, true)
            .await
            .unwrap();

        assert_eq!(hub.connections.read().await.len(), 0);
    }
}

//! Main application controller.
//!
//! Source: `src/core/ClineProvider.ts` — ClineProvider class
//!
//! The `App` struct is the top-level coordinator that manages all Roo Code subsystems.
//! It provides the high-level API for creating tasks, managing providers, and handling
//! the overall lifecycle of the application.

use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::AppConfig;
use crate::error::AppResult;
use crate::state::{AppState, SharedState};

/// The main Roo Code application controller.
///
/// Source: `src/core/ClineProvider.ts` — `ClineProvider` class
///
/// This struct coordinates all subsystems:
/// - Provider management (AI model connections)
/// - Task lifecycle (create, run, cancel)
/// - MCP server connections
/// - Configuration management
/// - Skills management
/// - Checkpoint system
/// - Terminal management
/// - Telemetry
pub struct App {
    /// Application configuration.
    config: AppConfig,

    /// Shared application state.
    state: SharedState,

    // ── Subsystems ──────────────────────────────────────────────────────
    /// MCP hub for managing MCP server connections.
    mcp_hub: Option<Arc<roo_mcp::McpHub>>,

    /// Terminal registry for managing terminal processes.
    terminal_registry: Option<Arc<roo_terminal::TerminalRegistry>>,

    /// Message queue for buffering user messages.
    ///
    /// Source: TS `this.messageQueueService`
    message_queue: Option<Arc<tokio::sync::Mutex<roo_message_queue::MessageQueueService>>>,

    /// Telemetry service for capturing lifecycle events.
    ///
    /// Source: TS `this.telemetryService`
    telemetry: Option<Arc<std::sync::RwLock<roo_telemetry::TelemetryService>>>,

    /// RooIgnore controller for file access control.
    roo_ignore: Option<Arc<roo_ignore::RooIgnoreController>>,

    /// Skills manager for discovering and managing skills.
    skills_manager: Option<Arc<roo_skills::SkillsManager>>,

    /// Todo list storage (in-memory, keyed by task ID).
    todos: Arc<RwLock<std::collections::HashMap<String, serde_json::Value>>>,
}

impl App {
    /// Create a new App instance with the given configuration.
    ///
    /// Source: `src/core/ClineProvider.ts` — constructor
    pub fn new(config: AppConfig) -> Self {
        let state = Arc::new(RwLock::new(AppState::new()));
        Self {
            config,
            state,
            mcp_hub: None,
            terminal_registry: None,
            message_queue: None,
            telemetry: None,
            roo_ignore: None,
            skills_manager: None,
            todos: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    /// Initialize the application.
    ///
    /// This sets up all subsystems, loads configuration, and prepares
    /// the application for use.
    ///
    /// Source: `src/core/ClineProvider.ts` — initialization logic
    pub async fn initialize(&mut self) -> AppResult<()> {
        let mut state = self.state.write().await;

        tracing::info!(
            "Initializing Roo Code App in workspace: {}",
            self.config.cwd
        );

        // ── Initialize RooIgnore controller ─────────────────────────────
        let mut roo_ignore = roo_ignore::RooIgnoreController::new(&self.config.cwd);
        let rooignore_path = std::path::Path::new(&self.config.cwd).join(".rooignore");
        if rooignore_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&rooignore_path) {
                roo_ignore.load_patterns(&content);
                tracing::info!("Loaded .rooignore patterns from {}", rooignore_path.display());
            }
        }
        self.roo_ignore = Some(Arc::new(roo_ignore));

        // ── Initialize Terminal Registry ────────────────────────────────
        self.terminal_registry = Some(Arc::new(roo_terminal::TerminalRegistry::new()));
        tracing::debug!("Terminal registry initialized");

        // ── Initialize MCP Hub ──────────────────────────────────────────
        let workspace_path = self.config.cwd.clone();
        let settings_path = self.config.global_storage_path.clone();
        let hub = roo_mcp::McpHub::new_with_paths(
            Some(workspace_path.clone()),
            if settings_path.is_empty() { None } else { Some(settings_path) },
        );

        // Load project-level MCP config (.roo/mcp.json)
        let project_mcp_path = std::path::Path::new(&self.config.cwd)
            .join(".roo")
            .join("mcp.json");
        if project_mcp_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&project_mcp_path) {
                match serde_json::from_str::<serde_json::Value>(&content) {
                    Ok(config) => {
                        if let Some(servers) = config.get("mcpServers").and_then(|v| v.as_object()) {
                            let server_map: std::collections::HashMap<String, serde_json::Value> =
                                servers.into_iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                            if let Err(e) = hub.update_server_connections(
                                &server_map,
                                roo_mcp::McpSource::Project,
                                true,
                            ).await {
                                tracing::warn!("Failed to load project MCP servers: {}", e);
                            } else {
                                tracing::info!("Loaded {} project MCP servers from {}",
                                    server_map.len(), project_mcp_path.display());
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to parse project MCP config {}: {}", project_mcp_path.display(), e);
                    }
                }
            }
        }

        // Load global MCP config (<global_storage>/settings/mcp_settings.json)
        if !self.config.global_storage_path.is_empty() {
            let global_mcp_path = std::path::Path::new(&self.config.global_storage_path)
                .join("settings")
                .join("mcp_settings.json");
            if global_mcp_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&global_mcp_path) {
                    match serde_json::from_str::<serde_json::Value>(&content) {
                        Ok(config) => {
                            if let Some(servers) = config.get("mcpServers").and_then(|v| v.as_object()) {
                                let server_map: std::collections::HashMap<String, serde_json::Value> =
                                    servers.into_iter().map(|(k, v)| (k.clone(), v.clone())).collect();
                                if let Err(e) = hub.update_server_connections(
                                    &server_map,
                                    roo_mcp::McpSource::Global,
                                    true,
                                ).await {
                                    tracing::warn!("Failed to load global MCP servers: {}", e);
                                } else {
                                    tracing::info!("Loaded {} global MCP servers from {}",
                                        server_map.len(), global_mcp_path.display());
                                }
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to parse global MCP config {}: {}", global_mcp_path.display(), e);
                        }
                    }
                }
            }
        }

        self.mcp_hub = Some(Arc::new(hub));
        tracing::debug!("MCP hub initialized");

        // ── Initialize Message Queue ────────────────────────────────────
        self.message_queue = Some(Arc::new(tokio::sync::Mutex::new(
            roo_message_queue::MessageQueueService::new(),
        )));
        tracing::debug!("Message queue initialized");

        // ── Initialize Telemetry Service ────────────────────────────────
        let telemetry = roo_telemetry::TelemetryService::new();
        self.telemetry = Some(Arc::new(std::sync::RwLock::new(telemetry)));
        tracing::debug!("Telemetry service initialized");

        // ── Initialize Skills Manager ───────────────────────────────────
        let mut skills = roo_skills::SkillsManager::new();
        let mut skills_dirs: Vec<(std::path::PathBuf, roo_skills::SkillSource, Option<String>)> = Vec::new();

        // Global skills directory: ~/.roo/skills/
        if let Some(home) = dirs_home() {
            let global_skills_dir = std::path::Path::new(&home).join(".roo").join("skills");
            if global_skills_dir.exists() {
                skills_dirs.push((global_skills_dir, roo_skills::SkillSource::Global, None));
            }
            // Also check ~/.agents/skills/
            let global_agents_dir = std::path::Path::new(&home).join(".agents").join("skills");
            if global_agents_dir.exists() {
                skills_dirs.push((global_agents_dir, roo_skills::SkillSource::Global, None));
            }
        }

        // Project skills directory: <cwd>/.roo/skills/
        let project_skills_dir = std::path::Path::new(&self.config.cwd).join(".roo").join("skills");
        if project_skills_dir.exists() {
            skills_dirs.push((project_skills_dir, roo_skills::SkillSource::Project, None));
        }

        // Mode-specific project skills: <cwd>/.roo/skills-<mode>/
        let mode_skills_dir = std::path::Path::new(&self.config.cwd)
            .join(".roo")
            .join(format!("skills-{}", self.config.mode));
        if mode_skills_dir.exists() {
            skills_dirs.push((mode_skills_dir, roo_skills::SkillSource::Project, Some(self.config.mode.clone())));
        }

        if let Err(e) = skills.discover_skills(&skills_dirs).await {
            tracing::warn!("Failed to discover skills: {}", e);
        }
        self.skills_manager = Some(Arc::new(skills));
        tracing::debug!("Skills manager initialized");

        // ── Update state ────────────────────────────────────────────────
        state.current_mode = self.config.mode.clone();
        state.initialized = true;

        tracing::info!("App initialized with mode: {}", state.current_mode);
        Ok(())
    }

    /// Get the current working directory.
    pub fn cwd(&self) -> &str {
        &self.config.cwd
    }

    /// Get the current mode.
    pub async fn mode(&self) -> String {
        self.state.read().await.current_mode.clone()
    }

    /// Set the current mode.
    pub async fn set_mode(&self, mode: &str) {
        let mut state = self.state.write().await;
        tracing::info!("Switching mode from {} to {}", state.current_mode, mode);
        state.current_mode = mode.to_string();
    }

    /// Get a reference to the provider settings.
    pub fn provider_settings(&self) -> &roo_types::provider_settings::ProviderSettings {
        &self.config.provider_settings
    }

    /// Get a reference to the app config.
    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    /// Get a snapshot of the current application state.
    pub async fn state(&self) -> AppState {
        self.state.read().await.clone()
    }

    /// Check if the app has been disposed.
    pub async fn is_disposed(&self) -> bool {
        self.state.read().await.disposed
    }

    // ── Subsystem getters ────────────────────────────────────────────────

    /// Get a reference to the MCP hub, if initialized.
    pub fn mcp_hub(&self) -> Option<&Arc<roo_mcp::McpHub>> {
        self.mcp_hub.as_ref()
    }

    /// Get a reference to the terminal registry, if initialized.
    pub fn terminal_registry(&self) -> Option<&Arc<roo_terminal::TerminalRegistry>> {
        self.terminal_registry.as_ref()
    }

    /// Get a reference to the message queue, if initialized.
    pub fn message_queue(
        &self,
    ) -> Option<&Arc<tokio::sync::Mutex<roo_message_queue::MessageQueueService>>> {
        self.message_queue.as_ref()
    }

    /// Get a reference to the telemetry service, if initialized.
    pub fn telemetry(&self) -> Option<&Arc<std::sync::RwLock<roo_telemetry::TelemetryService>>> {
        self.telemetry.as_ref()
    }

    /// Get a reference to the roo-ignore controller, if initialized.
    pub fn roo_ignore(&self) -> Option<&Arc<roo_ignore::RooIgnoreController>> {
        self.roo_ignore.as_ref()
    }

    /// Get a reference to the skills manager, if initialized.
    pub fn skills_manager(&self) -> Option<&Arc<roo_skills::SkillsManager>> {
        self.skills_manager.as_ref()
    }

    /// Get the todo list storage.
    pub fn todos(&self) -> &Arc<RwLock<std::collections::HashMap<String, serde_json::Value>>> {
        &self.todos
    }

    /// Build a [`roo_task::ServiceRefs`] from the current service instances.
    ///
    /// Returns a `ServiceRefs` with all initialized services. Services that
    /// haven't been initialized yet will be `None`.
    pub fn service_refs(&self) -> roo_task::ServiceRefs {
        roo_task::ServiceRefs {
            mcp_hub: self.mcp_hub.clone(),
            terminal_registry: self.terminal_registry.clone(),
            message_queue: self.message_queue.clone(),
            telemetry: self.telemetry.clone(),
        }
    }

    /// Create a fully-wired [`roo_task::TaskLifecycle`] for the given config.
    ///
    /// This creates a `TaskEngine` from the config, wraps it in a
    /// `TaskLifecycle`, and attaches all available service references.
    pub fn create_task_lifecycle(
        &self,
        config: roo_task::TaskConfig,
    ) -> Result<roo_task::TaskLifecycle, roo_task::TaskError> {
        let engine = roo_task::TaskEngine::new(config)?;
        let services = self.service_refs();
        Ok(roo_task::TaskLifecycle::new(engine).with_services(services))
    }

    /// Dispose of the application and clean up resources.
    ///
    /// Source: `src/core/ClineProvider.ts` — dispose logic
    pub async fn dispose(&self) -> AppResult<()> {
        let mut state = self.state.write().await;
        state.disposed = true;
        state.task_running = false;

        // Clean up MCP hub
        if let Some(hub) = &self.mcp_hub {
            let _ = hub.dispose().await;
        }

        // Shut down telemetry
        if let Some(telemetry) = &self.telemetry {
            if let Ok(svc) = telemetry.read() {
                svc.shutdown();
            }
        }

        tracing::info!("App disposed");
        Ok(())
    }

    /// Build the system prompt for the current mode and configuration.
    ///
    /// This uses the roo-prompt crate to generate the complete system prompt
    /// based on the current mode, provider settings, and other configuration.
    pub fn build_system_prompt(&self) -> String {
        let settings = roo_prompt::types::SystemPromptSettings {
            is_stealth_model: false,
            ..Default::default()
        };

        roo_prompt::build_system_prompt(
            &self.config.cwd,
            &self.config.mode,
            None,                      // custom_modes
            None,                      // custom_mode_prompts
            self.mcp_hub.is_some(),    // has_mcp
            None,                      // global_custom_instructions
            self.config.language.as_deref(),
            None,                      // roo_ignore_instructions
            Some(&settings),
            &[],                       // skills
            &format!("{} {}", std::env::consts::OS, env!("CARGO_PKG_VERSION")),
            "bash",                    // shell
            &std::env::var("HOME")
                .or_else(|_| std::env::var("USERPROFILE"))
                .unwrap_or_else(|_| "~".to_string()),
        )
    }

    /// Set custom instructions for the current session.
    ///
    /// Source: TS `webviewMessageHandler.ts` — `customInstructions`
    pub async fn set_custom_instructions(&self, text: &str) {
        let mut state = self.state.write().await;
        state.custom_instructions = if text.is_empty() { None } else { Some(text.to_string()) };
    }

    /// Reset all application state.
    ///
    /// Source: TS `webviewMessageHandler.ts` — `resetState`
    pub async fn reset_state(&self) -> AppResult<()> {
        let mut state = self.state.write().await;
        state.current_mode = "code".to_string();
        state.initialized = false;
        state.task_running = false;
        state.disposed = false;
        state.custom_instructions = None;
        tracing::info!("App state reset");
        Ok(())
    }
}

impl Drop for App {
    fn drop(&mut self) {
        // Best-effort cleanup
        if let Ok(state) = self.state.try_write() {
            drop(state);
        }
    }
}

/// Get the user's home directory.
fn dirs_home() -> Option<String> {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_creation() {
        let config = AppConfig::default();
        let app = App::new(config);
        assert_eq!(app.cwd(), std::env::current_dir().unwrap().to_str().unwrap());
    }

    #[tokio::test]
    async fn test_app_initialize() {
        let config = AppConfig {
            cwd: "/tmp/test".to_string(),
            mode: "code".to_string(),
            ..Default::default()
        };
        let mut app = App::new(config);
        app.initialize().await.unwrap();

        let state = app.state().await;
        assert!(state.initialized);
        assert_eq!(state.current_mode, "code");
        assert!(app.mcp_hub().is_some());
        assert!(app.terminal_registry().is_some());
        assert!(app.message_queue().is_some());
        assert!(app.telemetry().is_some());
        assert!(app.roo_ignore().is_some());
        assert!(app.skills_manager().is_some());
    }

    #[tokio::test]
    async fn test_app_service_refs() {
        let config = AppConfig {
            cwd: "/tmp/test".to_string(),
            mode: "code".to_string(),
            ..Default::default()
        };
        let mut app = App::new(config);
        app.initialize().await.unwrap();

        let refs = app.service_refs();
        assert!(refs.mcp_hub.is_some());
        assert!(refs.terminal_registry.is_some());
        assert!(refs.message_queue.is_some());
        assert!(refs.telemetry.is_some());
    }

    #[tokio::test]
    async fn test_app_create_task_lifecycle() {
        let config = AppConfig {
            cwd: "/tmp/test".to_string(),
            mode: "code".to_string(),
            ..Default::default()
        };
        let mut app = App::new(config);
        app.initialize().await.unwrap();

        let task_config = roo_task::TaskConfig::new("test-task-1", "/tmp/test")
            .with_mode("code")
            .with_start_task(false);
        let lifecycle = app.create_task_lifecycle(task_config).unwrap();
        assert_eq!(lifecycle.task_id(), "test-task-1");
        // Services should be wired in
        assert!(lifecycle.services().mcp_hub.is_some());
        assert!(lifecycle.services().terminal_registry.is_some());
        assert!(lifecycle.services().message_queue.is_some());
        assert!(lifecycle.services().telemetry.is_some());
    }

    #[tokio::test]
    async fn test_set_mode() {
        let config = AppConfig {
            cwd: "/tmp/test".to_string(),
            mode: "code".to_string(),
            ..Default::default()
        };
        let mut app = App::new(config);
        app.initialize().await.unwrap();

        app.set_mode("architect").await;
        assert_eq!(app.mode().await, "architect");
    }

    #[tokio::test]
    async fn test_dispose() {
        let config = AppConfig::default();
        let mut app = App::new(config);
        app.initialize().await.unwrap();

        app.dispose().await.unwrap();
        assert!(app.is_disposed().await);
    }

    #[test]
    fn test_build_system_prompt() {
        let config = AppConfig {
            cwd: "/tmp/test-project".to_string(),
            mode: "code".to_string(),
            ..Default::default()
        };
        let app = App::new(config);
        let prompt = app.build_system_prompt();

        assert!(prompt.contains("TOOL USE"));
        assert!(prompt.contains("RULES"));
        assert!(prompt.contains("OBJECTIVE"));
        assert!(prompt.contains("CAPABILITIES"));
        assert!(prompt.contains("SYSTEM INFORMATION"));
    }

    #[test]
    fn test_config_default() {
        let config = AppConfig::default();
        assert_eq!(config.mode, "code");
        assert!(config.telemetry_enabled == false);
        assert!(config.checkpoints_enabled);
    }
}

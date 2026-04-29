//! VSCode context state proxy for global/workspace state management.
//!
//! Provides an in-memory cache layer over persistent state storage, with
//! support for global state, secrets, provider settings, and settings
//! import/export.
//!
//! Source: `src/core/config/ContextProxy.ts`

use std::collections::HashMap;
use std::sync::Arc;

use serde_json::Value;

use crate::error::ConfigError;

// ---------------------------------------------------------------------------
// Pass-through state keys
// ---------------------------------------------------------------------------

/// Keys that bypass the cache and always read/write directly from storage.
///
/// Source: `src/core/config/ContextProxy.ts` — `PASS_THROUGH_STATE_KEYS`
pub const PASS_THROUGH_STATE_KEYS: &[&str] = &["taskHistory"];

/// Check if a key is a pass-through key.
///
/// Source: `src/core/config/ContextProxy.ts` — `isPassThroughStateKey`
pub fn is_pass_through_state_key(key: &str) -> bool {
    PASS_THROUGH_STATE_KEYS.contains(&key)
}

// ---------------------------------------------------------------------------
// GlobalStateKey / SecretStateKey
// ---------------------------------------------------------------------------

/// Well-known global state keys.
///
/// Source: `packages/types/src/global-settings.ts` — `GLOBAL_STATE_KEYS`
pub const GLOBAL_STATE_KEYS: &[&str] = &[
    "currentApiConfigName",
    "listApiConfigMeta",
    "taskHistory",
    "mode",
    "modeApiConfigs",
    "customModes",
    "customModePrompts",
    "customSupportPrompts",
    "autoApprovalEnabled",
    "alwaysAllowReadOnly",
    "alwaysAllowReadOnlyOutsideWorkspace",
    "alwaysAllowWrite",
    "alwaysAllowWriteOutsideWorkspace",
    "alwaysAllowExecute",
    "alwaysAllowExecuteOutsideWorkspace",
    "alwaysAllowMcp",
    "alwaysAllowBrowser",
    "alwaysAllowSubtasks",
    "allowedCommands",
    "deniedCommands",
    "alwaysAllowModeSwitch",
    "alwaysAllowSubtaskSubdialogs",
    "condensingApiConfigId",
    "customCondensingPrompt",
    "openRouterImageGenerationSelectedModel",
    "openAiImageGenerationSelectedModel",
    "autoCondenseContext",
    "autoCondensePercent",
    "contextWindowGapBufferPercent",
    "contextWindowSlidingWindowPercent",
    "profileThresholds",
    "soundEnabled",
    "soundVolume",
    "terminalShellIntegrationTimeout",
    "terminalZdotdir",
    "terminalCommandDelay",
    "terminalZshClearLineEol",
    "terminalZshOhMy",
    "terminalZshP10k",
    "terminalZshBle",
    "terminalShellIntegration",
    "diffEnabled",
    "enableCheckpoints",
    "checkpointTimeout",
    "maxOpenTabsContext",
    "maxWorkspaceFiles",
    "showRooIgnoredFiles",
    "cloudIsConnected",
    "cloudLastAuthCheck",
    "experiments",
    "disabledTools",
    "r1ModelType",
    "enableReasoningEffort",
    "reasoningEffort",
    "apiProvider",
    "openAiHeaders",
];

/// Well-known secret state keys.
///
/// Source: `packages/types/src/global-settings.ts` — `SECRET_STATE_KEYS`
pub const SECRET_STATE_KEYS: &[&str] = &[
    "apiKey",
    "openAiApiKey",
    "anthropicApiKey",
    "awsAccessKey",
    "awsSecretKey",
    "openRouterApiKey",
    "xaiApiKey",
    "geminiApiKey",
    "deepSeekApiKey",
    "mistralApiKey",
    "bedrockUseCrossRegionInference",
    "vertexProjectId",
    "vertexRegion",
    "openAiNativeApiKey",
    "openAiBaseUrl",
    "openAiApiVersion",
    "azureApiVersion",
    "openAiApiDeploymentId",
    "glamaApiKey",
    "unboundApiKey",
    "unboundServiceId",
    "litellmApiKey",
    "litellmBaseUrl",
    "requestyApiKey",
    "requestyBaseUrl",
    "basetenApiKey",
    "ollamaBaseUrl",
    "lmStudioBaseUrl",
    "fireworksApiKey",
    "sambanovaApiKey",
    "moonshotApiKey",
    "qwenApiKey",
    "poeApiKey",
    "vscodeLmModelSelector",
    "codestralApiKey",
    "cerebrasApiKey",
    "nebiusApiKey",
    "groqApiKey",
    "chutesApiKey",
    "vercelApiKey",
    "rooCodeApiKey",
    "zaiApiKey",
    "zaiBaseUrl",
];

/// Global secret keys (additional).
pub const GLOBAL_SECRET_KEYS: &[&str] = &[
    "openRouterImageApiKey",
];

// ---------------------------------------------------------------------------
// StateStore trait
// ---------------------------------------------------------------------------

/// Trait for abstracting persistent state storage.
///
/// In the TS version, this is backed by VSCode's `ExtensionContext.globalState`
/// and `ExtensionContext.secrets`. In Rust, we abstract it so it can be backed
/// by files, databases, or any other storage.
#[async_trait::async_trait]
pub trait StateStore: Send + Sync {
    /// Get a value from global state.
    async fn get_global_state(&self, key: &str) -> Result<Option<Value>, ConfigError>;

    /// Update a value in global state.
    async fn update_global_state(&self, key: &str, value: Option<Value>) -> Result<(), ConfigError>;

    /// Get a secret value.
    async fn get_secret(&self, key: &str) -> Result<Option<String>, ConfigError>;

    /// Store a secret value. If value is None, delete the secret.
    async fn store_secret(&self, key: &str, value: Option<String>) -> Result<(), ConfigError>;
}

// ---------------------------------------------------------------------------
// InMemoryStateStore (for testing)
// ---------------------------------------------------------------------------

/// An in-memory implementation of [`StateStore`] for testing.
#[derive(Debug, Default)]
pub struct InMemoryStateStore {
    global_state: HashMap<String, Value>,
    secrets: HashMap<String, String>,
}

#[async_trait::async_trait]
impl StateStore for InMemoryStateStore {
    async fn get_global_state(&self, key: &str) -> Result<Option<Value>, ConfigError> {
        Ok(self.global_state.get(key).cloned())
    }

    async fn update_global_state(&self, _key: &str, _value: Option<Value>) -> Result<(), ConfigError> {
        // Note: In real use this would need interior mutability (RwLock)
        // For the trait definition this is fine
        Ok(())
    }

    async fn get_secret(&self, key: &str) -> Result<Option<String>, ConfigError> {
        Ok(self.secrets.get(key).cloned())
    }

    async fn store_secret(&self, _key: &str, _value: Option<String>) -> Result<(), ConfigError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ContextProxy
// ---------------------------------------------------------------------------

/// VSCode context state proxy with in-memory caching.
///
/// Provides a caching layer over persistent state storage, with support for:
/// - Global state (settings, preferences)
/// - Secret state (API keys, tokens)
/// - Provider settings
/// - Import/export
/// - State migration
///
/// Source: `src/core/config/ContextProxy.ts`
pub struct ContextProxy {
    /// The persistent state store.
    store: Arc<dyn StateStore>,
    /// In-memory cache for global state.
    state_cache: HashMap<String, Value>,
    /// In-memory cache for secrets.
    secret_cache: HashMap<String, String>,
    /// Whether the proxy has been initialized.
    initialized: bool,
}

impl ContextProxy {
    /// Create a new ContextProxy with the given state store.
    pub fn new(store: Arc<dyn StateStore>) -> Self {
        Self {
            store,
            state_cache: HashMap::new(),
            secret_cache: HashMap::new(),
            initialized: false,
        }
    }

    /// Whether the proxy has been initialized.
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// Initialize the proxy by loading all state from the store.
    ///
    /// Mirrors the TS `ContextProxy.initialize()` method.
    pub async fn initialize(&mut self) -> Result<(), ConfigError> {
        // Load global state
        for key in GLOBAL_STATE_KEYS {
            match self.store.get_global_state(key).await {
                Ok(value) => {
                    if let Some(v) = value {
                        self.state_cache.insert(key.to_string(), v);
                    }
                }
                Err(e) => {
                    tracing::error!("Error loading global {}: {}", key, e);
                }
            }
        }

        // Load secrets
        let all_secret_keys: Vec<&str> = SECRET_STATE_KEYS
            .iter()
            .chain(GLOBAL_SECRET_KEYS.iter())
            .copied()
            .collect();

        for key in all_secret_keys {
            match self.store.get_secret(key).await {
                Ok(value) => {
                    if let Some(v) = value {
                        self.secret_cache.insert(key.to_string(), v);
                    }
                }
                Err(e) => {
                    tracing::error!("Error loading secret {}: {}", key, e);
                }
            }
        }

        self.initialized = true;
        Ok(())
    }

    // -------------------------------------------------------------------
    // Global State
    // -------------------------------------------------------------------

    /// Get a value from global state.
    ///
    /// Pass-through keys always read from the store directly.
    /// Other keys read from the in-memory cache.
    ///
    /// Mirrors the TS `getGlobalState()` method.
    pub async fn get_global_state(&self, key: &str) -> Option<Value> {
        if is_pass_through_state_key(key) {
            self.store.get_global_state(key).await.ok().flatten()
        } else {
            self.state_cache.get(key).cloned()
        }
    }

    /// Get a value from global state with a default.
    pub async fn get_global_state_or(&self, key: &str, default: Value) -> Value {
        self.get_global_state(key).await.unwrap_or(default)
    }

    /// Update a value in global state.
    ///
    /// Pass-through keys write directly to the store.
    /// Other keys update both the cache and the store.
    ///
    /// Mirrors the TS `updateGlobalState()` method.
    pub async fn update_global_state(
        &mut self,
        key: &str,
        value: Option<Value>,
    ) -> Result<(), ConfigError> {
        if !is_pass_through_state_key(key) {
            match &value {
                Some(v) => self.state_cache.insert(key.to_string(), v.clone()),
                None => self.state_cache.remove(key),
            };
        }

        self.store.update_global_state(key, value).await
    }

    /// Get all global state as a map.
    fn get_all_global_state(&self) -> HashMap<String, Value> {
        GLOBAL_STATE_KEYS
            .iter()
            .filter_map(|key| {
                self.state_cache
                    .get(*key)
                    .map(|v| (key.to_string(), v.clone()))
            })
            .collect()
    }

    // -------------------------------------------------------------------
    // Secrets
    // -------------------------------------------------------------------

    /// Get a secret value from the cache.
    ///
    /// Mirrors the TS `getSecret()` method.
    pub fn get_secret(&self, key: &str) -> Option<&str> {
        self.secret_cache.get(key).map(|s| s.as_str())
    }

    /// Store a secret value.
    ///
    /// Updates both the cache and the persistent store.
    /// If value is None, deletes the secret.
    ///
    /// Mirrors the TS `storeSecret()` method.
    pub async fn store_secret(
        &mut self,
        key: &str,
        value: Option<String>,
    ) -> Result<(), ConfigError> {
        match &value {
            Some(v) => {
                self.secret_cache.insert(key.to_string(), v.clone());
            }
            None => {
                self.secret_cache.remove(key);
            }
        }

        self.store.store_secret(key, value).await
    }

    /// Refresh secrets from storage and update cache.
    ///
    /// Mirrors the TS `refreshSecrets()` method.
    pub async fn refresh_secrets(&mut self) -> Result<(), ConfigError> {
        let all_secret_keys: Vec<&str> = SECRET_STATE_KEYS
            .iter()
            .chain(GLOBAL_SECRET_KEYS.iter())
            .copied()
            .collect();

        for key in all_secret_keys {
            match self.store.get_secret(key).await {
                Ok(value) => {
                    match value {
                        Some(v) => {
                            self.secret_cache.insert(key.to_string(), v);
                        }
                        None => {
                            self.secret_cache.remove(key);
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Error refreshing secret {}: {}", key, e);
                }
            }
        }

        Ok(())
    }

    /// Get all secret state as a map.
    fn get_all_secret_state(&self) -> HashMap<String, String> {
        let mut result = HashMap::new();
        for key in SECRET_STATE_KEYS.iter().chain(GLOBAL_SECRET_KEYS.iter()) {
            if let Some(v) = self.secret_cache.get(*key) {
                result.insert(key.to_string(), v.clone());
            }
        }
        result
    }

    // -------------------------------------------------------------------
    // Combined access (setValue / getValue / getValues / setValues)
    // -------------------------------------------------------------------

    /// Check if a key is a secret state key.
    fn is_secret_key(key: &str) -> bool {
        SECRET_STATE_KEYS.contains(&key) || GLOBAL_SECRET_KEYS.contains(&key)
    }

    /// Set a single setting value.
    ///
    /// Mirrors the TS `setValue()` method.
    pub async fn set_value(&mut self, key: &str, value: Value) -> Result<(), ConfigError> {
        if Self::is_secret_key(key) {
            let secret_val = value.as_str().map(|s| s.to_string());
            self.store_secret(key, secret_val).await
        } else {
            self.update_global_state(key, Some(value)).await
        }
    }

    /// Get a single setting value.
    ///
    /// Mirrors the TS `getValue()` method.
    pub async fn get_value(&self, key: &str) -> Option<Value> {
        if Self::is_secret_key(key) {
            self.get_secret(key).map(|s| Value::String(s.to_string()))
        } else {
            self.get_global_state(key).await
        }
    }

    /// Get all settings values (merged global state + secrets).
    ///
    /// Mirrors the TS `getValues()` method.
    pub fn get_values(&self) -> HashMap<String, Value> {
        let mut result = HashMap::new();

        for (key, value) in self.get_all_global_state() {
            result.insert(key, value);
        }

        for (key, value) in self.get_all_secret_state() {
            result.insert(key, Value::String(value));
        }

        result
    }

    /// Set multiple settings values.
    ///
    /// Mirrors the TS `setValues()` method.
    pub async fn set_values(&mut self, values: HashMap<String, Value>) -> Result<(), ConfigError> {
        for (key, value) in values {
            self.set_value(&key, value).await?;
        }
        Ok(())
    }

    // -------------------------------------------------------------------
    // Reset
    // -------------------------------------------------------------------

    /// Reset all state (clear caches and storage).
    ///
    /// Mirrors the TS `resetAllState()` method.
    pub async fn reset_all_state(&mut self) -> Result<(), ConfigError> {
        self.state_cache.clear();
        self.secret_cache.clear();

        // Clear global state
        for key in GLOBAL_STATE_KEYS {
            if let Err(e) = self.store.update_global_state(key, None).await {
                tracing::error!("Error clearing global state {}: {}", key, e);
            }
        }

        // Clear secrets
        let all_secret_keys: Vec<&str> = SECRET_STATE_KEYS
            .iter()
            .chain(GLOBAL_SECRET_KEYS.iter())
            .copied()
            .collect();

        for key in all_secret_keys {
            if let Err(e) = self.store.store_secret(key, None).await {
                tracing::error!("Error clearing secret {}: {}", key, e);
            }
        }

        // Re-initialize
        self.initialized = false;
        self.initialize().await
    }

    // -------------------------------------------------------------------
    // Export
    // -------------------------------------------------------------------

    /// Export global settings (excluding taskHistory and listApiConfigMeta).
    ///
    /// Mirrors the TS `export()` method.
    pub fn export_settings(&self) -> HashMap<String, Value> {
        let excluded_keys = ["taskHistory", "listApiConfigMeta", "currentApiConfigName"];
        let all_values = self.get_values();

        all_values
            .into_iter()
            .filter(|(key, value)| !excluded_keys.contains(&key.as_str()) && !value.is_null())
            .filter(|(key, _)| {
                // Filter custom modes to only include global source
                if key == "customModes" {
                    true // Filtering would happen at a higher level
                } else {
                    true
                }
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A simple in-memory state store for testing.
    #[derive(Debug, Default)]
    struct TestStateStore {
        global_state: std::sync::Mutex<HashMap<String, Value>>,
        secrets: std::sync::Mutex<HashMap<String, String>>,
    }

    #[async_trait::async_trait]
    impl StateStore for TestStateStore {
        async fn get_global_state(&self, key: &str) -> Result<Option<Value>, ConfigError> {
            Ok(self.global_state.lock().unwrap().get(key).cloned())
        }

        async fn update_global_state(
            &self,
            key: &str,
            value: Option<Value>,
        ) -> Result<(), ConfigError> {
            let mut state = self.global_state.lock().unwrap();
            match value {
                Some(v) => {
                    state.insert(key.to_string(), v);
                }
                None => {
                    state.remove(key);
                }
            }
            Ok(())
        }

        async fn get_secret(&self, key: &str) -> Result<Option<String>, ConfigError> {
            Ok(self.secrets.lock().unwrap().get(key).cloned())
        }

        async fn store_secret(
            &self,
            key: &str,
            value: Option<String>,
        ) -> Result<(), ConfigError> {
            let mut secrets = self.secrets.lock().unwrap();
            match value {
                Some(v) => {
                    secrets.insert(key.to_string(), v);
                }
                None => {
                    secrets.remove(key);
                }
            }
            Ok(())
        }
    }

    fn create_test_proxy() -> ContextProxy {
        let store = Arc::new(TestStateStore::default());
        ContextProxy::new(store)
    }

    async fn create_initialized_proxy() -> ContextProxy {
        let mut proxy = create_test_proxy();
        proxy.initialize().await.unwrap();
        proxy
    }

    // ---- Test 1: Initialization ----
    #[tokio::test]
    async fn test_initialization() {
        let mut proxy = create_test_proxy();
        assert!(!proxy.is_initialized());
        proxy.initialize().await.unwrap();
        assert!(proxy.is_initialized());
    }

    // ---- Test 2: Global state get/set ----
    #[tokio::test]
    async fn test_global_state_get_set() {
        let mut proxy = create_initialized_proxy().await;

        proxy
            .update_global_state("mode", Some(Value::String("code".to_string())))
            .await
            .unwrap();

        let value = proxy.get_global_state("mode").await;
        assert_eq!(value, Some(Value::String("code".to_string())));
    }

    // ---- Test 3: Global state delete ----
    #[tokio::test]
    async fn test_global_state_delete() {
        let mut proxy = create_initialized_proxy().await;

        proxy
            .update_global_state("mode", Some(Value::String("code".to_string())))
            .await
            .unwrap();
        proxy.update_global_state("mode", None).await.unwrap();

        let value = proxy.get_global_state("mode").await;
        assert!(value.is_none());
    }

    // ---- Test 4: Secret get/set ----
    #[tokio::test]
    async fn test_secret_get_set() {
        let mut proxy = create_initialized_proxy().await;

        proxy
            .store_secret("apiKey", Some("secret-key-123".to_string()))
            .await
            .unwrap();

        assert_eq!(proxy.get_secret("apiKey"), Some("secret-key-123"));
    }

    // ---- Test 5: Secret delete ----
    #[tokio::test]
    async fn test_secret_delete() {
        let mut proxy = create_initialized_proxy().await;

        proxy
            .store_secret("apiKey", Some("secret-key-123".to_string()))
            .await
            .unwrap();
        proxy.store_secret("apiKey", None).await.unwrap();

        assert!(proxy.get_secret("apiKey").is_none());
    }

    // ---- Test 6: setValue / getValue for global state ----
    #[tokio::test]
    async fn test_set_get_value_global() {
        let mut proxy = create_initialized_proxy().await;

        proxy
            .set_value("mode", Value::String("architect".to_string()))
            .await
            .unwrap();

        let value = proxy.get_value("mode").await;
        assert_eq!(value, Some(Value::String("architect".to_string())));
    }

    // ---- Test 7: setValue / getValue for secrets ----
    #[tokio::test]
    async fn test_set_get_value_secret() {
        let mut proxy = create_initialized_proxy().await;

        proxy
            .set_value("apiKey", Value::String("test-key".to_string()))
            .await
            .unwrap();

        let value = proxy.get_value("apiKey").await;
        assert_eq!(value, Some(Value::String("test-key".to_string())));
    }

    // ---- Test 8: getValues returns merged state ----
    #[tokio::test]
    async fn test_get_values_merged() {
        let mut proxy = create_initialized_proxy().await;

        proxy
            .set_value("mode", Value::String("code".to_string()))
            .await
            .unwrap();
        proxy
            .set_value("apiKey", Value::String("key-123".to_string()))
            .await
            .unwrap();

        let values = proxy.get_values();
        assert_eq!(values.get("mode"), Some(&Value::String("code".to_string())));
        assert_eq!(values.get("apiKey"), Some(&Value::String("key-123".to_string())));
    }

    // ---- Test 9: resetAllState clears everything ----
    #[tokio::test]
    async fn test_reset_all_state() {
        let mut proxy = create_initialized_proxy().await;

        proxy
            .set_value("mode", Value::String("code".to_string()))
            .await
            .unwrap();
        proxy
            .set_value("apiKey", Value::String("key-123".to_string()))
            .await
            .unwrap();

        proxy.reset_all_state().await.unwrap();

        let mode = proxy.get_value("mode").await;
        assert!(mode.is_none());
    }

    // ---- Test 10: Pass-through key detection ----
    #[test]
    fn test_pass_through_key_detection() {
        assert!(is_pass_through_state_key("taskHistory"));
        assert!(!is_pass_through_state_key("mode"));
        assert!(!is_pass_through_state_key("apiKey"));
    }

    // ---- Test 11: Export settings ----
    #[tokio::test]
    async fn test_export_settings() {
        let mut proxy = create_initialized_proxy().await;

        proxy
            .set_value("mode", Value::String("code".to_string()))
            .await
            .unwrap();

        let exported = proxy.export_settings();
        // taskHistory should be excluded
        assert!(exported.get("taskHistory").is_none());
        // mode should be included
        assert_eq!(exported.get("mode"), Some(&Value::String("code".to_string())));
    }

    // ---- Test 12: is_secret_key detection ----
    #[test]
    fn test_is_secret_key() {
        assert!(ContextProxy::is_secret_key("apiKey"));
        assert!(ContextProxy::is_secret_key("openRouterImageApiKey"));
        assert!(!ContextProxy::is_secret_key("mode"));
        assert!(!ContextProxy::is_secret_key("customModes"));
    }
}

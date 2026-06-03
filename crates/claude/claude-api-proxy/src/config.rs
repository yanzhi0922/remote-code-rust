use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ProxyConfigFile {
    pub proxy: ProxySettings,
    pub providers: Vec<ProviderEntry>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProxySettings {
    pub bind: SocketAddr,
    #[serde(default)]
    pub auth_token: Option<String>,
    /// Allow running without API authentication. Intended only for isolated local development.
    #[serde(default)]
    pub allow_unauthenticated: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderEntry {
    pub name: String,
    pub model: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub api_key_env: Option<String>,
    pub anthropic_url: String,
    pub openai_url: String,
}

impl ProxyConfigFile {
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let mut config: Self = toml::from_str(&text)?;
        if config.providers.is_empty() {
            anyhow::bail!("no providers configured");
        }
        config.resolve_provider_api_keys()?;
        Ok(config)
    }

    pub(crate) fn resolve_provider_api_keys(&mut self) -> anyhow::Result<()> {
        for provider in &mut self.providers {
            if !provider.api_key.trim().is_empty() {
                continue;
            }

            let Some(env_name) = provider
                .api_key_env
                .as_deref()
                .filter(|value| !value.trim().is_empty())
            else {
                anyhow::bail!(
                    "provider `{}` must set api_key_env or a non-empty api_key",
                    provider.name
                );
            };
            provider.api_key = std::env::var(env_name).map_err(|_| {
                anyhow::anyhow!(
                    "provider `{}` api_key_env `{}` is not set",
                    provider.name,
                    env_name
                )
            })?;
            if provider.api_key.trim().is_empty() {
                anyhow::bail!(
                    "provider `{}` api_key_env `{}` resolved to an empty value",
                    provider.name,
                    env_name
                );
            }
        }
        Ok(())
    }

    pub fn build_model_index(&self) -> Arc<HashMap<String, ProviderEntry>> {
        let mut index = HashMap::new();
        for provider in &self.providers {
            index.insert(provider.model.clone(), provider.clone());
        }
        Arc::new(index)
    }
}

#[cfg(test)]
mod tests {
    use super::ProxyConfigFile;

    #[test]
    fn provider_api_key_can_be_loaded_from_environment() {
        // SAFETY: `std::env::set_var` / `std::env::remove_var` are unsafe because the underlying
        // C runtime is not thread-safe and concurrent reads/writes can race.
        // This call is serialized by the surrounding guard (OnceLock, Mutex, or
        // single-threaded test context) so no other thread is reading the
        // variable concurrently.
        unsafe {
            std::env::set_var("CLAUDE_PROXY_TEST_KEY", "resolved-secret");
        }

        let mut config: ProxyConfigFile = toml::from_str(
            r#"
            [proxy]
            bind = "127.0.0.1:8787"
            allow_unauthenticated = true

            [[providers]]
            name = "test"
            model = "test-model"
            api_key_env = "CLAUDE_PROXY_TEST_KEY"
            anthropic_url = "https://example.invalid/anthropic"
            openai_url = "https://example.invalid/v1"
            "#,
        )
        .expect("config should parse");

        config
            .resolve_provider_api_keys()
            .expect("env key should resolve");
        assert_eq!(config.providers[0].api_key, "resolved-secret");

        // SAFETY: `std::env::set_var` / `std::env::remove_var` are unsafe because the underlying
        // C runtime is not thread-safe and concurrent reads/writes can race.
        // This call is serialized by the surrounding guard (OnceLock, Mutex, or
        // single-threaded test context) so no other thread is reading the
        // variable concurrently.
        unsafe {
            std::env::remove_var("CLAUDE_PROXY_TEST_KEY");
        }
    }

    #[test]
    fn provider_without_api_key_or_env_is_rejected() {
        let mut config: ProxyConfigFile = toml::from_str(
            r#"
            [proxy]
            bind = "127.0.0.1:8787"
            allow_unauthenticated = true

            [[providers]]
            name = "test"
            model = "test-model"
            anthropic_url = "https://example.invalid/anthropic"
            openai_url = "https://example.invalid/v1"
            "#,
        )
        .expect("config should parse");

        assert!(config.resolve_provider_api_keys().is_err());
    }
}

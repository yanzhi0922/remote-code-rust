//! Provider configuration types.
//!
//! Corresponds to `src/utils/settings/types.ts` (ProviderTypeSchema, ProviderConfigSchema).

use serde::{Deserialize, Serialize};

/// Provider type enumeration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderType {
    FirstParty,
    Bedrock,
    Vertex,
    Foundry,
    #[serde(rename = "anthropic-compatible")]
    AnthropicCompatible,
    #[serde(rename = "openai-compatible")]
    OpenAiCompatible,
    #[serde(rename = "github-models")]
    GitHubModels,
    #[serde(rename = "github-copilot")]
    GitHubCopilot,
}

impl ProviderType {
    /// Get the string representation.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FirstParty => "firstParty",
            Self::Bedrock => "bedrock",
            Self::Vertex => "vertex",
            Self::Foundry => "foundry",
            Self::AnthropicCompatible => "anthropic-compatible",
            Self::OpenAiCompatible => "openai-compatible",
            Self::GitHubModels => "github-models",
            Self::GitHubCopilot => "github-copilot",
        }
    }
}

/// Provider configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderConfig {
    /// The provider type.
    pub r#type: Option<ProviderType>,
    /// Human-readable provider name.
    pub name: Option<String>,
    /// Base URL for API calls.
    pub base_url: Option<String>,
    /// Environment variable name for API key.
    pub api_key_env: Option<String>,
    /// Environment variable name for auth token.
    pub auth_token_env: Option<String>,
    /// Default model for this provider.
    pub default_model: Option<String>,
    /// Available models for this provider.
    pub models: Option<Vec<String>>,
    /// Small/fast model for this provider.
    pub small_fast_model: Option<String>,
    /// Cloud region (for Bedrock/Vertex).
    pub region: Option<String>,
    /// Project ID (for Vertex).
    pub project_id: Option<String>,
    /// Resource identifier.
    pub resource: Option<String>,
}

impl ProviderConfig {
    /// Create a new empty provider config.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if this is a first-party provider.
    #[must_use]
    pub fn is_first_party(&self) -> bool {
        self.r#type.as_ref() == Some(&ProviderType::FirstParty)
    }

    /// Check if this is a Bedrock provider.
    #[must_use]
    pub fn is_bedrock(&self) -> bool {
        self.r#type.as_ref() == Some(&ProviderType::Bedrock)
    }

    /// Check if this is a Vertex provider.
    #[must_use]
    pub fn is_vertex(&self) -> bool {
        self.r#type.as_ref() == Some(&ProviderType::Vertex)
    }

    /// Get the effective base URL.
    #[must_use]
    pub fn effective_base_url<'a>(&'a self, default: &'a str) -> &'a str {
        match &self.base_url {
            Some(u) if !u.is_empty() => u.as_str(),
            _ => default,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_type_serialization() {
        let pt = ProviderType::AnthropicCompatible;
        let json = serde_json::to_string(&pt).expect("provider type should serialize");
        assert_eq!(json, "\"anthropic-compatible\"");

        let deserialized: ProviderType =
            serde_json::from_str(&json).expect("provider type should deserialize");
        assert_eq!(deserialized, ProviderType::AnthropicCompatible);
    }

    #[test]
    fn provider_type_as_str() {
        assert_eq!(ProviderType::FirstParty.as_str(), "firstParty");
        assert_eq!(ProviderType::Bedrock.as_str(), "bedrock");
        assert_eq!(ProviderType::GitHubCopilot.as_str(), "github-copilot");
    }

    #[test]
    fn provider_config_default() {
        let config = ProviderConfig::default();
        assert!(config.r#type.is_none());
        assert!(config.base_url.is_none());
    }

    #[test]
    fn provider_config_type_checks() {
        let config = ProviderConfig {
            r#type: Some(ProviderType::Bedrock),
            region: Some("us-east-1".to_string()),
            ..Default::default()
        };
        assert!(config.is_bedrock());
        assert!(!config.is_first_party());
        assert!(!config.is_vertex());
    }

    #[test]
    fn provider_config_serialization() {
        let config = ProviderConfig {
            r#type: Some(ProviderType::OpenAiCompatible),
            base_url: Some("https://api.example.com/v1".to_string()),
            api_key_env: Some("MY_API_KEY".to_string()),
            default_model: Some("gpt-4".to_string()),
            ..Default::default()
        };
        let json = serde_json::to_string(&config).expect("provider config should serialize");
        assert!(json.contains("openai-compatible"));
        assert!(json.contains("api.example.com"));
        assert!(json.contains("MY_API_KEY"));

        let deserialized: ProviderConfig =
            serde_json::from_str(&json).expect("provider config should deserialize");
        assert_eq!(deserialized.base_url, config.base_url);
    }

    #[test]
    fn effective_base_url() {
        let config = ProviderConfig {
            base_url: Some("https://custom.api.com".to_string()),
            ..Default::default()
        };
        assert_eq!(
            config.effective_base_url("https://default.com"),
            "https://custom.api.com"
        );

        let empty = ProviderConfig::default();
        assert_eq!(
            empty.effective_base_url("https://default.com"),
            "https://default.com"
        );
    }
}

//! Profile Validator
//!
//! Organization allow-list structures and profile validation functions.
//! Mirrors `profileValidator.ts`.

use std::collections::HashMap;

use crate::api::ProviderName;
use crate::provider_settings::ProviderSettings;

// ---------------------------------------------------------------------------
// Allow-list types
// ---------------------------------------------------------------------------

/// Organization-level allow list that gates which providers and models may be used.
#[derive(Debug, Clone, Default)]
pub struct OrganizationAllowList {
    /// When true, all providers and models are allowed.
    pub allow_all: bool,
    /// Per-provider allow lists. Keyed by provider name string (e.g. "anthropic").
    pub providers: HashMap<String, ProviderAllowList>,
}

/// Per-provider allow list that gates which models may be used.
#[derive(Debug, Clone, Default)]
pub struct ProviderAllowList {
    /// When true, all models for this provider are allowed.
    pub allow_all: bool,
    /// Specific model IDs allowed. `None` means all models are allowed
    /// (equivalent to `allow_all`).
    pub models: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// Validation functions
// ---------------------------------------------------------------------------

/// Returns `true` when the profile's provider is permitted by the organization allow list.
pub fn is_profile_allowed(profile: &ProviderSettings, allow_list: &OrganizationAllowList) -> bool {
    if allow_list.allow_all {
        return true;
    }
    let provider_name = match profile.api_provider {
        Some(p) => p,
        None => return true, // no provider → no restriction
    };
    is_provider_allowed(provider_name, allow_list)
}

/// Returns `true` when the given provider is permitted by the organization allow list.
pub fn is_provider_allowed(provider: ProviderName, allow_list: &OrganizationAllowList) -> bool {
    if allow_list.allow_all {
        return true;
    }
    match allow_list.providers.get(provider.as_str()) {
        Some(entry) => entry.allow_all,
        None => false,
    }
}

/// Returns `true` when the given model is permitted for the given provider.
pub fn is_model_allowed(
    provider: ProviderName,
    model_id: &str,
    allow_list: &OrganizationAllowList,
) -> bool {
    if allow_list.allow_all {
        return true;
    }
    let entry = match allow_list.providers.get(provider.as_str()) {
        Some(e) => e,
        None => return false,
    };
    if entry.allow_all {
        return true;
    }
    match &entry.models {
        Some(models) => models.iter().any(|m| m == model_id),
        None => true,
    }
}

// ---------------------------------------------------------------------------
// Model ID extraction
// ---------------------------------------------------------------------------

/// Returns the model ID field relevant to the given provider from the profile settings.
///
/// Each provider stores its model identifier in a different field.
/// This function maps the provider name to the correct field.
pub fn get_model_id_from_profile(profile: &ProviderSettings) -> Option<String> {
    let provider = profile.api_provider?;

    match provider {
        ProviderName::Anthropic => profile.api_model_id.clone(),
        ProviderName::Openai => profile.open_ai_model_id.clone(),
        ProviderName::OpenaiNative => profile.api_model_id.clone(),
        ProviderName::OpenaiCodex => profile.api_model_id.clone(),
        ProviderName::Gemini => profile.api_model_id.clone(),
        ProviderName::GeminiCli => profile.api_model_id.clone(),
        ProviderName::Vertex => profile.api_model_id.clone(),
        ProviderName::Bedrock => profile
            .aws_bedrock_custom_model_id
            .as_ref()
            .or(profile.api_model_id.as_ref())
            .cloned(),
        ProviderName::OpenRouter => profile.open_router_model_id.clone(),
        ProviderName::Ollama => profile.ollama_model_id.clone(),
        ProviderName::LmStudio => profile.lm_studio_model_id.clone(),
        ProviderName::DeepSeek => profile.api_model_id.clone(),
        ProviderName::Xai => profile.api_model_id.clone(),
        ProviderName::MiniMax => profile.api_model_id.clone(),
        ProviderName::Moonshot => profile.api_model_id.clone(),
        ProviderName::QwenCode => profile.api_model_id.clone(),
        ProviderName::Zai => profile.api_model_id.clone(),
        ProviderName::Mistral => profile.api_model_id.clone(),
        ProviderName::Fireworks => profile.api_model_id.clone(),
        ProviderName::SambaNova => profile.api_model_id.clone(),
        ProviderName::Baseten => profile.baseten_model_id.clone(),
        ProviderName::VscodeLm => profile.api_model_id.clone(),
        ProviderName::Poe => profile.poe_model_id.clone(),
        ProviderName::LiteLlm => profile.litellm_model_id.clone(),
        ProviderName::Requesty => profile.requesty_model_id.clone(),
        ProviderName::Unbound => profile.unbound_model_id.clone(),
        ProviderName::Roo => profile.api_model_id.clone(),
        ProviderName::VercelAiGateway => profile.vercel_ai_gateway_model_id.clone(),
        ProviderName::FakeAi => profile.api_model_id.clone(),
        // Retired providers
        ProviderName::Cerebras
        | ProviderName::Chutes
        | ProviderName::Deepinfra
        | ProviderName::Doubao
        | ProviderName::Featherless
        | ProviderName::Groq
        | ProviderName::Huggingface
        | ProviderName::IoIntelligence => profile.api_model_id.clone(),
    }
}

// ---------------------------------------------------------------------------
// check_exist_api_config
// ---------------------------------------------------------------------------

/// Returns `true` when the profile has enough configuration to attempt an API call.
///
/// Certain providers (fake-ai, openai-codex, qwen-code, roo) are always
/// considered "configured" because they don't need an explicit API key.
/// Other providers require at least one identifying credential.
pub fn check_exist_api_config(profile: &ProviderSettings) -> bool {
    let provider = match profile.api_provider {
        Some(p) => p,
        None => return false,
    };

    match provider {
        // Providers that need no configuration
        ProviderName::FakeAi
        | ProviderName::OpenaiCodex
        | ProviderName::QwenCode
        | ProviderName::Roo => true,

        // Anthropic: needs API key or base URL with auth token
        ProviderName::Anthropic => {
            profile.api_key.is_some()
                || profile.anthropic_base_url.is_some()
                    && profile.anthropic_use_auth_token == Some(true)
        }

        // OpenAI family
        ProviderName::Openai | ProviderName::OpenaiNative => {
            profile.open_ai_api_key.is_some() || profile.open_ai_base_url.is_some()
        }

        // Gemini
        ProviderName::Gemini => {
            profile.gemini_api_key.is_some()
                || profile.google_api_key.is_some()
                || profile.google_gemini_base_url.is_some()
                || profile.gemini_base_url.is_some()
        }

        // Gemini CLI
        ProviderName::GeminiCli => profile.gemini_cli_oauth_path.is_some(),

        // Vertex
        ProviderName::Vertex => {
            profile.vertex_project_id.is_some()
                || profile.vertex_key_file.is_some()
                || profile.vertex_json_credentials.is_some()
        }

        // Bedrock — needs AWS region or profile or API key
        ProviderName::Bedrock => {
            profile.aws_region.is_some()
                || profile.aws_profile.is_some()
                || profile.aws_api_key.is_some()
                || profile.aws_access_key.is_some()
        }

        // OpenRouter
        ProviderName::OpenRouter => {
            profile.open_router_api_key.is_some() || profile.open_router_base_url.is_some()
        }

        // Ollama — just needs a model ID
        ProviderName::Ollama => profile.ollama_model_id.is_some(),

        // LM Studio — just needs a model ID
        ProviderName::LmStudio => profile.lm_studio_model_id.is_some(),

        // VS Code LM — needs model selector
        ProviderName::VscodeLm => profile.vs_code_lm_model_selector.is_some(),

        // DeepSeek
        ProviderName::DeepSeek => {
            profile.deep_seek_api_key.is_some() || profile.deep_seek_base_url.is_some()
        }

        // xAI
        ProviderName::Xai => profile.xai_api_key.is_some(),

        // MiniMax
        ProviderName::MiniMax => {
            profile.minimax_api_key.is_some() || profile.minimax_base_url.is_some()
        }

        // Moonshot
        ProviderName::Moonshot => {
            profile.moonshot_api_key.is_some() || profile.moonshot_base_url.is_some()
        }

        // ZAI
        ProviderName::Zai => profile.zai_api_key.is_some(),

        // Mistral
        ProviderName::Mistral => {
            profile.mistral_api_key.is_some() || profile.mistral_base_url.is_some()
        }

        // Fireworks
        ProviderName::Fireworks => {
            profile.fireworks_api_key.is_some() || profile.fireworks_base_url.is_some()
        }

        // SambaNova
        ProviderName::SambaNova => {
            profile.samba_nova_api_key.is_some() || profile.samba_nova_base_url.is_some()
        }

        // Baseten
        ProviderName::Baseten => {
            profile.baseten_api_key.is_some() || profile.baseten_base_url.is_some()
        }

        // Poe
        ProviderName::Poe => profile.poe_api_key.is_some(),

        // LiteLLM
        ProviderName::LiteLlm => {
            profile.litellm_api_key.is_some() || profile.litellm_base_url.is_some()
        }

        // Requesty
        ProviderName::Requesty => {
            profile.requesty_api_key.is_some() || profile.requesty_base_url.is_some()
        }

        // Unbound
        ProviderName::Unbound => {
            profile.unbound_api_key.is_some() || profile.unbound_base_url.is_some()
        }

        // Vercel AI Gateway
        ProviderName::VercelAiGateway => {
            profile.vercel_ai_gateway_api_key.is_some()
                || profile.vercel_api_key.is_some()
                || profile.vercel_base_url.is_some()
        }

        // Retired providers — always false (they no longer work)
        ProviderName::Cerebras
        | ProviderName::Chutes
        | ProviderName::Deepinfra
        | ProviderName::Doubao
        | ProviderName::Featherless
        | ProviderName::Groq
        | ProviderName::Huggingface
        | ProviderName::IoIntelligence => false,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_profile_allowed_allow_all() {
        let allow_list = OrganizationAllowList {
            allow_all: true,
            providers: HashMap::new(),
        };
        let profile = ProviderSettings {
            api_provider: Some(ProviderName::Anthropic),
            ..Default::default()
        };
        assert!(is_profile_allowed(&profile, &allow_list));
    }

    #[test]
    fn test_is_profile_allowed_specific_provider() {
        let mut providers = HashMap::new();
        providers.insert(
            "anthropic".to_string(),
            ProviderAllowList {
                allow_all: true,
                models: None,
            },
        );
        let allow_list = OrganizationAllowList {
            allow_all: false,
            providers,
        };
        let profile = ProviderSettings {
            api_provider: Some(ProviderName::Anthropic),
            ..Default::default()
        };
        assert!(is_profile_allowed(&profile, &allow_list));

        let profile2 = ProviderSettings {
            api_provider: Some(ProviderName::Openai),
            ..Default::default()
        };
        assert!(!is_profile_allowed(&profile2, &allow_list));
    }

    #[test]
    fn test_is_model_allowed() {
        let mut providers = HashMap::new();
        providers.insert(
            "anthropic".to_string(),
            ProviderAllowList {
                allow_all: false,
                models: Some(vec!["claude-3-5-sonnet".to_string()]),
            },
        );
        let allow_list = OrganizationAllowList {
            allow_all: false,
            providers,
        };
        assert!(is_model_allowed(
            ProviderName::Anthropic,
            "claude-3-5-sonnet",
            &allow_list
        ));
        assert!(!is_model_allowed(
            ProviderName::Anthropic,
            "claude-3-opus",
            &allow_list
        ));
    }

    #[test]
    fn test_get_model_id_anthropic() {
        let profile = ProviderSettings {
            api_provider: Some(ProviderName::Anthropic),
            api_model_id: Some("claude-3-5-sonnet".to_string()),
            ..Default::default()
        };
        assert_eq!(
            get_model_id_from_profile(&profile),
            Some("claude-3-5-sonnet".to_string())
        );
    }

    #[test]
    fn test_get_model_id_openrouter() {
        let profile = ProviderSettings {
            api_provider: Some(ProviderName::OpenRouter),
            open_router_model_id: Some("anthropic/claude-3".to_string()),
            ..Default::default()
        };
        assert_eq!(
            get_model_id_from_profile(&profile),
            Some("anthropic/claude-3".to_string())
        );
    }

    #[test]
    fn test_get_model_id_ollama() {
        let profile = ProviderSettings {
            api_provider: Some(ProviderName::Ollama),
            ollama_model_id: Some("llama3".to_string()),
            ..Default::default()
        };
        assert_eq!(
            get_model_id_from_profile(&profile),
            Some("llama3".to_string())
        );
    }

    #[test]
    fn test_get_model_id_no_provider() {
        let profile = ProviderSettings {
            api_provider: None,
            api_model_id: Some("whatever".to_string()),
            ..Default::default()
        };
        assert_eq!(get_model_id_from_profile(&profile), None);
    }

    #[test]
    fn test_check_exist_api_config_fake_ai() {
        let profile = ProviderSettings {
            api_provider: Some(ProviderName::FakeAi),
            ..Default::default()
        };
        assert!(check_exist_api_config(&profile));
    }

    #[test]
    fn test_check_exist_api_config_anthropic_with_key() {
        let profile = ProviderSettings {
            api_provider: Some(ProviderName::Anthropic),
            api_key: Some("sk-test".to_string()),
            ..Default::default()
        };
        assert!(check_exist_api_config(&profile));
    }

    #[test]
    fn test_check_exist_api_config_anthropic_without_key() {
        let profile = ProviderSettings {
            api_provider: Some(ProviderName::Anthropic),
            ..Default::default()
        };
        assert!(!check_exist_api_config(&profile));
    }

    #[test]
    fn test_check_exist_api_config_no_provider() {
        let profile = ProviderSettings::default();
        assert!(!check_exist_api_config(&profile));
    }

    #[test]
    fn test_check_exist_api_config_ollama_with_model() {
        let profile = ProviderSettings {
            api_provider: Some(ProviderName::Ollama),
            ollama_model_id: Some("llama3".to_string()),
            ..Default::default()
        };
        assert!(check_exist_api_config(&profile));
    }

    #[test]
    fn test_check_exist_api_config_bedrock_with_region() {
        let profile = ProviderSettings {
            api_provider: Some(ProviderName::Bedrock),
            aws_region: Some("us-east-1".to_string()),
            ..Default::default()
        };
        assert!(check_exist_api_config(&profile));
    }
}

//! API Protocol Detection
//!
//! Returns the API protocol ("anthropic" or "openai") for a given provider and model.
//! Mirrors `getApiProtocol.ts`.

use roo_types::api::ProviderName;

/// Returns the API protocol string for the given provider and optional model ID.
///
/// - `"anthropic"` for Anthropic-style providers (anthropic, bedrock, minimax),
///   Vertex with claude models, and Vercel/Roo with `anthropic/` prefix models.
/// - `"openai"` for everything else.
pub fn get_api_protocol(provider: ProviderName, model_id: Option<&str>) -> &'static str {
    match provider {
        // Always anthropic-style
        ProviderName::Anthropic | ProviderName::Bedrock | ProviderName::MiniMax => "anthropic",

        // Vertex uses anthropic protocol for claude models
        ProviderName::Vertex => {
            match model_id {
                Some(id) if id.starts_with("claude") => "anthropic",
                _ => "openai",
            }
        }

        // Roo / Vercel AI Gateway: anthropic protocol if model starts with "anthropic/"
        ProviderName::Roo | ProviderName::VercelAiGateway => {
            match model_id {
                Some(id) if id.starts_with("anthropic/") => "anthropic",
                _ => "openai",
            }
        }

        // All other providers use OpenAI protocol
        _ => "openai",
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_anthropic_provider() {
        assert_eq!(get_api_protocol(ProviderName::Anthropic, None), "anthropic");
        assert_eq!(
            get_api_protocol(ProviderName::Anthropic, Some("claude-3-5-sonnet")),
            "anthropic"
        );
    }

    #[test]
    fn test_bedrock_provider() {
        assert_eq!(get_api_protocol(ProviderName::Bedrock, None), "anthropic");
    }

    #[test]
    fn test_minimax_provider() {
        assert_eq!(get_api_protocol(ProviderName::MiniMax, None), "anthropic");
    }

    #[test]
    fn test_vertex_with_claude() {
        assert_eq!(
            get_api_protocol(ProviderName::Vertex, Some("claude-3-5-sonnet")),
            "anthropic"
        );
    }

    #[test]
    fn test_vertex_with_gemini() {
        assert_eq!(
            get_api_protocol(ProviderName::Vertex, Some("gemini-pro")),
            "openai"
        );
    }

    #[test]
    fn test_vertex_no_model() {
        assert_eq!(get_api_protocol(ProviderName::Vertex, None), "openai");
    }

    #[test]
    fn test_roo_with_anthropic_prefix() {
        assert_eq!(
            get_api_protocol(ProviderName::Roo, Some("anthropic/claude-3-5-sonnet")),
            "anthropic"
        );
    }

    #[test]
    fn test_roo_with_openai_model() {
        assert_eq!(
            get_api_protocol(ProviderName::Roo, Some("gpt-4")),
            "openai"
        );
    }

    #[test]
    fn test_vercel_with_anthropic_prefix() {
        assert_eq!(
            get_api_protocol(ProviderName::VercelAiGateway, Some("anthropic/claude-3")),
            "anthropic"
        );
    }

    #[test]
    fn test_openai_provider() {
        assert_eq!(get_api_protocol(ProviderName::Openai, None), "openai");
    }

    #[test]
    fn test_gemini_provider() {
        assert_eq!(get_api_protocol(ProviderName::Gemini, None), "openai");
    }

    #[test]
    fn test_ollama_provider() {
        assert_eq!(get_api_protocol(ProviderName::Ollama, Some("llama3")), "openai");
    }

    #[test]
    fn test_deepseek_provider() {
        assert_eq!(get_api_protocol(ProviderName::DeepSeek, None), "openai");
    }
}
//! OpenAI Codex provider handler.
//!
//! Uses the OpenAI **Responses API** routed through the Codex backend
//! (`https://chatgpt.com/backend-api/codex/responses`) with OAuth
//! Bearer token authentication. All models are subscription-based
//! (zero per-token cost).

use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::StreamExt;
use reqwest::Client;
use roo_provider::error::{ProviderError, Result};
use roo_provider::{ApiStream, CreateMessageMetadata, Provider};
use roo_types::api::ProviderName;
use roo_types::model::ModelInfo;
use uuid::Uuid;

use crate::models;
use crate::responses_api::{self, RequestBodyParams};
use crate::types::OpenAiCodexConfig;

/// OpenAI Codex API provider handler.
///
/// Key differences from [`crate::handler::OpenAiNativeHandler`]:
/// - Uses OAuth Bearer tokens instead of API keys
/// - Routes requests to Codex backend (`chatgpt.com/backend-api/codex`)
/// - Subscription-based pricing (no per-token costs)
/// - Custom headers: `originator: "roo-code"`, `ChatGPT-Account-Id`
/// - Omits `max_output_tokens`, `service_tier`, `prompt_cache_retention`
pub struct OpenAiCodexHandler {
    /// HTTP client for making API requests.
    http_client: Client,
    /// OAuth access token.
    access_token: String,
    /// ChatGPT account ID for organisation subscriptions.
    account_id: Option<String>,
    /// Resolved model ID.
    model_id: String,
    /// Model information.
    model_info: ModelInfo,
    /// Session ID for request tracking.
    session_id: String,
    /// Reasoning effort.
    reasoning_effort: Option<String>,
    /// Request timeout in milliseconds (used during client construction).
    #[allow(dead_code)]
    request_timeout: Option<u64>,
}

impl OpenAiCodexHandler {
    /// Create a new OpenAI Codex handler from configuration.
    pub fn new(config: OpenAiCodexConfig) -> Result<Self> {
        let model_id = config
            .model_id
            .unwrap_or_else(|| models::OPENAI_CODEX_DEFAULT_MODEL_ID.to_string());
        let all_models = models::openai_codex_models();
        let model_info = all_models
            .get(&model_id)
            .cloned()
            .unwrap_or_else(|| ModelInfo {
                max_tokens: Some(128_000),
                context_window: 400_000,
                supports_images: Some(true),
                input_price: Some(0.0),
                output_price: Some(0.0),
                description: Some("OpenAI Codex model (unknown variant)".to_string()),
                ..Default::default()
            });

        let http_client = Client::builder()
            .timeout(std::time::Duration::from_millis(
                config.request_timeout.unwrap_or(120_000),
            ))
            .build()
            .map_err(ProviderError::Reqwest)?;

        Ok(Self {
            http_client,
            access_token: config.access_token,
            account_id: config.account_id,
            model_id,
            model_info,
            session_id: Uuid::now_v7().to_string(),
            reasoning_effort: config.reasoning_effort,
            request_timeout: config.request_timeout,
        })
    }

    /// Create a new OpenAI Codex handler from provider settings.
    pub fn from_settings(
        settings: &roo_types::provider_settings::ProviderSettings,
    ) -> Result<Self> {
        let config =
            OpenAiCodexConfig::from_settings(settings).ok_or(ProviderError::ApiKeyRequired)?;
        Self::new(config)
    }

    /// Update the access token (e.g. after OAuth refresh).
    pub fn set_access_token(&mut self, token: String) {
        self.access_token = token;
    }

    /// Resolve the effective reasoning effort for the current model.
    fn get_reasoning_effort(&self) -> Option<String> {
        // First check if user explicitly set reasoning_effort
        if let Some(ref effort) = self.reasoning_effort {
            let s = effort.trim();
            if !s.is_empty() && s != "disable" && s != "none" {
                return Some(s.to_string());
            }
        }

        // Then check model_info.reasoning_effort
        self.model_info
            .reasoning_effort
            .map(|e| {
                serde_json::to_string(&e)
                    .unwrap_or_default()
                    .trim_matches('"')
                    .to_string()
            })
            .filter(|s| !s.is_empty() && s != "disable" && s != "none")
    }

    /// Build the full URL for the Codex Responses API endpoint.
    fn responses_url(&self) -> String {
        format!("{}/responses", OpenAiCodexConfig::CODEX_BASE_URL)
    }

    /// Execute a streaming Responses API request to the Codex backend.
    async fn execute_streaming_request(
        &self,
        body: &crate::types::ResponsesApiRequestBody,
        task_id: Option<&str>,
    ) -> Result<ApiStream> {
        let url = self.responses_url();
        let session_header = task_id
            .map(|s| s.to_string())
            .unwrap_or_else(|| self.session_id.clone());

        let body_json = serde_json::to_string(body).map_err(ProviderError::Json)?;

        let mut request = self
            .http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .header(
                "Authorization",
                format!("Bearer {}", self.access_token),
            )
            .header("originator", "roo-code")
            .header("session_id", session_header)
            .header("User-Agent", "roo-code/rust (codex-handler)");

        if let Some(ref account_id) = self.account_id {
            request = request.header("ChatGPT-Account-Id", account_id.as_str());
        }

        let response = request.body(body_json).send().await.map_err(|e| {
            ProviderError::ApiError(
                "OpenAI Codex".to_string(),
                format!("Failed to connect to Codex API: {}", e),
            )
        })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_text = response.text().await.unwrap_or_default();
            let error_msg = Self::format_error_message(status, &error_text);
            return Err(ProviderError::api_error_response(
                "OpenAI Codex",
                status,
                error_msg,
            ));
        }

        // Parse SSE stream using eventsource
        let provider_name = "OpenAI Codex".to_string();
        let stream = response
            .bytes_stream()
            .eventsource()
            .map(move |event| {
                match event {
                    Ok(event) => {
                        if event.data == "[DONE]" {
                            return None;
                        }
                        match responses_api::parse_sse_event(&event.data, &provider_name) {
                            Ok(Some(chunk)) => Some(Ok(chunk)),
                            Ok(None) => None,
                            Err(e) => Some(Err(e)),
                        }
                    }
                    Err(e) => Some(Err(ProviderError::StreamError(format!(
                        "{}: SSE error: {}",
                        provider_name, e
                    )))),
                }
            })
            .filter_map(|item| async move { item });

        Ok(Box::pin(stream))
    }

    /// Format a user-friendly error message from HTTP status and body.
    fn format_error_message(status: u16, body: &str) -> String {
        let details = serde_json::from_str::<serde_json::Value>(body)
            .ok()
            .and_then(|v| {
                v["error"]["message"]
                    .as_str()
                    .map(|s| s.to_string())
                    .or_else(|| v["message"].as_str().map(|s| s.to_string()))
                    .or_else(|| v["detail"].as_str().map(|s| s.to_string()))
            })
            .unwrap_or_else(|| body.to_string());

        match status {
            400 => format!("Invalid request to Codex API - {}", details),
            401 => format!(
                "Authentication failed. Please sign in using the OpenAI Codex OAuth flow - {}",
                details
            ),
            403 => format!("Access denied - {}", details),
            404 => format!("Codex API endpoint not found - {}", details),
            429 => format!("Rate limit exceeded - {}", details),
            500 | 502 | 503 => format!("Codex service error - {}", details),
            _ => format!("Codex API error ({}) - {}", status, details),
        }
    }
}

#[async_trait]
impl Provider for OpenAiCodexHandler {
    async fn create_message(
        &self,
        system_prompt: &str,
        messages: Vec<roo_types::api::ApiMessage>,
        tools: Option<Vec<serde_json::Value>>,
        metadata: CreateMessageMetadata,
    ) -> Result<ApiStream> {
        let reasoning_effort = self.get_reasoning_effort();

        // Format conversation
        let formatted_input = responses_api::format_full_conversation(&messages);

        // Build request body — Codex omits max_output_tokens, service_tier, prompt_cache_retention
        let body = responses_api::build_request_body(RequestBodyParams {
            model_id: &self.model_id,
            formatted_input,
            system_prompt,
            tools: tools.as_deref(),
            tool_choice: metadata.tool_choice.as_ref(),
            parallel_tool_calls: metadata.parallel_tool_calls,
            reasoning_effort: reasoning_effort.as_deref(),
            enable_reasoning_summary: true,
            temperature: None,
            supports_temperature: Some(false),
            max_output_tokens: None,
            supports_verbosity: self.model_info.supports_verbosity,
            verbosity: None,
            service_tier: None,
            prompt_cache_retention: None,
            stream: true,
        });

        self.execute_streaming_request(&body, metadata.task_id.as_deref())
            .await
    }

    fn get_model(&self) -> (String, ModelInfo) {
        (self.model_id.clone(), self.model_info.clone())
    }

    async fn complete_prompt(&self, prompt: &str) -> Result<String> {
        let reasoning_effort = self.get_reasoning_effort();

        let body = responses_api::build_request_body(RequestBodyParams {
            model_id: &self.model_id,
            formatted_input: vec![serde_json::json!({
                "role": "user",
                "content": [{ "type": "input_text", "text": prompt }]
            })],
            system_prompt: "",
            tools: None,
            tool_choice: None,
            parallel_tool_calls: None,
            reasoning_effort: reasoning_effort.as_deref(),
            enable_reasoning_summary: true,
            temperature: None,
            supports_temperature: Some(false),
            max_output_tokens: None,
            supports_verbosity: self.model_info.supports_verbosity,
            verbosity: None,
            service_tier: None,
            prompt_cache_retention: None,
            stream: false,
        });

        let url = self.responses_url();
        let body_json = serde_json::to_string(&body).map_err(ProviderError::Json)?;

        let mut request = self
            .http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .header(
                "Authorization",
                format!("Bearer {}", self.access_token),
            )
            .header("originator", "roo-code")
            .header("session_id", &self.session_id);

        if let Some(ref account_id) = self.account_id {
            request = request.header("ChatGPT-Account-Id", account_id.as_str());
        }

        let response = request.body(body_json).send().await.map_err(|e| {
            ProviderError::ApiError(
                "OpenAI Codex".to_string(),
                format!("Failed to connect: {}", e),
            )
        })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_text = response.text().await.unwrap_or_default();
            return Err(ProviderError::api_error_response(
                "OpenAI Codex",
                status,
                Self::format_error_message(status, &error_text),
            ));
        }

        let response_data: serde_json::Value =
            response.json().await.map_err(ProviderError::Reqwest)?;

        // Extract text from the response
        if let Some(output) = response_data["output"].as_array() {
            for output_item in output {
                if output_item["type"] == "message" {
                    if let Some(content) = output_item["content"].as_array() {
                        for c in content {
                            if c["type"] == "output_text" || c["type"] == "text" {
                                if let Some(text) = c["text"].as_str() {
                                    return Ok(text.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(text) = response_data["text"].as_str() {
            return Ok(text.to_string());
        }

        Ok(String::new())
    }

    fn provider_name(&self) -> ProviderName {
        ProviderName::OpenaiCodex
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_creation_with_config() {
        let config = OpenAiCodexConfig {
            access_token: "oauth-token".to_string(),
            account_id: None,
            model_id: None,
            reasoning_effort: None,
            request_timeout: None,
        };
        let handler = OpenAiCodexHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_handler_uses_default_model() {
        let config = OpenAiCodexConfig {
            access_token: "oauth-token".to_string(),
            account_id: None,
            model_id: None,
            reasoning_effort: None,
            request_timeout: None,
        };
        let handler = OpenAiCodexHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, models::OPENAI_CODEX_DEFAULT_MODEL_ID);
    }

    #[test]
    fn test_handler_custom_model() {
        let config = OpenAiCodexConfig {
            access_token: "oauth-token".to_string(),
            account_id: None,
            model_id: Some("gpt-5.2-codex".to_string()),
            reasoning_effort: None,
            request_timeout: None,
        };
        let handler = OpenAiCodexHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, "gpt-5.2-codex");
    }

    #[test]
    fn test_handler_provider_name() {
        let config = OpenAiCodexConfig {
            access_token: "oauth-token".to_string(),
            account_id: None,
            model_id: None,
            reasoning_effort: None,
            request_timeout: None,
        };
        let handler = OpenAiCodexHandler::new(config).unwrap();
        assert_eq!(handler.provider_name(), ProviderName::OpenaiCodex);
    }

    #[test]
    fn test_handler_unknown_model_fallback() {
        let config = OpenAiCodexConfig {
            access_token: "oauth-token".to_string(),
            account_id: None,
            model_id: Some("gpt-future-codex".to_string()),
            reasoning_effort: None,
            request_timeout: None,
        };
        let handler = OpenAiCodexHandler::new(config).unwrap();
        let (model_id, info) = handler.get_model();
        assert_eq!(model_id, "gpt-future-codex");
        assert!(info.max_tokens.is_some());
    }

    #[test]
    fn test_responses_url() {
        let config = OpenAiCodexConfig {
            access_token: "oauth-token".to_string(),
            account_id: None,
            model_id: None,
            reasoning_effort: None,
            request_timeout: None,
        };
        let handler = OpenAiCodexHandler::new(config).unwrap();
        assert_eq!(
            handler.responses_url(),
            "https://chatgpt.com/backend-api/codex/responses"
        );
    }

    #[test]
    fn test_codex_models_are_free() {
        let config = OpenAiCodexConfig {
            access_token: "oauth-token".to_string(),
            account_id: None,
            model_id: None,
            reasoning_effort: None,
            request_timeout: None,
        };
        let handler = OpenAiCodexHandler::new(config).unwrap();
        let (_, info) = handler.get_model();
        assert_eq!(info.input_price, Some(0.0));
        assert_eq!(info.output_price, Some(0.0));
    }

    #[test]
    fn test_set_access_token() {
        let config = OpenAiCodexConfig {
            access_token: "old-token".to_string(),
            account_id: None,
            model_id: None,
            reasoning_effort: None,
            request_timeout: None,
        };
        let mut handler = OpenAiCodexHandler::new(config).unwrap();
        handler.set_access_token("new-token".to_string());
        assert_eq!(handler.access_token, "new-token");
    }

    #[test]
    fn test_format_error_message_401() {
        let msg = OpenAiCodexHandler::format_error_message(
            401,
            r#"{"error":{"message":"Invalid token"}}"#,
        );
        assert!(msg.contains("Authentication failed"));
    }

    #[test]
    fn test_format_error_message_403() {
        let msg = OpenAiCodexHandler::format_error_message(403, "Forbidden");
        assert!(msg.contains("Access denied"));
    }

    #[test]
    fn test_handler_with_account_id() {
        let config = OpenAiCodexConfig {
            access_token: "oauth-token".to_string(),
            account_id: Some("acct-123".to_string()),
            model_id: None,
            reasoning_effort: None,
            request_timeout: None,
        };
        let handler = OpenAiCodexHandler::new(config).unwrap();
        assert_eq!(handler.account_id, Some("acct-123".to_string()));
    }
}

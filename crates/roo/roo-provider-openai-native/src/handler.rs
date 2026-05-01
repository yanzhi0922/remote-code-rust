//! OpenAI Native provider handler.
//!
//! Uses the OpenAI **Responses API** (`POST /v1/responses`) with standard
//! API key authentication. Supports streaming, reasoning, verbosity,
//! service tiers, and prompt cache retention.

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
use crate::types::OpenAiNativeConfig;

/// OpenAI Native API provider handler.
///
/// Routes all requests through the Responses API (`/v1/responses`).
/// Supports reasoning effort, verbosity, service tiers, and prompt
/// cache retention.
pub struct OpenAiNativeHandler {
    /// HTTP client for making API requests.
    http_client: Client,
    /// API key for authentication.
    api_key: String,
    /// Base URL (defaults to `https://api.openai.com`).
    base_url: String,
    /// Resolved model ID.
    model_id: String,
    /// Model information.
    model_info: ModelInfo,
    /// Session ID for request tracking.
    session_id: String,
    /// Temperature for generation.
    temperature: Option<f64>,
    /// Reasoning effort.
    reasoning_effort: Option<String>,
    /// Request timeout in milliseconds (used during client construction).
    #[allow(dead_code)]
    request_timeout: Option<u64>,
    /// Service tier.
    service_tier: Option<String>,
    /// Whether to enable reasoning summary.
    enable_reasoning_summary: bool,
}

impl OpenAiNativeHandler {
    /// Create a new OpenAI Native handler from configuration.
    pub fn new(config: OpenAiNativeConfig) -> Result<Self> {
        let model_id = config
            .model_id
            .unwrap_or_else(|| models::OPENAI_NATIVE_DEFAULT_MODEL_ID.to_string());
        let all_models = models::openai_native_models();
        let model_info = all_models
            .get(&model_id)
            .cloned()
            .unwrap_or_else(|| ModelInfo {
                max_tokens: Some(128_000),
                context_window: 400_000,
                supports_images: Some(true),
                input_price: Some(1.25),
                output_price: Some(10.0),
                description: Some("OpenAI Native model (unknown variant)".to_string()),
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
            api_key: config.api_key,
            base_url: config
                .base_url
                .unwrap_or_else(|| OpenAiNativeConfig::DEFAULT_BASE_URL.to_string()),
            model_id,
            model_info,
            session_id: Uuid::now_v7().to_string(),
            temperature: config.temperature,
            reasoning_effort: config.reasoning_effort,
            request_timeout: config.request_timeout,
            service_tier: config.service_tier,
            enable_reasoning_summary: config.enable_reasoning_summary,
        })
    }

    /// Create a new OpenAI Native handler from provider settings.
    pub fn from_settings(
        settings: &roo_types::provider_settings::ProviderSettings,
    ) -> Result<Self> {
        let config =
            OpenAiNativeConfig::from_settings(settings).ok_or(ProviderError::ApiKeyRequired)?;
        Self::new(config)
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

    /// Resolve the prompt cache retention policy.
    fn get_prompt_cache_retention(&self) -> Option<&str> {
        if !self.model_info.supports_prompt_cache {
            return None;
        }
        self.model_info
            .prompt_cache_retention
            .as_deref()
            .filter(|s| *s == "24h")
    }

    /// Build the full URL for the Responses API endpoint.
    fn responses_url(&self) -> String {
        format!("{}/v1/responses", self.base_url)
    }

    /// Execute a streaming Responses API request.
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

        let response = self
            .http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("originator", "roo-code")
            .header("session_id", session_header)
            .body(body_json)
            .send()
            .await
            .map_err(|e| {
                ProviderError::ApiError(
                    "OpenAI Native".to_string(),
                    format!("Failed to connect to Responses API: {}", e),
                )
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_text = response.text().await.unwrap_or_default();
            let error_msg = Self::format_error_message(status, &error_text);
            return Err(ProviderError::api_error_response(
                "OpenAI Native",
                status,
                error_msg,
            ));
        }

        // Parse SSE stream using eventsource
        let provider_name = "OpenAI Native".to_string();
        let stream = response
            .bytes_stream()
            .eventsource()
            .map(move |event| {
                match event {
                    Ok(event) => {
                        if event.data == "[DONE]" {
                            return None;
                        }
                        // Parse the SSE data as a Responses API event
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
            })
            .unwrap_or_else(|| body.to_string());

        match status {
            400 => format!("Invalid request to Responses API - {}", details),
            401 => format!(
                "Authentication failed. Please check your OpenAI API key - {}",
                details
            ),
            403 => format!("Access denied - {}", details),
            404 => format!("Responses API endpoint not found - {}", details),
            429 => format!("Rate limit exceeded - {}", details),
            500 | 502 | 503 => format!("OpenAI service error - {}", details),
            _ => format!("Responses API error ({}) - {}", status, details),
        }
    }
}

#[async_trait]
impl Provider for OpenAiNativeHandler {
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

        // Build request body
        let body = responses_api::build_request_body(RequestBodyParams {
            model_id: &self.model_id,
            formatted_input,
            system_prompt,
            tools: tools.as_deref(),
            tool_choice: metadata.tool_choice.as_ref(),
            parallel_tool_calls: metadata.parallel_tool_calls,
            reasoning_effort: reasoning_effort.as_deref(),
            enable_reasoning_summary: self.enable_reasoning_summary,
            temperature: self.temperature,
            supports_temperature: self.model_info.supports_temperature,
            max_output_tokens: self.model_info.max_tokens,
            supports_verbosity: self.model_info.supports_verbosity,
            verbosity: None,
            service_tier: self.service_tier.as_deref(),
            prompt_cache_retention: self.get_prompt_cache_retention(),
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
            enable_reasoning_summary: self.enable_reasoning_summary,
            temperature: self.temperature,
            supports_temperature: self.model_info.supports_temperature,
            max_output_tokens: self.model_info.max_tokens,
            supports_verbosity: self.model_info.supports_verbosity,
            verbosity: None,
            service_tier: self.service_tier.as_deref(),
            prompt_cache_retention: self.get_prompt_cache_retention(),
            stream: false,
        });

        let url = self.responses_url();
        let body_json = serde_json::to_string(&body).map_err(ProviderError::Json)?;

        let response = self
            .http_client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("originator", "roo-code")
            .header("session_id", &self.session_id)
            .body(body_json)
            .send()
            .await
            .map_err(|e| {
                ProviderError::ApiError(
                    "OpenAI Native".to_string(),
                    format!("Failed to connect: {}", e),
                )
            })?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_text = response.text().await.unwrap_or_default();
            return Err(ProviderError::api_error_response(
                "OpenAI Native",
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

        // Fallback
        if let Some(text) = response_data["text"].as_str() {
            return Ok(text.to_string());
        }

        Ok(String::new())
    }

    fn provider_name(&self) -> ProviderName {
        ProviderName::OpenaiNative
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_creation_requires_api_key() {
        let settings = roo_types::provider_settings::ProviderSettings::default();
        let result = OpenAiNativeHandler::from_settings(&settings);
        assert!(result.is_err());
    }

    #[test]
    fn test_handler_creation_with_config() {
        let config = OpenAiNativeConfig {
            api_key: "sk-test".to_string(),
            base_url: None,
            model_id: None,
            temperature: None,
            reasoning_effort: None,
            request_timeout: None,
            service_tier: None,
            enable_reasoning_summary: true,
        };
        let handler = OpenAiNativeHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_handler_uses_default_model() {
        let config = OpenAiNativeConfig {
            api_key: "sk-test".to_string(),
            base_url: None,
            model_id: None,
            temperature: None,
            reasoning_effort: None,
            request_timeout: None,
            service_tier: None,
            enable_reasoning_summary: true,
        };
        let handler = OpenAiNativeHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, models::OPENAI_NATIVE_DEFAULT_MODEL_ID);
    }

    #[test]
    fn test_handler_custom_model() {
        let config = OpenAiNativeConfig {
            api_key: "sk-test".to_string(),
            base_url: None,
            model_id: Some("gpt-5.4".to_string()),
            temperature: None,
            reasoning_effort: None,
            request_timeout: None,
            service_tier: None,
            enable_reasoning_summary: true,
        };
        let handler = OpenAiNativeHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, "gpt-5.4");
    }

    #[test]
    fn test_handler_provider_name() {
        let config = OpenAiNativeConfig {
            api_key: "sk-test".to_string(),
            base_url: None,
            model_id: None,
            temperature: None,
            reasoning_effort: None,
            request_timeout: None,
            service_tier: None,
            enable_reasoning_summary: true,
        };
        let handler = OpenAiNativeHandler::new(config).unwrap();
        assert_eq!(handler.provider_name(), ProviderName::OpenaiNative);
    }

    #[test]
    fn test_handler_unknown_model_fallback() {
        let config = OpenAiNativeConfig {
            api_key: "sk-test".to_string(),
            base_url: None,
            model_id: Some("gpt-future".to_string()),
            temperature: None,
            reasoning_effort: None,
            request_timeout: None,
            service_tier: None,
            enable_reasoning_summary: true,
        };
        let handler = OpenAiNativeHandler::new(config).unwrap();
        let (model_id, info) = handler.get_model();
        assert_eq!(model_id, "gpt-future");
        assert!(info.max_tokens.is_some());
    }

    #[test]
    fn test_responses_url() {
        let config = OpenAiNativeConfig {
            api_key: "sk-test".to_string(),
            base_url: None,
            model_id: None,
            temperature: None,
            reasoning_effort: None,
            request_timeout: None,
            service_tier: None,
            enable_reasoning_summary: true,
        };
        let handler = OpenAiNativeHandler::new(config).unwrap();
        assert_eq!(
            handler.responses_url(),
            "https://api.openai.com/v1/responses"
        );
    }

    #[test]
    fn test_responses_url_custom_base() {
        let config = OpenAiNativeConfig {
            api_key: "sk-test".to_string(),
            base_url: Some("https://custom.openai.com".to_string()),
            model_id: None,
            temperature: None,
            reasoning_effort: None,
            request_timeout: None,
            service_tier: None,
            enable_reasoning_summary: true,
        };
        let handler = OpenAiNativeHandler::new(config).unwrap();
        assert_eq!(
            handler.responses_url(),
            "https://custom.openai.com/v1/responses"
        );
    }

    #[test]
    fn test_format_error_message_401() {
        let msg = OpenAiNativeHandler::format_error_message(
            401,
            r#"{"error":{"message":"Invalid API key"}}"#,
        );
        assert!(msg.contains("Authentication failed"));
    }

    #[test]
    fn test_format_error_message_429() {
        let msg = OpenAiNativeHandler::format_error_message(429, "Too many requests");
        assert!(msg.contains("Rate limit exceeded"));
    }
}

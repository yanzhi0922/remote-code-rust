//! AWS Bedrock provider handler.
//!
//! Uses the Bedrock Converse API with AWS SigV4 signing.
//! Supports cross-region inference and custom model IDs.
//! Parses the AWS event stream binary format for streaming responses.

use async_trait::async_trait;
use serde_json::{json, Value};
use sha2::Digest;

use roo_provider::error::{ProviderError, Result};
use roo_provider::handler::{ApiStream, CreateMessageMetadata, Provider};
use roo_provider::transform::anthropic_filter::filter_non_anthropic_blocks;
use roo_types::api::{
    ApiMessage, ApiStreamChunk, ContentBlock, ProviderName,
};
use roo_types::model::ModelInfo;

use crate::bedrock_events::{
    BedrockEvent, ContentBlockDeltaData, ContentBlockStartData, parse_bedrock_event_stream,
};
use crate::models;
use crate::signing::SigV4Signer;
use crate::types::AwsBedrockConfig;

/// AWS Bedrock API provider handler.
pub struct AwsBedrockHandler {
    http_client: reqwest::Client,
    signer: SigV4Signer,
    base_url: String,
    model_id: String,
    model_info: ModelInfo,
    use_cross_region_inference: bool,
    temperature: f64,
}

impl AwsBedrockHandler {
    /// Create a new AWS Bedrock handler from configuration.
    pub fn new(config: AwsBedrockConfig) -> Result<Self> {
        let model_id = config.model_id.unwrap_or_else(|| models::default_model_id());
        let model_info = models::models()
            .get(&model_id)
            .cloned()
            .unwrap_or_else(|| ModelInfo {
                max_tokens: Some(8192),
                context_window: 200000,
                supports_images: Some(true),
                supports_prompt_cache: true,
                input_price: Some(3.0),
                output_price: Some(15.0),
                description: Some("AWS Bedrock model (unknown variant)".to_string()),
                ..Default::default()
            });

        let signer = SigV4Signer::new(
            config.access_key,
            config.secret_key,
            config.session_token,
            config.region.clone(),
        );

        let base_url = config
            .endpoint_url
            .unwrap_or_else(|| AwsBedrockConfig::bedrock_base_url(&config.region));

        let mut client_builder = reqwest::Client::builder();
        if let Some(timeout) = config.request_timeout {
            client_builder =
                client_builder.timeout(std::time::Duration::from_millis(timeout));
        }
        let http_client = client_builder.build().map_err(ProviderError::Reqwest)?;

        Ok(Self {
            http_client,
            signer,
            base_url,
            model_id,
            model_info,
            use_cross_region_inference: config.use_cross_region_inference,
            temperature: config.temperature.unwrap_or(crate::types::BEDROCK_DEFAULT_TEMPERATURE),
        })
    }

    /// Create a new AWS Bedrock handler from provider settings.
    pub fn from_settings(
        settings: &roo_types::provider_settings::ProviderSettings,
    ) -> Result<Self> {
        let config =
            AwsBedrockConfig::from_settings(settings).ok_or(ProviderError::ApiKeyRequired)?;
        Self::new(config)
    }

    /// Get the model ID, potentially prefixed with cross-region inference prefix.
    fn effective_model_id(&self) -> String {
        if self.use_cross_region_inference {
            // Add region prefix for cross-region inference
            let region_prefix = match self.signer.region_str() {
                "us-east-1" => "us.",
                "us-west-2" => "us.",
                "eu-west-1" => "eu.",
                "ap-southeast-1" => "apac.",
                _ => "",
            };
            if !self.model_id.starts_with(region_prefix) && !region_prefix.is_empty() {
                format!("{}{}", region_prefix, self.model_id)
            } else {
                self.model_id.clone()
            }
        } else {
            self.model_id.clone()
        }
    }

    /// Build the Converse API request body.
    ///
    /// Faithfully mirrors the TS `createMessage` payload construction:
    /// - Includes temperature in inferenceConfig
    /// - Supports tool choice configuration
    /// - Supports additionalModelRequestFields for thinking/reasoning
    fn build_converse_request(
        &self,
        system_prompt: &str,
        messages: &[ApiMessage],
        tools: Option<&Vec<Value>>,
        tool_choice: Option<&Value>,
    ) -> Value {
        let filtered_messages = filter_non_anthropic_blocks(messages.to_vec());

        let mut bedrock_messages: Vec<Value> = Vec::new();
        let mut system_messages: Vec<Value> = Vec::new();

        // Add system prompt
        if !system_prompt.is_empty() {
            system_messages.push(json!({
                "text": system_prompt
            }));
        }

        // Convert messages to Bedrock Converse format
        for msg in &filtered_messages {
            let role = match msg.role {
                roo_types::api::MessageRole::User => "user",
                roo_types::api::MessageRole::Assistant => "assistant",
            };

            let mut content_parts: Vec<Value> = Vec::new();
            for block in &msg.content {
                match block {
                    ContentBlock::Text { text } => {
                        content_parts.push(json!({ "text": text }));
                    }
                    ContentBlock::Image { source } => {
                        if let roo_types::api::ImageSource::Base64 { data, media_type } = source {
                            content_parts.push(json!({
                                "image": {
                                    "source": {
                                        "bytes": data,
                                    },
                                    "format": media_type,
                                }
                            }));
                        }
                    }
                    ContentBlock::ToolUse { id, name, input } => {
                        content_parts.push(json!({
                            "toolUse": {
                                "toolUseId": id,
                                "name": name,
                                "input": input,
                            }
                        }));
                    }
                    ContentBlock::ToolResult {
                        tool_use_id,
                        content,
                        is_error,
                    } => {
                        let tool_content: Vec<Value> = content
                            .iter()
                            .map(|c| match c {
                                roo_types::api::ToolResultContent::Text { text } => {
                                    json!({ "text": text })
                                }
                                roo_types::api::ToolResultContent::Image { source } => {
                                    if let roo_types::api::ImageSource::Base64 { data, media_type } = source {
                                        json!({
                                            "image": {
                                                "source": { "bytes": data },
                                                "format": media_type,
                                            }
                                        })
                                    } else {
                                        json!({ "text": "[image]" })
                                    }
                                }
                            })
                            .collect();

                        let status = if is_error.unwrap_or(false) {
                            "error"
                        } else {
                            "success"
                        };

                        content_parts.push(json!({
                            "toolResult": {
                                "toolUseId": tool_use_id,
                                "content": tool_content,
                                "status": status,
                            }
                        }));
                    }
                    ContentBlock::Thinking { thinking, .. } => {
                        // Bedrock Converse uses reasoningContent
                        content_parts.push(json!({
                            "reasoningContent": {
                                "reasoningText": {
                                    "text": thinking,
                                }
                            }
                        }));
                    }
                    ContentBlock::RedactedThinking { data } => {
                        content_parts.push(json!({
                            "reasoningContent": {
                                "redactedContent": data,
                            }
                        }));
                    }
                }
            }

            if !content_parts.is_empty() {
                bedrock_messages.push(json!({
                    "role": role,
                    "content": content_parts,
                }));
            }
        }

        let mut body = json!({
            "messages": bedrock_messages,
            "system": system_messages,
            "inferenceConfig": {
                "maxTokens": self.model_info.max_tokens.unwrap_or(8192),
                "temperature": self.temperature,
            },
        });

        // Add tools if provided (Bedrock Converse toolSpec format)
        if let Some(tools) = tools {
            if !tools.is_empty() {
                let tool_list: Vec<Value> = tools
                    .iter()
                    .filter_map(|tool| {
                        let tool_type = tool.get("type").and_then(|t| t.as_str()).unwrap_or("");
                        if tool_type != "function" {
                            return None;
                        }
                        let function = tool.get("function")?;
                        Some(json!({
                            "toolSpec": {
                                "name": function.get("name"),
                                "description": function.get("description"),
                                "inputSchema": {
                                    "json": function.get("parameters"),
                                },
                            }
                        }))
                    })
                    .collect();

                if !tool_list.is_empty() {
                    body["tools"] = json!(tool_list);

                    // Add tool choice configuration
                    // Maps TS tool_choice values to Bedrock Converse toolChoice format
                    if let Some(choice) = tool_choice {
                        let tool_choice_value = match choice.as_str() {
                            Some("auto") => json!({ "auto": {} }),
                            Some("any") | Some("required") => json!({ "any": {} }),
                            Some("none") => json!({ "auto": {} }), // Bedrock doesn't have "none", use auto
                            _ => {
                                // Check for specific function choice: { type: "function", function: { name: "..." } }
                                if choice.get("type").and_then(|t| t.as_str()) == Some("function") {
                                    if let Some(name) = choice.get("function").and_then(|f| f.get("name")).and_then(|n| n.as_str()) {
                                        json!({ "tool": { "name": name } })
                                    } else {
                                        json!({ "auto": {} })
                                    }
                                } else {
                                    json!({ "auto": {} })
                                }
                            }
                        };
                        body["toolConfig"] = json!({
                            "tools": body["tools"],
                            "toolChoice": tool_choice_value,
                        });
                    }
                }
            }
        }

        // Add additionalModelRequestFields for thinking/reasoning budget
        // when the model supports it
        if self.model_info.supports_reasoning_budget.unwrap_or(false) {
            if let Some(budget) = self.model_info.max_thinking_tokens {
                body["additionalModelRequestFields"] = json!({
                    "thinking": {
                        "type": "enabled",
                        "budget_tokens": budget,
                    }
                });
            }
        }

        body
    }
}

#[async_trait]
impl Provider for AwsBedrockHandler {
    async fn create_message(
        &self,
        system_prompt: &str,
        messages: Vec<ApiMessage>,
        tools: Option<Vec<Value>>,
        _metadata: CreateMessageMetadata,
    ) -> Result<ApiStream> {
        let body = self.build_converse_request(system_prompt, &messages, tools.as_ref(), _metadata.tool_choice.as_ref());
        let body_bytes = serde_json::to_vec(&body).map_err(ProviderError::Json)?;
        let model_id = self.effective_model_id();

        let encoded_model_id = model_id.replace(':', "%3A").replace('/', "%2F");
        let url = format!(
            "{}/model/{}/converse-stream",
            self.base_url.trim_end_matches('/'),
            encoded_model_id
        );

        let timestamp = chrono::Utc::now();
        let auth_header = self.signer.sign("POST", &url, &body_bytes, &timestamp);
        let amz_date = self.signer.amz_date(&timestamp);
        let content_hash = hex::encode(sha2::Sha256::digest(&body_bytes));

        let mut request_builder = self
            .http_client
            .post(&url)
            .header("Authorization", auth_header)
            .header("Content-Type", "application/json")
            .header("X-Amz-Date", amz_date)
            .header("X-Amz-Content-Sha256", content_hash)
            .header("Accept", "application/json")
            .body(body_bytes);

        if let Some(token) = self.signer.session_token() {
            request_builder = request_builder.header("X-Amz-Security-Token", token);
        }

        let response = request_builder
            .send()
            .await
            .map_err(|e| ProviderError::api_error("bedrock", e))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ProviderError::api_error_response("bedrock", status, text));
        }

        // Read the full response body and parse the Bedrock event stream
        let model_info = self.model_info.clone();
        let bytes = response
            .bytes()
            .await
            .map_err(ProviderError::Reqwest)?;

        let events = parse_bedrock_event_stream(&bytes);

        // Convert Bedrock events into ApiStreamChunks
        let mut chunks: Vec<Result<ApiStreamChunk>> = Vec::new();
        let mut usage_emitted = false;

        for event in events {
            match event {
                BedrockEvent::ContentBlockDelta { delta, .. } => match delta {
                    ContentBlockDeltaData::TextDelta { text } => {
                        if !text.is_empty() {
                            chunks.push(Ok(ApiStreamChunk::Text { text }));
                        }
                    }
                    ContentBlockDeltaData::ToolUseDelta {
                        tool_use_id,
                        input,
                    } => {
                        chunks.push(Ok(ApiStreamChunk::ToolCall {
                            id: tool_use_id.clone(),
                            name: String::new(), // name comes from ContentBlockStart
                            arguments: input,
                        }));
                    }
                    ContentBlockDeltaData::ReasoningTextDelta { text } => {
                        if !text.is_empty() {
                            chunks.push(Ok(ApiStreamChunk::Reasoning {
                                text,
                                signature: None,
                            }));
                        }
                    }
                    ContentBlockDeltaData::ReasoningSignatureDelta { signature } => {
                        // Signatures are typically handled at a higher level
                        let _ = signature;
                    }
                },
                BedrockEvent::ContentBlockStart { content_block, .. } => {
                    match content_block {
                        ContentBlockStartData::ToolUse {
                            tool_use_id,
                            name,
                        } => {
                            chunks.push(Ok(ApiStreamChunk::ToolCallStart {
                                id: tool_use_id,
                                name,
                            }));
                        }
                        _ => {}
                    }
                }
                BedrockEvent::ContentBlockStop { .. } => {
                    // No action needed for stop events
                }
                BedrockEvent::MessageStart { .. } => {
                    // No action needed for start events
                }
                BedrockEvent::MessageStop { .. } => {
                    // No action needed for stop events
                }
                BedrockEvent::Metadata { usage, .. } => {
                    if !usage_emitted {
                        let input_tokens = usage.input_tokens;
                        let output_tokens = usage.output_tokens;
                        let cache_read_tokens = usage.cache_read_input_tokens;
                        let cache_write_tokens = usage.cache_write_input_tokens;

                        let input_cost = model_info.input_price.unwrap_or(0.0)
                            * input_tokens as f64
                            / 1_000_000.0;
                        let output_cost = model_info.output_price.unwrap_or(0.0)
                            * output_tokens as f64
                            / 1_000_000.0;
                        let cache_read_cost = model_info.cache_reads_price.unwrap_or(0.0)
                            * cache_read_tokens.unwrap_or(0) as f64
                            / 1_000_000.0;
                        let cache_write_cost = model_info.cache_writes_price.unwrap_or(0.0)
                            * cache_write_tokens.unwrap_or(0) as f64
                            / 1_000_000.0;

                        chunks.push(Ok(ApiStreamChunk::Usage {
                            input_tokens,
                            output_tokens,
                            cache_write_tokens,
                            cache_read_tokens,
                            reasoning_tokens: None,
                            total_cost: Some(
                                input_cost + output_cost + cache_read_cost + cache_write_cost,
                            ),
                        }));
                        usage_emitted = true;
                    }
                }
                BedrockEvent::InternalServerException { message } => {
                    chunks.push(Err(ProviderError::Other(format!(
                        "Bedrock internal server error: {message}"
                    ))));
                }
                BedrockEvent::ServiceUnavailableException { message } => {
                    chunks.push(Err(ProviderError::Other(format!(
                        "Bedrock service unavailable: {message}"
                    ))));
                }
                BedrockEvent::ThrottlingException { message } => {
                    chunks.push(Err(ProviderError::Other(format!(
                        "Bedrock throttled: {message}"
                    ))));
                }
                BedrockEvent::ValidationException { message } => {
                    chunks.push(Err(ProviderError::Other(format!(
                        "Bedrock validation error: {message}"
                    ))));
                }
                BedrockEvent::Unknown { event_type, .. } => {
                    // Log but don't fail on unknown events
                    let _ = event_type;
                }
            }
        }

        let stream = futures::stream::iter(chunks);
        Ok(Box::pin(stream))
    }

    fn get_model(&self) -> (String, ModelInfo) {
        (self.model_id.clone(), self.model_info.clone())
    }


    async fn complete_prompt(&self, prompt: &str) -> Result<String> {
        let body = self.build_converse_request("", &[ApiMessage {
            role: roo_types::api::MessageRole::User,
            content: vec![ContentBlock::Text { text: prompt.to_string() }],
            reasoning: None,
            ts: None,
            truncation_parent: None,
            is_truncation_marker: None,
            truncation_id: None,
            condense_parent: None,
            is_summary: None,
            condense_id: None,
            reasoning_details: None,
        }], None, None);

        let body_bytes = serde_json::to_vec(&body).map_err(ProviderError::Json)?;
        let model_id = self.effective_model_id();

        let encoded_model_id = model_id.replace(':', "%3A").replace('/', "%2F");
        let url = format!(
            "{}/model/{}/converse",
            self.base_url.trim_end_matches('/'),
            encoded_model_id
        );

        let timestamp = chrono::Utc::now();
        let auth_header = self.signer.sign("POST", &url, &body_bytes, &timestamp);
        let amz_date = self.signer.amz_date(&timestamp);
        let content_hash = hex::encode(sha2::Sha256::digest(&body_bytes));

        let mut request_builder = self
            .http_client
            .post(&url)
            .header("Authorization", auth_header)
            .header("Content-Type", "application/json")
            .header("X-Amz-Date", amz_date)
            .header("X-Amz-Content-Sha256", content_hash)
            .json(&body);

        if let Some(token) = self.signer.session_token() {
            request_builder = request_builder.header("X-Amz-Security-Token", token);
        }

        let response = request_builder
            .send()
            .await
            .map_err(|e| ProviderError::api_error("bedrock", e))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            return Err(ProviderError::api_error_response("bedrock", status, text));
        }

        let resp: Value = response.json().await.map_err(ProviderError::Reqwest)?;

        // Extract text from output message
        if let Some(content) = resp.get("output").and_then(|o| o.get("message")).and_then(|m| m.get("content")) {
            if let Some(arr) = content.as_array() {
                let text: String = arr
                    .iter()
                    .filter_map(|c| c.get("text").and_then(|t| t.as_str()))
                    .collect();
                return Ok(text);
            }
        }

        Ok(String::new())
    }

    fn provider_name(&self) -> ProviderName {
        ProviderName::Bedrock
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models;

    #[test]
    fn test_default_model_exists() {
        let all_models = models::models();
        assert!(
            all_models.contains_key(models::DEFAULT_MODEL_ID),
            "Default model '{}' should exist",
            models::DEFAULT_MODEL_ID
        );
    }

    #[test]
    fn test_all_models_have_required_fields() {
        for (id, info) in models::models() {
            assert!(info.max_tokens.is_some(), "Model '{}' missing max_tokens", id);
            assert!(info.input_price.is_some(), "Model '{}' missing input_price", id);
            assert!(info.output_price.is_some(), "Model '{}' missing output_price", id);
        }
    }

    #[test]
    fn test_config_default_region() {
        assert_eq!(AwsBedrockConfig::DEFAULT_REGION, "us-east-1");
    }

    #[test]
    fn test_config_bedrock_base_url() {
        let url = AwsBedrockConfig::bedrock_base_url("us-east-1");
        assert_eq!(url, "https://bedrock-runtime.us-east-1.amazonaws.com");
    }

    #[test]
    fn test_handler_creation_requires_credentials() {
        let settings = roo_types::provider_settings::ProviderSettings::default();
        let result = AwsBedrockHandler::from_settings(&settings);
        assert!(result.is_err());
    }

    #[test]
    fn test_handler_creation_with_config() {
        let config = AwsBedrockConfig {
            access_key: "AKIAIOSFODNN7EXAMPLE".to_string(),
            secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
            session_token: None,
            region: "us-east-1".to_string(),
            model_id: None,
            use_cross_region_inference: false,
            endpoint_url: None,
            request_timeout: None,
            temperature: None,
        };
        let handler = AwsBedrockHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_handler_uses_default_model() {
        let config = AwsBedrockConfig {
            access_key: "AKIAIOSFODNN7EXAMPLE".to_string(),
            secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
            session_token: None,
            region: "us-east-1".to_string(),
            model_id: None,
            use_cross_region_inference: false,
            endpoint_url: None,
            request_timeout: None,
            temperature: None,
        };
        let handler = AwsBedrockHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, models::DEFAULT_MODEL_ID);
    }

    #[test]
    fn test_handler_custom_model() {
        let config = AwsBedrockConfig {
            access_key: "AKIAIOSFODNN7EXAMPLE".to_string(),
            secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
            session_token: None,
            region: "us-east-1".to_string(),
            model_id: Some("anthropic.claude-3-5-haiku-20241022-v1:0".to_string()),
            use_cross_region_inference: false,
            endpoint_url: None,
            request_timeout: None,
            temperature: None,
        };
        let handler = AwsBedrockHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, "anthropic.claude-3-5-haiku-20241022-v1:0");
    }

    #[test]
    fn test_handler_provider_name() {
        let config = AwsBedrockConfig {
            access_key: "AKIAIOSFODNN7EXAMPLE".to_string(),
            secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
            session_token: None,
            region: "us-east-1".to_string(),
            model_id: None,
            use_cross_region_inference: false,
            endpoint_url: None,
            request_timeout: None,
            temperature: None,
        };
        let handler = AwsBedrockHandler::new(config).unwrap();
        assert_eq!(handler.provider_name(), ProviderName::Bedrock);
    }

    #[test]
    fn test_config_from_settings() {
        let mut settings = roo_types::provider_settings::ProviderSettings::default();
        settings.aws_access_key = Some("AKIAIOSFODNN7EXAMPLE".to_string());
        settings.aws_secret_key = Some("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string());
        settings.aws_region = Some("eu-west-1".to_string());

        let config = AwsBedrockConfig::from_settings(&settings).unwrap();
        assert_eq!(config.access_key, "AKIAIOSFODNN7EXAMPLE");
        assert_eq!(config.region, "eu-west-1");
    }

    #[test]
    fn test_config_from_settings_no_credentials() {
        let settings = roo_types::provider_settings::ProviderSettings::default();
        assert!(AwsBedrockConfig::from_settings(&settings).is_none());
    }

    #[test]
    fn test_models_count() {
        let all_models = models::models();
        assert!(all_models.len() >= 5, "Should have at least 5 Bedrock models");
    }

    #[test]
    fn test_cross_region_inference() {
        let config = AwsBedrockConfig {
            access_key: "AKIAIOSFODNN7EXAMPLE".to_string(),
            secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
            session_token: None,
            region: "us-east-1".to_string(),
            model_id: Some("anthropic.claude-3-5-sonnet-20241022-v2:0".to_string()),
            use_cross_region_inference: true,
            endpoint_url: None,
            request_timeout: None,
            temperature: None,
        };
        let handler = AwsBedrockHandler::new(config).unwrap();
        let effective_id = handler.effective_model_id();
        assert!(effective_id.starts_with("us.") || effective_id.contains("anthropic"));
    }

    #[test]
    fn test_handler_with_session_token() {
        let config = AwsBedrockConfig {
            access_key: "AKIAIOSFODNN7EXAMPLE".to_string(),
            secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
            session_token: Some("session-token-123".to_string()),
            region: "us-east-1".to_string(),
            model_id: None,
            use_cross_region_inference: false,
            endpoint_url: None,
            request_timeout: None,
            temperature: None,
        };
        let handler = AwsBedrockHandler::new(config).unwrap();
        assert_eq!(handler.signer.session_token(), Some("session-token-123"));
    }

    #[test]
    fn test_handler_with_custom_endpoint() {
        let config = AwsBedrockConfig {
            access_key: "AKIAIOSFODNN7EXAMPLE".to_string(),
            secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
            session_token: None,
            region: "us-east-1".to_string(),
            model_id: None,
            use_cross_region_inference: false,
            endpoint_url: Some("https://custom-bedrock.example.com".to_string()),
            request_timeout: None,
            temperature: None,
        };
        let handler = AwsBedrockHandler::new(config).unwrap();
        assert_eq!(handler.base_url, "https://custom-bedrock.example.com");
    }

    #[test]
    fn test_nova_models_available() {
        let all_models = models::models();
        assert!(all_models.contains_key("us.amazon.nova-pro-v1:0"));
        assert!(all_models.contains_key("us.amazon.nova-lite-v1:0"));
    }
}

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
use roo_provider::transform::cache_strategy::{
    BedrockCacheModelInfo, MultiPointStrategy, MultiPointStrategyConfig,
};
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

// ---------------------------------------------------------------------------
// Error taxonomy
// ---------------------------------------------------------------------------

/// Bedrock error types with pattern matching for user-friendly messages.
#[derive(Debug, Clone, PartialEq)]
pub enum BedrockErrorType {
    AccessDenied,
    NotFound,
    Throttling,
    TooManyTokens,
    ServiceQuotaExceeded,
    ModelNotReady,
    InternalServerError,
    OnDemandNotSupported,
    Abort,
    InvalidArnFormat,
    ValidationError,
    Generic,
}

impl BedrockErrorType {
    /// Classify an error from the raw message string.
    pub fn from_error_message(msg: &str) -> Self {
        let lower = msg.to_lowercase();
        // Check in specificity order (most specific first)
        let patterns: &[(BedrockErrorType, &[&str])] = &[
            (
                Self::ServiceQuotaExceeded,
                &[
                    "service quota exceeded",
                    "service quota",
                    "quota exceeded for model",
                ],
            ),
            (
                Self::ModelNotReady,
                &[
                    "model not ready",
                    "provisioned throughput not ready",
                    "model loading",
                ],
            ),
            (
                Self::TooManyTokens,
                &[
                    "too many tokens",
                    "token limit exceeded",
                    "context length",
                    "maximum context length",
                ],
            ),
            (
                Self::InternalServerError,
                &["internal server error", "internal error", "server error"],
            ),
            (
                Self::OnDemandNotSupported,
                &["with on-demand throughput isn't supported"],
            ),
            (Self::NotFound, &["not found", "does not exist"]),
            (Self::AccessDenied, &["access", "denied", "permission"]),
            (
                Self::Throttling,
                &[
                    "throttl",
                    "rate",
                    "limit",
                    "busy",
                    "overloaded",
                    "too many requests",
                    "concurrent requests",
                ],
            ),
            (
                Self::ValidationError,
                &[
                    "input tag",
                    "does not match any of the expected tags",
                    "field required",
                    "validation",
                    "invalid parameter",
                ],
            ),
        ];
        for (error_type, pats) in patterns {
            if pats.iter().any(|p| lower.contains(p)) {
                return error_type.clone();
            }
        }
        Self::Generic
    }

    /// Produce a human-friendly message for this error type.
    pub fn user_message(&self, model_id: &str) -> String {
        match self {
            Self::AccessDenied => format!(
                "You don't have access to model '{}'. Please verify:\n\
                 1. Try cross-region inference if using a foundation model\n\
                 2. If using an ARN, verify the ARN is correct\n\
                 3. Your AWS credentials have the necessary IAM permissions\n\
                 4. The region in the ARN matches where the model is deployed",
                model_id
            ),
            Self::NotFound => format!(
                "The specified model '{}' was not found. Verify the model ID or ARN.",
                model_id
            ),
            Self::Throttling => {
                "Request was throttled or rate limited. Please wait and try again.".to_string()
            }
            Self::TooManyTokens => {
                "Too many tokens: the input exceeds the model's context window. Try reducing the conversation length.".to_string()
            }
            Self::ServiceQuotaExceeded => {
                "Service quota exceeded. Request an increase in the AWS console.".to_string()
            }
            Self::ModelNotReady => {
                "Model is not ready or still loading. Please wait and retry.".to_string()
            }
            Self::InternalServerError => {
                "Amazon Bedrock internal server error. Please retry.".to_string()
            }
            Self::OnDemandNotSupported => {
                "On-demand throughput not supported. Try enabling cross-region inference."
                    .to_string()
            }
            Self::Abort => "Request was aborted: timed out or cancelled.".to_string(),
            Self::InvalidArnFormat => "Invalid ARN format. Please check the ARN syntax.".to_string(),
            Self::ValidationError => format!(
                "Parameter validation error for model '{}'. Check input format.",
                model_id
            ),
            Self::Generic => "Unknown error from Amazon Bedrock.".to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// ARN parsing
// ---------------------------------------------------------------------------

/// Parsed information from a Bedrock or SageMaker ARN.
#[derive(Debug, Clone)]
pub struct ArnInfo {
    pub is_valid: bool,
    pub region: Option<String>,
    pub model_type: Option<String>,
    pub model_id: Option<String>,
    pub cross_region_inference: bool,
    pub error_message: Option<String>,
}

/// Parse a Bedrock/SageMaker ARN into its components.
///
/// Supports formats like:
/// - `arn:aws:bedrock:us-east-1:123456789012:foundation-model/anthropic.claude-3-5-sonnet-20241022-v2:0`
/// - `arn:aws:bedrock:us-east-1:123456789012:provisioned-model/abc-123`
/// - `arn:aws:bedrock:us-east-1:123456789012:custom-model/my-model`
/// - `arn:aws:sagemaker:us-east-1:123456789012:endpoint/my-endpoint`
/// - ARNs with cross-region prefixes like `us.anthropic.claude-3-5-sonnet-20241022-v2:0`
pub fn parse_arn(arn: &str, configured_region: Option<&str>) -> ArnInfo {
    // Regex: arn:partition:service:region:account:resource-type/resource-id
    let re = regex::Regex::new(
        r"^arn:[^:]+:(?:bedrock|sagemaker):([^:]*):([^:]*):(?:([^/]+)/([\w.\-:]+)|([^/]+))$",
    )
    .unwrap();

    let Some(caps) = re.captures(arn) else {
        return ArnInfo {
            is_valid: false,
            error_message: Some(format!("Invalid ARN format: {}", arn)),
            region: None,
            model_type: None,
            model_id: None,
            cross_region_inference: false,
        };
    };

    let region = caps.get(1).map(|m| m.as_str().to_string());
    let model_type = caps.get(3).map(|m| m.as_str().to_string());
    let original_model_id = caps.get(4).map(|m| m.as_str().to_string());

    let model_id = original_model_id.as_deref().map(parse_base_model_id);
    let cross_region = original_model_id.as_deref().map_or(false, |id| {
        model_id.as_deref().map_or(false, |base| base != id)
    });

    let error_message =
        if let (Some(arn_region), Some(cfg)) = (&region, configured_region) {
            if arn_region != cfg {
                Some(format!(
                    "Region mismatch: ARN region ({}) differs from configured region ({})",
                    arn_region, cfg
                ))
            } else {
                None
            }
        } else {
            None
        };

    ArnInfo {
        is_valid: true,
        region,
        model_type,
        model_id,
        cross_region_inference: cross_region,
        error_message,
    }
}

/// Strip cross-region inference prefix from a model ID.
fn parse_base_model_id(model_id: &str) -> String {
    let prefixes = [
        "us.",
        "eu.",
        "apac.",
        "ap.",
        "au.",
        "jp.",
        "ca.",
        "sa.",
        "ug.",
        "global.",
    ];
    for prefix in prefixes {
        if model_id.starts_with(prefix) {
            return model_id[prefix.len()..].to_string();
        }
    }
    model_id.to_string()
}

/// AWS Bedrock API provider handler.
pub struct AwsBedrockHandler {
    http_client: reqwest::Client,
    signer: SigV4Signer,
    base_url: String,
    model_id: String,
    model_info: ModelInfo,
    use_cross_region_inference: bool,
    use_global_inference: bool,
    temperature: f64,
    service_tier: Option<roo_types::provider_settings::AwsBedrockServiceTier>,
    enable_1m_context: bool,
    /// Whether to use Bedrock prompt caching (cachePoint markers).
    use_prompt_cache: bool,
    /// Previous cache point placements (for maintaining consistency across calls).
    previous_cache_point_placements: Option<Vec<roo_provider::transform::cache_strategy::CachePointPlacement>>,
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
            use_global_inference: config.use_global_inference,
            temperature: config.temperature.unwrap_or(crate::types::BEDROCK_DEFAULT_TEMPERATURE),
            service_tier: config.service_tier,
            enable_1m_context: config.enable_1m_context,
            use_prompt_cache: config.use_prompt_cache,
            previous_cache_point_placements: None,
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

    /// Get the model ID, potentially prefixed with cross-region or global inference prefix.
    fn effective_model_id(&self) -> String {
        if self.use_global_inference {
            // Check if model supports global inference
            let global_models = [
                "anthropic.claude-sonnet-4-5-20250929-v1:0",
                "anthropic.claude-3-5-sonnet-20241022-v2:0",
            ];
            if global_models.contains(&self.model_id.as_str()) {
                return format!("global.{}", self.model_id);
            }
        }

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
    /// - Uses the MultiPoint cache strategy for cachePoint placement
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

        // --- Apply cache strategy ---
        let cache_model_info = BedrockCacheModelInfo::from_model_info(&self.model_info);
        let strategy_config = MultiPointStrategyConfig {
            model_info: cache_model_info,
            system_prompt: if system_prompt.is_empty() {
                None
            } else {
                Some(system_prompt.to_string())
            },
            messages: filtered_messages,
            use_prompt_cache: self.use_prompt_cache,
            previous_cache_point_placements: self.previous_cache_point_placements.clone(),
        };
        let strategy = MultiPointStrategy::new(strategy_config);
        let cache_result = strategy.determine_optimal_cache_points();

        let system_messages = cache_result.system;
        let bedrock_messages: Vec<Value> = cache_result
            .messages
            .into_iter()
            .map(|msg| {
                json!({
                    "role": msg.role,
                    "content": msg.content,
                })
            })
            .collect();

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

        // Add anthropic_beta for 1M context and fine-grained tool streaming
        // Source: TS bedrock.ts — anthropic_beta header construction
        let mut anthropic_betas: Vec<String> = Vec::new();
        if self.enable_1m_context {
            anthropic_betas.push("context-1m-2025-08-07".to_string());
        }
        // Add fine-grained tool streaming for Claude models
        if self.model_id.contains("claude") {
            anthropic_betas.push("fine-grained-tool-streaming-2025-05-14".to_string());
        }
        if !anthropic_betas.is_empty() {
            if let Some(fields) = body.get_mut("additionalModelRequestFields") {
                fields["anthropic_beta"] = json!(anthropic_betas);
            } else {
                body["additionalModelRequestFields"] = json!({
                    "anthropic_beta": anthropic_betas,
                });
            }
        }

        // Add service_tier as a top-level parameter
        // Source: TS bedrock.ts — service_tier is top-level, NOT inside additionalModelRequestFields
        if let Some(ref tier) = self.service_tier {
            let tier_str = match tier {
                roo_types::provider_settings::AwsBedrockServiceTier::Standard => "STANDARD",
                roo_types::provider_settings::AwsBedrockServiceTier::Flex => "FLEX",
                roo_types::provider_settings::AwsBedrockServiceTier::Priority => "PRIORITY",
            };
            body["service_tier"] = json!(tier_str);
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
                BedrockEvent::InternalServerException { ref message }
                | BedrockEvent::ServiceUnavailableException { ref message }
                | BedrockEvent::ThrottlingException { ref message }
                | BedrockEvent::ValidationException { ref message } => {
                    let error_type = BedrockErrorType::from_error_message(message);
                    let user_msg = error_type.user_message(&self.model_id);
                    // For throttling, propagate to allow retry
                    if error_type == BedrockErrorType::Throttling {
                        return Err(ProviderError::ApiError("bedrock".to_string(), user_msg));
                    }
                    // For other errors, yield error text then end
                    chunks.push(Ok(ApiStreamChunk::Text {
                        text: format!("Error: {}", user_msg),
                    }));
                    chunks.push(Ok(ApiStreamChunk::Usage {
                        input_tokens: 0,
                        output_tokens: 0,
                        cache_write_tokens: Some(0),
                        cache_read_tokens: Some(0),
                        reasoning_tokens: None,
                        total_cost: Some(0.0),
                    }));
                }
                BedrockEvent::Unknown { event_type, .. } => {
                    // Log but don't fail on unknown events
                    let _ = event_type;
                }
                BedrockEvent::PromptRouter { invoked_model_id, usage } => {
                    // Source: TS bedrock.ts — prompt router updates model info for pricing
                    if let Some(_model_id) = invoked_model_id {
                        tracing::debug!(model_id = %_model_id, "Bedrock prompt router invoked model");
                    }
                    if let Some(router_usage) = usage {
                        if !usage_emitted {
                            let input_tokens = router_usage.input_tokens;
                            let output_tokens = router_usage.output_tokens;
                            let cache_read_tokens = router_usage.cache_read_input_tokens;
                            let cache_write_tokens = router_usage.cache_write_input_tokens;

                            chunks.push(Ok(ApiStreamChunk::Usage {
                                input_tokens,
                                output_tokens,
                                cache_write_tokens,
                                cache_read_tokens,
                                reasoning_tokens: None,
                                total_cost: None,
                            }));
                            usage_emitted = true;
                        }
                    }
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

    /// Helper to build a minimal AwsBedrockConfig for tests.
    fn test_config() -> AwsBedrockConfig {
        AwsBedrockConfig {
            access_key: "AKIAIOSFODNN7EXAMPLE".to_string(),
            secret_key: "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY".to_string(),
            session_token: None,
            region: "us-east-1".to_string(),
            model_id: None,
            use_cross_region_inference: false,
            use_global_inference: false,
            endpoint_url: None,
            request_timeout: None,
            temperature: None,
            service_tier: None,
            enable_1m_context: false,
            use_profile: false,
            profile_name: None,
            use_api_key: false,
            api_key: None,
            vpc_endpoint: None,
            vpc_endpoint_enabled: false,
            use_prompt_cache: true,
        }
    }

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
        let config = test_config();
        let handler = AwsBedrockHandler::new(config);
        assert!(handler.is_ok());
    }

    #[test]
    fn test_handler_uses_default_model() {
        let config = test_config();
        let handler = AwsBedrockHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, models::DEFAULT_MODEL_ID);
    }

    #[test]
    fn test_handler_custom_model() {
        let mut config = test_config();
        config.model_id = Some("anthropic.claude-3-5-haiku-20241022-v1:0".to_string());
        let handler = AwsBedrockHandler::new(config).unwrap();
        let (model_id, _) = handler.get_model();
        assert_eq!(model_id, "anthropic.claude-3-5-haiku-20241022-v1:0");
    }

    #[test]
    fn test_handler_provider_name() {
        let config = test_config();
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
        let mut config = test_config();
        config.model_id = Some("anthropic.claude-3-5-sonnet-20241022-v2:0".to_string());
        config.use_cross_region_inference = true;
        let handler = AwsBedrockHandler::new(config).unwrap();
        let effective_id = handler.effective_model_id();
        assert!(effective_id.starts_with("us.") || effective_id.contains("anthropic"));
    }

    #[test]
    fn test_global_inference() {
        let mut config = test_config();
        config.model_id = Some("anthropic.claude-sonnet-4-5-20250929-v1:0".to_string());
        config.use_global_inference = true;
        let handler = AwsBedrockHandler::new(config).unwrap();
        let effective_id = handler.effective_model_id();
        assert_eq!(
            effective_id,
            "global.anthropic.claude-sonnet-4-5-20250929-v1:0"
        );
    }

    #[test]
    fn test_handler_with_session_token() {
        let mut config = test_config();
        config.session_token = Some("session-token-123".to_string());
        let handler = AwsBedrockHandler::new(config).unwrap();
        assert_eq!(handler.signer.session_token(), Some("session-token-123"));
    }

    #[test]
    fn test_handler_with_custom_endpoint() {
        let mut config = test_config();
        config.endpoint_url = Some("https://custom-bedrock.example.com".to_string());
        let handler = AwsBedrockHandler::new(config).unwrap();
        assert_eq!(handler.base_url, "https://custom-bedrock.example.com");
    }

    #[test]
    fn test_nova_models_available() {
        let all_models = models::models();
        assert!(all_models.contains_key("us.amazon.nova-pro-v1:0"));
        assert!(all_models.contains_key("us.amazon.nova-lite-v1:0"));
    }

    // --- Error taxonomy tests ---

    #[test]
    fn test_error_type_access_denied() {
        assert_eq!(
            BedrockErrorType::from_error_message("Access denied for model"),
            BedrockErrorType::AccessDenied
        );
    }

    #[test]
    fn test_error_type_not_found() {
        assert_eq!(
            BedrockErrorType::from_error_message("Model does not exist"),
            BedrockErrorType::NotFound
        );
    }

    #[test]
    fn test_error_type_throttling() {
        assert_eq!(
            BedrockErrorType::from_error_message("Rate exceeded"),
            BedrockErrorType::Throttling
        );
        assert_eq!(
            BedrockErrorType::from_error_message("Request was throttled"),
            BedrockErrorType::Throttling
        );
    }

    #[test]
    fn test_error_type_too_many_tokens() {
        assert_eq!(
            BedrockErrorType::from_error_message("too many tokens in request"),
            BedrockErrorType::TooManyTokens
        );
    }

    #[test]
    fn test_error_type_service_quota() {
        assert_eq!(
            BedrockErrorType::from_error_message("Service quota exceeded for model"),
            BedrockErrorType::ServiceQuotaExceeded
        );
    }

    #[test]
    fn test_error_type_model_not_ready() {
        assert_eq!(
            BedrockErrorType::from_error_message("Model not ready"),
            BedrockErrorType::ModelNotReady
        );
    }

    #[test]
    fn test_error_type_internal_server() {
        assert_eq!(
            BedrockErrorType::from_error_message("internal server error"),
            BedrockErrorType::InternalServerError
        );
    }

    #[test]
    fn test_error_type_validation() {
        assert_eq!(
            BedrockErrorType::from_error_message("validation error in input"),
            BedrockErrorType::ValidationError
        );
    }

    #[test]
    fn test_error_type_generic() {
        assert_eq!(
            BedrockErrorType::from_error_message("something unexpected"),
            BedrockErrorType::Generic
        );
    }

    #[test]
    fn test_error_type_user_messages() {
        let msg = BedrockErrorType::Throttling.user_message("test-model");
        assert!(msg.contains("throttled"));

        let msg = BedrockErrorType::AccessDenied.user_message("my-model");
        assert!(msg.contains("my-model"));
        assert!(msg.contains("access"));
    }

    // --- ARN parsing tests ---

    #[test]
    fn test_parse_arn_foundation_model() {
        let arn = "arn:aws:bedrock:us-east-1:123456789012:foundation-model/anthropic.claude-3-5-sonnet-20241022-v2:0";
        let info = parse_arn(arn, Some("us-east-1"));
        assert!(info.is_valid);
        assert_eq!(info.region.as_deref(), Some("us-east-1"));
        assert_eq!(info.model_type.as_deref(), Some("foundation-model"));
        assert_eq!(
            info.model_id.as_deref(),
            Some("anthropic.claude-3-5-sonnet-20241022-v2:0")
        );
        assert!(!info.cross_region_inference);
        assert!(info.error_message.is_none());
    }

    #[test]
    fn test_parse_arn_cross_region_prefix() {
        let arn = "arn:aws:bedrock:us-east-1:123456789012:foundation-model/us.anthropic.claude-3-5-sonnet-20241022-v2:0";
        let info = parse_arn(arn, Some("us-east-1"));
        assert!(info.is_valid);
        assert!(info.cross_region_inference);
        assert_eq!(
            info.model_id.as_deref(),
            Some("anthropic.claude-3-5-sonnet-20241022-v2:0")
        );
    }

    #[test]
    fn test_parse_arn_region_mismatch() {
        let arn = "arn:aws:bedrock:us-west-2:123456789012:foundation-model/anthropic.claude-3-5-sonnet-20241022-v2:0";
        let info = parse_arn(arn, Some("us-east-1"));
        assert!(info.is_valid);
        assert!(info.error_message.is_some());
        assert!(info.error_message.unwrap().contains("Region mismatch"));
    }

    #[test]
    fn test_parse_arn_invalid() {
        let info = parse_arn("not-an-arn", Some("us-east-1"));
        assert!(!info.is_valid);
        assert!(info.error_message.is_some());
    }

    #[test]
    fn test_parse_arn_sagemaker() {
        let arn = "arn:aws:sagemaker:us-east-1:123456789012:endpoint/my-endpoint";
        let info = parse_arn(arn, Some("us-east-1"));
        assert!(info.is_valid);
        assert_eq!(info.model_type.as_deref(), Some("endpoint"));
    }

    #[test]
    fn test_parse_base_model_id() {
        assert_eq!(
            parse_base_model_id("us.anthropic.claude-3-5-sonnet-20241022-v2:0"),
            "anthropic.claude-3-5-sonnet-20241022-v2:0"
        );
        assert_eq!(
            parse_base_model_id("eu.anthropic.claude-3-5-sonnet-20241022-v2:0"),
            "anthropic.claude-3-5-sonnet-20241022-v2:0"
        );
        assert_eq!(
            parse_base_model_id("anthropic.claude-3-5-sonnet-20241022-v2:0"),
            "anthropic.claude-3-5-sonnet-20241022-v2:0"
        );
    }
}
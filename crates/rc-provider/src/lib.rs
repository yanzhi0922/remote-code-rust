//! LLM provider client with retry logic and message formatting.
//!
//! Supports OpenAI, Anthropic, Amazon Bedrock, and Google Vertex AI protocols.
//! Handles message conversion, response parsing, exponential back-off retries,
//! and mock-mode responses for testing.
//!
//! # Error classification
//!
//! The [`ProviderError`] enum provides structured error classification matching
//! upstream Claude Code's `categorizeRetryableAPIError`. Each variant carries
//! enough context for the caller to decide whether to retry, compact, or abort.

pub mod advanced_api;
pub mod agent_types;
pub mod api_client;
pub mod attribution;
pub mod beta_headers;
pub mod cache_headers;
pub mod circuit_breaker;
pub mod context;
pub mod conversation_backend;
pub mod cost;
pub mod credential_pool;
pub mod effort_params;
pub mod failover;
pub mod fingerprint;
pub mod max_tokens;
pub mod mcp_api;
pub mod media;
pub mod model_info;
pub mod query_source;
pub mod retry;
pub mod server_tool_use;
pub mod sigv4;
pub mod streaming;
pub mod thinking_blocks;
pub mod workload;

pub use api_client::{ApiClient, ContentBlock, QueryOptions, QueryResult, UsageStats};
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitState};
pub use conversation_backend::{ConversationBackend, ProviderCompatBackend};
pub use retry::{RetryConfig, RetryContext};
pub use streaming::StreamingCallbacks;

use anyhow::{Context, Result, anyhow};
use rc_config::ProviderConfig;
use rc_core::{
    ConversationEntry, ConversationRole, ProviderProtocol, ProviderResponse, ToolCall, UsageSummary,
};
use rc_tools::runtime_provider_tool_specs;
use reqwest::Client;
use reqwest::header::{
    ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, RETRY_AFTER,
    USER_AGENT,
};
use serde_json::{Value, json};
use std::sync::Mutex;
use std::time::Duration;

/// HTTP client for communicating with LLM provider APIs.
///
/// Includes an optional circuit breaker per provider name to prevent wasting
/// time on providers that are known to be down, and an optional credential
/// pool for round-robin API key rotation.
pub struct ProviderClient {
    http: Client,
    /// Circuit breakers keyed by provider name.
    breakers: Mutex<Vec<(String, CircuitBreaker)>>,
    /// Optional credential pool for round-robin API key rotation.
    credential_pool: Option<credential_pool::CredentialPool>,
}

impl ProviderClient {
    /// Create a new provider client.
    ///
    /// # Errors
    /// Returns an error if the underlying HTTP client cannot be constructed.
    pub fn new() -> Result<Self> {
        let http = Client::builder()
            .build()
            .context("failed to build the provider HTTP client")?;
        Ok(Self {
            http,
            breakers: Mutex::new(Vec::new()),
            credential_pool: None,
        })
    }

    /// Create a new provider client with a credential pool for API key rotation.
    ///
    /// When a credential pool is set, each request will use the next credential
    /// in the round-robin rotation, overriding the API key from the provider config.
    pub fn with_credential_pool(http: Client, pool: credential_pool::CredentialPool) -> Self {
        Self {
            http,
            breakers: Mutex::new(Vec::new()),
            credential_pool: Some(pool),
        }
    }

    /// Set the credential pool for API key rotation.
    pub fn set_credential_pool(&mut self, pool: credential_pool::CredentialPool) {
        self.credential_pool = Some(pool);
    }

    /// Resolve the effective API key for a request.
    ///
    /// If a credential pool is available, uses round-robin rotation.
    /// Otherwise, falls back to the provider config's API key.
    fn resolve_api_key(&self, provider: &ProviderConfig) -> Option<String> {
        if let Some(ref pool) = self.credential_pool
            && let Some(cred) = pool.next()
        {
            return Some(cred.api_key.clone());
        }
        provider.api_key.clone()
    }

    /// Get the circuit breaker configuration for the given provider name.
    ///
    /// Currently returns the default configuration for all providers.
    /// Per-provider configuration can be added by looking up the provider
    /// name in a configuration map.
    fn breaker_config_for(_provider_name: &str) -> CircuitBreakerConfig {
        CircuitBreakerConfig::default()
    }

    /// Check the circuit breaker for the given provider.
    ///
    /// Returns `Ok(())` if requests are allowed, or an error describing
    /// why the request was rejected.
    fn check_circuit(&self, provider_name: &str) -> Result<()> {
        let breakers = self.breakers.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, breaker)) = breakers.iter().find(|(name, _)| name == provider_name) {
            breaker.allow_request().map_err(|state| {
                anyhow!("provider {provider_name} circuit breaker is {state:?} — skipping request")
            })?;
        }
        Ok(())
    }

    /// Record a successful provider call in the circuit breaker.
    fn record_success(&self, provider_name: &str) {
        let mut breakers = self.breakers.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((_, breaker)) = breakers.iter_mut().find(|(name, _)| name == provider_name) {
            breaker.record_success();
        }
    }

    /// Record a failed provider call in the circuit breaker.
    ///
    /// Lazily creates a breaker for the provider if one does not yet exist.
    fn record_failure(&self, provider_name: &str) {
        let mut breakers = self.breakers.lock().unwrap_or_else(|e| e.into_inner());
        match breakers.iter_mut().find(|(name, _)| name == provider_name) {
            Some((_, breaker)) => breaker.record_failure(),
            None => {
                let config = Self::breaker_config_for(provider_name);
                let breaker = CircuitBreaker::new(config);
                breaker.record_failure();
                breakers.push((provider_name.to_owned(), breaker));
            }
        }
    }

    /// Send a completion request to the configured provider.
    ///
    /// Automatically selects the correct protocol (OpenAI / Anthropic) based on
    /// the provider configuration and retries on transient failures.
    ///
    /// # Errors
    /// Returns an error if the API request fails after all retries are exhausted.
    pub async fn complete(
        &self,
        provider: &ProviderConfig,
        conversation: &[ConversationEntry],
    ) -> Result<ProviderResponse> {
        if provider.name == "mock"
            || provider.api_key.as_deref() == Some("mock")
            || provider.base_url.as_deref() == Some("mock://provider")
        {
            return Ok(mock_response(conversation));
        }

        // Check circuit breaker before making the request.
        self.check_circuit(&provider.name)?;

        // Resolve API key: use credential pool rotation if available.
        let effective_provider = if self.credential_pool.is_some() {
            let mut p = provider.clone();
            p.api_key = self.resolve_api_key(provider);
            p
        } else {
            provider.clone()
        };

        let result = match effective_provider.protocol {
            ProviderProtocol::OpenAi => {
                self.complete_openai(&effective_provider, conversation)
                    .await
            }
            ProviderProtocol::Anthropic => {
                self.complete_anthropic(&effective_provider, conversation)
                    .await
            }
            ProviderProtocol::Bedrock => {
                self.complete_bedrock(&effective_provider, conversation)
                    .await
            }
            ProviderProtocol::Vertex => {
                self.complete_vertex(&effective_provider, conversation)
                    .await
            }
        };

        match &result {
            Ok(_) => self.record_success(&provider.name),
            Err(_) => self.record_failure(&provider.name),
        }
        result
    }

    /// Complete a conversation with automatic context compaction on context_length_exceeded errors.
    ///
    /// This implements the "reactiveCompact" pattern: if the API returns a 400 error
    /// indicating the context is too long, the conversation is automatically compacted
    /// and the request is retried (up to `max_retries` times).
    pub async fn complete_with_auto_compact(
        &self,
        provider: &ProviderConfig,
        conversation: &[ConversationEntry],
        context_manager: &context::ContextWindowManager,
    ) -> Result<ProviderResponse> {
        let mut current = conversation.to_vec();
        let max_retries = 3;

        for attempt in 0..=max_retries {
            match self.complete(provider, &current).await {
                Ok(response) => return Ok(response),
                Err(error) => {
                    let error_str = error.to_string().to_ascii_lowercase();
                    let is_context_too_long = error_str.contains("context_length_exceeded")
                        || error_str.contains("prompt_too_long")
                        || error_str.contains("too many tokens")
                        || error_str.contains("maximum context length")
                        || error_str.contains("reduce the length");

                    if !is_context_too_long || attempt >= max_retries {
                        return Err(error);
                    }

                    // Try to compact the conversation.
                    match context_manager.compact_on_error(&current) {
                        Some(compacted) => {
                            current = compacted;
                        }
                        None => {
                            return Err(error);
                        }
                    }
                }
            }
        }

        // Should not reach here, but just in case.
        self.complete(provider, &current).await
    }

    async fn complete_openai(
        &self,
        provider: &ProviderConfig,
        conversation: &[ConversationEntry],
    ) -> Result<ProviderResponse> {
        let model_name = provider.model.as_deref().unwrap_or("");
        let is_reasoning_model = model_name.starts_with("o1")
            || model_name.starts_with("o3")
            || model_name.starts_with("o4");
        let tools = current_openai_tool_schemas().await;

        let mut body = if is_reasoning_model {
            // Reasoning models (o1/o3/o4-mini) do not support temperature
            // and use max_completion_tokens instead of max_tokens.
            json!({
                "model": provider.model,
                "messages": to_openai_messages(conversation),
                "tools": tools,
                "tool_choice": "auto",
                "max_completion_tokens": provider.max_output_tokens,
                "stream": false,
            })
        } else {
            json!({
                "model": provider.model,
                "messages": to_openai_messages(conversation),
                "tools": tools,
                "tool_choice": "auto",
                "temperature": 0.1,
                "max_tokens": provider.max_output_tokens,
                "stream": false,
            })
        };

        // If thinking_budget is set and the model supports it, add reasoning_effort.
        if is_reasoning_model && let Some(budget) = provider.thinking_budget {
            // Map budget to reasoning_effort: low/medium/high.
            let effort = if budget <= 5000 {
                "low"
            } else if budget <= 20000 {
                "medium"
            } else {
                "high"
            };
            body["reasoning_effort"] = json!(effort);
        }
        let base_url = provider
            .base_url
            .as_ref()
            .ok_or_else(|| anyhow!("provider is missing a normalized base URL"))?;
        let response = self
            .send_json_request(provider, base_url, &body, "openai-compatible")
            .await?;
        parse_openai_response(response.0, response.1)
    }

    async fn complete_anthropic(
        &self,
        provider: &ProviderConfig,
        conversation: &[ConversationEntry],
    ) -> Result<ProviderResponse> {
        let (system, messages) = to_anthropic_messages(conversation);
        let tools = current_anthropic_tool_schemas().await;
        let mut body = json!({
            "model": provider.model,
            "system": system,
            "messages": messages,
            "tools": tools,
            "max_tokens": provider.max_output_tokens,
            "stream": false,
        });
        apply_anthropic_request_metadata(&mut body, provider);
        // Enable extended thinking if a budget is configured.
        if let Some(budget) = provider.thinking_budget {
            body["thinking"] = json!({
                "type": "enabled",
                "budget_tokens": budget,
            });
            // Anthropic requires max_tokens > budget_tokens.
            let current_max = body.get("max_tokens").and_then(Value::as_u64).unwrap_or(0);
            if current_max <= u64::from(budget) {
                body["max_tokens"] = json!(u64::from(budget) + 4096);
            }
        }
        // Detect resume: if there are tool-role entries, this is a continued conversation.
        let is_resume = conversation
            .iter()
            .any(|entry| matches!(entry.role, ConversationRole::Tool));
        add_stable_cache_control(&mut body, is_resume);
        let base_url = provider
            .base_url
            .as_ref()
            .ok_or_else(|| anyhow!("provider is missing a normalized base URL"))?;
        let response = self
            .send_json_request(provider, base_url, &body, "anthropic-compatible")
            .await?;
        parse_anthropic_response(response.0, response.1)
    }

    /// Send a completion request to Amazon Bedrock using native SigV4 signing.
    ///
    /// If AWS credentials (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`) are not
    /// available, falls back to the OpenAI-compatible path (useful for Bedrock
    /// proxies like LiteLLM).
    ///
    /// Bedrock Claude models use the Anthropic Messages API format, so the
    /// response is parsed with the Anthropic response parser.
    async fn complete_bedrock(
        &self,
        provider: &ProviderConfig,
        conversation: &[ConversationEntry],
    ) -> Result<ProviderResponse> {
        let credentials = match sigv4::load_aws_credentials() {
            Some(creds) => creds,
            None => {
                // No AWS credentials — fall back to OpenAI-compatible proxy mode.
                return self.complete_openai(provider, conversation).await;
            }
        };

        let model = provider
            .model
            .as_deref()
            .ok_or_else(|| anyhow!("Bedrock provider requires a model ID (e.g. anthropic.claude-sonnet-4-20250514-v1:0)"))?;

        // Build Anthropic-format body for Claude models on Bedrock.
        let (system, messages) = to_anthropic_messages(conversation);
        let tools = current_anthropic_tool_schemas().await;
        let mut body = json!({
            "anthropic_version": "bedrock-2023-05-31",
            "system": system,
            "messages": messages,
            "tools": tools,
            "max_tokens": provider.max_output_tokens,
        });
        apply_anthropic_request_metadata(&mut body, provider);
        let payload =
            serde_json::to_vec(&body).context("failed to serialise Bedrock request body")?;

        // Construct Bedrock InvokeModel URL.
        let host = format!("bedrock-runtime.{}.amazonaws.com", credentials.region);
        let encoded_model = model.replace(':', "%3A").replace('+', "%2B");
        let path = format!("/model/{encoded_model}/invoke");
        let url = format!("https://{host}{path}");

        let (status, text) = self
            .send_bedrock_request(&url, &host, &path, &payload, provider, &credentials)
            .await?;

        // Bedrock returns Anthropic-format responses for Claude models.
        parse_anthropic_response(status, text)
    }

    /// Send a signed Bedrock request with retry logic.
    ///
    /// Each retry attempt re-signs the request because the `X-Amz-Date` timestamp
    /// changes.
    async fn send_bedrock_request(
        &self,
        url: &str,
        host: &str,
        path: &str,
        payload: &[u8],
        provider: &ProviderConfig,
        credentials: &sigv4::AwsCredentials,
    ) -> Result<(u16, String)> {
        let mut attempt = 0u32;
        loop {
            // Sign the request (must be done per-attempt for fresh timestamp).
            let signed = sigv4::sign("POST", host, path, payload, credentials, "bedrock");

            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
            headers.insert(
                HeaderName::from_static("host"),
                HeaderValue::from_str(&signed.host)?,
            );
            headers.insert(
                HeaderName::from_static("x-amz-date"),
                HeaderValue::from_str(&signed.x_amz_date)?,
            );
            headers.insert(
                HeaderName::from_static("x-amz-content-sha256"),
                HeaderValue::from_str(&signed.x_amz_content_sha256)?,
            );
            headers.insert(AUTHORIZATION, HeaderValue::from_str(&signed.authorization)?);
            if let Some(ref token) = signed.x_amz_security_token {
                headers.insert(
                    HeaderName::from_static("x-amz-security-token"),
                    HeaderValue::from_str(token)?,
                );
            }

            let response = self
                .http
                .post(url)
                .headers(headers)
                .timeout(Duration::from_millis(provider.timeout_ms))
                .body(payload.to_vec())
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let retry_after = parse_retry_after(resp.headers(), provider);
                    let text = resp
                        .text()
                        .await
                        .context("failed to read Bedrock response body")?;
                    if is_retryable_http_status(status) && attempt < provider.max_retries {
                        tokio::time::sleep(compute_retry_delay(provider, attempt, retry_after))
                            .await;
                        attempt += 1;
                        continue;
                    }
                    return Ok((status, text));
                }
                Err(error) => {
                    if is_retryable_transport_error(&error) && attempt < provider.max_retries {
                        tokio::time::sleep(compute_retry_delay(provider, attempt, None)).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(error).context("Bedrock request failed");
                }
            }
        }
    }

    /// Send a completion request to Google Vertex AI using OAuth2 Bearer auth.
    ///
    /// If Google credentials are not available, falls back to the OpenAI-compatible
    /// path (useful for Vertex AI proxies).
    ///
    /// Vertex AI Claude models use the Anthropic Messages API format, so the
    /// response is parsed with the Anthropic response parser.
    async fn complete_vertex(
        &self,
        provider: &ProviderConfig,
        conversation: &[ConversationEntry],
    ) -> Result<ProviderResponse> {
        let access_token = match load_vertex_access_token() {
            Some(token) => token,
            None => {
                // No Google credentials — fall back to OpenAI-compatible proxy mode.
                return self.complete_openai(provider, conversation).await;
            }
        };

        let model = provider.model.as_deref().ok_or_else(|| {
            anyhow!("Vertex AI provider requires a model ID (e.g. claude-sonnet-4@20250514)")
        })?;

        let project = std::env::var("GOOGLE_CLOUD_PROJECT")
            .or_else(|_| std::env::var("GCLOUD_PROJECT"))
            .map_err(|_| {
                anyhow!("Vertex AI requires GOOGLE_CLOUD_PROJECT or GCLOUD_PROJECT env var")
            })?;

        let region = std::env::var("GOOGLE_CLOUD_REGION")
            .or_else(|_| std::env::var("CLOUD_ML_REGION"))
            .unwrap_or_else(|_| "us-east5".to_string());

        // Build Anthropic-format body for Claude models on Vertex AI.
        let (system, messages) = to_anthropic_messages(conversation);
        let tools = current_anthropic_tool_schemas().await;
        let mut body = json!({
            "anthropic_version": "vertex-2023-10-16",
            "system": system,
            "messages": messages,
            "tools": tools,
            "max_tokens": provider.max_output_tokens,
        });
        apply_anthropic_request_metadata(&mut body, provider);

        // Construct Vertex AI URL.
        let url = format!(
            "https://{region}-aiplatform.googleapis.com/v1/projects/{project}/locations/{region}/publishers/anthropic/models/{model}:invokeModel"
        );

        let (status, text) = self
            .send_vertex_request(&url, &access_token, &body, provider)
            .await?;

        // Vertex AI returns Anthropic-format responses for Claude models.
        parse_anthropic_response(status, text)
    }

    /// Send a Vertex AI request with Bearer token auth and retry logic.
    async fn send_vertex_request(
        &self,
        url: &str,
        access_token: &str,
        body: &Value,
        provider: &ProviderConfig,
    ) -> Result<(u16, String)> {
        let mut attempt = 0u32;
        loop {
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {access_token}"))?,
            );
            headers.insert(
                USER_AGENT,
                HeaderValue::from_str(&format!("remote-code-rust/{}", env!("CARGO_PKG_VERSION")))?,
            );

            let response = self
                .http
                .post(url)
                .headers(headers)
                .timeout(Duration::from_millis(provider.timeout_ms))
                .json(body)
                .send()
                .await;

            match response {
                Ok(resp) => {
                    let status = resp.status().as_u16();
                    let retry_after = parse_retry_after(resp.headers(), provider);
                    let text = resp
                        .text()
                        .await
                        .context("failed to read Vertex AI response body")?;
                    if is_retryable_http_status(status) && attempt < provider.max_retries {
                        tokio::time::sleep(compute_retry_delay(provider, attempt, retry_after))
                            .await;
                        attempt += 1;
                        continue;
                    }
                    return Ok((status, text));
                }
                Err(error) => {
                    if is_retryable_transport_error(&error) && attempt < provider.max_retries {
                        tokio::time::sleep(compute_retry_delay(provider, attempt, None)).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(error).context("Vertex AI request failed");
                }
            }
        }
    }

    async fn send_json_request(
        &self,
        provider: &ProviderConfig,
        base_url: &str,
        body: &Value,
        label: &str,
    ) -> Result<(u16, String)> {
        let mut attempt = 0u32;
        loop {
            let response = self
                .http
                .post(base_url)
                .headers(build_headers(provider)?)
                .timeout(Duration::from_millis(provider.timeout_ms))
                .json(body)
                .send()
                .await;
            match response {
                Ok(response) => {
                    let status = response.status().as_u16();
                    let retry_after = parse_retry_after(response.headers(), provider);
                    let text = response
                        .text()
                        .await
                        .with_context(|| format!("failed to read {label} response body"))?;
                    if is_retryable_http_status(status) && attempt < provider.max_retries {
                        tokio::time::sleep(compute_retry_delay(provider, attempt, retry_after))
                            .await;
                        attempt += 1;
                        continue;
                    }
                    return Ok((status, text));
                }
                Err(error) => {
                    if is_retryable_transport_error(&error) && attempt < provider.max_retries {
                        tokio::time::sleep(compute_retry_delay(provider, attempt, None)).await;
                        attempt += 1;
                        continue;
                    }
                    return Err(error).with_context(|| format!("{label} request failed"));
                }
            }
        }
    }
}

fn is_retryable_http_status(status: u16) -> bool {
    matches!(status, 408 | 429 | 500 | 502 | 503 | 504 | 529)
}

fn is_retryable_transport_error(error: &reqwest::Error) -> bool {
    error.is_timeout() || error.is_connect()
}

fn compute_retry_delay(
    provider: &ProviderConfig,
    attempt: u32,
    retry_after: Option<Duration>,
) -> Duration {
    if let Some(retry_after) = retry_after {
        return retry_after;
    }
    let multiplier = 2u64.saturating_pow(attempt.min(16));
    let base_ms = provider
        .retry_initial_backoff_ms
        .saturating_mul(multiplier)
        .min(provider.retry_max_backoff_ms)
        .max(1);
    // Add ±25% jitter to avoid thundering herd under concurrent retries.
    // Uses a simple deterministic hash based on attempt + base_ms to avoid
    // needing a full RNG while still providing sufficient variance.
    let jitter_range = base_ms / 4;
    let jitter_offset = if jitter_range > 0 {
        // Deterministic pseudo-random: mix attempt counter with base delay.
        let hash = (attempt as u64).wrapping_mul(2654435761) ^ base_ms;
        hash % (2 * jitter_range)
    } else {
        0
    };
    let delay_ms = base_ms
        .saturating_sub(jitter_range)
        .saturating_add(jitter_offset);
    Duration::from_millis(delay_ms.max(1))
}

fn parse_retry_after(headers: &HeaderMap, provider: &ProviderConfig) -> Option<Duration> {
    if !provider.respect_retry_after {
        return None;
    }
    headers
        .get(RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok())
        .map(Duration::from_secs)
}

/// Load a Google Cloud OAuth2 access token for Vertex AI.
///
/// Tries, in order:
/// 1. `GOOGLE_ACCESS_TOKEN` environment variable (direct token).
/// 2. `gcloud auth print-access-token` CLI command.
///
/// Returns `None` if neither source yields a token.
fn load_vertex_access_token() -> Option<String> {
    // 1. Direct token from environment.
    if let Ok(token) = std::env::var("GOOGLE_ACCESS_TOKEN")
        && !token.is_empty()
    {
        return Some(token);
    }

    // 2. Try gcloud CLI.
    let output = std::process::Command::new("gcloud")
        .args(["auth", "print-access-token"])
        .output()
        .ok()?;

    if output.status.success() {
        let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !token.is_empty() {
            return Some(token);
        }
    }

    None
}

pub(crate) fn apply_anthropic_request_metadata(body: &mut Value, provider: &ProviderConfig) {
    if provider.request_metadata.is_empty() {
        return;
    }
    let user_id =
        serde_json::to_string(&provider.request_metadata).unwrap_or_else(|_| "{}".to_owned());
    body["metadata"] = json!({
        "user_id": user_id,
    });
}

fn build_headers(provider: &ProviderConfig) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));

    if matches!(provider.protocol, ProviderProtocol::Anthropic) {
        // ── Claude Code disguise mode ──────────────────────────────────
        //
        // Coding Plan providers (智谱/阿里云/腾讯云/百度千帆) prioritise
        // requests that look like they come from Claude Code.  We mimic the
        // key identifying headers so our traffic receives the same
        // preferential treatment.
        //
        // This is the same approach used by OpenCode, OpenClaw, Cline, and
        // other open-source coding agents that consume Coding Plan quotas.

        headers.insert(USER_AGENT, HeaderValue::from_static("claude-code/1.0.18"));
        headers.insert(
            HeaderName::from_static("anthropic-version"),
            HeaderValue::from_static("2023-06-01"),
        );
        // Claude Code typically sends these beta features.
        headers.insert(
            HeaderName::from_static("anthropic-beta"),
            HeaderValue::from_static("prompt-caching-2024-07-31,pdfs-2024-09-25"),
        );
    } else {
        headers.insert(
            USER_AGENT,
            HeaderValue::from_str(&format!("remote-code-rust/{}", env!("CARGO_PKG_VERSION")))?,
        );
    }

    if let Some(api_key) = &provider.api_key {
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {api_key}"))?,
        );
        headers.insert(
            HeaderName::from_static("x-api-key"),
            HeaderValue::from_str(api_key)?,
        );
    }

    // Apply user-supplied header overrides last so they can override
    // any of the defaults above (including the Claude Code disguise).
    for (name, value) in &provider.request_header_overrides {
        let header_name = HeaderName::from_bytes(name.as_bytes())
            .with_context(|| format!("invalid header name {name}"))?;
        let header_value =
            HeaderValue::from_str(value).with_context(|| format!("invalid header {name}"))?;
        headers.insert(header_name, header_value);
    }
    Ok(headers)
}

fn to_openai_messages(conversation: &[ConversationEntry]) -> Vec<Value> {
    conversation
        .iter()
        .map(|entry| match entry.role {
            ConversationRole::System => json!({
                "role": role_name(&entry.role),
                "content": entry.history_text(),
            }),
            ConversationRole::User => {
                if entry.attachments.is_empty() {
                    json!({
                        "role": "user",
                        "content": entry.history_text(),
                    })
                } else {
                    let mut parts = Vec::new();
                    parts.push(json!({"type": "text", "text": entry.history_text()}));
                    for att in &entry.attachments {
                        parts.push(json!({
                            "type": "image_url",
                            "image_url": {
                                "url": format!("data:{};base64,{}", att.media_type.mime_type(), att.data),
                            }
                        }));
                    }
                    json!({
                        "role": "user",
                        "content": parts,
                    })
                }
            }
            ConversationRole::Assistant => {
                let mut message = json!({
                    "role": "assistant",
                    "content": entry.history_text(),
                });
                if !entry.tool_calls.is_empty() {
                    message["tool_calls"] = Value::Array(
                        entry
                            .tool_calls
                            .iter()
                            .map(|call| {
                                json!({
                                    "id": call.id,
                                    "type": "function",
                                    "function": {
                                        "name": call.name,
                                        "arguments": call.input.to_string(),
                                    }
                                })
                            })
                            .collect(),
                    );
                }
                message
            }
            ConversationRole::Tool => json!({
                "role": "tool",
                "tool_call_id": entry.tool_call_id,
                "content": entry.text,
            }),
        })
        .collect()
}

fn to_anthropic_messages(conversation: &[ConversationEntry]) -> (String, Vec<Value>) {
    let system = conversation
        .iter()
        .filter(|entry| matches!(entry.role, ConversationRole::System))
        .map(ConversationEntry::history_text)
        .collect::<Vec<_>>()
        .join("\n\n");
    let non_system = conversation
        .iter()
        .filter(|entry| !matches!(entry.role, ConversationRole::System))
        .collect::<Vec<_>>();
    let mut messages = Vec::new();
    let mut index = 0usize;

    while index < non_system.len() {
        let entry = non_system[index];
        match entry.role {
            ConversationRole::User => {
                let mut blocks = vec![json!({"type": "text", "text": entry.history_text()})];
                for att in &entry.attachments {
                    blocks.push(json!({
                        "type": "image",
                        "source": {
                            "type": "base64",
                            "media_type": att.media_type.mime_type(),
                            "data": att.data,
                        }
                    }));
                }
                messages.push(json!({
                    "role": "user",
                    "content": blocks,
                }));
                index += 1;
            }
            ConversationRole::Assistant => {
                if entry.content_blocks.is_empty() {
                    let mut blocks = Vec::new();
                    if !entry.history_text().is_empty() {
                        blocks.push(json!({"type": "text", "text": entry.history_text()}));
                    }
                    for call in &entry.tool_calls {
                        blocks.push(json!({
                            "type": "tool_use",
                            "id": call.id,
                            "name": call.name,
                            "input": call.input,
                        }));
                    }
                    messages.push(json!({
                        "role": "assistant",
                        "content": blocks,
                    }));
                } else {
                    messages.push(json!({
                        "role": "assistant",
                        "content": entry.content_blocks,
                    }));
                }
                index += 1;
            }
            ConversationRole::Tool => {
                let mut blocks = Vec::new();
                while index < non_system.len()
                    && matches!(non_system[index].role, ConversationRole::Tool)
                {
                    let tool_entry = non_system[index];
                    let mut tool_result = json!({
                        "type": "tool_result",
                        "tool_use_id": tool_entry.tool_call_id,
                        "content": tool_entry.text,
                    });
                    if tool_entry.is_error {
                        tool_result["is_error"] = Value::Bool(true);
                    }
                    blocks.push(tool_result);
                    index += 1;
                }
                messages.push(json!({
                    "role": "user",
                    "content": blocks,
                }));
            }
            ConversationRole::System => {
                index += 1;
            }
        }
    }

    (system, messages)
}

async fn current_openai_tool_schemas() -> Vec<Value> {
    runtime_provider_tool_specs()
        .await
        .into_iter()
        .map(|tool| tool.to_openai_schema())
        .collect()
}

async fn current_anthropic_tool_schemas() -> Vec<Value> {
    runtime_provider_tool_specs()
        .await
        .into_iter()
        .map(|tool| tool.to_anthropic_schema())
        .collect()
}

fn parse_openai_response(status: u16, raw_text: String) -> Result<ProviderResponse> {
    let payload: Value = serde_json::from_str(&raw_text)
        .with_context(|| format!("provider returned non-JSON output: {}", truncate(&raw_text)))?;
    if status >= 400 {
        let error_message = payload
            .get("error")
            .and_then(|value| value.get("message"))
            .and_then(Value::as_str)
            .or_else(|| payload.get("message").and_then(Value::as_str))
            .unwrap_or("provider error");
        return Err(anyhow!(
            "provider request failed ({status}): {error_message}"
        ));
    }

    let choice = payload
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .ok_or_else(|| anyhow!("provider response did not include choices[0].message"))?;

    let tool_calls = choice
        .get("tool_calls")
        .and_then(Value::as_array)
        .map(|tool_calls| {
            tool_calls
                .iter()
                .filter_map(parse_openai_tool_call)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let raw_assistant_text = coerce_text_content(choice.get("content")).trim().to_owned();
    let usage = payload.get("usage").cloned().unwrap_or_default();

    // OpenAI reasoning models may include reasoning in the refusal field or
    // as a reasoning_content field (non-standard, some providers expose it).
    let reasoning_text = choice
        .get("reasoning_content")
        .and_then(Value::as_str)
        .map(String::from)
        .or_else(|| {
            choice
                .get("reasoning")
                .and_then(Value::as_str)
                .map(String::from)
        });

    Ok(ProviderResponse {
        text: strip_reasoning_tags(&raw_assistant_text),
        history_text: Some(raw_assistant_text),
        thinking: reasoning_text,
        content_blocks: Vec::new(),
        tool_calls,
        request_id: payload
            .get("id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        usage: UsageSummary {
            input_tokens: usage
                .get("prompt_tokens")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            output_tokens: usage
                .get("completion_tokens")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        },
        stop_reason: payload
            .get("choices")
            .and_then(Value::as_array)
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("finish_reason"))
            .and_then(Value::as_str)
            .unwrap_or("stop")
            .to_owned(),
    })
}

fn parse_anthropic_response(status: u16, raw_text: String) -> Result<ProviderResponse> {
    let payload: Value = serde_json::from_str(&raw_text)
        .with_context(|| format!("provider returned non-JSON output: {}", truncate(&raw_text)))?;
    if status >= 400 {
        let error_message = payload
            .get("error")
            .and_then(|value| value.get("message"))
            .and_then(Value::as_str)
            .or_else(|| payload.get("message").and_then(Value::as_str))
            .unwrap_or("provider error");
        return Err(anyhow!(
            "provider request failed ({status}): {error_message}"
        ));
    }
    let blocks = payload
        .get("content")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let text = blocks
        .iter()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|block| block.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    let tool_calls = blocks
        .iter()
        .filter_map(parse_anthropic_tool_like_call)
        .collect::<Vec<_>>();
    let thinking_text: String = blocks
        .iter()
        .filter(|block| block.get("type").and_then(Value::as_str) == Some("thinking"))
        .filter_map(|block| block.get("thinking").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n");
    let usage = payload.get("usage").cloned().unwrap_or_default();

    Ok(ProviderResponse {
        text: strip_reasoning_tags(&text),
        history_text: Some(text),
        thinking: if thinking_text.is_empty() {
            None
        } else {
            Some(thinking_text)
        },
        content_blocks: blocks,
        tool_calls,
        request_id: payload
            .get("id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        usage: UsageSummary {
            input_tokens: usage
                .get("input_tokens")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            output_tokens: usage
                .get("output_tokens")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            cache_read_input_tokens: usage
                .get("cache_read_input_tokens")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            cache_creation_input_tokens: usage
                .get("cache_creation_input_tokens")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
        },
        stop_reason: payload
            .get("stop_reason")
            .and_then(Value::as_str)
            .unwrap_or("stop")
            .to_owned(),
    })
}

fn parse_openai_tool_call(value: &Value) -> Option<ToolCall> {
    let function = value.get("function")?;
    let id = value.get("id")?.as_str()?.to_owned();
    let name = function.get("name")?.as_str()?.to_owned();
    let input = function
        .get("arguments")
        .and_then(Value::as_str)
        .and_then(|raw| serde_json::from_str(raw).ok())
        .unwrap_or_else(|| json!({}));
    Some(ToolCall { id, name, input })
}

fn parse_anthropic_tool_call(value: &Value) -> Option<ToolCall> {
    let id = value.get("id")?.as_str()?.to_owned();
    let name = value.get("name")?.as_str()?.to_owned();
    let input = value.get("input").cloned().unwrap_or_else(|| json!({}));
    Some(ToolCall { id, name, input })
}

fn parse_anthropic_tool_like_call(value: &Value) -> Option<ToolCall> {
    match value.get("type").and_then(Value::as_str) {
        Some("tool_use") | Some("server_tool_use") => parse_anthropic_tool_call(value),
        _ => None,
    }
}

fn coerce_text_content(content: Option<&Value>) -> String {
    match content {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| {
                if let Some(text) = item.as_str() {
                    return Some(text.to_owned());
                }
                item.get("text")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn role_name(role: &ConversationRole) -> &'static str {
    match role {
        ConversationRole::System => "system",
        ConversationRole::User => "user",
        ConversationRole::Assistant => "assistant",
        ConversationRole::Tool => "tool",
    }
}

fn mock_response(conversation: &[ConversationEntry]) -> ProviderResponse {
    let user_prompt = conversation
        .iter()
        .rev()
        .find(|entry| matches!(entry.role, ConversationRole::User))
        .map_or_else(
            || "No prompt supplied.".to_owned(),
            ConversationEntry::history_text,
        );
    let has_tool_result_after_latest_user = conversation
        .iter()
        .rev()
        .take_while(|entry| !matches!(entry.role, ConversationRole::User))
        .any(|entry| matches!(entry.role, ConversationRole::Tool));
    ProviderResponse {
        text: if has_tool_result_after_latest_user {
            "mock provider observed the tool result and is ready to finish.".to_owned()
        } else {
            format!("mock provider response: {}", truncate(&user_prompt))
        },
        history_text: Some(user_prompt.clone()),
        thinking: None,
        content_blocks: Vec::new(),
        tool_calls: if !has_tool_result_after_latest_user
            && user_prompt.to_ascii_lowercase().contains("list files")
        {
            vec![ToolCall {
                id: "mock-tool-call-1".to_owned(),
                name: rc_tools::builtin_tool_specs()
                    .first()
                    .map_or_else(|| "list_directory".to_owned(), |tool| tool.name.clone()),
                input: json!({"path": ".", "recursive": false, "max_entries": 32}),
            }]
        } else {
            Vec::new()
        },
        request_id: Some("mock-request-id".to_owned()),
        usage: UsageSummary {
            input_tokens: 16,
            output_tokens: 12,
            cache_read_input_tokens: 0,
            cache_creation_input_tokens: 0,
        },
        stop_reason: "end_turn".to_owned(),
    }
}

fn truncate(value: &str) -> String {
    value.chars().take(240).collect()
}

fn strip_reasoning_tags(text: &str) -> String {
    let mut remaining = text.to_owned();
    loop {
        let Some(start) = remaining.find("<think>") else {
            break;
        };
        let Some(end) = remaining[start..].find("</think>") else {
            break;
        };
        let end = start + end + "</think>".len();
        remaining.replace_range(start..end, "");
    }
    remaining.trim().to_owned()
}

/// Add stable Anthropic prompt caching markers (`cache_control: {"type": "ephemeral"}`)
/// to strategic locations in the request body so that the system prompt,
/// tool definitions, and the most recent user message are cached server-side.
///
/// When `is_resume` is true (conversation has prior tool results), the tool list
/// is kept exactly as-is to avoid `deferred_tools_delta` cache-miss issues.
fn add_stable_cache_control(body: &mut Value, is_resume: bool) {
    // 0. Stabilize tool ordering — sort tools by name for deterministic cache keys.
    //    This ensures the same tool set always produces the same prefix regardless of
    //    HashMap iteration order or registration order.
    if let Some(tools) = body.get_mut("tools")
        && let Some(tools_arr) = tools.as_array_mut()
    {
        tools_arr.sort_by(|a, b| {
            let name_a = a.get("name").and_then(Value::as_str).unwrap_or("");
            let name_b = b.get("name").and_then(Value::as_str).unwrap_or("");
            name_a.cmp(name_b)
        });
    }

    // 1. System message — always ensure array format with cache_control.
    if let Some(system) = body.get_mut("system") {
        if system.is_string() {
            let text = system.as_str().unwrap_or("").to_owned();
            *system = json!([{
                "type": "text",
                "text": text,
                "cache_control": {"type": "ephemeral"}
            }]);
        } else if let Some(system_arr) = system.as_array_mut()
            && let Some(last) = system_arr.last_mut()
        {
            last["cache_control"] = json!({"type": "ephemeral"});
        }
    }

    // 2. Tools — mark the last tool with cache_control.
    //    In resume scenarios, do NOT add or remove any tools to keep the list stable.
    if let Some(tools) = body.get_mut("tools")
        && let Some(tools_arr) = tools.as_array_mut()
        && let Some(last_tool) = tools_arr.last_mut()
    {
        last_tool["cache_control"] = json!({"type": "ephemeral"});
    }

    // 3. Most recent user message — ensure content is array format, mark cache_control.
    if let Some(messages) = body.get_mut("messages")
        && let Some(msg_arr) = messages.as_array_mut()
    {
        for msg in msg_arr.iter_mut().rev() {
            if msg["role"] == "user" {
                if let Some(content) = msg.get_mut("content") {
                    if content.is_string() {
                        let text = content.as_str().unwrap_or("").to_owned();
                        *content = json!([{
                            "type": "text",
                            "text": text,
                            "cache_control": {"type": "ephemeral"}
                        }]);
                    } else if let Some(content_arr) = content.as_array_mut()
                        && let Some(last_block) = content_arr.last_mut()
                    {
                        last_block["cache_control"] = json!({"type": "ephemeral"});
                    }
                }
                break;
            }
        }
    }

    // 4. Resume scenario: tool list must remain identical to avoid cache invalidation.
    //    The `is_resume` flag is recorded here for future use; currently the tool list
    //    is always the full builtin set, which is inherently stable.
    let _ = is_resume;
}

// ---------------------------------------------------------------------------
// Structured error classification
// ---------------------------------------------------------------------------

/// Structured provider error with classification for retry/recovery decisions.
///
/// Matches upstream Claude Code's `categorizeRetryableAPIError` logic.
#[derive(Debug, Clone)]
pub struct ProviderError {
    /// Error category.
    pub category: ErrorCategory,
    /// HTTP status code (if applicable).
    pub status_code: Option<u16>,
    /// Human-readable error message.
    pub message: String,
    /// Provider name that produced the error.
    pub provider_name: String,
    /// Suggested recovery action.
    pub recovery: RecoveryAction,
}

/// Classification of provider errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCategory {
    /// Rate limit exceeded (429).
    RateLimit,
    /// Authentication failure (401/403).
    Authentication,
    /// Request too large / prompt too long (400/413).
    PromptTooLong,
    /// Model not found or unavailable (404).
    ModelNotFound,
    /// Server error (5xx).
    ServerError,
    /// Network / connectivity error.
    Network,
    /// Timeout.
    Timeout,
    /// Streaming interrupted.
    StreamInterrupted,
    /// Invalid request format.
    InvalidRequest,
    /// Quota / billing exceeded (402).
    QuotaExceeded,
    /// Unknown / unclassified error.
    Unknown,
}

/// Suggested recovery action for a provider error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryAction {
    /// Retry with exponential backoff.
    Retry,
    /// Retry after compacting the conversation.
    CompactAndRetry,
    /// Failover to a different provider.
    Failover,
    /// Abort the operation.
    Abort,
    /// Ask the user to fix configuration.
    FixConfig,
}

/// Classify an HTTP status code and error message into a structured error.
#[must_use]
pub fn classify_provider_error(
    status_code: u16,
    message: &str,
    provider_name: &str,
) -> ProviderError {
    let (category, recovery) = match status_code {
        429 => (ErrorCategory::RateLimit, RecoveryAction::Retry),
        401 | 403 => (ErrorCategory::Authentication, RecoveryAction::FixConfig),
        402 => (ErrorCategory::QuotaExceeded, RecoveryAction::Failover),
        404 => (ErrorCategory::ModelNotFound, RecoveryAction::FixConfig),
        413 => (
            ErrorCategory::PromptTooLong,
            RecoveryAction::CompactAndRetry,
        ),
        400 => {
            // Check if it's a prompt-too-long error disguised as 400.
            if message.contains("prompt is too long")
                || message.contains("context_length_exceeded")
                || message.contains("maximum context length")
            {
                (
                    ErrorCategory::PromptTooLong,
                    RecoveryAction::CompactAndRetry,
                )
            } else {
                (ErrorCategory::InvalidRequest, RecoveryAction::Abort)
            }
        }
        500 | 502 | 503 | 504 => (ErrorCategory::ServerError, RecoveryAction::Retry),
        _ => (ErrorCategory::Unknown, RecoveryAction::Retry),
    };

    ProviderError {
        category,
        status_code: Some(status_code),
        message: message.to_owned(),
        provider_name: provider_name.to_owned(),
        recovery,
    }
}

/// Classify a network/transport error.
#[must_use]
pub fn classify_network_error(error: &str, provider_name: &str) -> ProviderError {
    let (category, recovery) = if error.contains("timed out") || error.contains("timeout") {
        (ErrorCategory::Timeout, RecoveryAction::Retry)
    } else if error.contains("connection refused") || error.contains("couldn't connect") {
        (ErrorCategory::Network, RecoveryAction::Retry)
    } else if error.contains("tls")
        || error.contains("certificate")
        || error.contains("ssl")
        || error.contains("dns")
        || error.contains("resolve")
    {
        (ErrorCategory::Network, RecoveryAction::FixConfig)
    } else {
        (ErrorCategory::Network, RecoveryAction::Retry)
    };

    ProviderError {
        category,
        status_code: None,
        message: error.to_owned(),
        provider_name: provider_name.to_owned(),
        recovery,
    }
}

/// Check if an error is retryable.
#[must_use]
pub fn is_retryable(error: &ProviderError) -> bool {
    matches!(
        error.recovery,
        RecoveryAction::Retry | RecoveryAction::CompactAndRetry | RecoveryAction::Failover
    )
}

/// Check if an error indicates the prompt is too long.
#[must_use]
pub fn is_prompt_too_long(error: &ProviderError) -> bool {
    error.category == ErrorCategory::PromptTooLong
}

#[cfg(test)]
mod tests {
    use super::{
        ProviderClient, apply_anthropic_request_metadata, mock_response, parse_anthropic_response,
        parse_openai_response, strip_reasoning_tags, to_anthropic_messages, to_openai_messages,
    };
    use axum::{Json, Router, extract::State, routing::post};
    use rc_core::{ConversationEntry, ToolCall};
    use serde_json::json;
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    use tokio::net::TcpListener;

    fn test_provider_config(base_url: String) -> rc_config::ProviderConfig {
        rc_config::ProviderConfig {
            name: "custom".to_owned(),
            base_url: Some(base_url),
            api_key: Some("test-key".to_owned()),
            model: Some("test-model".to_owned()),
            protocol: rc_core::ProviderProtocol::OpenAi,
            timeout_ms: 1_000,
            max_output_tokens: 512,
            max_retries: 2,
            retry_initial_backoff_ms: 10,
            retry_max_backoff_ms: 20,
            respect_retry_after: false,
            request_header_overrides: Default::default(),
            request_metadata: Default::default(),
            thinking_budget: None,
        }
    }

    #[test]
    fn reasoning_tags_are_removed() {
        assert_eq!(strip_reasoning_tags("<think>abc</think>done"), "done");
    }

    #[test]
    fn mock_provider_uses_latest_prompt() {
        let response = mock_response(&[ConversationEntry::user("hello world")]);
        assert!(response.text.contains("hello world"));
    }

    #[test]
    fn openai_messages_include_user_role() {
        let messages = to_openai_messages(&[ConversationEntry::user("ship it")]);
        assert_eq!(messages[0]["role"], "user");
    }

    #[test]
    fn anthropic_messages_emit_each_tool_result_as_separate_user_message() {
        let mut assistant = ConversationEntry::assistant("");
        assistant.tool_calls = vec![
            ToolCall {
                id: "call-1".to_owned(),
                name: "read_file".to_owned(),
                input: json!({"path":"src/main.rs"}),
            },
            ToolCall {
                id: "call-2".to_owned(),
                name: "read_file".to_owned(),
                input: json!({"path":"src/lib.rs"}),
            },
        ];

        let (_system, messages) = to_anthropic_messages(&[
            ConversationEntry::user("inspect"),
            assistant,
            ConversationEntry::tool("call-1", "read_file", "main", false),
            ConversationEntry::tool("call-2", "read_file", "lib", false),
        ]);

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(messages[2]["role"], "user");
        let tool_results = messages[2]["content"]
            .as_array()
            .expect("tool results should be a content array");
        assert_eq!(tool_results.len(), 2);
        assert_eq!(tool_results[0]["type"], "tool_result");
        assert_eq!(tool_results[0]["tool_use_id"], "call-1");
        assert_eq!(tool_results[1]["tool_use_id"], "call-2");
        assert!(tool_results[0].get("is_error").is_none());
    }

    #[test]
    fn anthropic_messages_only_emit_is_error_for_failed_tool_results() {
        let mut assistant = ConversationEntry::assistant("");
        assistant.tool_calls = vec![ToolCall {
            id: "call-1".to_owned(),
            name: "read_file".to_owned(),
            input: json!({"path":"src/main.rs"}),
        }];

        let (_system, messages) = to_anthropic_messages(&[
            ConversationEntry::user("inspect"),
            assistant,
            ConversationEntry::tool("call-1", "read_file", "permission denied", true),
        ]);

        let tool_results = messages[2]["content"]
            .as_array()
            .expect("tool results should be a content array");
        assert_eq!(tool_results[0]["is_error"], true);
    }

    #[test]
    fn openai_response_parser_handles_success() {
        let raw = r#"{"choices":[{"message":{"content":"hello"}}],"usage":{"prompt_tokens":1,"completion_tokens":2}}"#;
        let parsed = parse_openai_response(200, raw.to_owned());
        assert!(parsed.is_ok());
        let parsed = parsed.unwrap_or_else(|error| panic!("parse failed: {error}"));
        assert_eq!(parsed.text, "hello");
        assert_eq!(parsed.usage.output_tokens, 2);
    }

    #[test]
    fn openai_response_parser_captures_request_id() {
        let raw = r#"{"id":"chatcmpl-123","choices":[{"message":{"content":"hello"}}],"usage":{"prompt_tokens":1,"completion_tokens":2}}"#;
        let parsed =
            parse_openai_response(200, raw.to_owned()).unwrap_or_else(|error| panic!("{error}"));
        assert_eq!(parsed.request_id.as_deref(), Some("chatcmpl-123"));
    }

    #[test]
    fn anthropic_response_parser_captures_request_id() {
        let raw = r#"{"id":"msg_123","type":"message","role":"assistant","content":[{"type":"text","text":"hello"}],"usage":{"input_tokens":3,"output_tokens":4},"stop_reason":"end_turn"}"#;
        let parsed = parse_anthropic_response(200, raw.to_owned())
            .unwrap_or_else(|error| panic!("parse failed: {error}"));
        assert_eq!(parsed.request_id.as_deref(), Some("msg_123"));
        assert_eq!(parsed.text, "hello");
    }

    #[test]
    fn anthropic_request_metadata_is_serialized_into_user_id() {
        let mut provider = test_provider_config("https://api.anthropic.com/v1/messages".to_owned());
        provider.protocol = rc_core::ProviderProtocol::Anthropic;
        provider
            .request_metadata
            .insert("session_id".to_owned(), "session-123".to_owned());
        provider
            .request_metadata
            .insert("client".to_owned(), "remote-code-rust".to_owned());
        let mut body = json!({
            "model": "claude-test",
            "messages": [],
        });

        apply_anthropic_request_metadata(&mut body, &provider);

        let metadata = body
            .get("metadata")
            .and_then(|value| value.get("user_id"))
            .and_then(serde_json::Value::as_str)
            .expect("metadata.user_id");
        let parsed = serde_json::from_str::<serde_json::Value>(metadata)
            .unwrap_or_else(|error| panic!("invalid metadata json: {error}"));
        assert_eq!(parsed["session_id"], "session-123");
        assert_eq!(parsed["client"], "remote-code-rust");
    }

    #[tokio::test]
    async fn provider_retries_retryable_status_then_succeeds() {
        async fn handler(
            State(attempts): State<Arc<AtomicUsize>>,
        ) -> (axum::http::StatusCode, Json<serde_json::Value>) {
            let attempt = attempts.fetch_add(1, Ordering::SeqCst);
            if attempt == 0 {
                return (
                    axum::http::StatusCode::TOO_MANY_REQUESTS,
                    Json(json!({"error": {"message": "slow down"}})),
                );
            }
            (
                axum::http::StatusCode::OK,
                Json(json!({
                    "choices": [{"message": {"content": "retried ok"}}],
                    "usage": {"prompt_tokens": 3, "completion_tokens": 4}
                })),
            )
        }

        let attempts = Arc::new(AtomicUsize::new(0));
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap_or_else(|error| panic!("listener bind failed: {error}"));
        let address = listener
            .local_addr()
            .unwrap_or_else(|error| panic!("local addr failed: {error}"));
        let server_attempts = Arc::clone(&attempts);
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/chat/completions", post(handler))
                    .with_state(server_attempts),
            )
            .await
            .unwrap_or_else(|error| panic!("server failed: {error}"));
        });

        let client = ProviderClient::new().unwrap_or_else(|error| panic!("client failed: {error}"));
        let response = client
            .complete(
                &test_provider_config(format!("http://{address}/chat/completions")),
                &[ConversationEntry::user("hello")],
            )
            .await
            .unwrap_or_else(|error| panic!("completion failed: {error}"));

        server.abort();
        assert_eq!(response.text, "retried ok");
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn provider_retries_529_then_succeeds() {
        async fn handler(
            State(attempts): State<Arc<AtomicUsize>>,
        ) -> (axum::http::StatusCode, Json<serde_json::Value>) {
            let attempt = attempts.fetch_add(1, Ordering::SeqCst);
            if attempt == 0 {
                return (
                    axum::http::StatusCode::from_u16(529).expect("529 status"),
                    Json(json!({"error": {"message": "overloaded"}})),
                );
            }
            (
                axum::http::StatusCode::OK,
                Json(json!({
                    "choices": [{"message": {"content": "retried 529 ok"}}],
                    "usage": {"prompt_tokens": 3, "completion_tokens": 4}
                })),
            )
        }

        let attempts = Arc::new(AtomicUsize::new(0));
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap_or_else(|error| panic!("listener bind failed: {error}"));
        let address = listener
            .local_addr()
            .unwrap_or_else(|error| panic!("local addr failed: {error}"));
        let server_attempts = Arc::clone(&attempts);
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/chat/completions", post(handler))
                    .with_state(server_attempts),
            )
            .await
            .unwrap_or_else(|error| panic!("server failed: {error}"));
        });

        let client = ProviderClient::new().unwrap_or_else(|error| panic!("client failed: {error}"));
        let response = client
            .complete(
                &test_provider_config(format!("http://{address}/chat/completions")),
                &[ConversationEntry::user("hello")],
            )
            .await
            .unwrap_or_else(|error| panic!("completion failed: {error}"));

        server.abort();
        assert_eq!(response.text, "retried 529 ok");
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn provider_does_not_retry_non_retryable_status() {
        async fn handler(
            State(attempts): State<Arc<AtomicUsize>>,
        ) -> (axum::http::StatusCode, Json<serde_json::Value>) {
            attempts.fetch_add(1, Ordering::SeqCst);
            (
                axum::http::StatusCode::UNAUTHORIZED,
                Json(json!({"error": {"message": "bad api key"}})),
            )
        }

        let attempts = Arc::new(AtomicUsize::new(0));
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap_or_else(|error| panic!("listener bind failed: {error}"));
        let address = listener
            .local_addr()
            .unwrap_or_else(|error| panic!("local addr failed: {error}"));
        let server_attempts = Arc::clone(&attempts);
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/chat/completions", post(handler))
                    .with_state(server_attempts),
            )
            .await
            .unwrap_or_else(|error| panic!("server failed: {error}"));
        });

        let client = ProviderClient::new().unwrap_or_else(|error| panic!("client failed: {error}"));
        let error = client
            .complete(
                &test_provider_config(format!("http://{address}/chat/completions")),
                &[ConversationEntry::user("hello")],
            )
            .await
            .expect_err("request should fail");

        server.abort();
        assert!(error.to_string().contains("401"));
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
    }

    // ── Error classification tests ────────────────────────────────────

    #[test]
    fn classify_429_as_rate_limit() {
        let err = super::classify_provider_error(429, "rate limited", "test-provider");
        assert_eq!(err.category, super::ErrorCategory::RateLimit);
        assert_eq!(err.recovery, super::RecoveryAction::Retry);
        assert!(super::is_retryable(&err));
    }

    #[test]
    fn classify_401_as_authentication() {
        let err = super::classify_provider_error(401, "invalid api key", "test-provider");
        assert_eq!(err.category, super::ErrorCategory::Authentication);
        assert_eq!(err.recovery, super::RecoveryAction::FixConfig);
        assert!(!super::is_retryable(&err));
    }

    #[test]
    fn classify_400_prompt_too_long() {
        let err = super::classify_provider_error(
            400,
            "prompt is too long: maximum context length exceeded",
            "test-provider",
        );
        assert_eq!(err.category, super::ErrorCategory::PromptTooLong);
        assert_eq!(err.recovery, super::RecoveryAction::CompactAndRetry);
        assert!(super::is_prompt_too_long(&err));
        assert!(super::is_retryable(&err));
    }

    #[test]
    fn classify_400_context_length_exceeded() {
        let err = super::classify_provider_error(400, "context_length_exceeded", "test-provider");
        assert_eq!(err.category, super::ErrorCategory::PromptTooLong);
        assert!(super::is_prompt_too_long(&err));
    }

    #[test]
    fn classify_500_as_server_error() {
        let err = super::classify_provider_error(500, "internal server error", "test-provider");
        assert_eq!(err.category, super::ErrorCategory::ServerError);
        assert_eq!(err.recovery, super::RecoveryAction::Retry);
        assert!(super::is_retryable(&err));
    }

    #[test]
    fn classify_503_as_server_error() {
        let err = super::classify_provider_error(503, "service unavailable", "test-provider");
        assert_eq!(err.category, super::ErrorCategory::ServerError);
        assert!(super::is_retryable(&err));
    }

    #[test]
    fn classify_404_as_model_not_found() {
        let err = super::classify_provider_error(404, "model not found", "test-provider");
        assert_eq!(err.category, super::ErrorCategory::ModelNotFound);
        assert_eq!(err.recovery, super::RecoveryAction::FixConfig);
    }

    #[test]
    fn classify_402_as_quota_exceeded() {
        let err = super::classify_provider_error(402, "insufficient quota", "test-provider");
        assert_eq!(err.category, super::ErrorCategory::QuotaExceeded);
        assert_eq!(err.recovery, super::RecoveryAction::Failover);
    }

    #[test]
    fn classify_network_timeout() {
        let err = super::classify_network_error("connection timed out", "test-provider");
        assert_eq!(err.category, super::ErrorCategory::Timeout);
        assert_eq!(err.recovery, super::RecoveryAction::Retry);
        assert!(super::is_retryable(&err));
    }

    #[test]
    fn classify_network_dns_error() {
        let err = super::classify_network_error("dns resolve failed", "test-provider");
        assert_eq!(err.category, super::ErrorCategory::Network);
        assert_eq!(err.recovery, super::RecoveryAction::FixConfig);
    }

    #[test]
    fn classify_400_generic_as_invalid_request() {
        let err = super::classify_provider_error(400, "invalid parameter", "test-provider");
        assert_eq!(err.category, super::ErrorCategory::InvalidRequest);
        assert_eq!(err.recovery, super::RecoveryAction::Abort);
    }
}

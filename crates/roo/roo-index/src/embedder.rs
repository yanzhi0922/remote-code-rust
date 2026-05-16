//! Embedder trait and implementations for code indexing.
//!
//! Corresponds to the `embedders/` directory in the TypeScript source.
//!
//! Provides a unified trait interface for creating embeddings from text,
//! with support for multiple providers (OpenAI, Ollama, Bedrock, etc.).

use async_trait::async_trait;
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::Digest;

use crate::types::IndexError;

/// Response from an embedding creation request.
#[derive(Clone, Debug)]
pub struct EmbeddingResponse {
    /// The embedding vectors.
    pub embeddings: Vec<Vec<f64>>,
}

/// Trait for embedding providers.
///
/// Corresponds to `IEmbedder` in the TypeScript source's interfaces.
#[async_trait]
pub trait Embedder: Send + Sync {
    /// Creates embeddings for the given texts.
    async fn create_embeddings(&self, texts: &[&str]) -> Result<EmbeddingResponse, IndexError>;

    /// Returns the dimension of the embeddings produced by this embedder.
    fn dimension(&self) -> usize;

    /// Validates that the embedder is properly configured.
    fn validate_configuration(&self) -> Result<bool, IndexError> {
        Ok(true)
    }
}

/// Configuration for creating an embedder.
#[derive(Clone, Debug)]
pub enum EmbedderConfig {
    Openai {
        api_key: String,
        model_id: Option<String>,
    },
    Ollama {
        base_url: String,
        model_id: Option<String>,
    },
    OpenaiCompatible {
        base_url: String,
        api_key: String,
        model_id: Option<String>,
    },
    Gemini {
        api_key: String,
        model_id: Option<String>,
    },
    Mistral {
        api_key: String,
        model_id: Option<String>,
    },
    Bedrock {
        region: String,
        profile: Option<String>,
        model_id: Option<String>,
    },
    Openrouter {
        api_key: String,
        model_id: Option<String>,
        specific_provider: Option<String>,
    },
    VercelAiGateway {
        api_key: String,
        model_id: Option<String>,
    },
}

// ---------------------------------------------------------------------------
// NoopEmbedder — returns zero vectors (testing / placeholder)
// ---------------------------------------------------------------------------

/// A simple embedder that returns zero vectors.
/// Used for testing and as a placeholder when no real embedder is configured.
pub struct NoopEmbedder {
    dimension: usize,
}

impl NoopEmbedder {
    pub fn new(dimension: usize) -> Self {
        Self { dimension }
    }
}

#[async_trait]
impl Embedder for NoopEmbedder {
    async fn create_embeddings(&self, texts: &[&str]) -> Result<EmbeddingResponse, IndexError> {
        let embeddings = texts.iter().map(|_| vec![0.0; self.dimension]).collect();
        Ok(EmbeddingResponse { embeddings })
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn validate_configuration(&self) -> Result<bool, IndexError> {
        Ok(true)
    }
}

// ---------------------------------------------------------------------------
// HttpEmbedder — OpenAI-compatible and Ollama embedder
// ---------------------------------------------------------------------------

/// OpenAI-compatible embedder (used for OpenAI, OpenRouter, Gemini, Mistral,
/// Vercel AI Gateway, OpenAI Compatible).
struct HttpEmbedder {
    client: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
    dimension: usize,
    /// Whether to use /api/embed (Ollama format) or /v1/embeddings (OpenAI format).
    is_ollama: bool,
    /// Custom headers to include in every request.
    extra_headers: Vec<(String, String)>,
}

// -- OpenAI request / response types --

#[derive(Serialize)]
struct OpenAiEmbedRequest {
    model: String,
    input: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    encoding_format: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiEmbedResponse {
    data: Vec<OpenAiEmbedData>,
}

#[derive(Deserialize)]
struct OpenAiEmbedData {
    embedding: serde_json::Value, // Can be array of f64 or base64 string
}

// -- Ollama request / response types --

#[derive(Serialize)]
struct OllamaEmbedRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Deserialize)]
struct OllamaEmbedResponse {
    embeddings: Vec<Vec<f64>>,
}

#[async_trait]
impl Embedder for HttpEmbedder {
    async fn create_embeddings(&self, texts: &[&str]) -> Result<EmbeddingResponse, IndexError> {
        if self.is_ollama {
            self.embed_ollama(texts).await
        } else {
            self.embed_openai(texts).await
        }
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn validate_configuration(&self) -> Result<bool, IndexError> {
        Ok(!self.api_key.is_empty() || self.is_ollama)
    }
}

impl HttpEmbedder {
    /// Call the OpenAI `/v1/embeddings` endpoint.
    async fn embed_openai(&self, texts: &[&str]) -> Result<EmbeddingResponse, IndexError> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", self.api_key).parse().map_err(
                |e: reqwest::header::InvalidHeaderValue| {
                    IndexError::GeneralError(format!("Invalid API key header: {e}"))
                },
            )?,
        );
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        for (k, v) in &self.extra_headers {
            let header_name = reqwest::header::HeaderName::from_bytes(k.as_bytes())
                .map_err(|e| IndexError::GeneralError(format!("Invalid header name '{k}': {e}")))?;
            let header_value = v
                .parse()
                .map_err(|e: reqwest::header::InvalidHeaderValue| {
                    IndexError::GeneralError(format!("Invalid header value for '{k}': {e}"))
                })?;
            headers.insert(header_name, header_value);
        }

        let body = OpenAiEmbedRequest {
            model: self.model.clone(),
            input: texts.iter().map(|t| t.to_string()).collect(),
            encoding_format: Some("float".to_string()),
        };

        let url = format!("{}/embeddings", self.base_url.trim_end_matches('/'));

        let resp = self
            .client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| IndexError::GeneralError(format!("Embedding request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(IndexError::GeneralError(format!(
                "Embedding API returned status {}: {text}",
                status.as_u16()
            )));
        }

        let embed_resp: OpenAiEmbedResponse = resp.json().await.map_err(|e| {
            IndexError::GeneralError(format!("Failed to parse embedding response: {e}"))
        })?;

        let mut result = Vec::with_capacity(embed_resp.data.len());
        for item in &embed_resp.data {
            match &item.embedding {
                serde_json::Value::Array(arr) => {
                    result.push(arr.iter().filter_map(|v| v.as_f64()).collect());
                }
                serde_json::Value::String(s) => {
                    // Base64 encoded floats - decode
                    let bytes = base64::engine::general_purpose::STANDARD
                        .decode(s)
                        .map_err(|e| {
                            IndexError::GeneralError(format!("Base64 decode error: {e}"))
                        })?;
                    let floats: Vec<f64> = bytes
                        .chunks_exact(4)
                        .map(|chunk| {
                            f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as f64
                        })
                        .collect();
                    result.push(floats);
                }
                _ => {
                    return Err(IndexError::GeneralError(
                        "Unexpected embedding format in response".to_string(),
                    ));
                }
            }
        }

        Ok(EmbeddingResponse { embeddings: result })
    }

    /// Call the Ollama `/api/embed` endpoint.
    async fn embed_ollama(&self, texts: &[&str]) -> Result<EmbeddingResponse, IndexError> {
        let body = OllamaEmbedRequest {
            model: self.model.clone(),
            input: texts.iter().map(|t| t.to_string()).collect(),
        };

        // Strip /v1 suffix if present to get the base Ollama URL
        let base = self.base_url.trim_end_matches("/v1").trim_end_matches('/');
        let url = format!("{}/api/embed", base);

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                IndexError::GeneralError(format!("Ollama embedding request failed: {e}"))
            })?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(IndexError::GeneralError(format!(
                "Ollama embedding API returned status {}: {text}",
                status.as_u16()
            )));
        }

        let embed_resp: OllamaEmbedResponse = resp.json().await.map_err(|e| {
            IndexError::GeneralError(format!("Failed to parse Ollama embedding response: {e}"))
        })?;

        Ok(EmbeddingResponse {
            embeddings: embed_resp.embeddings,
        })
    }
}

// ---------------------------------------------------------------------------
// BedrockEmbedder — AWS Bedrock Runtime invoke-model embedder
// ---------------------------------------------------------------------------

/// AWS Bedrock embedder using SigV4-signed InvokeModel API calls.
///
/// Parity with TypeScript `BedrockEmbedder` in `embedders/bedrock.ts`.
struct BedrockEmbedder {
    client: reqwest::Client,
    region: String,
    access_key: String,
    secret_key: String,
    session_token: Option<String>,
    model: String,
    dimension: usize,
}

#[async_trait]
impl Embedder for BedrockEmbedder {
    async fn create_embeddings(&self, texts: &[&str]) -> Result<EmbeddingResponse, IndexError> {
        let mut all_embeddings = Vec::with_capacity(texts.len());
        // Bedrock InvokeModel processes one text at a time (like the TS source)
        for text in texts {
            let embedding = self.invoke_model(text).await?;
            all_embeddings.push(embedding);
        }
        Ok(EmbeddingResponse {
            embeddings: all_embeddings,
        })
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    fn validate_configuration(&self) -> Result<bool, IndexError> {
        if self.access_key.is_empty() || self.secret_key.is_empty() {
            return Err(IndexError::GeneralError(
                "Bedrock credentials (AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY) are required"
                    .to_string(),
            ));
        }
        Ok(true)
    }
}

impl BedrockEmbedder {
    /// Invoke a Bedrock embedding model for a single text.
    ///
    /// Handles model-specific request/response formats:
    /// - Nova multimodal: `taskType= SINGLE_EMBEDDING`
    /// - Titan: `inputText`
    /// - Cohere v4: `texts` + `embedding_types`
    /// - Cohere v3: `texts`
    async fn invoke_model(&self, text: &str) -> Result<Vec<f64>, IndexError> {
        let url = format!(
            "https://bedrock-runtime.{}.amazonaws.com/model/{}/invoke",
            self.region, self.model
        );

        let body = self.build_request_body(text);
        let body_bytes = serde_json::to_vec(&body).map_err(|e| {
            IndexError::GeneralError(format!("Failed to serialize Bedrock request: {e}"))
        })?;

        let timestamp = chrono::Utc::now();
        let auth_header = sign_sigv4(
            "POST",
            &url,
            &body_bytes,
            &timestamp,
            &self.access_key,
            &self.secret_key,
            &self.region,
            "bedrock",
        );

        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            auth_header
                .parse()
                .map_err(|e: reqwest::header::InvalidHeaderValue| {
                    IndexError::GeneralError(format!("Invalid auth header: {e}"))
                })?,
        );
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        headers.insert(
            "x-amz-date",
            timestamp
                .format("%Y%m%dT%H%M%SZ")
                .to_string()
                .parse()
                .unwrap(),
        );
        let payload_hash = hex_encode(&sha2::Sha256::digest(&body_bytes));
        headers.insert("x-amz-content-sha256", payload_hash.parse().unwrap());
        if let Some(ref token) = self.session_token {
            headers.insert(
                "x-amz-security-token",
                token
                    .parse()
                    .map_err(|e: reqwest::header::InvalidHeaderValue| {
                        IndexError::GeneralError(format!("Invalid session token: {e}"))
                    })?,
            );
        }

        let resp = self
            .client
            .post(&url)
            .headers(headers)
            .body(body_bytes)
            .send()
            .await
            .map_err(|e| IndexError::GeneralError(format!("Bedrock request failed: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(IndexError::GeneralError(format!(
                "Bedrock API returned status {}: {text}",
                status.as_u16()
            )));
        }

        let response_body: serde_json::Value = resp.json().await.map_err(|e| {
            IndexError::GeneralError(format!("Failed to parse Bedrock response: {e}"))
        })?;

        self.extract_embedding(&response_body)
    }

    /// Build the model-specific request body.
    fn build_request_body(&self, text: &str) -> serde_json::Value {
        if self.model.starts_with("amazon.nova-2-multimodal") {
            serde_json::json!({
                "taskType": "SINGLE_EMBEDDING",
                "singleEmbeddingParams": {
                    "embeddingPurpose": "GENERIC_INDEX",
                    "embeddingDimension": 1024,
                    "text": {
                        "truncationMode": "END",
                        "value": text
                    }
                }
            })
        } else if self.model.starts_with("amazon.titan-embed") {
            serde_json::json!({ "inputText": text })
        } else if self.model.starts_with("cohere.embed-v4") {
            serde_json::json!({
                "texts": [text],
                "input_type": "search_document",
                "embedding_types": ["float"]
            })
        } else if self.model.starts_with("cohere.embed") {
            serde_json::json!({
                "texts": [text],
                "input_type": "search_document"
            })
        } else {
            // Default to Titan format
            serde_json::json!({ "inputText": text })
        }
    }

    /// Extract the embedding vector from a model-specific response.
    fn extract_embedding(&self, body: &serde_json::Value) -> Result<Vec<f64>, IndexError> {
        let embedding = if self.model.starts_with("amazon.nova-2-multimodal") {
            // Nova: { embeddings: [{ embedding: [...] }] }
            body.get("embeddings")
                .and_then(|e| e.get(0))
                .and_then(|e| e.get("embedding"))
                .cloned()
                .unwrap_or_else(|| {
                    body.get("embedding")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null)
                })
        } else if self.model.starts_with("amazon.titan-embed") {
            body.get("embedding")
                .cloned()
                .unwrap_or(serde_json::Value::Null)
        } else if self.model.starts_with("cohere.embed-v4") {
            // Cohere v4: { embeddings: { float: [[...]] } }
            body.get("embeddings")
                .and_then(|e| e.get("float"))
                .and_then(|e| e.get(0))
                .cloned()
                .or_else(|| {
                    body.get("embeddings")
                        .and_then(|e| e.as_array())
                        .and_then(|a| a.first().cloned())
                })
                .unwrap_or(serde_json::Value::Null)
        } else if self.model.starts_with("cohere.embed") {
            // Cohere v3: { embeddings: [[...]] }
            body.get("embeddings")
                .and_then(|e| e.as_array())
                .and_then(|a| a.first().cloned())
                .unwrap_or(serde_json::Value::Null)
        } else {
            body.get("embedding")
                .cloned()
                .unwrap_or(serde_json::Value::Null)
        };

        match embedding {
            serde_json::Value::Array(arr) => Ok(arr.iter().filter_map(|v| v.as_f64()).collect()),
            other => Err(IndexError::GeneralError(format!(
                "Unexpected Bedrock embedding format: {other}"
            ))),
        }
    }
}

// ---------------------------------------------------------------------------
// SigV4 signing helpers (inline — mirrors roo-provider-aws/src/signing.rs)
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn sign_sigv4(
    method: &str,
    url: &str,
    body: &[u8],
    timestamp: &chrono::DateTime<chrono::Utc>,
    access_key: &str,
    secret_key: &str,
    region: &str,
    service: &str,
) -> String {
    let date_stamp = timestamp.format("%Y%m%d").to_string();
    let amz_date = timestamp.format("%Y%m%dT%H%M%SZ").to_string();

    let (host, path, query) = parse_url_sigv4(url);
    let payload_hash = hex_encode(&sha2::Sha256::digest(body));

    let canonical_headers = format!(
        "content-type:application/json\nhost:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n"
    );
    let signed_headers = "content-type;host;x-amz-content-sha256;x-amz-date";
    let canonical_querystring = query.as_deref().unwrap_or("");

    let canonical_request = format!(
        "{method}\n{path}\n{canonical_querystring}\n{canonical_headers}\n{signed_headers}\n{payload_hash}"
    );

    let credential_scope = format!("{date_stamp}/{region}/{service}/aws4_request");
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{credential_scope}\n{}",
        hex_encode(&sha2::Sha256::digest(canonical_request.as_bytes()))
    );

    let signing_key = get_signature_key(secret_key, &date_stamp, region, service);
    let signature_hex = hex_encode(&hmac_sha256(&signing_key, string_to_sign.as_bytes()));

    format!(
        "AWS4-HMAC-SHA256 Credential={access_key}/{credential_scope}, SignedHeaders={signed_headers}, Signature={signature_hex}"
    )
}

fn parse_url_sigv4(url: &str) -> (String, String, Option<String>) {
    let without_scheme = url.strip_prefix("https://").unwrap_or(url);
    let parts: Vec<&str> = without_scheme.splitn(2, '/').collect();
    let host = parts[0].to_string();
    let rest = parts.get(1).unwrap_or(&"");
    let (path, query) = if let Some(pos) = rest.find('?') {
        (rest[..pos].to_string(), Some(rest[pos + 1..].to_string()))
    } else {
        (rest.to_string(), None)
    };
    let path = if path.is_empty() {
        "/".to_string()
    } else {
        format!("/{path}")
    };
    (host, path, query)
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC key");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

fn get_signature_key(secret_key: &str, date_stamp: &str, region: &str, service: &str) -> Vec<u8> {
    let k_date = hmac_sha256(
        format!("AWS4{secret_key}").as_bytes(),
        date_stamp.as_bytes(),
    );
    let k_region = hmac_sha256(&k_date, region.as_bytes());
    let k_service = hmac_sha256(&k_region, service.as_bytes());
    hmac_sha256(&k_service, b"aws4_request")
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------
// Factory function
// ---------------------------------------------------------------------------

/// Factory function to create an embedder from configuration.
pub fn create_embedder(config: &EmbedderConfig) -> Result<Box<dyn Embedder>, IndexError> {
    match config {
        EmbedderConfig::Openai { api_key, model_id } => {
            if api_key.is_empty() {
                return Err(IndexError::GeneralError(
                    "OpenAI API key is required".to_string(),
                ));
            }
            let model = model_id
                .as_deref()
                .unwrap_or("text-embedding-3-small")
                .to_string();
            let dimension = 1536;
            Ok(Box::new(HttpEmbedder {
                client: reqwest::Client::new(),
                base_url: "https://api.openai.com/v1".to_string(),
                api_key: api_key.clone(),
                model,
                dimension,
                is_ollama: false,
                extra_headers: vec![],
            }))
        }
        EmbedderConfig::Ollama { base_url, model_id } => {
            if base_url.is_empty() {
                return Err(IndexError::GeneralError(
                    "Ollama base URL is required".to_string(),
                ));
            }
            let model = model_id
                .as_deref()
                .unwrap_or("nomic-embed-text")
                .to_string();
            Ok(Box::new(HttpEmbedder {
                client: reqwest::Client::new(),
                base_url: base_url.clone(),
                api_key: String::new(), // Ollama doesn't need an API key
                model,
                dimension: 4096,
                is_ollama: true,
                extra_headers: vec![],
            }))
        }
        EmbedderConfig::OpenaiCompatible {
            base_url,
            api_key,
            model_id,
        } => {
            if base_url.is_empty() || api_key.is_empty() {
                return Err(IndexError::GeneralError(
                    "OpenAI Compatible base URL and API key are required".to_string(),
                ));
            }
            let model = model_id
                .as_deref()
                .unwrap_or("text-embedding-3-small")
                .to_string();
            Ok(Box::new(HttpEmbedder {
                client: reqwest::Client::new(),
                base_url: base_url.clone(),
                api_key: api_key.clone(),
                model,
                dimension: 1536,
                is_ollama: false,
                extra_headers: vec![],
            }))
        }
        EmbedderConfig::Gemini { api_key, model_id } => {
            if api_key.is_empty() {
                return Err(IndexError::GeneralError(
                    "Gemini API key is required".to_string(),
                ));
            }
            let model = model_id
                .as_deref()
                .unwrap_or("gemini-embedding-001")
                .to_string();
            Ok(Box::new(HttpEmbedder {
                client: reqwest::Client::new(),
                base_url: "https://generativelanguage.googleapis.com/v1beta/openai/".to_string(),
                api_key: api_key.clone(),
                model,
                dimension: 768,
                is_ollama: false,
                extra_headers: vec![],
            }))
        }
        EmbedderConfig::Mistral { api_key, model_id } => {
            if api_key.is_empty() {
                return Err(IndexError::GeneralError(
                    "Mistral API key is required".to_string(),
                ));
            }
            let model = model_id
                .as_deref()
                .unwrap_or("codestral-embed-2505")
                .to_string();
            Ok(Box::new(HttpEmbedder {
                client: reqwest::Client::new(),
                base_url: "https://api.mistral.ai/v1".to_string(),
                api_key: api_key.clone(),
                model,
                dimension: 1024,
                is_ollama: false,
                extra_headers: vec![],
            }))
        }
        EmbedderConfig::Bedrock {
            region,
            profile: _,
            model_id,
        } => {
            if region.is_empty() {
                return Err(IndexError::GeneralError(
                    "Bedrock region is required".to_string(),
                ));
            }
            let model = model_id
                .as_deref()
                .unwrap_or("amazon.titan-embed-text-v2:0")
                .to_string();
            let dimension = if model.starts_with("amazon.nova-2-multimodal") {
                1024
            } else if model.starts_with("amazon.titan-embed-text-v1") {
                1536
            } else {
                1024
            };
            Ok(Box::new(BedrockEmbedder {
                client: reqwest::Client::new(),
                region: region.clone(),
                access_key: std::env::var("AWS_ACCESS_KEY_ID").unwrap_or_default(),
                secret_key: std::env::var("AWS_SECRET_ACCESS_KEY").unwrap_or_default(),
                session_token: std::env::var("AWS_SESSION_TOKEN").ok(),
                model,
                dimension,
            }))
        }
        EmbedderConfig::Openrouter {
            api_key,
            model_id,
            specific_provider: _,
        } => {
            if api_key.is_empty() {
                return Err(IndexError::GeneralError(
                    "OpenRouter API key is required".to_string(),
                ));
            }
            let model = model_id
                .as_deref()
                .unwrap_or("openai/text-embedding-3-large")
                .to_string();
            Ok(Box::new(HttpEmbedder {
                client: reqwest::Client::new(),
                base_url: "https://openrouter.ai/api/v1".to_string(),
                api_key: api_key.clone(),
                model,
                dimension: 1536,
                is_ollama: false,
                extra_headers: vec![
                    (
                        "HTTP-Referer".to_string(),
                        "https://github.com/RooVetGit/Roo-Code".to_string(),
                    ),
                    ("X-Title".to_string(), "Roo Code".to_string()),
                ],
            }))
        }
        EmbedderConfig::VercelAiGateway { api_key, model_id } => {
            if api_key.is_empty() {
                return Err(IndexError::GeneralError(
                    "Vercel AI Gateway API key is required".to_string(),
                ));
            }
            let model = model_id
                .as_deref()
                .unwrap_or("openai/text-embedding-3-large")
                .to_string();
            Ok(Box::new(HttpEmbedder {
                client: reqwest::Client::new(),
                base_url: "https://ai-gateway.vercel.sh/v1".to_string(),
                api_key: api_key.clone(),
                model,
                dimension: 1536,
                is_ollama: false,
                extra_headers: vec![],
            }))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_noop_embedder() {
        let embedder = NoopEmbedder::new(128);
        assert_eq!(embedder.dimension(), 128);

        let result = embedder
            .create_embeddings(&["hello", "world"])
            .await
            .unwrap();
        assert_eq!(result.embeddings.len(), 2);
        assert_eq!(result.embeddings[0].len(), 128);
    }

    #[test]
    fn test_create_embedder_openai() {
        let config = EmbedderConfig::Openai {
            api_key: "test-key".to_string(),
            model_id: None,
        };
        let embedder = create_embedder(&config).unwrap();
        assert_eq!(embedder.dimension(), 1536);
    }

    #[test]
    fn test_create_embedder_openai_no_key() {
        let config = EmbedderConfig::Openai {
            api_key: String::new(),
            model_id: None,
        };
        assert!(create_embedder(&config).is_err());
    }

    #[test]
    fn test_create_embedder_ollama() {
        let config = EmbedderConfig::Ollama {
            base_url: "http://localhost:11434".to_string(),
            model_id: None,
        };
        let embedder = create_embedder(&config).unwrap();
        assert_eq!(embedder.dimension(), 4096);
    }

    #[test]
    fn test_create_embedder_ollama_no_url() {
        let config = EmbedderConfig::Ollama {
            base_url: String::new(),
            model_id: None,
        };
        assert!(create_embedder(&config).is_err());
    }

    #[tokio::test]
    async fn test_noop_embedder_validate() {
        let embedder = NoopEmbedder::new(128);
        assert!(embedder.validate_configuration().unwrap());
    }

    #[test]
    fn test_create_embedder_vercel_ai_gateway() {
        let config = EmbedderConfig::VercelAiGateway {
            api_key: "test-key".to_string(),
            model_id: None,
        };
        let embedder = create_embedder(&config).unwrap();
        assert_eq!(embedder.dimension(), 1536);
    }

    #[test]
    fn test_create_embedder_vercel_ai_gateway_no_key() {
        let config = EmbedderConfig::VercelAiGateway {
            api_key: String::new(),
            model_id: None,
        };
        assert!(create_embedder(&config).is_err());
    }

    #[test]
    fn test_create_embedder_gemini() {
        let config = EmbedderConfig::Gemini {
            api_key: "test-key".to_string(),
            model_id: None,
        };
        let embedder = create_embedder(&config).unwrap();
        assert_eq!(embedder.dimension(), 768);
    }

    #[test]
    fn test_create_embedder_mistral() {
        let config = EmbedderConfig::Mistral {
            api_key: "test-key".to_string(),
            model_id: None,
        };
        let embedder = create_embedder(&config).unwrap();
        assert_eq!(embedder.dimension(), 1024);
    }

    #[test]
    fn test_create_embedder_openrouter() {
        let config = EmbedderConfig::Openrouter {
            api_key: "test-key".to_string(),
            model_id: None,
            specific_provider: None,
        };
        let embedder = create_embedder(&config).unwrap();
        assert_eq!(embedder.dimension(), 1536);
    }

    #[test]
    fn test_create_embedder_bedrock() {
        let config = EmbedderConfig::Bedrock {
            region: "us-east-1".to_string(),
            profile: None,
            model_id: None,
        };
        let embedder = create_embedder(&config).unwrap();
        // Default model is titan-embed-text-v2:0 → dimension 1024
        assert_eq!(embedder.dimension(), 1024);
    }

    #[test]
    fn test_create_embedder_bedrock_nova() {
        let config = EmbedderConfig::Bedrock {
            region: "us-east-1".to_string(),
            profile: None,
            model_id: Some("amazon.nova-2-multimodal".to_string()),
        };
        let embedder = create_embedder(&config).unwrap();
        assert_eq!(embedder.dimension(), 1024);
    }

    #[test]
    fn test_create_embedder_bedrock_titan_v1() {
        let config = EmbedderConfig::Bedrock {
            region: "us-east-1".to_string(),
            profile: None,
            model_id: Some("amazon.titan-embed-text-v1".to_string()),
        };
        let embedder = create_embedder(&config).unwrap();
        assert_eq!(embedder.dimension(), 1536);
    }

    #[test]
    fn test_create_embedder_bedrock_no_region() {
        let config = EmbedderConfig::Bedrock {
            region: String::new(),
            profile: None,
            model_id: None,
        };
        assert!(create_embedder(&config).is_err());
    }

    #[test]
    fn test_http_embedder_validate_with_key() {
        let config = EmbedderConfig::Openai {
            api_key: "test-key".to_string(),
            model_id: None,
        };
        let embedder = create_embedder(&config).unwrap();
        assert!(embedder.validate_configuration().unwrap());
    }

    #[test]
    fn test_http_embedder_ollama_validate_no_key() {
        // Ollama doesn't require an API key
        let config = EmbedderConfig::Ollama {
            base_url: "http://localhost:11434".to_string(),
            model_id: None,
        };
        let embedder = create_embedder(&config).unwrap();
        assert!(embedder.validate_configuration().unwrap());
    }

    #[test]
    fn test_create_embedder_custom_model_ids() {
        let config = EmbedderConfig::Openai {
            api_key: "test-key".to_string(),
            model_id: Some("text-embedding-3-large".to_string()),
        };
        let embedder = create_embedder(&config).unwrap();
        assert_eq!(embedder.dimension(), 1536);
    }
}

//! AWS Bedrock-specific configuration types.

use roo_types::provider_settings::ProviderSettings;

/// Default temperature for Bedrock models.
/// Matches BEDROCK_DEFAULT_TEMPERATURE from the TS source.
pub const BEDROCK_DEFAULT_TEMPERATURE: f64 = 0.0;

/// Configuration for the AWS Bedrock provider.
#[derive(Debug, Clone)]
pub struct AwsBedrockConfig {
    /// AWS Access Key ID.
    pub access_key: String,
    /// AWS Secret Access Key.
    pub secret_key: String,
    /// AWS Session Token (optional, for temporary credentials).
    pub session_token: Option<String>,
    /// AWS Region.
    pub region: String,
    /// Model ID to use (can be a custom model ID).
    pub model_id: Option<String>,
    /// Whether to use cross-region inference.
    pub use_cross_region_inference: bool,
    /// Custom Bedrock endpoint URL.
    pub endpoint_url: Option<String>,
    /// Request timeout in milliseconds.
    pub request_timeout: Option<u64>,
    /// Temperature for generation.
    pub temperature: Option<f64>,
}

impl AwsBedrockConfig {
    /// Default AWS region.
    pub const DEFAULT_REGION: &'static str = "us-east-1";

    /// Default Bedrock base URL pattern.
    pub fn bedrock_base_url(region: &str) -> String {
        format!("https://bedrock-runtime.{}.amazonaws.com", region)
    }

    /// Create configuration from provider settings.
    pub fn from_settings(settings: &ProviderSettings) -> Option<Self> {
        let access_key = settings.aws_access_key.clone()?;
        let secret_key = settings.aws_secret_key.clone()?;

        let region = settings
            .aws_region
            .clone()
            .unwrap_or_else(|| Self::DEFAULT_REGION.to_string());

        Some(Self {
            access_key,
            secret_key,
            session_token: settings.aws_session_token.clone(),
            region,
            model_id: settings
                .aws_bedrock_custom_model_id
                .clone()
                .or(settings.api_model_id.clone()),
            use_cross_region_inference: settings.aws_use_cross_region_inference.unwrap_or(false),
            endpoint_url: settings.aws_bedrock_endpoint.clone(),
            request_timeout: settings.request_timeout,
            temperature: settings.model_temperature.flatten(),
        })
    }
}

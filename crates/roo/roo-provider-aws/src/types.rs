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
    /// Whether to use global inference.
    pub use_global_inference: bool,
    /// Custom Bedrock endpoint URL.
    pub endpoint_url: Option<String>,
    /// Request timeout in milliseconds.
    pub request_timeout: Option<u64>,
    /// Temperature for generation.
    pub temperature: Option<f64>,
    /// Service tier for supported models (e.g., "STANDARD", "FLEX", "PRIORITY").
    /// Source: TS `awsBedrockServiceTier` option.
    pub service_tier: Option<roo_types::provider_settings::AwsBedrockServiceTier>,
    /// Whether 1M context is enabled for supported Claude 4 models.
    /// Source: TS `awsBedrock1MContext` option.
    pub enable_1m_context: bool,

    // --- Auth method fields ---
    /// Whether to use an AWS named profile for auth.
    pub use_profile: bool,
    /// AWS named profile name.
    pub profile_name: Option<String>,
    /// Whether to use an API key for auth.
    pub use_api_key: bool,
    /// API key value.
    pub api_key: Option<String>,
    /// VPC endpoint URL.
    pub vpc_endpoint: Option<String>,
    /// Whether VPC endpoint is enabled.
    pub vpc_endpoint_enabled: bool,
    /// Whether to use Bedrock prompt caching (cachePoint markers).
    /// Defaults to `true`.
    pub use_prompt_cache: bool,
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
            use_global_inference: settings.aws_use_global_inference.unwrap_or(false),
            endpoint_url: settings.aws_bedrock_endpoint.clone(),
            request_timeout: settings.request_timeout,
            temperature: settings.model_temperature.flatten(),
            service_tier: settings.aws_bedrock_service_tier,
            enable_1m_context: settings.aws_bedrock_1m_context.unwrap_or(false),
            use_profile: settings.aws_use_profile.unwrap_or(false),
            profile_name: settings.aws_profile.clone(),
            use_api_key: settings.aws_use_api_key.unwrap_or(false),
            api_key: settings.aws_api_key.clone(),
            vpc_endpoint: None,
            vpc_endpoint_enabled: settings.aws_bedrock_endpoint_enabled.unwrap_or(false),
            use_prompt_cache: true,
        })
    }
}

/// Apply service-tier pricing multipliers to a ModelInfo.
pub fn apply_service_tier_pricing(
    info: &mut roo_types::model::ModelInfo,
    tier: &roo_types::provider_settings::AwsBedrockServiceTier,
) {
    let multiplier = match tier {
        roo_types::provider_settings::AwsBedrockServiceTier::Standard => 1.0,
        roo_types::provider_settings::AwsBedrockServiceTier::Flex => 0.5,
        roo_types::provider_settings::AwsBedrockServiceTier::Priority => 1.75,
    };
    if multiplier != 1.0 {
        if let Some(ref mut p) = info.input_price {
            *p *= multiplier;
        }
        if let Some(ref mut p) = info.output_price {
            *p *= multiplier;
        }
        if let Some(ref mut p) = info.cache_writes_price {
            *p *= multiplier;
        }
        if let Some(ref mut p) = info.cache_reads_price {
            *p *= multiplier;
        }
    }
}

//! Error types for the provider layer.
//!
//! Derived from error handling patterns in `src/api/providers/`.

use std::fmt;

/// Errors that can occur during provider operations.
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    /// The API key is missing or invalid.
    #[error("API key is required")]
    ApiKeyRequired,

    /// The requested model is not supported by this provider.
    #[error("Unsupported model: {0}")]
    UnsupportedModel(String),

    /// A network or HTTP error occurred while communicating with the API.
    #[error("{0} API error: {1}")]
    ApiError(String, String),

    /// The API returned an error response.
    #[error("{0} API error ({1}): {2}")]
    ApiErrorResponse(String, u16, String),

    /// A streaming error occurred.
    #[error("Stream error: {0}")]
    StreamError(String),

    /// Failed to parse a response from the API.
    #[error("Failed to parse response: {0}")]
    ParseError(String),

    /// The request timed out.
    #[error("Request timed out after {0}ms")]
    Timeout(u64),

    /// Rate limit exceeded.
    #[error("Rate limit exceeded")]
    RateLimitExceeded,

    /// A retired/no-longer-supported provider was requested.
    #[error("Sorry, this provider is no longer supported. We saw very few Roo users actually using it and we need to reduce the surface area of our codebase so we can keep shipping fast and serving our community well in this space. It was a really hard decision but it lets us focus on what matters most to you. It sucks, we know.\n\nPlease select a different provider in your API profile settings.")]
    RetiredProvider,

    /// Generic error.
    #[error("{0}")]
    Other(String),

    /// An error from the reqwest HTTP client.
    #[error("HTTP request failed: {0}")]
    Reqwest(#[from] reqwest::Error),

    /// An error from serde_json.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl ProviderError {
    /// Create a provider-specific API error.
    pub fn api_error(provider: &str, message: impl fmt::Display) -> Self {
        Self::ApiError(provider.to_string(), message.to_string())
    }

    /// Create a provider-specific API error response with status code.
    pub fn api_error_response(provider: &str, status_code: u16, message: impl fmt::Display) -> Self {
        Self::ApiErrorResponse(provider.to_string(), status_code, message.to_string())
    }
}

/// Result type alias for provider operations.
pub type Result<T> = std::result::Result<T, ProviderError>;

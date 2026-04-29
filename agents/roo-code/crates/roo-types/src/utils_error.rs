//! Error handling utility types.
//!
//! Derived from `src/utils/errors.ts`.
//!
//! Provides specialized error types used across the Roo Code codebase.

use thiserror::Error;

/// Error thrown when an organization's allow list is violated.
///
/// Source: `src/utils/errors.ts` — `OrganizationAllowListViolationError`
#[derive(Debug, Error)]
#[error("Organization allow list violation: {0}")]
pub struct OrganizationAllowListViolationError(pub String);

impl OrganizationAllowListViolationError {
    /// Creates a new organization allow list violation error.
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

/// Error thrown when a rate limit is exceeded.
#[derive(Debug, Error)]
#[error("Rate limit exceeded: {0}")]
pub struct RateLimitError(pub String);

impl RateLimitError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

/// Error thrown when a token budget is exceeded.
#[derive(Debug, Error)]
#[error("Token budget exceeded: used {used}, limit {limit}")]
pub struct TokenBudgetError {
    pub used: u64,
    pub limit: u64,
}

impl TokenBudgetError {
    pub fn new(used: u64, limit: u64) -> Self {
        Self { used, limit }
    }
}

/// Error thrown when a configuration validation fails.
#[derive(Debug, Error)]
#[error("Configuration validation error: {0}")]
pub struct ConfigValidationError(pub String);

impl ConfigValidationError {
    pub fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_organization_allow_list_violation_error() {
        let err = OrganizationAllowListViolationError::new("test model not allowed");
        assert!(err.to_string().contains("Organization allow list violation"));
        assert!(err.to_string().contains("test model not allowed"));
    }

    #[test]
    fn test_rate_limit_error() {
        let err = RateLimitError::new("Too many requests");
        assert!(err.to_string().contains("Rate limit exceeded"));
        assert!(err.to_string().contains("Too many requests"));
    }

    #[test]
    fn test_token_budget_error() {
        let err = TokenBudgetError::new(1000, 500);
        assert!(err.to_string().contains("1000"));
        assert!(err.to_string().contains("500"));
    }

    #[test]
    fn test_config_validation_error() {
        let err = ConfigValidationError::new("Missing API key");
        assert!(err.to_string().contains("Configuration validation error"));
        assert!(err.to_string().contains("Missing API key"));
    }
}

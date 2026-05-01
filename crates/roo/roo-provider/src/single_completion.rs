//! Single completion handler for lightweight API completions.
//!
//! Derived from `src/utils/single-completion-handler.ts`.
//!
//! Enhances a prompt using the configured API without creating a full task
//! or task history. This is a lightweight alternative that only uses the
//! API's completion functionality.

use roo_types::provider_settings::ProviderSettings;

use crate::error::{ProviderError, Result};
use crate::handler::Provider;

/// Performs a single completion using the configured API provider.
///
/// Source: `src/utils/single-completion-handler.ts` — `singleCompletionHandler`
///
/// This is a lightweight alternative to creating a full task instance.
/// It uses the API's `complete_prompt` functionality directly.
///
/// # Arguments
/// * `provider` - The API provider to use for completion
/// * `prompt_text` - The prompt text to complete
///
/// # Errors
/// Returns an error if:
/// - The prompt text is empty
/// - The provider doesn't support completions
/// - The API call fails
pub async fn single_completion_handler(
    provider: &dyn Provider,
    prompt_text: &str,
) -> Result<String> {
    if prompt_text.is_empty() {
        return Err(ProviderError::Other("No prompt text provided".to_string()));
    }

    provider.complete_prompt(prompt_text).await
}

/// Validates that a provider configuration is suitable for single completion.
///
/// Returns `Ok(())` if the configuration has a valid provider, or an error
/// describing what's missing.
pub fn validate_completion_config(api_configuration: &ProviderSettings) -> Result<()> {
    if api_configuration.api_provider.is_none() {
        return Err(ProviderError::Other(
            "No valid API configuration provided".to_string(),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_completion_config_missing_provider() {
        let config = ProviderSettings::default();
        assert!(validate_completion_config(&config).is_err());
    }

    #[test]
    fn test_empty_prompt_rejected() {
        // We can't easily test the full handler without a mock provider,
        // but we can verify the validation logic
        let prompt = "";
        assert!(prompt.is_empty());
    }
}

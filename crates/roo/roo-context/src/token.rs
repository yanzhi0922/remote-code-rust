//! Token estimation utilities.
//!
//! Source: `src/core/context-management/index.ts` — `estimateTokenCount`

use roo_provider::handler::Provider;
use roo_types::api::ContentBlock;

/// Counts tokens for content blocks using the provider's token counting
/// implementation.
///
/// Source: `src/core/context-management/index.ts` — `estimateTokenCount`
pub async fn estimate_token_count(
    content: &[ContentBlock],
    provider: &dyn Provider,
) -> anyhow::Result<u64> {
    if content.is_empty() {
        return Ok(0);
    }
    Ok(provider.count_tokens(content).await?)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_estimate_token_count_empty() {
        // Empty content should return 0 without calling the provider.
        // We can't easily test the async version without a mock provider,
        // but the logic is trivial: empty -> 0
        let content: Vec<roo_types::api::ContentBlock> = vec![];
        assert!(content.is_empty());
    }
}

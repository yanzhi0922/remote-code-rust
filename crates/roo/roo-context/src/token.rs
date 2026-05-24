//! Token counting utilities.
//!
//! Uses real BPE tokenization (tiktoken o200k_base) matching the TypeScript
//! reference's `tiktoken` utility. Falls back to chars/4 heuristic only when
//! the BPE encoder fails to initialize.
//!
//! Source: `src/core/context-management/index.ts` — `estimateTokenCount`
//! Source: `src/utils/tiktoken.ts` — `tiktoken`

use roo_types::api::ContentBlock;

use crate::tiktoken;

/// Counts tokens for content blocks using real BPE tokenization.
///
/// Uses the o200k_base encoding (GPT-4o family) with a 1.5x fudge factor,
/// matching the TypeScript `tiktoken` utility exactly.
///
/// Source: `src/utils/tiktoken.ts` — `tiktoken`
pub async fn estimate_token_count(content: &[ContentBlock]) -> u64 {
    tiktoken::count_tokens(content).await
}

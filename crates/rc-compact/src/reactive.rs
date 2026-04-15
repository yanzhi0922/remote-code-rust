//! Reactive Compact strategy.
//!
//! Triggered in response to API prompt-too-long errors.  Attempts to recover
//! by progressively trimming the conversation until it fits within the context
//! window.  Mirrors `services/compact/reactiveCompact.ts`.

use rc_core::Message;

use crate::engine::{compact_conversation, ERROR_MESSAGE_PROMPT_TOO_LONG};
use crate::strategy::{
    CompactOptions, CompactProgressEvent, CompactStrategy, CompactStrategyType, CompactionResult,
    ProgressCallback, SummaryProvider,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of reactive compaction retries before giving up.
pub const MAX_REACTIVE_COMPACT_RETRIES: u32 = 3;

/// Fraction of messages to drop per retry (0.2 = 20%).
const DROP_FRACTION: f64 = 0.2;

// ---------------------------------------------------------------------------
// Reactive compact config
// ---------------------------------------------------------------------------

/// Configuration for reactive compaction.
#[derive(Debug, Clone)]
pub struct ReactiveCompactConfig {
    /// Maximum retry attempts.
    pub max_retries: u32,
    /// Context window size for the model.
    pub context_window_size: u64,
}

impl Default for ReactiveCompactConfig {
    fn default() -> Self {
        Self {
            max_retries: MAX_REACTIVE_COMPACT_RETRIES,
            context_window_size: 200_000,
        }
    }
}

// ---------------------------------------------------------------------------
// Reactive compact strategy
// ---------------------------------------------------------------------------

/// Reactive-compact strategy that responds to prompt-too-long errors.
pub struct ReactiveCompactStrategy {
    /// Configuration for this strategy.
    pub config: ReactiveCompactConfig,
}

impl Default for ReactiveCompactStrategy {
    fn default() -> Self {
        Self {
            config: ReactiveCompactConfig::default(),
        }
    }
}

impl ReactiveCompactStrategy {
    /// Create a new reactive-compact strategy with custom config.
    pub fn new(config: ReactiveCompactConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl CompactStrategy for ReactiveCompactStrategy {
    fn strategy_type(&self) -> CompactStrategyType {
        CompactStrategyType::Reactive
    }

    async fn compact(
        &self,
        messages: &[Message],
        options: &CompactOptions,
        provider: &dyn SummaryProvider,
        progress: Option<&ProgressCallback>,
    ) -> Result<CompactionResult, anyhow::Error> {
        reactive_compact(messages, &self.config, options, provider, progress).await
    }
}

// ---------------------------------------------------------------------------
// Core reactive-compact implementation
// ---------------------------------------------------------------------------

/// Perform reactive compaction.
///
/// This is called when the main query loop receives a prompt-too-long error.
/// It progressively drops the oldest messages and retries compaction.
pub async fn reactive_compact(
    messages: &[Message],
    config: &ReactiveCompactConfig,
    options: &CompactOptions,
    provider: &dyn SummaryProvider,
    progress: Option<&ProgressCallback>,
) -> Result<CompactionResult, anyhow::Error> {
    if let Some(sink) = progress {
        sink(CompactProgressEvent::Started {
            strategy: CompactStrategyType::Reactive,
        });
    }

    let mut current_messages = messages.to_vec();
    let mut attempt: u32 = 0;

    loop {
        attempt += 1;

        if let Some(sink) = progress {
            sink(CompactProgressEvent::Summarizing {
                messages_processed: current_messages.len(),
            });
        }

        // Try compacting the current set of messages
        match compact_conversation(&current_messages, options, provider, None).await {
            Ok(mut result) => {
                result.strategy_used = CompactStrategyType::Reactive;
                if let Some(sink) = progress {
                    sink(CompactProgressEvent::Completed(result.clone()));
                }
                return Ok(result);
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains(ERROR_MESSAGE_PROMPT_TOO_LONG) {
                    if attempt > config.max_retries {
                        if let Some(sink) = progress {
                            sink(CompactProgressEvent::Failed(
                                ERROR_MESSAGE_PROMPT_TOO_LONG.to_string(),
                            ));
                        }
                        return Err(e);
                    }

                    // Drop the oldest 20% of messages and retry
                    let drop_count =
                        std::cmp::max(1, (current_messages.len() as f64 * DROP_FRACTION) as usize);
                    let remaining = current_messages.len().saturating_sub(drop_count);

                    if remaining == 0 {
                        if let Some(sink) = progress {
                            sink(CompactProgressEvent::Failed(
                                "Cannot compact: no messages left to drop".into(),
                            ));
                        }
                        return Err(anyhow::anyhow!(ERROR_MESSAGE_PROMPT_TOO_LONG));
                    }

                    current_messages = current_messages.into_iter().skip(drop_count).collect();
                    tracing::warn!(
                        attempt,
                        dropped = drop_count,
                        remaining = current_messages.len(),
                        "Reactive compact: prompt-too-long, dropping oldest messages"
                    );
                } else {
                    if let Some(sink) = progress {
                        sink(CompactProgressEvent::Failed(error_msg));
                    }
                    return Err(e);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reactive_compact_config_default() {
        let config = ReactiveCompactConfig::default();
        assert_eq!(config.max_retries, MAX_REACTIVE_COMPACT_RETRIES);
        assert_eq!(config.context_window_size, 200_000);
    }
}

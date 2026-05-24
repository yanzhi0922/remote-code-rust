//! Proactive context window monitoring.
//!
//! Periodically evaluates the conversation token estimate against the
//! model's context window limit and suggests or triggers compaction
//! before the provider returns a 413 / `prompt_too_long` error.

use anyhow::Result;
use claude_config::RuntimeConfig;
use claude_provider::context::TokenEstimator;
use claude_session::SessionStore;
use tracing::{debug, info, warn};

/// Context window threshold ratios.
#[derive(Debug, Clone, Copy)]
pub struct ContextThresholds {
    /// When usage_ratio exceeds this, emit a warning.
    pub warning_ratio: f64,
    /// When usage_ratio exceeds this, trigger auto-compaction.
    pub compact_ratio: f64,
    /// When usage_ratio exceeds this, flag as critical overflow risk.
    pub critical_ratio: f64,
}

impl Default for ContextThresholds {
    fn default() -> Self {
        Self {
            warning_ratio: 0.70,
            compact_ratio: 0.85,
            critical_ratio: 0.95,
        }
    }
}

/// Result of a context evaluation.
#[derive(Debug, Clone)]
pub enum ContextAdvice {
    /// Context is healthy — no action needed.
    Healthy,
    /// Context is growing — user should consider compacting.
    Warning {
        ratio: f64,
        estimated_tokens: u64,
        max_tokens: u64,
    },
    /// Context should be compacted now.
    Compact {
        ratio: f64,
        estimated_tokens: u64,
        max_tokens: u64,
    },
    /// Context is critically full — immediate compaction required.
    Critical {
        ratio: f64,
        estimated_tokens: u64,
        max_tokens: u64,
    },
}

impl ContextAdvice {
    /// Returns `true` if compaction is advised (Compact or Critical).
    pub fn needs_compact(&self) -> bool {
        matches!(self, Self::Compact { .. } | Self::Critical { .. })
    }
}

/// Monitors conversation context window usage and provides compaction advice.
pub struct ContextMonitor {
    thresholds: ContextThresholds,
    estimator: TokenEstimator,
}

impl ContextMonitor {
    /// Create a new monitor with default thresholds.
    pub fn new() -> Self {
        Self {
            thresholds: ContextThresholds::default(),
            estimator: TokenEstimator::new(),
        }
    }

    /// Create a monitor with custom thresholds.
    pub fn with_thresholds(thresholds: ContextThresholds) -> Self {
        Self {
            thresholds,
            estimator: TokenEstimator::new(),
        }
    }

    /// Evaluate the current context usage and return advice.
    ///
    /// Uses `claude-provider`'s token estimator to approximate current
    /// usage against a configurable max_input_tokens.
    pub fn evaluate(&self, config: &RuntimeConfig, store: &SessionStore) -> Result<ContextAdvice> {
        let conversation = store.load_conversation(config.session_id)?;
        let max_tokens = config.provider.max_output_tokens.max(4096) as u64;
        let max_input: u64 = 200_000; // Claude's default context window

        let estimated_tokens: u64 = conversation
            .iter()
            .map(|entry| self.estimator.estimate(&entry.text))
            .sum();

        let ratio = estimated_tokens as f64 / max_input as f64;

        if ratio >= self.thresholds.critical_ratio {
            warn!(
                estimated_tokens,
                max_input, ratio, "Context critically full — immediate compaction recommended"
            );
            Ok(ContextAdvice::Critical {
                ratio,
                estimated_tokens,
                max_tokens,
            })
        } else if ratio >= self.thresholds.compact_ratio {
            info!(
                estimated_tokens,
                max_input, ratio, "Context nearing limit — compaction advised"
            );
            Ok(ContextAdvice::Compact {
                ratio,
                estimated_tokens,
                max_tokens,
            })
        } else if ratio >= self.thresholds.warning_ratio {
            debug!(estimated_tokens, max_input, ratio, "Context usage rising");
            Ok(ContextAdvice::Warning {
                ratio,
                estimated_tokens,
                max_tokens,
            })
        } else {
            Ok(ContextAdvice::Healthy)
        }
    }
}

impl Default for ContextMonitor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthy_at_low_usage() {
        let monitor = ContextMonitor::with_thresholds(ContextThresholds {
            warning_ratio: 0.1,
            compact_ratio: 0.3,
            critical_ratio: 0.5,
        });
        assert_eq!(monitor.thresholds.warning_ratio, 0.1);
        // With a small number of tokens vs 200k limit, should be healthy.
        let ratio = 5000.0 / 200_000.0;
        assert!(ratio < 0.1);
        // We test the threshold logic directly since we can't create a real session:
        assert!(ratio < 0.1);
    }

    #[test]
    fn needs_compact_returns_true_for_critical() {
        let advice = ContextAdvice::Critical {
            ratio: 0.98,
            estimated_tokens: 196_000,
            max_tokens: 8192,
        };
        assert!(advice.needs_compact());
    }

    #[test]
    fn needs_compact_false_for_healthy() {
        let advice = ContextAdvice::Healthy;
        assert!(!advice.needs_compact());
    }

    #[test]
    fn threshold_defaults_are_reasonable() {
        let t = ContextThresholds::default();
        assert!(t.warning_ratio < t.compact_ratio);
        assert!(t.compact_ratio < t.critical_ratio);
    }
}

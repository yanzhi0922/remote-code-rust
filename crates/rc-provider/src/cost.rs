//! Cost tracking for API usage.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::Mutex;

/// Per-model cost breakdown.
#[derive(Debug, Clone, Default)]
pub struct ModelCost {
    /// Total input tokens for this model.
    pub input_tokens: u64,
    /// Total output tokens for this model.
    pub output_tokens: u64,
    /// Estimated cost in USD for this model.
    pub cost_usd: f64,
}

/// Thread-safe cost tracker that accumulates token usage and estimated costs.
#[derive(Debug)]
pub struct CostTracker {
    inner: Mutex<CostTrackerInner>,
}

#[derive(Debug, Clone, Default)]
struct CostTrackerInner {
    total_input_tokens: u64,
    total_output_tokens: u64,
    total_cache_read_tokens: u64,
    total_cache_creation_tokens: u64,
    estimated_cost_usd: f64,
    per_model: HashMap<String, ModelCost>,
}

impl CostTracker {
    /// Create a new, empty cost tracker.
    #[must_use]
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(CostTrackerInner::default()),
        }
    }

    /// Record a single API call's token usage.
    pub fn record(&self, model: &str, input_tokens: u64, output_tokens: u64) {
        let cost = estimate_cost(model, input_tokens, output_tokens);
        let mut inner = self.inner.lock().expect("cost tracker mutex poisoned");
        inner.total_input_tokens += input_tokens;
        inner.total_output_tokens += output_tokens;
        inner.estimated_cost_usd += cost;

        let entry = inner.per_model.entry(model.to_owned()).or_default();
        entry.input_tokens += input_tokens;
        entry.output_tokens += output_tokens;
        entry.cost_usd += cost;
    }

    /// Record cache-related token usage.
    pub fn record_cache(&self, cache_read_tokens: u64, cache_creation_tokens: u64) {
        let mut inner = self.inner.lock().expect("cost tracker mutex poisoned");
        inner.total_cache_read_tokens += cache_read_tokens;
        inner.total_cache_creation_tokens += cache_creation_tokens;
    }

    /// Get the total estimated cost in USD.
    pub fn total_cost_usd(&self) -> f64 {
        self.inner
            .lock()
            .expect("cost tracker mutex poisoned")
            .estimated_cost_usd
    }

    /// Get the total input tokens across all models.
    pub fn total_input_tokens(&self) -> u64 {
        self.inner
            .lock()
            .expect("cost tracker mutex poisoned")
            .total_input_tokens
    }

    /// Get the total output tokens across all models.
    pub fn total_output_tokens(&self) -> u64 {
        self.inner
            .lock()
            .expect("cost tracker mutex poisoned")
            .total_output_tokens
    }

    /// Generate a human-readable summary of accumulated costs.
    pub fn summary(&self) -> String {
        let inner = self.inner.lock().expect("cost tracker mutex poisoned");
        let mut out = String::new();

        let _ = writeln!(
            out,
            "=== Cost Summary ===\nTotal input tokens:  {}\nTotal output tokens: {}\nCache read tokens:   {}\nCache creation tokens: {}\nEstimated cost:      ${:.6} USD",
            inner.total_input_tokens,
            inner.total_output_tokens,
            inner.total_cache_read_tokens,
            inner.total_cache_creation_tokens,
            inner.estimated_cost_usd,
        );

        if !inner.per_model.is_empty() {
            let _ = writeln!(out, "\nPer-model breakdown:");
            let mut models: Vec<_> = inner.per_model.iter().collect();
            models.sort_by(|a, b| b.1.cost_usd.partial_cmp(&a.1.cost_usd).unwrap_or(std::cmp::Ordering::Equal));
            for (model, cost) in models {
                let _ = writeln!(
                    out,
                    "  {}: {} in / {} out → ${:.6}",
                    model, cost.input_tokens, cost.output_tokens, cost.cost_usd
                );
            }
        }

        out
    }
}

impl Default for CostTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Estimate the cost in USD for a single API call.
///
/// Pricing is per million tokens and uses publicly available rates for
/// well-known models. Unknown models default to GPT-4o-mini pricing.
pub fn estimate_cost(model: &str, input_tokens: u64, output_tokens: u64) -> f64 {
    let (input_per_m, output_per_m) = pricing_for_model(model);
    let input_cost = (input_tokens as f64 / 1_000_000.0) * input_per_m;
    let output_cost = (output_tokens as f64 / 1_000_000.0) * output_per_m;
    input_cost + output_cost
}

/// Return (input_price_per_million, output_price_per_million) for a model.
fn pricing_for_model(model: &str) -> (f64, f64) {
    let lower = model.to_ascii_lowercase();

    // GPT-4o family
    if lower.contains("gpt-4o") && !lower.contains("mini") {
        // $2.50 / $10.00 per million tokens
        return (2.50, 10.00);
    }
    if lower.contains("gpt-4o-mini") {
        // $0.15 / $0.60 per million tokens
        return (0.15, 0.60);
    }

    // GPT-4 Turbo
    if lower.contains("gpt-4-turbo") || lower.contains("gpt-4-0") {
        return (10.00, 30.00);
    }

    // GPT-4
    if lower.contains("gpt-4") {
        return (30.00, 60.00);
    }

    // GPT-3.5 Turbo
    if lower.contains("gpt-3.5") {
        return (0.50, 1.50);
    }

    // Claude 3.5 Sonnet
    if lower.contains("claude-3-5-sonnet") || lower.contains("claude-3.5-sonnet") {
        return (3.00, 15.00);
    }

    // Claude 3 Opus
    if lower.contains("claude-3-opus") || lower.contains("claude-3-opus") {
        return (15.00, 75.00);
    }

    // Claude 3 Haiku
    if lower.contains("claude-3-haiku") || lower.contains("claude-3-haiku") {
        return (0.25, 1.25);
    }

    // GLM-4 family (pricing in CNY, approximate USD conversion at ~0.14)
    if lower.contains("glm-4") {
        // ¥0.05/千token ≈ $0.07/千token ≈ $70/M tokens (both in/out)
        let per_m = 0.05 / 1000.0 * 1_000_000.0 * 0.14;
        return (per_m, per_m);
    }

    // Default: use GPT-4o-mini pricing as a safe fallback
    (0.15, 0.60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_cost_gpt4o() {
        let cost = estimate_cost("gpt-4o", 1_000_000, 1_000_000);
        assert!(
            (cost - 12.50).abs() < 0.01,
            "expected ~12.50, got {cost}"
        );
    }

    #[test]
    fn estimate_cost_claude_sonnet() {
        let cost = estimate_cost("claude-3-5-sonnet-20241022", 1_000_000, 1_000_000);
        assert!(
            (cost - 18.00).abs() < 0.01,
            "expected ~18.00, got {cost}"
        );
    }

    #[test]
    fn estimate_cost_unknown_model_uses_default() {
        let cost = estimate_cost("unknown-model", 1_000_000, 1_000_000);
        assert!(
            (cost - 0.75).abs() < 0.01,
            "expected ~0.75, got {cost}"
        );
    }

    #[test]
    fn tracker_records_multiple_models() {
        let tracker = CostTracker::new();
        tracker.record("gpt-4o", 1000, 500);
        tracker.record("gpt-4o-mini", 2000, 1000);

        assert_eq!(tracker.total_input_tokens(), 3000);
        assert_eq!(tracker.total_output_tokens(), 1500);
        assert!(tracker.total_cost_usd() > 0.0);

        let summary = tracker.summary();
        assert!(summary.contains("gpt-4o"));
        assert!(summary.contains("gpt-4o-mini"));
    }

    #[test]
    fn tracker_record_cache() {
        let tracker = CostTracker::new();
        tracker.record_cache(500, 200);

        let summary = tracker.summary();
        assert!(summary.contains("500"));
        assert!(summary.contains("200"));
    }

    #[test]
    fn default_trait_works() {
        let tracker = CostTracker::default();
        assert_eq!(tracker.total_cost_usd(), 0.0);
    }
}

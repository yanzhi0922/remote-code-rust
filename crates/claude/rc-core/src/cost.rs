use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Cost tracker for multi-turn engine runs.
///
/// # Cost recording strategy
///
/// The tracker distinguishes between two types of cost recording:
///
/// - **Turn-level costs** ([`record_turn_cost`]): The authoritative total.
///   Call this once per completed turn with the total cost for that turn.
///   This is the only method that accumulates into [`total_cost_usd`].
///
/// - **Breakdown costs** ([`record_provider_cost`], [`record_tool_cost`]):
///   These track cost *by dimension* (provider, tool) for analytics and
///   reporting. They do **not** add to [`total_cost_usd`] to avoid
///   double-counting when the same cost is also recorded via
///   [`record_turn_cost`].
///
/// If you only need a single total, use [`record_turn_cost`] exclusively.
/// If you need per-provider or per-tool breakdowns, record the turn cost
/// once and use the breakdown methods for dimensional analysis of the
/// same cost.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct CostTracker {
    #[serde(default)]
    pub total_cost_usd: f64,
    #[serde(default)]
    pub turn_costs_usd: Vec<f64>,
    #[serde(default)]
    pub provider_costs_usd: BTreeMap<String, f64>,
    #[serde(default)]
    pub tool_costs_usd: BTreeMap<String, f64>,
}

impl CostTracker {
    /// Record the cost of a completed turn.
    ///
    /// This is the authoritative total cost accumulator. Each turn's cost
    /// should be recorded exactly once via this method.
    pub fn record_turn_cost(&mut self, cost_usd: f64) {
        self.total_cost_usd += cost_usd;
        self.turn_costs_usd.push(cost_usd);
    }

    /// Record provider-scoped cost contribution (breakdown only).
    ///
    /// This tracks costs per provider for analytics. It does **not** add to
    /// [`total_cost_usd`] to avoid double-counting with [`record_turn_cost`].
    pub fn record_provider_cost(&mut self, provider: impl Into<String>, cost_usd: f64) {
        *self.provider_costs_usd.entry(provider.into()).or_default() += cost_usd;
    }

    /// Record tool-scoped cost contribution (breakdown only).
    ///
    /// This tracks costs per tool for analytics. It does **not** add to
    /// [`total_cost_usd`] to avoid double-counting with [`record_turn_cost`].
    pub fn record_tool_cost(&mut self, tool_name: impl Into<String>, cost_usd: f64) {
        *self.tool_costs_usd.entry(tool_name.into()).or_default() += cost_usd;
    }
}

#[cfg(test)]
mod tests {
    use super::CostTracker;

    #[test]
    fn cost_tracker_accumulates_by_source() {
        let mut tracker = CostTracker::default();
        // Record a turn cost of $0.12 — this goes into total_cost_usd
        tracker.record_turn_cost(0.12);
        // Record breakdown costs — these do NOT add to total_cost_usd
        tracker.record_provider_cost("anthropic", 0.12);
        tracker.record_tool_cost("web_search", 0.12);

        assert_eq!(tracker.total_cost_usd, 0.12);
        assert_eq!(tracker.turn_costs_usd, vec![0.12]);
        assert_eq!(tracker.provider_costs_usd["anthropic"], 0.12);
        assert_eq!(tracker.tool_costs_usd["web_search"], 0.12);
    }

    #[test]
    fn cost_tracker_multiple_turns() {
        let mut tracker = CostTracker::default();
        tracker.record_turn_cost(0.10);
        tracker.record_turn_cost(0.20);
        tracker.record_provider_cost("anthropic", 0.10);
        tracker.record_provider_cost("anthropic", 0.20);
        tracker.record_tool_cost("bash", 0.10);
        tracker.record_tool_cost("web_search", 0.20);

        assert!((tracker.total_cost_usd - 0.30).abs() < 1e-9);
        assert_eq!(tracker.turn_costs_usd, vec![0.10, 0.20]);
        assert!((tracker.provider_costs_usd["anthropic"] - 0.30).abs() < 1e-9);
        assert_eq!(tracker.tool_costs_usd["bash"], 0.10);
        assert_eq!(tracker.tool_costs_usd["web_search"], 0.20);
    }
}

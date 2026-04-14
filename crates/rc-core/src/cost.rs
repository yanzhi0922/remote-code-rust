use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Cost tracker for multi-turn engine runs.
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
    pub fn record_turn_cost(&mut self, cost_usd: f64) {
        self.total_cost_usd += cost_usd;
        self.turn_costs_usd.push(cost_usd);
    }

    /// Record provider-scoped cost contribution.
    pub fn record_provider_cost(&mut self, provider: impl Into<String>, cost_usd: f64) {
        self.total_cost_usd += cost_usd;
        *self.provider_costs_usd.entry(provider.into()).or_default() += cost_usd;
    }

    /// Record tool-scoped cost contribution.
    pub fn record_tool_cost(&mut self, tool_name: impl Into<String>, cost_usd: f64) {
        self.total_cost_usd += cost_usd;
        *self.tool_costs_usd.entry(tool_name.into()).or_default() += cost_usd;
    }
}

#[cfg(test)]
mod tests {
    use super::CostTracker;

    #[test]
    fn cost_tracker_accumulates_by_source() {
        let mut tracker = CostTracker::default();
        tracker.record_turn_cost(0.12);
        tracker.record_provider_cost("anthropic", 0.34);
        tracker.record_tool_cost("web_search", 0.56);

        assert_eq!(tracker.total_cost_usd, 1.02);
        assert_eq!(tracker.turn_costs_usd, vec![0.12]);
        assert_eq!(tracker.provider_costs_usd["anthropic"], 0.34);
        assert_eq!(tracker.tool_costs_usd["web_search"], 0.56);
    }
}

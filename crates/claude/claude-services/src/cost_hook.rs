//! Cost threshold hook — monitors usage and fires alerts when budgets are exceeded.
//!
//! Mirrors `cc-haha/src/costHook.ts`. Tracks per-session cost against configurable
//! budgets and emits warning events when thresholds are crossed.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tracing::warn;

/// Cost threshold configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostHookConfig {
    /// Alert when total cost exceeds this amount (USD).
    pub cost_threshold_usd: f64,
    /// Alert when input tokens exceed this amount.
    pub input_token_threshold: u64,
    /// Alert when output tokens exceed this amount.
    pub output_token_threshold: u64,
    /// Alert when a single turn exceeds this cost (USD).
    pub turn_cost_threshold_usd: f64,
}

impl Default for CostHookConfig {
    fn default() -> Self {
        Self {
            cost_threshold_usd: 1.0,
            input_token_threshold: 500_000,
            output_token_threshold: 100_000,
            turn_cost_threshold_usd: 0.10,
        }
    }
}

/// A single cost alert event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostAlert {
    pub alert_type: CostAlertType,
    pub message: String,
    pub current_value: f64,
    pub threshold: f64,
    pub session_id: String,
}

/// Type of cost alert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostAlertType {
    TotalCostExceeded,
    InputTokenBudgetExceeded,
    OutputTokenBudgetExceeded,
    TurnCostExceeded,
}

/// Running cost state for a session.
#[derive(Debug, Clone, Default)]
pub struct SessionCostState {
    total_cost_usd: f64,
    total_input_tokens: u64,
    total_output_tokens: u64,
    last_turn_cost: f64,
    alerts_fired: Vec<CostAlertType>,
}

/// Cost hook that monitors spending and fires alerts.
pub struct CostHook {
    config: CostHookConfig,
    sessions: Mutex<std::collections::HashMap<String, SessionCostState>>,
}

impl CostHook {
    pub fn new(config: CostHookConfig) -> Self {
        Self {
            config,
            sessions: Mutex::new(std::collections::HashMap::new()),
        }
    }

    pub fn new_default() -> Self {
        Self::new(CostHookConfig::default())
    }

    /// Record a provider turn's usage and check thresholds.
    /// Returns any alerts that were triggered.
    pub fn record_turn(
        &self,
        session_id: &str,
        input_tokens: u64,
        output_tokens: u64,
        estimated_cost: f64,
    ) -> Vec<CostAlert> {
        let mut state = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        let session = state.entry(session_id.to_owned()).or_default();

        session.total_cost_usd += estimated_cost;
        session.total_input_tokens += input_tokens;
        session.total_output_tokens += output_tokens;
        session.last_turn_cost = estimated_cost;

        let mut alerts = Vec::new();

        // Check total cost
        if session.total_cost_usd >= self.config.cost_threshold_usd
            && !session
                .alerts_fired
                .contains(&CostAlertType::TotalCostExceeded)
        {
            session.alerts_fired.push(CostAlertType::TotalCostExceeded);
            alerts.push(CostAlert {
                alert_type: CostAlertType::TotalCostExceeded,
                message: format!(
                    "Total cost ${:.4} exceeded threshold ${:.4}",
                    session.total_cost_usd, self.config.cost_threshold_usd
                ),
                current_value: session.total_cost_usd,
                threshold: self.config.cost_threshold_usd,
                session_id: session_id.to_owned(),
            });
            warn!(
                session_id,
                cost = session.total_cost_usd,
                "Cost threshold exceeded"
            );
        }

        // Check input tokens
        if session.total_input_tokens >= self.config.input_token_threshold
            && !session
                .alerts_fired
                .contains(&CostAlertType::InputTokenBudgetExceeded)
        {
            session
                .alerts_fired
                .push(CostAlertType::InputTokenBudgetExceeded);
            alerts.push(CostAlert {
                alert_type: CostAlertType::InputTokenBudgetExceeded,
                message: format!(
                    "Input tokens {} exceeded threshold {}",
                    session.total_input_tokens, self.config.input_token_threshold
                ),
                current_value: session.total_input_tokens as f64,
                threshold: self.config.input_token_threshold as f64,
                session_id: session_id.to_owned(),
            });
        }

        // Check output tokens
        if session.total_output_tokens >= self.config.output_token_threshold
            && !session
                .alerts_fired
                .contains(&CostAlertType::OutputTokenBudgetExceeded)
        {
            session
                .alerts_fired
                .push(CostAlertType::OutputTokenBudgetExceeded);
            alerts.push(CostAlert {
                alert_type: CostAlertType::OutputTokenBudgetExceeded,
                message: format!(
                    "Output tokens {} exceeded threshold {}",
                    session.total_output_tokens, self.config.output_token_threshold
                ),
                current_value: session.total_output_tokens as f64,
                threshold: self.config.output_token_threshold as f64,
                session_id: session_id.to_owned(),
            });
        }

        // Check turn cost
        if estimated_cost >= self.config.turn_cost_threshold_usd
            && !session
                .alerts_fired
                .contains(&CostAlertType::TurnCostExceeded)
        {
            session.alerts_fired.push(CostAlertType::TurnCostExceeded);
            alerts.push(CostAlert {
                alert_type: CostAlertType::TurnCostExceeded,
                message: format!(
                    "Turn cost ${:.4} exceeded threshold ${:.4}",
                    estimated_cost, self.config.turn_cost_threshold_usd
                ),
                current_value: estimated_cost,
                threshold: self.config.turn_cost_threshold_usd,
                session_id: session_id.to_owned(),
            });
        }

        alerts
    }

    /// Get current usage summary for a session.
    pub fn summary(&self, session_id: &str) -> Option<SessionCostState> {
        let state = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        state.get(session_id).cloned()
    }

    /// Reset tracking for a session.
    pub fn reset(&self, session_id: &str) {
        let mut state = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        state.remove(session_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_alert_below_threshold() {
        let hook = CostHook::new_default();
        let alerts = hook.record_turn("test-session", 100, 50, 0.005);
        assert!(alerts.is_empty());
    }

    #[test]
    fn fires_total_cost_alert() {
        let hook = CostHook::new(CostHookConfig {
            cost_threshold_usd: 0.01,
            ..Default::default()
        });
        let alerts = hook.record_turn("test-session", 1000, 500, 0.015);
        assert!(
            alerts
                .iter()
                .any(|a| a.alert_type == CostAlertType::TotalCostExceeded)
        );
    }

    #[test]
    fn fires_only_once_per_threshold() {
        let hook = CostHook::new(CostHookConfig {
            cost_threshold_usd: 0.01,
            ..Default::default()
        });
        let _ = hook.record_turn("test-session", 1000, 500, 0.015);
        let alerts = hook.record_turn("test-session", 1000, 500, 0.015);
        assert!(
            !alerts
                .iter()
                .any(|a| a.alert_type == CostAlertType::TotalCostExceeded)
        );
    }

    #[test]
    fn reset_clears_state() {
        let hook = CostHook::new(CostHookConfig {
            cost_threshold_usd: 0.01,
            ..Default::default()
        });
        let _ = hook.record_turn("test-session", 1000, 500, 0.015);
        hook.reset("test-session");
        assert!(hook.summary("test-session").is_none());
    }

    #[test]
    fn independent_session_tracking() {
        let hook = CostHook::new_default();
        hook.record_turn("session-a", 100, 50, 0.005);
        hook.record_turn("session-b", 100, 50, 0.005);
        assert!(hook.summary("session-a").is_some());
        assert!(hook.summary("session-b").is_some());
    }
}

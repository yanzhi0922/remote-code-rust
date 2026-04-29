use serde::{Deserialize, Serialize};

/// Simple token/turn budget tracker for the Phase 2 compat engine.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BudgetTracker {
    pub max_turns: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_total_tokens: Option<u64>,
}

impl BudgetTracker {
    #[must_use]
    pub fn new(max_turns: u32, max_total_tokens: Option<u64>) -> Self {
        Self {
            max_turns,
            max_total_tokens,
        }
    }

    #[must_use]
    pub fn evaluate(&self, turn: u32, total_tokens: u64) -> TokenBudgetDecision {
        if turn >= self.max_turns {
            return TokenBudgetDecision::Stop {
                reason: format!("turn budget exceeded ({})", self.max_turns),
            };
        }
        if let Some(limit) = self.max_total_tokens
            && total_tokens >= limit
        {
            return TokenBudgetDecision::Stop {
                reason: format!("token budget exceeded ({limit})"),
            };
        }
        TokenBudgetDecision::Continue
    }
}

/// Decision produced by the budget tracker.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum TokenBudgetDecision {
    Continue,
    Stop { reason: String },
}

#[cfg(test)]
mod tests {
    use super::{BudgetTracker, TokenBudgetDecision};

    #[test]
    fn budget_tracker_stops_on_turn_limit() {
        let tracker = BudgetTracker::new(2, None);
        assert_eq!(tracker.evaluate(0, 0), TokenBudgetDecision::Continue);
        assert_eq!(
            tracker.evaluate(2, 0),
            TokenBudgetDecision::Stop {
                reason: "turn budget exceeded (2)".to_owned()
            }
        );
    }

    #[test]
    fn budget_tracker_stops_on_token_limit() {
        let tracker = BudgetTracker::new(5, Some(100));
        assert_eq!(
            tracker.evaluate(1, 100),
            TokenBudgetDecision::Stop {
                reason: "token budget exceeded (100)".to_owned()
            }
        );
    }
}

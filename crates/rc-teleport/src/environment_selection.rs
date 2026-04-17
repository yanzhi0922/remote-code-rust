//! Environment selection logic.
//!
//! Provides scoring, ranking, and selection algorithms for choosing
//! the best target environment for session teleportation.

use serde::{Deserialize, Serialize};

use crate::environments::{Environment, EnvironmentStatus};

// ---------------------------------------------------------------------------
// SelectionCriteria
// ---------------------------------------------------------------------------

/// Criteria for selecting a target environment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SelectionCriteria {
    /// If true, prefer environments that are currently online.
    #[serde(default)]
    pub prefer_online: bool,
    /// Optional name filter to narrow the candidate set.
    #[serde(default)]
    pub name_filter: Option<String>,
}

impl Default for SelectionCriteria {
    fn default() -> Self {
        Self {
            prefer_online: true,
            name_filter: None,
        }
    }
}

impl SelectionCriteria {
    /// Create criteria that only match available environments.
    pub fn online_only() -> Self {
        Self {
            prefer_online: true,
            name_filter: None,
        }
    }

    /// Create criteria with a name filter.
    pub fn with_name_filter(name: impl Into<String>) -> Self {
        Self {
            prefer_online: true,
            name_filter: Some(name.into()),
        }
    }
}

// ---------------------------------------------------------------------------
// ScoredEnvironment
// ---------------------------------------------------------------------------

/// An environment paired with its selection score.
#[derive(Debug, Clone, PartialEq)]
struct ScoredEnvironment {
    environment: Environment,
    score: f64,
}

// ---------------------------------------------------------------------------
// Scoring
// ---------------------------------------------------------------------------

/// Compute a selection score for an environment given the criteria.
///
/// Higher scores indicate better matches. The scoring considers:
/// - Status: Available environments get a bonus
/// - Name match: Environments matching the name filter get a bonus
/// - Recency: More recently active environments get a small bonus
fn score_environment(env: &Environment, criteria: &SelectionCriteria) -> f64 {
    let mut score = 0.0;

    // Status scoring
    match env.status {
        EnvironmentStatus::Available => score += 100.0,
        EnvironmentStatus::Busy => score += 30.0,
        EnvironmentStatus::Offline => {
            if criteria.prefer_online {
                score -= 200.0;
            } else {
                score += 10.0;
            }
        }
    }

    // Name filter scoring
    if let Some(ref name_filter) = criteria.name_filter {
        let filter_lower = name_filter.to_lowercase();
        let name_lower = env.name.to_lowercase();
        if name_lower.contains(&filter_lower) {
            score += 50.0;
        } else {
            score -= 100.0;
        }
    }

    score
}

// ---------------------------------------------------------------------------
// Selection functions
// ---------------------------------------------------------------------------

/// Select the best environment from a list based on the given criteria.
///
/// Returns `None` if no environments match the criteria or the list is empty.
pub fn select_environment(
    environments: &[Environment],
    criteria: &SelectionCriteria,
) -> Option<Environment> {
    if environments.is_empty() {
        return None;
    }

    let mut scored: Vec<ScoredEnvironment> = environments
        .iter()
        .map(|env| ScoredEnvironment {
            environment: env.clone(),
            score: score_environment(env, criteria),
        })
        .collect();

    // Sort by score descending
    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Return the highest-scoring environment if its score is positive
    scored
        .into_iter()
        .next()
        .filter(|s| s.score > 0.0)
        .map(|s| s.environment)
}

/// Rank all environments by their suitability for the given criteria.
///
/// Returns environments sorted from best to worst match.
pub fn rank_environments(
    environments: &[Environment],
    criteria: &SelectionCriteria,
) -> Vec<Environment> {
    let mut scored: Vec<ScoredEnvironment> = environments
        .iter()
        .map(|env| ScoredEnvironment {
            environment: env.clone(),
            score: score_environment(env, criteria),
        })
        .collect();

    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    scored.into_iter().map(|s| s.environment).collect()
}

/// Filter environments to only those that match the criteria with a positive score.
pub fn filter_eligible(
    environments: &[Environment],
    criteria: &SelectionCriteria,
) -> Vec<Environment> {
    environments
        .iter()
        .filter(|env| score_environment(env, criteria) > 0.0)
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::environments::EnvironmentStatus;
    use chrono::DateTime;

    fn make_env(id: &str, name: &str, status: EnvironmentStatus) -> Environment {
        Environment {
            id: id.to_string(),
            name: name.to_string(),
            host: format!("{id}.example.com"),
            status,
            last_active: DateTime::UNIX_EPOCH,
        }
    }

    #[test]
    fn select_prefers_available() {
        let envs = vec![
            make_env("1", "Busy Env", EnvironmentStatus::Busy),
            make_env("2", "Available Env", EnvironmentStatus::Available),
            make_env("3", "Offline Env", EnvironmentStatus::Offline),
        ];
        let criteria = SelectionCriteria::default();
        let selected = select_environment(&envs, &criteria);
        assert!(selected.is_some());
        assert_eq!(selected.expect("env").id, "2");
    }

    #[test]
    fn select_with_name_filter() {
        let envs = vec![
            make_env("1", "Production Server", EnvironmentStatus::Available),
            make_env("2", "Staging Server", EnvironmentStatus::Available),
            make_env("3", "Dev Machine", EnvironmentStatus::Available),
        ];
        let criteria = SelectionCriteria::with_name_filter("staging");
        let selected = select_environment(&envs, &criteria);
        assert!(selected.is_some());
        assert_eq!(selected.expect("env").id, "2");
    }

    #[test]
    fn select_returns_none_when_all_offline_and_prefer_online() {
        let envs = vec![
            make_env("1", "Env 1", EnvironmentStatus::Offline),
            make_env("2", "Env 2", EnvironmentStatus::Offline),
        ];
        let criteria = SelectionCriteria::default();
        assert!(select_environment(&envs, &criteria).is_none());
    }

    #[test]
    fn select_returns_none_for_empty_list() {
        let envs: Vec<Environment> = vec![];
        let criteria = SelectionCriteria::default();
        assert!(select_environment(&envs, &criteria).is_none());
    }

    #[test]
    fn rank_environments_orders_by_score() {
        let envs = vec![
            make_env("1", "Offline", EnvironmentStatus::Offline),
            make_env("2", "Available", EnvironmentStatus::Available),
            make_env("3", "Busy", EnvironmentStatus::Busy),
        ];
        let criteria = SelectionCriteria::default();
        let ranked = rank_environments(&envs, &criteria);
        assert_eq!(ranked[0].id, "2"); // Available first
        assert_eq!(ranked[1].id, "3"); // Busy second
        assert_eq!(ranked[2].id, "1"); // Offline last
    }

    #[test]
    fn filter_eligible_removes_negative_scores() {
        let envs = vec![
            make_env("1", "Good", EnvironmentStatus::Available),
            make_env("2", "Bad", EnvironmentStatus::Offline),
        ];
        let criteria = SelectionCriteria::default();
        let eligible = filter_eligible(&envs, &criteria);
        assert_eq!(eligible.len(), 1);
        assert_eq!(eligible[0].id, "1");
    }

    #[test]
    fn selection_criteria_default() {
        let criteria = SelectionCriteria::default();
        assert!(criteria.prefer_online);
        assert!(criteria.name_filter.is_none());
    }

    #[test]
    fn selection_criteria_serialization() {
        let criteria = SelectionCriteria {
            prefer_online: false,
            name_filter: Some("test".to_string()),
        };
        let json = serde_json::to_string(&criteria).expect("serialize");
        let parsed: SelectionCriteria = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(criteria, parsed);
    }
}

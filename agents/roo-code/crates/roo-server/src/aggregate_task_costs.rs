//! Task cost aggregation across task hierarchies.
//!
//! Derived from `src/core/webview/aggregateTaskCosts.ts`.
//!
//! Recursively aggregates costs for a task and all its subtasks,
//! supporting circular reference detection and detailed breakdowns.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use tracing::warn;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Aggregated costs for a task and its children.
///
/// Source: `src/core/webview/aggregateTaskCosts.ts` — `AggregatedCosts`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregatedCosts {
    /// This task's own API costs.
    pub own_cost: f64,
    /// Sum of all direct children costs (recursive).
    pub children_cost: f64,
    /// `own_cost + children_cost`
    pub total_cost: f64,
    /// Optional detailed breakdown by child ID.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub child_breakdown: Option<HashMap<String, AggregatedCosts>>,
}

impl Default for AggregatedCosts {
    fn default() -> Self {
        Self {
            own_cost: 0.0,
            children_cost: 0.0,
            total_cost: 0.0,
            child_breakdown: None,
        }
    }
}

/// A history item with cost and children information.
pub trait HistoryItem {
    /// The total cost of this task.
    fn total_cost(&self) -> f64;
    /// The IDs of child tasks.
    fn child_ids(&self) -> &[String];
}

// ---------------------------------------------------------------------------
// Aggregation
// ---------------------------------------------------------------------------

/// Recursively aggregates costs for a task and all its subtasks.
///
/// Source: `src/core/webview/aggregateTaskCosts.ts` — `aggregateTaskCostsRecursive`
///
/// # Arguments
/// * `task_id` - The task ID to aggregate costs for
/// * `get_task_history` - Function to load a `HistoryItem` by task ID
/// * `visited` - Set to prevent circular references
///
/// # Returns
/// `AggregatedCosts` with the task's own cost, children costs, and total.
pub async fn aggregate_task_costs_recursive<H>(
    task_id: &str,
    get_task_history: impl Fn(&str) -> Option<H>,
    visited: Option<HashSet<String>>,
) -> AggregatedCosts
where
    H: HistoryItem,
{
    let mut visited = visited.unwrap_or_default();

    // Prevent infinite loops
    if visited.contains(task_id) {
        warn!(
            "[aggregateTaskCostsRecursive] Circular reference detected: {}",
            task_id
        );
        return AggregatedCosts::default();
    }
    visited.insert(task_id.to_string());

    // Load this task's history
    let history = match get_task_history(task_id) {
        Some(h) => h,
        None => {
            warn!(
                "[aggregateTaskCostsRecursive] Task {} not found",
                task_id
            );
            return AggregatedCosts::default();
        }
    };

    let own_cost = history.total_cost();
    let mut children_cost = 0.0;
    let mut child_breakdown: HashMap<String, AggregatedCosts> = HashMap::new();

    // Recursively aggregate child costs
    let child_ids = history.child_ids();
    if !child_ids.is_empty() {
        for child_id in child_ids {
            let child_aggregated = aggregate_task_costs_recursive(
                child_id,
                &get_task_history,
                Some(visited.clone()),
            )
            .await;
            children_cost += child_aggregated.total_cost;
            child_breakdown.insert(child_id.clone(), child_aggregated);
        }
    }

    let total_cost = own_cost + children_cost;

    AggregatedCosts {
        own_cost,
        children_cost,
        total_cost,
        child_breakdown: if child_breakdown.is_empty() {
            None
        } else {
            Some(child_breakdown)
        },
    }
}

/// Synchronous version of cost aggregation for use in non-async contexts.
pub fn aggregate_task_costs_sync<H>(
    task_id: &str,
    get_task_history: &dyn Fn(&str) -> Option<H>,
    visited: Option<HashSet<String>>,
) -> AggregatedCosts
where
    H: HistoryItem,
{
    let mut visited = visited.unwrap_or_default();

    if visited.contains(task_id) {
        warn!(
            "[aggregateTaskCostsSync] Circular reference detected: {}",
            task_id
        );
        return AggregatedCosts::default();
    }
    visited.insert(task_id.to_string());

    let history = match get_task_history(task_id) {
        Some(h) => h,
        None => {
            warn!("[aggregateTaskCostsSync] Task {} not found", task_id);
            return AggregatedCosts::default();
        }
    };

    let own_cost = history.total_cost();
    let mut children_cost = 0.0;
    let mut child_breakdown: HashMap<String, AggregatedCosts> = HashMap::new();

    let child_ids = history.child_ids();
    if !child_ids.is_empty() {
        for child_id in child_ids {
            let child_aggregated = aggregate_task_costs_sync(
                child_id,
                get_task_history,
                Some(visited.clone()),
            );
            children_cost += child_aggregated.total_cost;
            child_breakdown.insert(child_id.clone(), child_aggregated);
        }
    }

    let total_cost = own_cost + children_cost;

    AggregatedCosts {
        own_cost,
        children_cost,
        total_cost,
        child_breakdown: if child_breakdown.is_empty() {
            None
        } else {
            Some(child_breakdown)
        },
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A simple history item implementation for testing.
    #[derive(Clone)]
    struct TestHistoryItem {
        cost: f64,
        children: Vec<String>,
    }

    impl HistoryItem for TestHistoryItem {
        fn total_cost(&self) -> f64 {
            self.cost
        }
        fn child_ids(&self) -> &[String] {
            &self.children
        }
    }

    #[test]
    fn test_aggregate_single_task() {
        let items: HashMap<String, TestHistoryItem> = vec![
            ("task1".to_string(), TestHistoryItem { cost: 1.5, children: vec![] }),
        ].into_iter().collect();

        let result = aggregate_task_costs_sync("task1", &|id| items.get(id).cloned(), None);
        assert_eq!(result.own_cost, 1.5);
        assert_eq!(result.children_cost, 0.0);
        assert_eq!(result.total_cost, 1.5);
        assert!(result.child_breakdown.is_none());
    }

    #[test]
    fn test_aggregate_with_children() {
        let items: HashMap<String, TestHistoryItem> = vec![
            ("parent".to_string(), TestHistoryItem { cost: 2.0, children: vec!["child1".to_string(), "child2".to_string()] }),
            ("child1".to_string(), TestHistoryItem { cost: 0.5, children: vec![] }),
            ("child2".to_string(), TestHistoryItem { cost: 1.0, children: vec![] }),
        ].into_iter().collect();

        let result = aggregate_task_costs_sync("parent", &|id| items.get(id).cloned(), None);
        assert_eq!(result.own_cost, 2.0);
        assert_eq!(result.children_cost, 1.5);
        assert_eq!(result.total_cost, 3.5);
        assert!(result.child_breakdown.is_some());
        let breakdown = result.child_breakdown.unwrap();
        assert_eq!(breakdown.len(), 2);
        assert_eq!(breakdown.get("child1").unwrap().total_cost, 0.5);
    }

    #[test]
    fn test_aggregate_not_found() {
        let result: AggregatedCosts =
            aggregate_task_costs_sync("nonexistent", &|_id| None::<TestHistoryItem>, None);
        assert_eq!(result.total_cost, 0.0);
    }

    #[test]
    fn test_aggregate_circular_reference() {
        let items: HashMap<String, TestHistoryItem> = vec![
            ("a".to_string(), TestHistoryItem { cost: 1.0, children: vec!["b".to_string()] }),
            ("b".to_string(), TestHistoryItem { cost: 1.0, children: vec!["a".to_string()] }),
        ].into_iter().collect();

        let result = aggregate_task_costs_sync("a", &|id| items.get(id).cloned(), None);
        // Should handle circular reference gracefully
        assert!(result.total_cost >= 2.0);
    }

    #[test]
    fn test_aggregate_nested_children() {
        let items: HashMap<String, TestHistoryItem> = vec![
            ("root".to_string(), TestHistoryItem { cost: 1.0, children: vec!["mid".to_string()] }),
            ("mid".to_string(), TestHistoryItem { cost: 2.0, children: vec!["leaf".to_string()] }),
            ("leaf".to_string(), TestHistoryItem { cost: 3.0, children: vec![] }),
        ].into_iter().collect();

        let result = aggregate_task_costs_sync("root", &|id| items.get(id).cloned(), None);
        assert_eq!(result.own_cost, 1.0);
        assert_eq!(result.children_cost, 5.0); // 2.0 + 3.0
        assert_eq!(result.total_cost, 6.0);
    }

    #[test]
    fn test_aggregated_costs_default() {
        let costs = AggregatedCosts::default();
        assert_eq!(costs.own_cost, 0.0);
        assert_eq!(costs.children_cost, 0.0);
        assert_eq!(costs.total_cost, 0.0);
        assert!(costs.child_breakdown.is_none());
    }

    #[test]
    fn test_aggregated_costs_serialization() {
        let costs = AggregatedCosts {
            own_cost: 1.5,
            children_cost: 2.5,
            total_cost: 4.0,
            child_breakdown: None,
        };
        let json = serde_json::to_string(&costs).unwrap();
        assert!(json.contains("own_cost"));
        assert!(json.contains("4.0"));
    }
}

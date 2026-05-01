//! Search signals and telemetry collection.
//!
//! [`SearchSignal`] records metadata about each search invocation for analytics.
//! [`SignalCollector`] accumulates signals and can export them.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A single search signal recorded for telemetry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchSignal {
    /// The query string that was issued.
    pub query: String,
    /// Number of results returned.
    pub result_count: usize,
    /// Index of the result the user selected (0-based), if any.
    pub selected_index: Option<usize>,
    /// Latency of the search in milliseconds.
    pub latency_ms: u64,
}

/// Collector that accumulates search signals.
#[derive(Debug, Clone)]
pub struct SignalCollector {
    signals: Arc<RwLock<Vec<SearchSignal>>>,
}

impl SignalCollector {
    /// Create a new, empty collector.
    pub fn new() -> Self {
        Self {
            signals: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Record a search signal.
    pub async fn record_signal(&self, signal: SearchSignal) {
        debug!(query = %signal.query, latency_ms = signal.latency_ms, "Recorded search signal");
        self.signals.write().await.push(signal);
    }

    /// Return the number of recorded signals.
    pub async fn count(&self) -> usize {
        self.signals.read().await.len()
    }

    /// Return all recorded signals.
    pub async fn all_signals(&self) -> Vec<SearchSignal> {
        self.signals.read().await.clone()
    }

    /// Compute the average latency of all recorded signals.
    pub async fn average_latency_ms(&self) -> f64 {
        let signals = self.signals.read().await;
        if signals.is_empty() {
            return 0.0;
        }
        let total: u64 = signals.iter().map(|s| s.latency_ms).sum();
        total as f64 / signals.len() as f64
    }

    /// Clear all recorded signals.
    pub async fn clear(&self) {
        self.signals.write().await.clear();
    }

    /// Export signals as a JSON string.
    pub async fn export_json(&self) -> anyhow::Result<String> {
        let signals = self.signals.read().await;
        Ok(serde_json::to_string_pretty(&*signals)?)
    }
}

impl Default for SignalCollector {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_signal(query: &str, count: usize, latency: u64) -> SearchSignal {
        SearchSignal {
            query: query.to_string(),
            result_count: count,
            selected_index: None,
            latency_ms: latency,
        }
    }

    #[tokio::test]
    async fn record_and_count() {
        let collector = SignalCollector::new();
        collector.record_signal(make_signal("test", 5, 10)).await;
        collector.record_signal(make_signal("test2", 3, 20)).await;
        assert_eq!(collector.count().await, 2);
    }

    #[tokio::test]
    async fn all_signals_returns_copy() {
        let collector = SignalCollector::new();
        collector.record_signal(make_signal("q", 1, 5)).await;
        let signals = collector.all_signals().await;
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0].query, "q");
    }

    #[tokio::test]
    async fn average_latency() {
        let collector = SignalCollector::new();
        collector.record_signal(make_signal("a", 1, 10)).await;
        collector.record_signal(make_signal("b", 1, 30)).await;
        let avg = collector.average_latency_ms().await;
        assert!((avg - 20.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn average_latency_empty() {
        let collector = SignalCollector::new();
        assert_eq!(collector.average_latency_ms().await, 0.0);
    }

    #[tokio::test]
    async fn clear_empties() {
        let collector = SignalCollector::new();
        collector.record_signal(make_signal("x", 0, 1)).await;
        collector.clear().await;
        assert_eq!(collector.count().await, 0);
    }

    #[tokio::test]
    async fn export_json_roundtrip() {
        let collector = SignalCollector::new();
        collector
            .record_signal(SearchSignal {
                query: "deploy".into(),
                result_count: 3,
                selected_index: Some(0),
                latency_ms: 42,
            })
            .await;
        let json = collector.export_json().await.expect("export");
        assert!(json.contains("deploy"));
        let parsed: Vec<SearchSignal> = serde_json::from_str(&json).expect("parse");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].latency_ms, 42);
    }

    #[tokio::test]
    async fn default_is_empty() {
        let collector = SignalCollector::default();
        assert_eq!(collector.count().await, 0);
    }

    #[test]
    fn signal_serde_roundtrip() {
        let signal = SearchSignal {
            query: "test".into(),
            result_count: 5,
            selected_index: Some(2),
            latency_ms: 100,
        };
        let json = serde_json::to_string(&signal).expect("serialize");
        let back: SearchSignal = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.query, "test");
        assert_eq!(back.selected_index, Some(2));
    }
}

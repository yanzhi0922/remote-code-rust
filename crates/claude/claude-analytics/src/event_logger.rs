//! Main analytics service.
//!
//! Provides `AnalyticsService` as the primary interface for logging
//! analytics events, with convenience methods for common event types
//! and background flush support.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use tokio::sync::Mutex;
use tracing::{debug, error, instrument};

use crate::config::AnalyticsConfig;
use crate::metadata::{AnalyticsEvent, EventMetadata};
use crate::sink::{AnalyticsSink, NullSink};

// ---------------------------------------------------------------------------
// AnalyticsService
// ---------------------------------------------------------------------------

/// Main analytics service for logging events.
///
/// Wraps an `AnalyticsSink` and provides typed convenience methods for
/// common analytics operations. Supports background flushing via tokio.
#[derive(Debug)]
pub struct AnalyticsService {
    config: AnalyticsConfig,
    sink: Arc<dyn AnalyticsSink>,
    flush_handle: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
}

impl AnalyticsService {
    /// Create a new analytics service with the given configuration.
    pub fn new(config: AnalyticsConfig) -> Self {
        let sink: Arc<dyn AnalyticsSink> = Arc::new(NullSink::new());

        Self {
            config,
            sink,
            flush_handle: Arc::new(Mutex::new(None)),
        }
    }

    /// Create a new analytics service with a custom sink.
    pub fn with_sink(config: AnalyticsConfig, sink: Arc<dyn AnalyticsSink>) -> Self {
        Self {
            config,
            sink,
            flush_handle: Arc::new(Mutex::new(None)),
        }
    }

    /// Create a disabled analytics service (all events discarded).
    pub fn disabled() -> Self {
        Self::new(AnalyticsConfig::default())
    }

    /// Log a generic analytics event.
    #[instrument(skip(self, metadata), fields(name = %name))]
    pub fn log_event(&self, name: &str, metadata: EventMetadata) {
        if !self.config.enabled {
            debug!(name = %name, "Analytics disabled, skipping event");
            return;
        }
        let event = AnalyticsEvent::new(name, metadata);
        self.sink.log_event(event);
    }

    /// Log a tool use event.
    pub fn log_tool_use(&self, tool: &str, duration_ms: u64, success: bool) {
        let metadata = EventMetadata::new()
            .with_tool_name(tool)
            .with_duration_ms(duration_ms)
            .with_success(success);
        self.log_event("tool_use", metadata);
    }

    /// Log a query event.
    pub fn log_query(&self, model: &str, token_count: u64, duration_ms: u64) {
        let metadata = EventMetadata::new()
            .with_model(model)
            .with_token_count(token_count)
            .with_duration_ms(duration_ms)
            .with_success(true);
        self.log_event("query", metadata);
    }

    /// Log a context compaction event.
    pub fn log_compact(&self, strategy: &str, before_tokens: u64, after_tokens: u64) {
        let metadata = EventMetadata::new()
            .with_extra("strategy", serde_json::json!(strategy))
            .with_extra("before_tokens", serde_json::json!(before_tokens))
            .with_extra("after_tokens", serde_json::json!(after_tokens))
            .with_success(true);
        self.log_event("compact", metadata);
    }

    /// Log a permission decision event.
    pub fn log_permission_decision(&self, tool: &str, decision: &str) {
        let metadata = EventMetadata::new()
            .with_tool_name(tool)
            .with_extra("decision", serde_json::json!(decision))
            .with_success(true);
        self.log_event("permission_decision", metadata);
    }

    /// Flush any buffered events.
    pub async fn flush(&self) -> Result<()> {
        self.sink.flush()
    }

    /// Start a background flush loop.
    ///
    /// Spawns a tokio task that periodically flushes events based on
    /// the configured `flush_interval_secs`.
    pub async fn start_background_flush(&self) {
        let interval = self.config.flush_interval_secs;
        if interval == 0 || !self.config.enabled {
            return;
        }

        let sink = self.sink.clone();
        let handle = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(Duration::from_secs(interval));
            loop {
                ticker.tick().await;
                if let Err(e) = sink.flush() {
                    error!("Background analytics flush failed: {e}");
                }
            }
        });

        let mut guard = self.flush_handle.lock().await;
        *guard = Some(handle);
    }

    /// Stop the background flush loop.
    pub async fn stop_background_flush(&self) {
        let mut guard = self.flush_handle.lock().await;
        if let Some(handle) = guard.take() {
            handle.abort();
            debug!("Background analytics flush stopped");
        }
    }

    /// Check if analytics is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get a reference to the configuration.
    pub fn config(&self) -> &AnalyticsConfig {
        &self.config
    }
}

impl Drop for AnalyticsService {
    fn drop(&mut self) {
        // Best-effort abort of the background flush task. If the tokio
        // runtime is still alive this cancels the periodic flush; if not,
        // the task is already gone.
        if let Ok(mut guard) = self.flush_handle.try_lock() {
            if let Some(handle) = guard.take() {
                handle.abort();
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sink::QueuedSink;

    #[test]
    fn disabled_service_discards_events() {
        let service = AnalyticsService::disabled();
        assert!(!service.is_enabled());
        // These should not panic
        service.log_tool_use("bash", 100, true);
        service.log_query("gpt-4", 500, 200);
    }

    #[test]
    fn service_logs_tool_use() {
        let config = AnalyticsConfig {
            enabled: true,
            ..Default::default()
        };
        let queued = Arc::new(QueuedSink::new());
        let service = AnalyticsService::with_sink(config, queued.clone());
        service.log_tool_use("read_file", 50, true);
        assert_eq!(queued.buffered_count(), 1);
    }

    #[test]
    fn service_logs_query() {
        let config = AnalyticsConfig {
            enabled: true,
            ..Default::default()
        };
        let queued = Arc::new(QueuedSink::new());
        let service = AnalyticsService::with_sink(config, queued.clone());
        service.log_query("claude-3", 1000, 300);
        assert_eq!(queued.buffered_count(), 1);
    }

    #[test]
    fn service_logs_compact() {
        let config = AnalyticsConfig {
            enabled: true,
            ..Default::default()
        };
        let queued = Arc::new(QueuedSink::new());
        let service = AnalyticsService::with_sink(config, queued.clone());
        service.log_compact("summary", 10000, 3000);
        assert_eq!(queued.buffered_count(), 1);
    }

    #[test]
    fn service_logs_permission_decision() {
        let config = AnalyticsConfig {
            enabled: true,
            ..Default::default()
        };
        let queued = Arc::new(QueuedSink::new());
        let service = AnalyticsService::with_sink(config, queued.clone());
        service.log_permission_decision("bash", "allow");
        assert_eq!(queued.buffered_count(), 1);
    }

    #[tokio::test]
    async fn background_flush_starts_and_stops() {
        let config = AnalyticsConfig {
            enabled: true,
            flush_interval_secs: 1,
            ..Default::default()
        };
        let service = AnalyticsService::new(config);
        service.start_background_flush().await;
        service.stop_background_flush().await;
    }
}

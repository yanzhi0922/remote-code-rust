//! Event sink trait and implementations.
//!
//! Provides the `AnalyticsSink` trait and multiple implementations:
//! - `NullSink` — Discards all events (default when analytics disabled)
//! - `QueuedSink` — Buffers events until a real sink is attached
//! - `CompositeSink` — Routes events to multiple backends

use anyhow::Result;
use std::sync::{Arc, Mutex};

use crate::exporter::EventExporter;
use crate::metadata::AnalyticsEvent;

// ---------------------------------------------------------------------------
// AnalyticsSink trait
// ---------------------------------------------------------------------------

/// Trait for analytics event sinks.
///
/// A sink receives analytics events and is responsible for delivering
/// them to one or more backends.
pub trait AnalyticsSink: Send + Sync + std::fmt::Debug {
    /// Log an analytics event.
    fn log_event(&self, event: AnalyticsEvent);

    /// Flush any buffered events to their destination.
    fn flush(&self) -> Result<()>;
}

// ---------------------------------------------------------------------------
// NullSink
// ---------------------------------------------------------------------------

/// A sink that discards all events.
///
/// Used as the default when analytics is disabled.
#[derive(Debug, Clone, Default)]
pub struct NullSink {
    _private: (),
}

impl NullSink {
    /// Create a new null sink.
    pub fn new() -> Self {
        Self::default()
    }
}

impl AnalyticsSink for NullSink {
    fn log_event(&self, _event: AnalyticsEvent) {
        // Discard the event
    }

    fn flush(&self) -> Result<()> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// QueuedSink
// ---------------------------------------------------------------------------

/// A sink that buffers events in memory until a real sink is attached.
///
/// Once a real sink is attached, all buffered events are forwarded to it,
/// and subsequent events go directly to the real sink.
#[derive(Debug)]
pub struct QueuedSink {
    buffer: Arc<Mutex<Vec<AnalyticsEvent>>>,
    target: Arc<Mutex<Option<Arc<dyn AnalyticsSink>>>>,
}

impl Default for QueuedSink {
    fn default() -> Self {
        Self::new()
    }
}

impl QueuedSink {
    /// Create a new queued sink with an empty buffer.
    pub fn new() -> Self {
        Self {
            buffer: Arc::new(Mutex::new(Vec::new())),
            target: Arc::new(Mutex::new(None)),
        }
    }

    /// Attach a real sink and flush all buffered events to it.
    pub fn attach(&self, sink: Arc<dyn AnalyticsSink>) -> Result<()> {
        let mut target = self.target.lock().map_err(|e| anyhow::anyhow!("Lock poisoned: {e}"))?;
        *target = Some(sink);

        // Flush buffered events to the new target
        let mut buffer = self.buffer.lock().map_err(|e| anyhow::anyhow!("Lock poisoned: {e}"))?;
        if let Some(ref real_sink) = *target {
            for event in buffer.drain(..) {
                real_sink.log_event(event);
            }
        }

        Ok(())
    }

    /// Number of events currently buffered.
    pub fn buffered_count(&self) -> usize {
        self.buffer
            .lock()
            .map(|b| b.len())
            .unwrap_or(0)
    }
}

impl AnalyticsSink for QueuedSink {
    fn log_event(&self, event: AnalyticsEvent) {
        // If a target is attached, forward directly
        if let Ok(target) = self.target.lock()
            && let Some(ref sink) = *target
        {
            sink.log_event(event);
            return;
        }
        // Otherwise, buffer the event
        if let Ok(mut buffer) = self.buffer.lock() {
            buffer.push(event);
        }
    }

    fn flush(&self) -> Result<()> {
        let target = self.target.lock().map_err(|e| anyhow::anyhow!("Lock poisoned: {e}"))?;
        if let Some(ref sink) = *target {
            sink.flush()?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// CompositeSink
// ---------------------------------------------------------------------------

/// A sink that routes events to multiple backends.
#[derive(Debug)]
pub struct CompositeSink {
    sinks: Vec<Arc<dyn AnalyticsSink>>,
}

impl CompositeSink {
    /// Create a new composite sink with no backends.
    pub fn new() -> Self {
        Self { sinks: Vec::new() }
    }

    /// Add a sink to the composite.
    pub fn add_sink(&mut self, sink: Arc<dyn AnalyticsSink>) {
        self.sinks.push(sink);
    }

    /// Number of sinks in the composite.
    pub fn sink_count(&self) -> usize {
        self.sinks.len()
    }
}

impl Default for CompositeSink {
    fn default() -> Self {
        Self::new()
    }
}

impl AnalyticsSink for CompositeSink {
    fn log_event(&self, event: AnalyticsEvent) {
        for sink in &self.sinks {
            sink.log_event(event.clone());
        }
    }

    fn flush(&self) -> Result<()> {
        for sink in &self.sinks {
            sink.flush()?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ExporterSink
// ---------------------------------------------------------------------------

/// A sink that delegates to an exporter for actual delivery.
#[derive(Debug)]
pub struct ExporterSink {
    exporter: Arc<Mutex<Box<dyn EventExporter>>>,
    buffer: Arc<Mutex<Vec<AnalyticsEvent>>>,
}

impl ExporterSink {
    /// Create a new exporter sink wrapping the given exporter.
    pub fn new(exporter: Box<dyn EventExporter>) -> Self {
        Self {
            exporter: Arc::new(Mutex::new(exporter)),
            buffer: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl AnalyticsSink for ExporterSink {
    fn log_event(&self, event: AnalyticsEvent) {
        if let Ok(mut buffer) = self.buffer.lock() {
            buffer.push(event);
        }
    }

    fn flush(&self) -> Result<()> {
        let mut buffer = self.buffer.lock().map_err(|e| anyhow::anyhow!("Lock poisoned: {e}"))?;
        if buffer.is_empty() {
            return Ok(());
        }

        let events: Vec<AnalyticsEvent> = buffer.drain(..).collect();
        let exporter = self.exporter.lock().map_err(|e| anyhow::anyhow!("Lock poisoned: {e}"))?;
        exporter.export(&events)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::EventMetadata;

    #[test]
    fn null_sink_discards_events() {
        let sink = NullSink::new();
        let event = AnalyticsEvent::simple("test");
        sink.log_event(event);
        sink.flush().expect("flush should succeed");
    }

    #[test]
    fn queued_sink_buffers_events() {
        let sink = QueuedSink::new();
        sink.log_event(AnalyticsEvent::simple("event1"));
        sink.log_event(AnalyticsEvent::simple("event2"));
        assert_eq!(sink.buffered_count(), 2);
    }

    #[test]
    fn queued_sink_forwards_to_target() {
        let queued = Arc::new(QueuedSink::new());
        queued.log_event(AnalyticsEvent::simple("buffered"));

        // Attach a null sink — events should be drained
        queued.attach(Arc::new(NullSink::new())).expect("attach");
        assert_eq!(queued.buffered_count(), 0);
    }

    #[test]
    fn composite_sink_routes_to_all() {
        let mut composite = CompositeSink::new();
        let q1 = Arc::new(QueuedSink::new());
        let q2 = Arc::new(QueuedSink::new());

        composite.add_sink(q1.clone());
        composite.add_sink(q2.clone());
        assert_eq!(composite.sink_count(), 2);

        composite.log_event(AnalyticsEvent::simple("test"));
        assert_eq!(q1.buffered_count(), 1);
        assert_eq!(q2.buffered_count(), 1);
    }

    #[test]
    fn exporter_sink_flushes_to_exporter() {
        use crate::exporter::FileExporter;
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("events.jsonl");
        let exporter = Box::new(FileExporter::new(path.to_string_lossy().to_string()));
        let sink = ExporterSink::new(exporter);

        sink.log_event(AnalyticsEvent::new("test", EventMetadata::new()));
        sink.flush().expect("flush");

        let contents = std::fs::read_to_string(&path).expect("read");
        assert!(contents.contains("test"));
    }
}

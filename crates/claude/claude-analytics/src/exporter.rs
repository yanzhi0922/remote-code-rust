//! Event exporter trait and implementations.
//!
//! Provides the `EventExporter` trait and implementations for different
//! analytics backends: Datadog, first-party logging, and local JSONL file.

use anyhow::Result;
use std::io::Write;
use std::path::Path;
use std::sync::OnceLock;

use crate::metadata::AnalyticsEvent;

/// Shared HTTP client for analytics exporters.
static EXPORT_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn export_client() -> &'static reqwest::Client {
    EXPORT_CLIENT.get_or_init(reqwest::Client::new)
}

// ---------------------------------------------------------------------------
// EventExporter trait
// ---------------------------------------------------------------------------

/// Trait for exporting analytics events to a backend.
pub trait EventExporter: Send + Sync + std::fmt::Debug {
    /// Export a batch of events.
    fn export(&self, events: &[AnalyticsEvent]) -> Result<()>;
}

// ---------------------------------------------------------------------------
// DatadogExporter
// ---------------------------------------------------------------------------

/// Exports events formatted for Datadog.
///
/// Sends event batches to the Datadog Logs Intake API using the
/// `DD-API-KEY` header for authentication. Each event is serialized
/// as a JSON line with Datadog-specific metadata fields (`ddsource`,
/// `service`) attached.
#[derive(Debug, Clone)]
pub struct DatadogExporter {
    /// Datadog API key.
    pub api_key: String,
    /// Datadog intake endpoint.
    pub endpoint: String,
}

impl DatadogExporter {
    /// Create a new Datadog exporter with the given API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            endpoint: "https://http-intake.logs.datadoghq.com/v1/input".to_string(),
        }
    }

    /// Format an event as a Datadog log line.
    fn format_event(&self, event: &AnalyticsEvent) -> Result<String> {
        let mut payload = serde_json::to_value(event)
            .map_err(|e| anyhow::anyhow!("Failed to serialize event: {e}"))?;

        if let Some(obj) = payload.as_object_mut() {
            obj.insert("ddsource".to_string(), serde_json::json!("remote-code"));
            obj.insert("service".to_string(), serde_json::json!("rc-analytics"));
        }

        serde_json::to_string(&payload).map_err(|e| anyhow::anyhow!("Failed to format event: {e}"))
    }
}

impl EventExporter for DatadogExporter {
    fn export(&self, events: &[AnalyticsEvent]) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }
        let mut payload = String::with_capacity(events.len() * 512);
        for event in events {
            let formatted = self.format_event(event)?;
            payload.push_str(&formatted);
            payload.push('\n');
        }
        let do_export = || async {
            let client = export_client();
            let resp = client
                .post(&self.endpoint)
                .header("DD-API-KEY", &self.api_key)
                .header("Content-Type", "application/json")
                .body(payload)
                .send()
                .await;
            match resp {
                Ok(r) if r.status().is_success() => Ok(()),
                Ok(r) => {
                    tracing::warn!("Datadog export returned status {}", r.status());
                    Ok(())
                }
                Err(e) => {
                    tracing::warn!("Datadog export failed: {e}");
                    Ok(())
                }
            }
        };
        // Safe blocking within an existing tokio runtime, or create one if
        // called from a plain sync context.
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(do_export())),
            Err(_) => {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                rt.block_on(do_export())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// FirstPartyExporter
// ---------------------------------------------------------------------------

/// Exports events for first-party analytics logging.
///
/// Formats events as JSON lines suitable for internal analytics pipelines.
#[derive(Debug, Clone)]
pub struct FirstPartyExporter {
    /// Endpoint for first-party analytics.
    pub endpoint: String,
}

impl FirstPartyExporter {
    /// Create a new first-party exporter with the given endpoint.
    pub fn new(endpoint: impl Into<String>) -> Self {
        Self {
            endpoint: endpoint.into(),
        }
    }
}

impl EventExporter for FirstPartyExporter {
    fn export(&self, events: &[AnalyticsEvent]) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }
        let body = serde_json::to_string(events)?;
        let do_export = || async {
            let client = export_client();
            let resp = client
                .post(&self.endpoint)
                .header("Content-Type", "application/json")
                .body(body)
                .send()
                .await;
            match resp {
                Ok(r) if r.status().is_success() => Ok(()),
                Ok(r) => {
                    tracing::warn!("1P analytics export returned status {}", r.status());
                    Ok(())
                }
                Err(e) => {
                    tracing::warn!("1P analytics export failed: {e}");
                    Ok(())
                }
            }
        };
        // Safe blocking within an existing tokio runtime, or create one if
        // called from a plain sync context.
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => tokio::task::block_in_place(|| handle.block_on(do_export())),
            Err(_) => {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                rt.block_on(do_export())
            }
        }
    }
}

// ---------------------------------------------------------------------------
// FileExporter
// ---------------------------------------------------------------------------

/// Exports events to a local JSONL (JSON Lines) file.
///
/// Each event is written as a single JSON line, making it easy to
/// append and parse incrementally.
#[derive(Debug, Clone)]
pub struct FileExporter {
    /// Path to the output JSONL file.
    pub path: String,
}

impl FileExporter {
    /// Create a new file exporter writing to the given path.
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }
}

impl EventExporter for FileExporter {
    fn export(&self, events: &[AnalyticsEvent]) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let path = Path::new(&self.path);

        // Create parent directories if needed
        if let Some(parent) = path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)?;
        }

        // Open file in append mode
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;

        for event in events {
            let line = serde_json::to_string(event)?;
            writeln!(file, "{line}")?;
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::EventMetadata;
    use chrono::DateTime;

    fn test_event(name: &str) -> AnalyticsEvent {
        AnalyticsEvent {
            name: name.to_string(),
            metadata: EventMetadata::new().with_tool_name("test_tool"),
            timestamp: DateTime::UNIX_EPOCH,
        }
    }

    #[test]
    fn datadog_exporter_formats_event() {
        let exporter = DatadogExporter::new("test-key");
        let event = test_event("test_event");
        let formatted = exporter.format_event(&event).expect("format");
        let parsed: serde_json::Value = serde_json::from_str(&formatted).expect("parse");
        assert_eq!(parsed["ddsource"], "remote-code");
        // dd_api_key is intentionally excluded from the JSON body; it is sent
        // via the DD-API-KEY HTTP header instead.
        assert!(parsed.get("dd_api_key").is_none());
    }

    #[test]
    fn datadog_exporter_accepts_events() {
        let exporter = DatadogExporter::new("test-key");
        let events = vec![test_event("e1"), test_event("e2")];
        exporter.export(&events).expect("export should succeed");
    }

    #[test]
    fn first_party_exporter_accepts_events() {
        let exporter = FirstPartyExporter::new("https://analytics.example.com");
        let events = vec![test_event("e1")];
        exporter.export(&events).expect("export should succeed");
    }

    #[test]
    fn file_exporter_writes_jsonl() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("events.jsonl");
        let exporter = FileExporter::new(path.to_string_lossy().to_string());

        let events = vec![test_event("event1"), test_event("event2")];
        exporter.export(&events).expect("export");

        let contents = std::fs::read_to_string(&path).expect("read");
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 2);

        let first: serde_json::Value = serde_json::from_str(lines[0]).expect("parse");
        assert_eq!(first["name"], "event1");
    }

    #[test]
    fn file_exporter_appends() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("events.jsonl");
        let exporter = FileExporter::new(path.to_string_lossy().to_string());

        exporter.export(&[test_event("first")]).expect("export1");
        exporter.export(&[test_event("second")]).expect("export2");

        let contents = std::fs::read_to_string(&path).expect("read");
        let lines: Vec<&str> = contents.lines().collect();
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn file_exporter_empty_events_is_ok() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("events.jsonl");
        let exporter = FileExporter::new(path.to_string_lossy().to_string());
        exporter.export(&[]).expect("empty export");
        assert!(!path.exists());
    }
}

/// Retry queue for cloud requests that failed due to network issues.
/// Mirrors packages/cloud/src/retry-queue/RetryQueue.ts and types.ts

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Type of queued request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RequestType {
    ApiCall,
    Telemetry,
    Settings,
    Other,
}

impl Default for RequestType {
    fn default() -> Self {
        Self::Other
    }
}

/// A queued request awaiting retry.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueuedRequest {
    pub id: String,
    pub url: String,
    pub method: String,
    pub body: Option<String>,
    pub headers: Option<HashMap<String, String>>,
    pub timestamp: u64,
    pub retry_count: u32,
    #[serde(default)]
    pub request_type: RequestType,
    pub operation: Option<String>,
    pub last_error: Option<String>,
}

/// Statistics about the retry queue.
#[derive(Clone, Debug, Default)]
pub struct QueueStats {
    pub total_queued: usize,
    pub by_type: HashMap<String, usize>,
    pub oldest_request: Option<Instant>,
    pub newest_request: Option<Instant>,
    pub total_retries: u64,
    pub failed_retries: u64,
}

/// Configuration for the retry queue.
#[derive(Clone, Debug)]
pub struct RetryQueueConfig {
    /// Maximum number of retries per request (0 means unlimited).
    pub max_retries: u32,
    /// Delay between retry attempts in milliseconds.
    pub retry_delay_ms: u64,
    /// Maximum number of items in the queue (FIFO eviction when full).
    pub max_queue_size: usize,
    /// Whether to persist the queue across restarts.
    pub persist_queue: bool,
    /// Interval for checking network connectivity in milliseconds.
    pub network_check_interval_ms: u64,
    /// Request timeout in milliseconds.
    pub request_timeout_ms: u64,
}

impl Default for RetryQueueConfig {
    fn default() -> Self {
        Self {
            max_retries: 0,
            retry_delay_ms: 60_000,
            max_queue_size: 100,
            persist_queue: true,
            network_check_interval_ms: 60_000,
            request_timeout_ms: 30_000,
        }
    }
}

/// A retry queue for failed cloud requests.
pub struct RetryQueue {
    queue: HashMap<String, QueuedRequest>,
    /// Ordered keys for FIFO eviction.
    order: Vec<String>,
    config: RetryQueueConfig,
    is_processing: bool,
    is_paused: bool,
    paused_until: Option<Instant>,
    total_retries: u64,
    failed_retries: u64,
}

impl RetryQueue {
    /// Create a new retry queue with the given configuration.
    pub fn new(config: Option<RetryQueueConfig>) -> Self {
        let config = config.unwrap_or_default();
        Self {
            queue: HashMap::new(),
            order: Vec::new(),
            config,
            is_processing: false,
            is_paused: false,
            paused_until: None,
            total_retries: 0,
            failed_retries: 0,
        }
    }

    /// Add a request to the retry queue.
    pub fn enqueue(
        &mut self,
        url: String,
        method: String,
        body: Option<String>,
        headers: Option<HashMap<String, String>>,
        request_type: RequestType,
        operation: Option<String>,
    ) -> String {
        // FIFO eviction if at capacity
        if self.queue.len() >= self.config.max_queue_size {
            if let Some(oldest_id) = self.order.first().cloned() {
                self.queue.remove(&oldest_id);
                self.order.remove(0);
            }
        }

        let id = format!(
            "{}-{:x}-{:x}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis(),
            self.queue.len() as u32,
            SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().subsec_nanos()
        );

        let request = QueuedRequest {
            id: id.clone(),
            url,
            method,
            body,
            headers,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            retry_count: 0,
            request_type,
            operation,
            last_error: None,
        };

        self.order.push(id.clone());
        self.queue.insert(id.clone(), request);
        id
    }

    /// Get the next request to retry (oldest first).
    pub fn next(&mut self) -> Option<QueuedRequest> {
        if self.is_paused {
            return None;
        }

        if let Some(paused_until) = self.paused_until {
            if Instant::now() < paused_until {
                return None;
            }
            self.paused_until = None;
        }

        // Find the oldest request
        for id in &self.order {
            if let Some(request) = self.queue.get(id) {
                return Some(request.clone());
            }
        }
        None
    }

    /// Mark a request as successfully retried.
    pub fn mark_success(&mut self, id: &str) {
        self.queue.remove(id);
        self.order.retain(|k| k != id);
    }

    /// Mark a request as failed, incrementing retry count.
    /// Returns true if the request should be kept for another retry.
    pub fn mark_failure(&mut self, id: &str, error: String) -> bool {
        self.total_retries += 1;

        if let Some(request) = self.queue.get_mut(id) {
            request.retry_count += 1;
            request.last_error = Some(error.clone());

            // Check max retries
            if self.config.max_retries > 0 && request.retry_count >= self.config.max_retries {
                self.failed_retries += 1;
                self.queue.remove(id);
                self.order.retain(|k| k != id);
                return false;
            }
            true
        } else {
            false
        }
    }

    /// Pause the queue until a specific time (e.g., for rate limiting).
    pub fn pause_until(&mut self, duration: Duration) {
        self.paused_until = Some(Instant::now() + duration);
    }

    /// Manually pause/resume the queue.
    pub fn set_paused(&mut self, paused: bool) {
        self.is_paused = paused;
    }

    /// Check if the queue is paused.
    pub fn is_paused(&self) -> bool {
        self.is_paused || self.paused_until.map_or(false, |until| Instant::now() < until)
    }

    /// Set processing state.
    pub fn set_processing(&mut self, processing: bool) {
        self.is_processing = processing;
    }

    /// Check if currently processing.
    pub fn is_processing(&self) -> bool {
        self.is_processing
    }

    /// Get the number of queued requests.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// Check if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// Clear all requests from the queue.
    pub fn clear(&mut self) {
        self.queue.clear();
        self.order.clear();
    }

    /// Get statistics about the queue.
    pub fn stats(&self) -> QueueStats {
        let mut by_type: HashMap<String, usize> = HashMap::new();

        for request in self.queue.values() {
            let type_str = match &request.request_type {
                RequestType::ApiCall => "api-call",
                RequestType::Telemetry => "telemetry",
                RequestType::Settings => "settings",
                RequestType::Other => "other",
            };
            *by_type.entry(type_str.to_string()).or_insert(0) += 1;
        }

        QueueStats {
            total_queued: self.queue.len(),
            by_type,
            total_retries: self.total_retries,
            failed_retries: self.failed_retries,
            ..Default::default()
        }
    }

    /// Serialize the queue for persistence.
    pub fn serialize(&self) -> Result<String, serde_json::Error> {
        let requests: Vec<&QueuedRequest> = self.order.iter().filter_map(|id| self.queue.get(id)).collect();
        serde_json::to_string(&requests)
    }

    /// Deserialize and load a persisted queue.
    pub fn deserialize_and_load(&mut self, data: &str) -> Result<(), serde_json::Error> {
        let requests: Vec<QueuedRequest> = serde_json::from_str(data)?;
        for request in requests {
            let id = request.id.clone();
            self.order.push(id.clone());
            self.queue.insert(id, request);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enqueue_dequeue() {
        let mut queue = RetryQueue::new(None);
        assert!(queue.is_empty());

        let id = queue.enqueue(
            "https://example.com/api".to_string(),
            "POST".to_string(),
            Some(r#"{"key":"value"}"#.to_string()),
            None,
            RequestType::ApiCall,
            Some("test-op".to_string()),
        );

        assert_eq!(1, queue.len());
        assert!(!id.is_empty());
    }

    #[test]
    fn test_fifo_eviction() {
        let config = RetryQueueConfig {
            max_queue_size: 2,
            ..Default::default()
        };
        let mut queue = RetryQueue::new(Some(config));

        let id1 = queue.enqueue("url1".to_string(), "GET".to_string(), None, None, RequestType::Other, None);
        let _id2 = queue.enqueue("url2".to_string(), "GET".to_string(), None, None, RequestType::Other, None);
        let _id3 = queue.enqueue("url3".to_string(), "GET".to_string(), None, None, RequestType::Other, None);

        // Should have evicted the first one
        assert_eq!(2, queue.len());
        assert!(queue.queue.get(&id1).is_none());
    }

    #[test]
    fn test_mark_success() {
        let mut queue = RetryQueue::new(None);
        let id = queue.enqueue("url".to_string(), "GET".to_string(), None, None, RequestType::Other, None);

        queue.mark_success(&id);
        assert!(queue.is_empty());
    }

    #[test]
    fn test_mark_failure_within_retries() {
        let config = RetryQueueConfig {
            max_retries: 3,
            ..Default::default()
        };
        let mut queue = RetryQueue::new(Some(config));
        let id = queue.enqueue("url".to_string(), "GET".to_string(), None, None, RequestType::Other, None);

        let should_keep = queue.mark_failure(&id, "network error".to_string());
        assert!(should_keep);
        assert_eq!(1, queue.len());
    }

    #[test]
    fn test_mark_failure_exceeds_retries() {
        let config = RetryQueueConfig {
            max_retries: 2,
            ..Default::default()
        };
        let mut queue = RetryQueue::new(Some(config));
        let id = queue.enqueue("url".to_string(), "GET".to_string(), None, None, RequestType::Other, None);

        queue.mark_failure(&id.clone(), "error1".to_string());
        queue.mark_failure(&id, "error2".to_string());
        assert!(queue.is_empty());
        assert_eq!(1, queue.stats().failed_retries);
    }

    #[test]
    fn test_pause_resume() {
        let mut queue = RetryQueue::new(None);
        assert!(!queue.is_paused());

        queue.set_paused(true);
        assert!(queue.is_paused());

        queue.set_paused(false);
        assert!(!queue.is_paused());
    }

    #[test]
    fn test_clear() {
        let mut queue = RetryQueue::new(None);
        queue.enqueue("url1".to_string(), "GET".to_string(), None, None, RequestType::Other, None);
        queue.enqueue("url2".to_string(), "GET".to_string(), None, None, RequestType::Other, None);

        queue.clear();
        assert!(queue.is_empty());
    }

    #[test]
    fn test_stats() {
        let mut queue = RetryQueue::new(None);
        queue.enqueue("url1".to_string(), "GET".to_string(), None, None, RequestType::ApiCall, None);
        queue.enqueue("url2".to_string(), "GET".to_string(), None, None, RequestType::Telemetry, None);

        let stats = queue.stats();
        assert_eq!(2, stats.total_queued);
        assert_eq!(Some(&1), stats.by_type.get("api-call"));
        assert_eq!(Some(&1), stats.by_type.get("telemetry"));
    }

    #[test]
    fn test_serialize_deserialize() {
        let mut queue = RetryQueue::new(None);
        queue.enqueue("url1".to_string(), "POST".to_string(), Some("body".to_string()), None, RequestType::ApiCall, Some("op".to_string()));
        queue.enqueue("url2".to_string(), "GET".to_string(), None, None, RequestType::Telemetry, None);

        let serialized = queue.serialize().unwrap();

        let mut queue2 = RetryQueue::new(None);
        queue2.deserialize_and_load(&serialized).unwrap();
        assert_eq!(2, queue2.len());
    }

    #[test]
    fn test_unlimited_retries() {
        let config = RetryQueueConfig {
            max_retries: 0, // unlimited
            ..Default::default()
        };
        let mut queue = RetryQueue::new(Some(config));
        let id = queue.enqueue("url".to_string(), "GET".to_string(), None, None, RequestType::Other, None);

        for _ in 0..100 {
            let should_keep = queue.mark_failure(&id, "error".to_string());
            assert!(should_keep);
        }
        assert_eq!(1, queue.len());
    }
}

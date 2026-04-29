//! Async event bus for decoupled module communication.
//!
//! Provides a typed, async, broadcast-style event bus that allows modules
//! to communicate without direct dependencies on each other. This is the
//! backbone for the GUI/Remote-Control architecture where the core engine,
//! UI frontends, and remote controllers all need to exchange events.
//!
//! # Architecture
//!
//! ```text
//! ┌────────────┐  publish  ┌────────────┐  deliver  ┌────────────┐
//! │ Core Engine │ ────────→ │ Event Bus  │ ────────→ │ TUI/GUI/RC │
//! └────────────┘           └────────────┘           └────────────┘
//! ┌────────────┐  publish  ┌────────────┐  deliver  ┌────────────┐
//! │  Plugin A  │ ────────→ │ Event Bus  │ ────────→ │  Plugin B  │
//! └────────────┘           └────────────┘           └────────────┘
//! ```
//!
//! # Example
//!
//! ```rust,ignore
//! use rc_event_bus::{EventBus, EventTopic};
//!
//! let bus = EventBus::new(1024);
//!
//! // Subscribe to a topic
//! let mut rx = bus.subscribe(EventTopic::ToolResult);
//!
//! // Publish an event
//! bus.publish(EventTopic::ToolResult, "tool completed".to_owned());
//!
//! // Receive the event
//! let event = rx.recv().await.unwrap();
//! ```

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use tokio::sync::broadcast;
use tokio::sync::broadcast::{Receiver, Sender};

// ---------------------------------------------------------------------------
// Event topics
// ---------------------------------------------------------------------------

/// Well-known event topics for the remote-code-rust system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventTopic {
    /// Conversation events (user input, assistant response).
    Conversation,
    /// Tool execution events (start, progress, result).
    ToolExecution,
    /// Provider API events (request, response, error).
    Provider,
    /// Permission events (request, decision).
    Permission,
    /// Context management events (compaction, usage).
    Context,
    /// Cost tracking events.
    Cost,
    /// Session lifecycle events.
    Session,
    /// Agent dispatch events.
    Agent,
    /// MCP server events.
    Mcp,
    /// Plugin events.
    Plugin,
    /// Hook events.
    Hook,
    /// Streaming events (chunk, start, end).
    Streaming,
    /// Telemetry / metrics events.
    Telemetry,
    /// Custom topic for user-defined events.
    Custom(u32),
}

impl EventTopic {
    /// Get a human-readable name for the topic.
    #[must_use]
    pub fn name(self) -> String {
        match self {
            Self::Conversation => "conversation".to_owned(),
            Self::ToolExecution => "tool_execution".to_owned(),
            Self::Provider => "provider".to_owned(),
            Self::Permission => "permission".to_owned(),
            Self::Context => "context".to_owned(),
            Self::Cost => "cost".to_owned(),
            Self::Session => "session".to_owned(),
            Self::Agent => "agent".to_owned(),
            Self::Mcp => "mcp".to_owned(),
            Self::Plugin => "plugin".to_owned(),
            Self::Hook => "hook".to_owned(),
            Self::Streaming => "streaming".to_owned(),
            Self::Telemetry => "telemetry".to_owned(),
            Self::Custom(id) => format!("custom_{id}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Event wrapper
// ---------------------------------------------------------------------------

/// A wrapped event with metadata.
#[derive(Debug, Clone)]
pub struct BusEvent {
    /// The topic this event was published to.
    pub topic: EventTopic,
    /// The event payload as a JSON string.
    pub payload: String,
    /// Monotonic sequence number.
    pub sequence: u64,
}

// ---------------------------------------------------------------------------
// EventBus
// ---------------------------------------------------------------------------

/// A typed, async, broadcast-style event bus.
///
/// The bus uses tokio broadcast channels internally, allowing multiple
/// subscribers per topic. Late subscribers will miss events published
/// before they subscribed (configurable buffer size).
pub struct EventBus {
    channels: RwLock<HashMap<u64, Sender<BusEvent>>>,
    buffer_size: usize,
    sequence: Mutex<u64>,
}

impl EventBus {
    /// Create a new event bus with the given per-topic buffer size.
    #[must_use]
    pub fn new(buffer_size: usize) -> Arc<Self> {
        Arc::new(Self {
            channels: RwLock::new(HashMap::new()),
            buffer_size,
            sequence: Mutex::new(0),
        })
    }

    /// Publish an event to a topic.
    pub fn publish(&self, topic: EventTopic, payload: String) {
        let seq = {
            let mut seq = self.sequence.lock().unwrap_or_else(|e| e.into_inner());
            *seq += 1;
            *seq
        };

        let event = BusEvent {
            topic,
            payload,
            sequence: seq,
        };

        let key = topic_key(topic);
        let channels = self.channels.read().unwrap_or_else(|e| e.into_inner());
        if let Some(sender) = channels.get(&key) {
            // Send returns Err if no receivers, which is fine.
            let _ = sender.send(event);
        }
    }

    /// Subscribe to a topic. Returns a receiver for async event consumption.
    pub fn subscribe(&self, topic: EventTopic) -> Receiver<BusEvent> {
        let key = topic_key(topic);
        let mut channels = self.channels.write().unwrap_or_else(|e| e.into_inner());
        channels
            .entry(key)
            .or_insert_with(|| broadcast::channel(self.buffer_size).0)
            .subscribe()
    }

    /// Subscribe to all topics. This creates subscriptions for all known topics.
    pub fn subscribe_all(&self) -> Vec<Receiver<BusEvent>> {
        let topics = [
            EventTopic::Conversation,
            EventTopic::ToolExecution,
            EventTopic::Provider,
            EventTopic::Permission,
            EventTopic::Context,
            EventTopic::Cost,
            EventTopic::Session,
            EventTopic::Agent,
            EventTopic::Mcp,
            EventTopic::Plugin,
            EventTopic::Hook,
            EventTopic::Streaming,
            EventTopic::Telemetry,
        ];
        topics.iter().map(|t| self.subscribe(*t)).collect()
    }

    /// Get the number of active subscribers for a topic.
    pub fn subscriber_count(&self, topic: EventTopic) -> usize {
        let key = topic_key(topic);
        let channels = self.channels.read().unwrap_or_else(|e| e.into_inner());
        channels.get(&key).map_or(0, |s| s.receiver_count())
    }

    /// Get the total number of events published.
    #[must_use]
    pub fn total_published(&self) -> u64 {
        *self.sequence.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Convert an EventTopic to a hashable key.
fn topic_key(topic: EventTopic) -> u64 {
    match topic {
        EventTopic::Conversation => 0,
        EventTopic::ToolExecution => 1,
        EventTopic::Provider => 2,
        EventTopic::Permission => 3,
        EventTopic::Context => 4,
        EventTopic::Cost => 5,
        EventTopic::Session => 6,
        EventTopic::Agent => 7,
        EventTopic::Mcp => 8,
        EventTopic::Plugin => 9,
        EventTopic::Hook => 10,
        EventTopic::Streaming => 11,
        EventTopic::Telemetry => 12,
        EventTopic::Custom(id) => 1000 + id as u64,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{Duration, timeout};

    #[test]
    fn topic_names_are_human_readable() {
        assert_eq!(EventTopic::Conversation.name(), "conversation");
        assert_eq!(EventTopic::ToolExecution.name(), "tool_execution");
        assert_eq!(EventTopic::Custom(42).name(), "custom_42");
    }

    #[tokio::test]
    async fn publish_and_receive_event() {
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe(EventTopic::Conversation);

        bus.publish(EventTopic::Conversation, "hello".to_owned());

        let event = timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("timeout")
            .expect("recv failed");
        assert_eq!(event.payload, "hello");
        assert_eq!(event.topic, EventTopic::Conversation);
        assert_eq!(event.sequence, 1);
    }

    #[tokio::test]
    async fn multiple_subscribers_receive_events() {
        let bus = EventBus::new(16);
        let mut rx1 = bus.subscribe(EventTopic::Provider);
        let mut rx2 = bus.subscribe(EventTopic::Provider);

        bus.publish(EventTopic::Provider, "api_call".to_owned());

        let e1 = timeout(Duration::from_millis(100), rx1.recv())
            .await
            .expect("timeout")
            .expect("recv failed");
        let e2 = timeout(Duration::from_millis(100), rx2.recv())
            .await
            .expect("timeout")
            .expect("recv failed");

        assert_eq!(e1.payload, "api_call");
        assert_eq!(e2.payload, "api_call");
    }

    #[tokio::test]
    async fn events_on_different_topics_are_isolated() {
        let bus = EventBus::new(16);
        let mut rx_conv = bus.subscribe(EventTopic::Conversation);
        let mut rx_tool = bus.subscribe(EventTopic::ToolExecution);

        bus.publish(EventTopic::Conversation, "user msg".to_owned());
        bus.publish(EventTopic::ToolExecution, "tool ran".to_owned());

        let conv_event = timeout(Duration::from_millis(100), rx_conv.recv())
            .await
            .expect("timeout")
            .expect("recv failed");
        assert_eq!(conv_event.payload, "user msg");

        let tool_event = timeout(Duration::from_millis(100), rx_tool.recv())
            .await
            .expect("timeout")
            .expect("recv failed");
        assert_eq!(tool_event.payload, "tool ran");
    }

    #[tokio::test]
    async fn sequence_numbers_increase() {
        let bus = EventBus::new(16);
        let mut rx = bus.subscribe(EventTopic::Session);

        bus.publish(EventTopic::Session, "first".to_owned());
        bus.publish(EventTopic::Session, "second".to_owned());
        bus.publish(EventTopic::Session, "third".to_owned());

        let e1 = timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("timeout")
            .expect("recv failed");
        let e2 = timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("timeout")
            .expect("recv failed");
        let e3 = timeout(Duration::from_millis(100), rx.recv())
            .await
            .expect("timeout")
            .expect("recv failed");

        assert!(e1.sequence < e2.sequence);
        assert!(e2.sequence < e3.sequence);
        assert_eq!(bus.total_published(), 3);
    }

    #[tokio::test]
    async fn subscriber_count_tracks_receivers() {
        let bus = EventBus::new(16);
        assert_eq!(bus.subscriber_count(EventTopic::Agent), 0);

        let rx1 = bus.subscribe(EventTopic::Agent);
        assert_eq!(bus.subscriber_count(EventTopic::Agent), 1);

        let rx2 = bus.subscribe(EventTopic::Agent);
        assert_eq!(bus.subscriber_count(EventTopic::Agent), 2);

        drop(rx1);
        drop(rx2);
        // After dropping, count should decrease
        assert_eq!(bus.subscriber_count(EventTopic::Agent), 0);
    }

    #[tokio::test]
    async fn subscribe_all_creates_receivers_for_all_topics() {
        let bus = EventBus::new(16);
        let receivers = bus.subscribe_all();
        assert_eq!(receivers.len(), 13);
    }

    #[test]
    fn topic_key_mapping_is_unique() {
        let keys = [
            topic_key(EventTopic::Conversation),
            topic_key(EventTopic::ToolExecution),
            topic_key(EventTopic::Provider),
            topic_key(EventTopic::Permission),
            topic_key(EventTopic::Context),
            topic_key(EventTopic::Cost),
            topic_key(EventTopic::Session),
            topic_key(EventTopic::Agent),
            topic_key(EventTopic::Mcp),
            topic_key(EventTopic::Plugin),
            topic_key(EventTopic::Hook),
            topic_key(EventTopic::Streaming),
            topic_key(EventTopic::Telemetry),
            topic_key(EventTopic::Custom(1)),
            topic_key(EventTopic::Custom(2)),
        ];
        // All keys should be unique
        let unique: std::collections::HashSet<u64> = keys.iter().copied().collect();
        assert_eq!(unique.len(), keys.len());
    }

    #[tokio::test]
    async fn publish_without_subscribers_does_not_panic() {
        let bus = EventBus::new(16);
        bus.publish(EventTopic::Custom(99), "orphan event".to_owned());
        assert_eq!(bus.total_published(), 1);
    }
}

use serde::{Deserialize, Serialize};

/// A message in the queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueuedMessage {
    /// Unique identifier for the message.
    pub id: String,
    /// Unix timestamp (milliseconds) when the message was created.
    pub timestamp: u64,
    /// Text content of the message.
    pub text: String,
    /// Optional list of image URLs or base64-encoded images.
    pub images: Option<Vec<String>>,
}

/// The state of the message queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageQueueState {
    /// Messages currently in the queue.
    pub messages: Vec<QueuedMessage>,
    /// Whether the queue is currently processing a message.
    pub is_processing: bool,
    /// Whether the queue is paused.
    pub is_paused: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queued_message_serialization() {
        let msg = QueuedMessage {
            id: "test-id".to_string(),
            timestamp: 1234567890,
            text: "Hello".to_string(),
            images: Some(vec!["image1.png".to_string()]),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: QueuedMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "test-id");
        assert_eq!(deserialized.text, "Hello");
        assert_eq!(deserialized.images.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_queued_message_no_images() {
        let msg = QueuedMessage {
            id: "test-id".to_string(),
            timestamp: 1234567890,
            text: "Hello".to_string(),
            images: None,
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: QueuedMessage = serde_json::from_str(&json).unwrap();
        assert!(deserialized.images.is_none());
    }

    #[test]
    fn test_message_queue_state_serialization() {
        let state = MessageQueueState {
            messages: vec![],
            is_processing: false,
            is_paused: false,
        };
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: MessageQueueState = serde_json::from_str(&json).unwrap();
        assert!(deserialized.messages.is_empty());
        assert!(!deserialized.is_processing);
        assert!(!deserialized.is_paused);
    }

    #[test]
    fn test_message_queue_state_with_messages() {
        let msg = QueuedMessage {
            id: "msg-1".to_string(),
            timestamp: 100,
            text: "Test".to_string(),
            images: None,
        };
        let state = MessageQueueState {
            messages: vec![msg],
            is_processing: true,
            is_paused: false,
        };
        let json = serde_json::to_string(&state).unwrap();
        let deserialized: MessageQueueState = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.messages.len(), 1);
        assert!(deserialized.is_processing);
    }

    #[test]
    fn test_queued_message_clone() {
        let msg = QueuedMessage {
            id: "test-id".to_string(),
            timestamp: 1234567890,
            text: "Hello".to_string(),
            images: Some(vec!["img.png".to_string()]),
        };
        let cloned = msg.clone();
        assert_eq!(cloned.id, msg.id);
        assert_eq!(cloned.text, msg.text);
    }
}

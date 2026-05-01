use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::QueuedMessage;

/// A simple FIFO message queue service.
///
/// Manages a queue of messages that can be added, removed, updated,
/// and dequeued in first-in-first-out order.
#[derive(Debug, Clone)]
pub struct MessageQueueService {
    messages: Vec<QueuedMessage>,
}

impl MessageQueueService {
    /// Create a new empty message queue.
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
        }
    }

    /// Add a message to the queue.
    ///
    /// Returns `None` if both `text` is empty and `images` is `None` or empty.
    /// Otherwise returns the created `QueuedMessage`.
    pub fn add_message(&mut self, text: &str, images: Option<Vec<String>>) -> Option<QueuedMessage> {
        if text.is_empty() && images.as_ref().map_or(true, |imgs| imgs.is_empty()) {
            return None;
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let message = QueuedMessage {
            id: uuid::Uuid::new_v4().to_string(),
            timestamp,
            text: text.to_string(),
            images,
        };

        self.messages.push(message.clone());
        Some(message)
    }

    /// Remove a message by its ID.
    ///
    /// Returns `true` if the message was found and removed, `false` otherwise.
    pub fn remove_message(&mut self, id: &str) -> bool {
        let index = self.messages.iter().position(|msg| msg.id == id);
        match index {
            Some(i) => {
                self.messages.remove(i);
                true
            }
            None => false,
        }
    }

    /// Update a message's text and images by its ID.
    ///
    /// Returns `true` if the message was found and updated, `false` otherwise.
    pub fn update_message(&mut self, id: &str, text: &str, images: Option<Vec<String>>) -> bool {
        if let Some(msg) = self.messages.iter_mut().find(|m| m.id == id) {
            msg.timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;
            msg.text = text.to_string();
            msg.images = images;
            true
        } else {
            false
        }
    }

    /// Dequeue the first message from the queue (FIFO).
    ///
    /// Returns `Some(QueuedMessage)` if the queue is non-empty,
    /// or `None` if the queue is empty.
    pub fn dequeue_message(&mut self) -> Option<QueuedMessage> {
        self.messages.pop_front()
    }

    /// Get a reference to the messages in the queue.
    pub fn messages(&self) -> &[QueuedMessage] {
        &self.messages
    }

    /// Check if the queue is empty.
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Get the number of messages in the queue.
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Clear all messages from the queue.
    pub fn clear(&mut self) {
        self.messages.clear();
    }
}

/// Helper trait to pop from the front of a Vec (FIFO behavior).
trait PopFront<T> {
    fn pop_front(&mut self) -> Option<T>;
}

impl<T> PopFront<T> for Vec<T> {
    fn pop_front(&mut self) -> Option<T> {
        if self.is_empty() {
            None
        } else {
            Some(self.remove(0))
        }
    }
}

impl Default for MessageQueueService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_queue_is_empty() {
        let queue = MessageQueueService::new();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_default_queue_is_empty() {
        let queue = MessageQueueService::default();
        assert!(queue.is_empty());
    }

    #[test]
    fn test_add_message_with_text() {
        let mut queue = MessageQueueService::new();
        let msg = queue.add_message("Hello", None);
        assert!(msg.is_some());
        let msg = msg.unwrap();
        assert_eq!(msg.text, "Hello");
        assert!(msg.images.is_none());
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn test_add_message_with_images() {
        let mut queue = MessageQueueService::new();
        let msg = queue.add_message("", Some(vec!["image.png".to_string()]));
        assert!(msg.is_some());
        let msg = msg.unwrap();
        assert!(msg.text.is_empty());
        assert_eq!(msg.images.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_add_message_empty_returns_none() {
        let mut queue = MessageQueueService::new();
        let msg = queue.add_message("", None);
        assert!(msg.is_none());
        assert!(queue.is_empty());
    }

    #[test]
    fn test_add_message_empty_text_empty_images_returns_none() {
        let mut queue = MessageQueueService::new();
        let msg = queue.add_message("", Some(vec![]));
        assert!(msg.is_none());
    }

    #[test]
    fn test_add_message_generates_unique_id() {
        let mut queue = MessageQueueService::new();
        let msg1 = queue.add_message("Hello", None).unwrap();
        let msg2 = queue.add_message("World", None).unwrap();
        assert_ne!(msg1.id, msg2.id);
    }

    #[test]
    fn test_add_message_has_timestamp() {
        let mut queue = MessageQueueService::new();
        let msg = queue.add_message("Hello", None).unwrap();
        assert!(msg.timestamp > 0);
    }

    #[test]
    fn test_remove_message_existing() {
        let mut queue = MessageQueueService::new();
        let msg = queue.add_message("Hello", None).unwrap();
        assert!(queue.remove_message(&msg.id));
        assert!(queue.is_empty());
    }

    #[test]
    fn test_remove_message_nonexistent() {
        let mut queue = MessageQueueService::new();
        queue.add_message("Hello", None);
        assert!(!queue.remove_message("nonexistent-id"));
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn test_update_message_existing() {
        let mut queue = MessageQueueService::new();
        let msg = queue.add_message("Hello", None).unwrap();
        assert!(queue.update_message(&msg.id, "Updated", Some(vec!["img.png".to_string()])));
        let messages = queue.messages();
        assert_eq!(messages[0].text, "Updated");
        assert_eq!(messages[0].images.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn test_update_message_nonexistent() {
        let mut queue = MessageQueueService::new();
        assert!(!queue.update_message("nonexistent", "Updated", None));
    }

    #[test]
    fn test_dequeue_message_fifo() {
        let mut queue = MessageQueueService::new();
        queue.add_message("First", None);
        queue.add_message("Second", None);
        queue.add_message("Third", None);

        let first = queue.dequeue_message().unwrap();
        assert_eq!(first.text, "First");
        let second = queue.dequeue_message().unwrap();
        assert_eq!(second.text, "Second");
        let third = queue.dequeue_message().unwrap();
        assert_eq!(third.text, "Third");
        assert!(queue.dequeue_message().is_none());
    }

    #[test]
    fn test_dequeue_empty_queue() {
        let mut queue = MessageQueueService::new();
        assert!(queue.dequeue_message().is_none());
    }

    #[test]
    fn test_messages_returns_slice() {
        let mut queue = MessageQueueService::new();
        queue.add_message("Hello", None);
        queue.add_message("World", None);
        assert_eq!(queue.messages().len(), 2);
        assert_eq!(queue.messages()[0].text, "Hello");
        assert_eq!(queue.messages()[1].text, "World");
    }

    #[test]
    fn test_clear() {
        let mut queue = MessageQueueService::new();
        queue.add_message("Hello", None);
        queue.add_message("World", None);
        queue.clear();
        assert!(queue.is_empty());
        assert_eq!(queue.len(), 0);
    }

    #[test]
    fn test_len() {
        let mut queue = MessageQueueService::new();
        assert_eq!(queue.len(), 0);
        queue.add_message("Hello", None);
        assert_eq!(queue.len(), 1);
        queue.add_message("World", None);
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn test_add_and_remove_multiple() {
        let mut queue = MessageQueueService::new();
        let _msg1 = queue.add_message("First", None).unwrap();
        let msg2 = queue.add_message("Second", None).unwrap();
        let _msg3 = queue.add_message("Third", None).unwrap();

        queue.remove_message(&msg2.id);
        assert_eq!(queue.len(), 2);
        assert_eq!(queue.messages()[0].text, "First");
        assert_eq!(queue.messages()[1].text, "Third");
    }

    #[test]
    fn test_update_timestamp_changes() {
        let mut queue = MessageQueueService::new();
        let msg = queue.add_message("Hello", None).unwrap();
        let original_ts = msg.timestamp;

        // Small delay to ensure timestamp changes
        std::thread::sleep(std::time::Duration::from_millis(10));
        queue.update_message(&msg.id, "Updated", None);

        let updated_msg = &queue.messages()[0];
        assert!(updated_msg.timestamp >= original_ts);
    }
}

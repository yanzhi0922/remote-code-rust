//! Roo-message-queue: Simple FIFO message queue service for Roo Code.

pub mod queue;
pub mod types;

pub use queue::MessageQueueService;
pub use types::{MessageQueueState, QueuedMessage};

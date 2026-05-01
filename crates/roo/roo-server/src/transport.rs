//! Transport layer for JSON-RPC communication.
//!
//! Provides transport implementations for different communication channels:
//! - **Stdio**: Content-Length framed messages over stdin/stdout (LSP-style)
//! - **TCP**: Line-delimited JSON over TCP sockets
//!
//! Source: The TypeScript version uses VS Code's webview.postMessage() API.
//! In the Rust standalone version, we use standard IPC transports.

use std::io::{self, BufRead, BufReader, Read, Write};

use roo_jsonrpc::types::Message;
use crate::error::{ServerError, ServerResult};

// ---------------------------------------------------------------------------
// Transport Trait
// ---------------------------------------------------------------------------

/// A transport layer for sending and receiving JSON-RPC messages.
///
/// This trait abstracts the underlying communication channel, allowing
/// the server to work with different transport implementations.
#[allow(async_fn_in_trait)]
pub trait Transport {
    /// Receive the next JSON-RPC message.
    async fn receive(&mut self) -> ServerResult<Option<Message>>;

    /// Send a JSON-RPC message.
    async fn send(&mut self, message: &Message) -> ServerResult<()>;

    /// Close the transport.
    async fn close(&mut self) -> ServerResult<()> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Stdio Transport
// ---------------------------------------------------------------------------

/// A transport that uses stdin/stdout with Content-Length framing.
///
/// This is the standard transport for LSP-style communication, using
/// the same framing as the Language Server Protocol:
///
/// ```text
/// Content-Length: 123\r\n
/// \r\n
/// {"jsonrpc":"2.0","method":"initialize","id":1,"params":{...}}
/// ```
pub struct StdioTransport {
    reader: BufReader<io::Stdin>,
    writer: io::Stdout,
}

impl StdioTransport {
    /// Create a new stdio transport.
    pub fn new() -> Self {
        Self {
            reader: BufReader::new(io::stdin()),
            writer: io::stdout(),
        }
    }
}

impl Default for StdioTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl StdioTransport {
    /// Read a Content-Length framed message from stdin.
    fn read_framed_message(&mut self) -> ServerResult<Option<String>> {
        let mut content_length: usize = 0;
        let mut header_line = String::new();

        // Read headers until empty line
        loop {
            header_line.clear();
            match self.reader.read_line(&mut header_line) {
                Ok(0) => return Ok(None), // EOF
                Ok(_) => {
                    let line = header_line.trim();
                    if line.is_empty() {
                        break; // End of headers
                    }
                    if let Some(len) = line.strip_prefix("Content-Length:") {
                        content_length = len.trim().parse::<usize>().map_err(|e| {
                            ServerError::Internal(format!("Invalid Content-Length: {}", e))
                        })?;
                    }
                }
                Err(e) => {
                    return Err(ServerError::Io(e));
                }
            }
        }

        if content_length == 0 {
            return Err(ServerError::Internal("Missing Content-Length header".into()));
        }

        // Read the body
        let mut body = vec![0u8; content_length];
        self.reader.read_exact(&mut body).map_err(ServerError::Io)?;

        let body_str = String::from_utf8(body)
            .map_err(|e| ServerError::Internal(format!("Invalid UTF-8 body: {}", e)))?;

        Ok(Some(body_str))
    }

    /// Write a Content-Length framed message to stdout.
    fn write_framed_message(&mut self, body: &str) -> ServerResult<()> {
        let content_length = body.len();
        let header = format!("Content-Length: {}\r\n\r\n", content_length);
        self.writer
            .write_all(header.as_bytes())
            .map_err(ServerError::Io)?;
        self.writer
            .write_all(body.as_bytes())
            .map_err(ServerError::Io)?;
        self.writer.flush().map_err(ServerError::Io)?;
        Ok(())
    }
}

impl Transport for StdioTransport {
    async fn receive(&mut self) -> ServerResult<Option<Message>> {
        // Content-Length framing read is synchronous
        match self.read_framed_message() {
            Ok(Some(body)) => {
                let message: Message = serde_json::from_str(&body)?;
                Ok(Some(message))
            }
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }

    async fn send(&mut self, message: &Message) -> ServerResult<()> {
        let body = serde_json::to_string(message)?;
        self.write_framed_message(&body)
    }
}

// ---------------------------------------------------------------------------
// Memory Transport (for testing)
// ---------------------------------------------------------------------------

/// An in-memory transport for testing purposes.
///
/// Uses channels to simulate bidirectional communication.
pub struct MemoryTransport {
    inbox: tokio::sync::mpsc::Receiver<Message>,
    outbox: tokio::sync::mpsc::Sender<Message>,
}

impl MemoryTransport {
    /// Create a new memory transport with the given channel capacity.
    pub fn new(capacity: usize) -> (MemoryTransportClient, Self) {
        let (client_to_server, server_inbox) = tokio::sync::mpsc::channel(capacity);
        let (server_to_client, server_outbox) = tokio::sync::mpsc::channel(capacity);

        let client = MemoryTransportClient {
            inbox: server_outbox,
            outbox: client_to_server.clone(),
        };

        let server = Self {
            inbox: server_inbox,
            outbox: server_to_client,
        };

        (client, server)
    }
}

impl Transport for MemoryTransport {
    async fn receive(&mut self) -> ServerResult<Option<Message>> {
        self.inbox.recv().await.map(Some).ok_or_else(|| {
            ServerError::Internal("Memory transport channel closed".into())
        })
    }

    async fn send(&mut self, message: &Message) -> ServerResult<()> {
        self.outbox.send(message.clone()).await.map_err(|e| {
            ServerError::Internal(format!("Failed to send message: {}", e))
        })
    }
}

/// The client side of a memory transport.
pub struct MemoryTransportClient {
    inbox: tokio::sync::mpsc::Receiver<Message>,
    outbox: tokio::sync::mpsc::Sender<Message>,
}

impl MemoryTransportClient {
    /// Send a message to the server.
    pub async fn send(&self, message: Message) -> ServerResult<()> {
        self.outbox.send(message).await.map_err(|e| {
            ServerError::Internal(format!("Failed to send message: {}", e))
        })
    }

    /// Receive a message from the server.
    pub async fn receive(&mut self) -> ServerResult<Option<Message>> {
        self.inbox.recv().await.map(Some).ok_or_else(|| {
            ServerError::Internal("Memory transport channel closed".into())
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use roo_jsonrpc::types::Message;
    use serde_json::json;

    #[tokio::test]
    async fn test_memory_transport_roundtrip() {
        let (client, mut server) = MemoryTransport::new(10);

        // Client sends a request
        let request = Message::request(1, "ping", json!(null));
        client.send(request.clone()).await.unwrap();

        // Server receives it
        let received = server.receive().await.unwrap().unwrap();
        assert_eq!(received.method, request.method);
        assert_eq!(received.id, request.id);

        // Server sends a response
        let response = Message::response(
            serde_json::Value::Number(1.into()),
            json!("pong"),
        );
        server.send(&response).await.unwrap();

        // Client receives it
        let mut client = client;
        let received = client.receive().await.unwrap().unwrap();
        assert_eq!(received.result, Some(json!("pong")));
    }

    #[tokio::test]
    async fn test_memory_transport_multiple_messages() {
        let (client, mut server) = MemoryTransport::new(10);

        // Send multiple messages
        for i in 0..5 {
            let msg = Message::request(i, "ping", json!(null));
            client.send(msg).await.unwrap();
        }

        // Receive all messages
        for i in 0..5 {
            let msg = server.receive().await.unwrap().unwrap();
            assert_eq!(msg.id_as_u64(), Some(i));
        }
    }

    #[test]
    fn test_stdio_transport_creation() {
        let _transport = StdioTransport::new();
        // Can't test actual read/write without stdin/stdout mocking
    }
}

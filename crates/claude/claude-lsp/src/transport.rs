//! LSP stdio transport — spawns a language server and communicates over JSON-RPC.
//!
//! Implements the LSP base protocol: messages are framed with a
//! `Content-Length` header followed by a JSON-RPC body, sent over the
//! language server's stdin/stdout pipes.

use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result, anyhow, bail};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::oneshot;

/// A running language server process with stdio transport.
pub struct StdioTransport {
    /// Child process handle.
    child: Child,
    /// Writer to the server's stdin.
    stdin: Arc<Mutex<Option<tokio::process::ChildStdin>>>,
    /// Next request ID (monotonically increasing).
    next_id: AtomicU64,
    /// Pending response receivers keyed by request ID.
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Value>>>>,
    /// Reader task JoinHandle.
    reader_handle: Option<tokio::task::JoinHandle<()>>,
}

impl StdioTransport {
    /// Spawn a language server process and start the response reader loop.
    pub async fn spawn(command: &str, args: &[&str]) -> Result<Arc<Self>> {
        let mut child = Command::new(command)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .with_context(|| format!("failed to spawn language server: {command}"))?;

        let stdin = child
            .stdin
            .take()
            .context("language server stdin not available")?;
        let stdout = child
            .stdout
            .take()
            .context("language server stdout not available")?;

        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Value>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let pending_clone = Arc::clone(&pending);

        let reader_handle = tokio::spawn(async move {
            Self::read_loop(stdout, pending_clone).await;
        });

        Ok(Arc::new(Self {
            child,
            stdin: Arc::new(Mutex::new(Some(stdin))),
            next_id: AtomicU64::new(1),
            pending,
            reader_handle: Some(reader_handle),
        }))
    }

    /// Allocate a new request ID.
    pub fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed)
    }

    /// Send a JSON-RPC request and wait for the response.
    pub async fn request(&self, method: &str, params: Option<Value>) -> Result<Value> {
        let id = self.next_id();
        let (tx, rx) = oneshot::channel();
        self.pending.lock().insert(id, tx);

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params.unwrap_or(Value::Null),
        });
        self.write_message(&msg).await?;

        let response = rx
            .await
            .map_err(|_| anyhow!("response channel closed for request {id}"))?;

        // Check for JSON-RPC error
        if let Some(error) = response.get("error") {
            let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
            let message = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("unknown error");
            bail!("LSP error {code}: {message}");
        }

        Ok(response.get("result").cloned().unwrap_or(Value::Null))
    }

    /// Send a JSON-RPC notification (no response expected).
    pub async fn notify(&self, method: &str, params: Option<Value>) -> Result<()> {
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params.unwrap_or(Value::Null),
        });
        self.write_message(&msg).await
    }

    /// Write a JSON-RPC message to the server's stdin with Content-Length framing.
    async fn write_message(&self, msg: &Value) -> Result<()> {
        let body = serde_json::to_string(msg)?;
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        let mut stdin = self
            .stdin
            .lock()
            .take()
            .ok_or_else(|| anyhow!("stdin already taken"))?;

        stdin
            .write_all(header.as_bytes())
            .await
            .context("write header to language server")?;
        stdin
            .write_all(body.as_bytes())
            .await
            .context("write body to language server")?;
        stdin.flush().await?;

        // Put stdin back
        *self.stdin.lock() = Some(stdin);
        Ok(())
    }

    /// Background reader loop: reads messages from stdout and routes responses.
    async fn read_loop(
        stdout: tokio::process::ChildStdout,
        pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Value>>>>,
    ) {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();

        loop {
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break, // EOF
                Ok(_) => {}
                Err(_) => break,
            };

            let content_length = match Self::parse_content_length(&line) {
                Some(len) => len,
                None => continue,
            };

            // Read the blank line separator
            line.clear();
            match reader.read_line(&mut line).await {
                Ok(0) => break,
                _ => {}
            };

            // Read the JSON body
            let mut buf = vec![0u8; content_length];
            match reader.read_exact(&mut buf).await {
                Ok(_) => {}
                Err(_) => break,
            };

            let value: Value = match serde_json::from_slice(&buf) {
                Ok(v) => v,
                Err(_) => continue,
            };

            // Route response to waiting request
            if let Some(id) = value.get("id").and_then(|v| v.as_u64()) {
                if let Some(tx) = pending.lock().remove(&id) {
                    let _ = tx.send(value);
                }
            }
        }
    }

    /// Parse `Content-Length: N` from a header line.
    fn parse_content_length(line: &str) -> Option<usize> {
        let line = line.trim();
        line.strip_prefix("Content-Length:")
            .or_else(|| line.strip_prefix("Content-Length :"))
            .and_then(|rest| rest.trim().parse::<usize>().ok())
    }

    /// Check if the language server process is still running.
    pub fn is_running(&mut self) -> bool {
        self.child.try_wait().ok().flatten().is_none()
    }

    /// Shut down the transport, killing the child process.
    pub async fn shutdown(&mut self) -> Result<()> {
        // Cancel the reader
        if let Some(handle) = self.reader_handle.take() {
            handle.abort();
        }
        // Kill the child process
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
        Ok(())
    }
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_content_length_valid() {
        assert_eq!(
            StdioTransport::parse_content_length("Content-Length: 42\r\n"),
            Some(42)
        );
        assert_eq!(
            StdioTransport::parse_content_length("Content-Length: 0\r\n"),
            Some(0)
        );
        assert_eq!(
            StdioTransport::parse_content_length("Content-Length : 100\r\n"),
            Some(100)
        );
    }

    #[test]
    fn parse_content_length_invalid() {
        assert_eq!(
            StdioTransport::parse_content_length("X-Header: foo\r\n"),
            None
        );
        assert_eq!(StdioTransport::parse_content_length("garbage"), None);
    }
}

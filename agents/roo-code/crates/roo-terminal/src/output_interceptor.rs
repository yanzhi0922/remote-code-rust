/// Output interceptor for terminal processes.
/// Mirrors src/integrations/terminal/OutputInterceptor.ts

use std::sync::Arc;
use tokio::sync::RwLock;

/// Intercepts and processes terminal output.
pub struct OutputInterceptor {
    buffer: Arc<RwLock<String>>,
    max_buffer_size: usize,
    line_filter: Option<fn(&str) -> bool>,
}

impl OutputInterceptor {
    /// Create a new OutputInterceptor.
    pub fn new(max_buffer_size: usize) -> Self {
        Self {
            buffer: Arc::new(RwLock::new(String::new())),
            max_buffer_size,
            line_filter: None,
        }
    }

    /// Create with a line filter function.
    pub fn with_filter(max_buffer_size: usize, filter: fn(&str) -> bool) -> Self {
        Self {
            buffer: Arc::new(RwLock::new(String::new())),
            max_buffer_size,
            line_filter: Some(filter),
        }
    }

    /// Process incoming output data.
    pub async fn intercept(&self, data: &str) {
        let mut buffer = self.buffer.write().await;

        for line in data.lines() {
            // Apply filter if set
            if let Some(filter) = self.line_filter {
                if !filter(line) {
                    continue;
                }
            }

            // Process carriage returns (overwrite current line)
            if line.contains('\r') {
                if let Some(pos) = line.rfind('\r') {
                    let after_cr = &line[pos + 1..];
                    // Replace last line in buffer
                    if let Some(last_newline) = buffer.rfind('\n') {
                        buffer.truncate(last_newline + 1);
                        buffer.push_str(after_cr);
                    } else {
                        buffer.clear();
                        buffer.push_str(after_cr);
                    }
                    continue;
                }
            }

            if !buffer.is_empty() && !buffer.ends_with('\n') {
                buffer.push('\n');
            }
            buffer.push_str(line);
        }

        // Trim buffer if it exceeds max size
        if buffer.len() > self.max_buffer_size {
            let excess = buffer.len() - self.max_buffer_size;
            let drain_to = buffer.char_indices().nth(excess).map(|(i, _)| i).unwrap_or(0);
            buffer.drain(..drain_to);
        }
    }

    /// Get the current buffered output.
    pub async fn get_output(&self) -> String {
        self.buffer.read().await.clone()
    }

    /// Clear the buffer.
    pub async fn clear(&self) {
        let mut buffer = self.buffer.write().await;
        buffer.clear();
    }

    /// Get the last N lines of output.
    pub async fn get_last_lines(&self, n: usize) -> Vec<String> {
        let buffer = self.buffer.read().await;
        buffer
            .lines()
            .rev()
            .take(n)
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_intercept_simple() {
        let interceptor = OutputInterceptor::new(1024);
        interceptor.intercept("hello\nworld\n").await;
        let output = interceptor.get_output().await;
        assert!(output.contains("hello"));
        assert!(output.contains("world"));
    }

    #[tokio::test]
    async fn test_intercept_carriage_return() {
        let interceptor = OutputInterceptor::new(1024);
        interceptor.intercept("loading 10%\rloading 50%\rloading 100%\n").await;
        let output = interceptor.get_output().await;
        assert!(output.contains("loading 100%"));
        assert!(!output.contains("loading 10%"));
    }

    #[tokio::test]
    async fn test_clear() {
        let interceptor = OutputInterceptor::new(1024);
        interceptor.intercept("some output\n").await;
        interceptor.clear().await;
        assert!(interceptor.get_output().await.is_empty());
    }

    #[tokio::test]
    async fn test_max_buffer_size() {
        let interceptor = OutputInterceptor::new(20);
        interceptor.intercept("this is a very long line that exceeds the buffer size\n").await;
        let output = interceptor.get_output().await;
        assert!(output.len() <= 20);
    }

    #[tokio::test]
    async fn test_with_filter() {
        let interceptor = OutputInterceptor::with_filter(1024, |line| {
            !line.contains("debug:")
        });
        interceptor.intercept("debug: something\nimportant line\n").await;
        let output = interceptor.get_output().await;
        assert!(!output.contains("debug:"));
        assert!(output.contains("important"));
    }

    #[tokio::test]
    async fn test_get_last_lines() {
        let interceptor = OutputInterceptor::new(1024);
        interceptor.intercept("line1\nline2\nline3\nline4\n").await;
        let last = interceptor.get_last_lines(2).await;
        assert_eq!(vec!["line3", "line4"], last);
    }
}

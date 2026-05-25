//! FakeAI provider handler.
//!
//! A test double that can return fixed responses, simulate streaming,
//! inject errors, and follow scripted response patterns.  It does **not**
//! wrap another provider — instead it produces chunks directly from its
//! configuration, mirroring the TS `FakeAI` interface where the caller
//! supplies the response logic up-front.

use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use futures::StreamExt;
use futures::stream;

use roo_provider::error::{ProviderError, Result};
use roo_provider::{ApiStream, CreateMessageMetadata, Provider};
use roo_types::api::{ApiStreamChunk, ProviderName};
use roo_types::model::ModelInfo;

use crate::types::{FakeAiConfig, ResponsePattern};

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

/// FakeAI provider — a test double for the `Provider` trait.
///
/// Create one with [`FakeAiHandler::new`] (or [`FakeAiHandler::from_config`])
/// and use it anywhere a `dyn Provider` is expected.
pub struct FakeAiHandler {
    config: FakeAiConfig,
    model_info: ModelInfo,
    /// Tracks the position inside a `ResponsePattern::Sequence`.
    sequence_index: AtomicUsize,
}

impl FakeAiHandler {
    /// Build a handler from a [`FakeAiConfig`].
    pub fn new(config: FakeAiConfig) -> Self {
        let model_info = ModelInfo {
            max_tokens: Some(4096),
            context_window: 128_000,
            supports_images: Some(false),
            description: Some("FakeAI test double".to_string()),
            ..ModelInfo::default()
        };
        Self {
            config,
            model_info,
            sequence_index: AtomicUsize::new(0),
        }
    }

    /// Convenience: build from individual fields with defaults for the rest.
    pub fn from_text_response(id: &str, text: &str) -> Self {
        Self::new(FakeAiConfig {
            id: id.to_string(),
            response_pattern: ResponsePattern::Text {
                text: text.to_string(),
            },
            ..FakeAiConfig::default()
        })
    }

    // -----------------------------------------------------------------------
    // Internals
    // -----------------------------------------------------------------------

    /// Optionally sleep when `delay_ms` is configured.
    async fn maybe_delay(&self) {
        if self.config.delay_ms > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(self.config.delay_ms)).await;
        }
    }

    /// Resolve the current response pattern, advancing the sequence index
    /// when configured with a `Sequence`.
    fn current_pattern(&self) -> ResponsePattern {
        match &self.config.response_pattern {
            ResponsePattern::Sequence { patterns } => {
                let idx = self.sequence_index.fetch_add(1, Ordering::Relaxed);
                patterns
                    .get(idx % patterns.len())
                    .cloned()
                    .unwrap_or_default()
            }
            other => other.clone(),
        }
    }

    /// Turn a [`ResponsePattern`] into a `Vec<ApiStreamChunk>`.
    fn pattern_to_chunks(pattern: &ResponsePattern) -> Vec<ApiStreamChunk> {
        match pattern {
            ResponsePattern::Text { text } => {
                vec![
                    ApiStreamChunk::Text { text: text.clone() },
                    ApiStreamChunk::Usage {
                        input_tokens: 10,
                        output_tokens: text.len() as u64 / 4,
                        cache_write_tokens: None,
                        cache_read_tokens: None,
                        reasoning_tokens: None,
                        total_cost: None,
                    },
                ]
            }
            ResponsePattern::Streaming {
                text,
                chunk_delay_ms: _,
                chunk_size,
            } => {
                let mut chunks: Vec<ApiStreamChunk> = Vec::new();
                for chunk in text.as_bytes().chunks(*chunk_size) {
                    // SAFETY: we split UTF-8 text by byte boundary at
                    // char boundaries may be broken; for a test double this
                    // is acceptable — the text is typically ASCII.
                    let s = String::from_utf8_lossy(chunk).to_string();
                    chunks.push(ApiStreamChunk::Text { text: s });
                }
                chunks.push(ApiStreamChunk::Usage {
                    input_tokens: 10,
                    output_tokens: text.len() as u64 / 4,
                    cache_write_tokens: None,
                    cache_read_tokens: None,
                    reasoning_tokens: None,
                    total_cost: None,
                });
                chunks
            }
            ResponsePattern::ToolCall {
                id,
                name,
                arguments,
            } => {
                vec![
                    ApiStreamChunk::ToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        arguments: arguments.clone(),
                    },
                    ApiStreamChunk::Usage {
                        input_tokens: 10,
                        output_tokens: 50,
                        cache_write_tokens: None,
                        cache_read_tokens: None,
                        reasoning_tokens: None,
                        total_cost: None,
                    },
                ]
            }
            ResponsePattern::Error { message } => {
                vec![ApiStreamChunk::Error {
                    error: "fake_error".to_string(),
                    message: message.clone(),
                }]
            }
            ResponsePattern::Sequence { .. } => {
                // Should not happen — `current_pattern` resolves sequences.
                vec![ApiStreamChunk::Text {
                    text: "<unresolved-sequence>".to_string(),
                }]
            }
        }
    }

    /// Build the `ApiStream` for a given pattern, injecting per-chunk delays
    /// when the pattern is `Streaming`.
    fn build_stream(&self, pattern: ResponsePattern) -> ApiStream {
        let is_streaming = matches!(pattern, ResponsePattern::Streaming { .. });
        let delay_ms = match &pattern {
            ResponsePattern::Streaming { chunk_delay_ms, .. } => *chunk_delay_ms,
            _ => 0,
        };
        let chunks = Self::pattern_to_chunks(&pattern);

        let stream = stream::iter(chunks.into_iter().map(Ok::<_, ProviderError>));

        if delay_ms > 0 && is_streaming {
            let delayed = stream.then(move |item| async move {
                tokio::time::sleep(tokio::time::Duration::from_millis(delay_ms)).await;
                item
            });
            Box::pin(delayed)
        } else {
            Box::pin(stream)
        }
    }
}

// ---------------------------------------------------------------------------
// Provider trait implementation
// ---------------------------------------------------------------------------

#[async_trait]
impl Provider for FakeAiHandler {
    async fn create_message(
        &self,
        _system_prompt: &str,
        _messages: &[roo_types::api::ApiMessage],
        _tools: Option<&[serde_json::Value]>,
        _metadata: CreateMessageMetadata,
    ) -> Result<ApiStream> {
        self.maybe_delay().await;
        let pattern = self.current_pattern();
        Ok(self.build_stream(pattern))
    }

    fn get_model(&self) -> (String, ModelInfo) {
        (self.config.model_id.clone(), self.model_info.clone())
    }

    async fn count_tokens(&self, _content: &[roo_types::api::ContentBlock]) -> Result<u64> {
        self.maybe_delay().await;
        Ok(self.config.fake_token_count)
    }

    async fn complete_prompt(&self, _prompt: &str) -> Result<String> {
        self.maybe_delay().await;
        Ok(self.config.complete_response.clone())
    }

    fn provider_name(&self) -> ProviderName {
        ProviderName::FakeAi
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    fn text_config(text: &str) -> FakeAiConfig {
        FakeAiConfig {
            id: "test".to_string(),
            response_pattern: ResponsePattern::Text {
                text: text.to_string(),
            },
            ..FakeAiConfig::default()
        }
    }

    #[tokio::test]
    async fn text_response_yields_text_and_usage_chunks() {
        let handler = FakeAiHandler::new(text_config("hello world"));
        let stream = handler
            .create_message("sys", &[], None, CreateMessageMetadata::default())
            .await
            .expect("create_message should succeed");

        let chunks: Vec<_> = stream.collect::<Vec<_>>().await;
        assert_eq!(chunks.len(), 2);

        match &chunks[0] {
            Ok(ApiStreamChunk::Text { text }) => assert_eq!(text, "hello world"),
            other => panic!("expected Text chunk, got {other:?}"),
        }

        match &chunks[1] {
            Ok(ApiStreamChunk::Usage { .. }) => {}
            other => panic!("expected Usage chunk, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn error_response_yields_error_chunk() {
        let config = FakeAiConfig {
            id: "err-test".to_string(),
            response_pattern: ResponsePattern::Error {
                message: "boom".to_string(),
            },
            ..FakeAiConfig::default()
        };
        let handler = FakeAiHandler::new(config);
        let stream = handler
            .create_message("sys", &[], None, CreateMessageMetadata::default())
            .await
            .expect("create_message should succeed");

        let chunks: Vec<_> = stream.collect::<Vec<_>>().await;
        assert_eq!(chunks.len(), 1);
        match &chunks[0] {
            Ok(ApiStreamChunk::Error { message, .. }) => assert_eq!(message, "boom"),
            other => panic!("expected Error chunk, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn tool_call_response() {
        let config = FakeAiConfig {
            id: "tool-test".to_string(),
            response_pattern: ResponsePattern::ToolCall {
                id: "call_1".to_string(),
                name: "read_file".to_string(),
                arguments: r#"{"path":"/tmp/x"}"#.to_string(),
            },
            ..FakeAiConfig::default()
        };
        let handler = FakeAiHandler::new(config);
        let stream = handler
            .create_message("sys", &[], None, CreateMessageMetadata::default())
            .await
            .expect("create_message should succeed");

        let chunks: Vec<_> = stream.collect::<Vec<_>>().await;
        assert_eq!(chunks.len(), 2);
        match &chunks[0] {
            Ok(ApiStreamChunk::ToolCall {
                id,
                name,
                arguments,
            }) => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "read_file");
                assert_eq!(arguments, r#"{"path":"/tmp/x"}"#);
            }
            other => panic!("expected ToolCall chunk, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn sequence_cycles_through_patterns() {
        let config = FakeAiConfig {
            id: "seq-test".to_string(),
            response_pattern: ResponsePattern::Sequence {
                patterns: vec![
                    ResponsePattern::Text {
                        text: "first".to_string(),
                    },
                    ResponsePattern::Text {
                        text: "second".to_string(),
                    },
                ],
            },
            ..FakeAiConfig::default()
        };
        let handler = FakeAiHandler::new(config);

        // First call — "first"
        let stream = handler
            .create_message("sys", &[], None, CreateMessageMetadata::default())
            .await
            .unwrap();
        let chunks: Vec<_> = stream.collect::<Vec<_>>().await;
        match &chunks[0] {
            Ok(ApiStreamChunk::Text { text }) => assert_eq!(text, "first"),
            other => panic!("expected 'first', got {other:?}"),
        }

        // Second call — "second"
        let stream = handler
            .create_message("sys", &[], None, CreateMessageMetadata::default())
            .await
            .unwrap();
        let chunks: Vec<_> = stream.collect::<Vec<_>>().await;
        match &chunks[0] {
            Ok(ApiStreamChunk::Text { text }) => assert_eq!(text, "second"),
            other => panic!("expected 'second', got {other:?}"),
        }

        // Third call — wraps back to "first"
        let stream = handler
            .create_message("sys", &[], None, CreateMessageMetadata::default())
            .await
            .unwrap();
        let chunks: Vec<_> = stream.collect::<Vec<_>>().await;
        match &chunks[0] {
            Ok(ApiStreamChunk::Text { text }) => assert_eq!(text, "first"),
            other => panic!("expected 'first' (wrap), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn get_model_returns_configured_values() {
        let handler = FakeAiHandler::new(text_config("hi"));
        let (model_id, info) = handler.get_model();
        assert_eq!(model_id, "fake-ai-model");
        assert_eq!(info.context_window, 128_000);
        assert_eq!(handler.provider_name(), ProviderName::FakeAi);
    }

    #[tokio::test]
    async fn count_tokens_returns_configured_value() {
        let handler = FakeAiHandler::new(text_config("hi"));
        let count = handler
            .count_tokens(&[])
            .await
            .expect("count_tokens should succeed");
        assert_eq!(count, 42);
    }

    #[tokio::test]
    async fn complete_prompt_returns_configured_response() {
        let handler = FakeAiHandler::new(text_config("hi"));
        let result = handler
            .complete_prompt("anything")
            .await
            .expect("complete_prompt should succeed");
        assert_eq!(result, "fake completion");
    }

    #[tokio::test]
    async fn streaming_response_splits_into_chunks() {
        let config = FakeAiConfig {
            id: "stream-test".to_string(),
            response_pattern: ResponsePattern::Streaming {
                text: "abcdef".to_string(),
                chunk_delay_ms: 0,
                chunk_size: 2,
            },
            ..FakeAiConfig::default()
        };
        let handler = FakeAiHandler::new(config);
        let stream = handler
            .create_message("sys", &[], None, CreateMessageMetadata::default())
            .await
            .unwrap();

        let chunks: Vec<_> = stream.collect::<Vec<_>>().await;
        // 3 text chunks ("ab","cd","ef") + 1 usage = 4
        assert_eq!(chunks.len(), 4);
        // First text chunk
        match &chunks[0] {
            Ok(ApiStreamChunk::Text { text }) => assert_eq!(text, "ab"),
            other => panic!("expected 'ab', got {other:?}"),
        }
    }

    #[tokio::test]
    async fn delay_injects_latency() {
        let config = FakeAiConfig {
            id: "delay-test".to_string(),
            delay_ms: 50,
            ..text_config("hi")
        };
        let handler = FakeAiHandler::new(config);

        let start = std::time::Instant::now();
        handler
            .complete_prompt("x")
            .await
            .expect("complete_prompt should succeed");
        let elapsed = start.elapsed();
        assert!(
            elapsed >= std::time::Duration::from_millis(50),
            "expected >= 50ms delay, got {elapsed:?}"
        );
    }
}

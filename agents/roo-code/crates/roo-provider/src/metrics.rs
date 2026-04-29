//! API request metrics aggregation.
//!
//! Provides utilities for tracking and combining API usage metrics
//! across multiple streaming responses and requests.

use roo_types::api::ApiStreamChunk;
use roo_types::model::ModelInfo;

use crate::cost::calculate_api_cost;

// ---------------------------------------------------------------------------
// ApiMetrics
// ---------------------------------------------------------------------------

/// Aggregated API request metrics.
///
/// Tracks cumulative token usage, cost, and request count across
/// one or more API calls.
#[derive(Debug, Clone, PartialEq)]
pub struct ApiMetrics {
    /// Total input tokens across all requests.
    pub total_input_tokens: u64,
    /// Total output tokens across all requests.
    pub total_output_tokens: u64,
    /// Total cache creation tokens across all requests.
    pub total_cache_creation_tokens: u64,
    /// Total cache read tokens across all requests.
    pub total_cache_read_tokens: u64,
    /// Total cost in USD across all requests.
    pub total_cost: f64,
    /// Number of API requests tracked.
    pub request_count: u64,
}

impl ApiMetrics {
    /// Creates a new, zeroed `ApiMetrics`.
    pub fn new() -> Self {
        Self {
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cache_creation_tokens: 0,
            total_cache_read_tokens: 0,
            total_cost: 0.0,
            request_count: 0,
        }
    }

    /// Records a single API request's token usage and calculates its cost.
    ///
    /// # Arguments
    /// * `input_tokens` — Input token count
    /// * `output_tokens` — Output token count
    /// * `cache_creation_tokens` — Tokens written to cache (optional)
    /// * `cache_read_tokens` — Tokens read from cache (optional)
    /// * `model_info` — Model pricing information for cost calculation
    pub fn add_request(
        &mut self,
        input_tokens: u64,
        output_tokens: u64,
        cache_creation_tokens: Option<u64>,
        cache_read_tokens: Option<u64>,
        model_info: &ModelInfo,
    ) {
        self.total_input_tokens += input_tokens;
        self.total_output_tokens += output_tokens;
        self.total_cache_creation_tokens += cache_creation_tokens.unwrap_or(0);
        self.total_cache_read_tokens += cache_read_tokens.unwrap_or(0);
        self.total_cost += calculate_api_cost(
            model_info,
            input_tokens,
            output_tokens,
            cache_creation_tokens,
            cache_read_tokens,
        );
        self.request_count += 1;
    }

    /// Merges another `ApiMetrics` into this one, summing all fields.
    pub fn merge(&mut self, other: &ApiMetrics) {
        self.total_input_tokens += other.total_input_tokens;
        self.total_output_tokens += other.total_output_tokens;
        self.total_cache_creation_tokens += other.total_cache_creation_tokens;
        self.total_cache_read_tokens += other.total_cache_read_tokens;
        self.total_cost += other.total_cost;
        self.request_count += other.request_count;
    }
}

impl Default for ApiMetrics {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CombinedApiResult
// ---------------------------------------------------------------------------

/// Combined result from aggregating multiple stream chunks.
///
/// Contains the full text content, collected tool calls, and usage metrics.
#[derive(Debug, Clone)]
pub struct CombinedApiResult {
    /// All text content concatenated.
    pub text: String,
    /// Aggregated metrics.
    pub metrics: ApiMetrics,
}

// ---------------------------------------------------------------------------
// combine_api_requests
// ---------------------------------------------------------------------------

/// Combines multiple API stream chunks into a single aggregated result.
///
/// Extracts text content from `Text` chunks and usage information from
/// `Usage` chunks, producing a combined text string and aggregated metrics.
///
/// # Arguments
/// * `chunks` — Slice of API stream chunks to combine
/// * `model_info` — Model pricing information for cost calculation
pub fn combine_api_requests(chunks: &[ApiStreamChunk], model_info: &ModelInfo) -> CombinedApiResult {
    let mut text = String::new();
    let mut metrics = ApiMetrics::new();

    for chunk in chunks {
        match chunk {
            ApiStreamChunk::Text { text: t } => {
                text.push_str(t);
            }
            ApiStreamChunk::Usage {
                input_tokens,
                output_tokens,
                cache_write_tokens,
                cache_read_tokens,
                total_cost,
                ..
            } => {
                // If the chunk already has a total_cost, use it directly;
                // otherwise calculate from token counts.
                let cost = total_cost.unwrap_or_else(|| {
                    calculate_api_cost(
                        model_info,
                        *input_tokens,
                        *output_tokens,
                        *cache_write_tokens,
                        *cache_read_tokens,
                    )
                });

                metrics.total_input_tokens += input_tokens;
                metrics.total_output_tokens += output_tokens;
                metrics.total_cache_creation_tokens += cache_write_tokens.unwrap_or(0);
                metrics.total_cache_read_tokens += cache_read_tokens.unwrap_or(0);
                metrics.total_cost += cost;
                metrics.request_count += 1;
            }
            _ => {}
        }
    }

    CombinedApiResult { text, metrics }
}

// ---------------------------------------------------------------------------
// get_api_metrics
// ---------------------------------------------------------------------------

/// Extracts API metrics from a slice of stream chunks.
///
/// This is a convenience function that calls [`combine_api_requests`] and
/// returns only the metrics portion.
///
/// # Arguments
/// * `chunks` — Slice of API stream chunks
/// * `model_info` — Model pricing information for cost calculation
pub fn get_api_metrics(chunks: &[ApiStreamChunk], model_info: &ModelInfo) -> ApiMetrics {
    combine_api_requests(chunks, model_info).metrics
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_model_info() -> ModelInfo {
        ModelInfo {
            input_price: Some(3.0),
            output_price: Some(15.0),
            cache_writes_price: Some(3.75),
            cache_reads_price: Some(0.3),
            context_window: 200_000,
            ..ModelInfo::default()
        }
    }

    #[test]
    fn test_api_metrics_new() {
        let metrics = ApiMetrics::new();
        assert_eq!(metrics.total_input_tokens, 0);
        assert_eq!(metrics.total_output_tokens, 0);
        assert_eq!(metrics.total_cache_creation_tokens, 0);
        assert_eq!(metrics.total_cache_read_tokens, 0);
        assert_eq!(metrics.total_cost, 0.0);
        assert_eq!(metrics.request_count, 0);
    }

    #[test]
    fn test_api_metrics_default() {
        let metrics = ApiMetrics::default();
        assert_eq!(metrics.request_count, 0);
    }

    #[test]
    fn test_api_metrics_add_request() {
        let model = sample_model_info();
        let mut metrics = ApiMetrics::new();

        metrics.add_request(1000, 500, Some(100), Some(200), &model);

        assert_eq!(metrics.total_input_tokens, 1000);
        assert_eq!(metrics.total_output_tokens, 500);
        assert_eq!(metrics.total_cache_creation_tokens, 100);
        assert_eq!(metrics.total_cache_read_tokens, 200);
        assert_eq!(metrics.request_count, 1);
        // cost: 3.0*1000/1M + 15.0*500/1M + 3.75*100/1M + 0.3*200/1M
        //     = 0.003 + 0.0075 + 0.000375 + 0.00006 = 0.010935
        assert!((metrics.total_cost - 0.010935).abs() < 1e-9);
    }

    #[test]
    fn test_api_metrics_add_multiple_requests() {
        let model = sample_model_info();
        let mut metrics = ApiMetrics::new();

        metrics.add_request(1000, 500, None, None, &model);
        metrics.add_request(2000, 1000, Some(100), None, &model);

        assert_eq!(metrics.total_input_tokens, 3000);
        assert_eq!(metrics.total_output_tokens, 1500);
        assert_eq!(metrics.total_cache_creation_tokens, 100);
        assert_eq!(metrics.request_count, 2);
    }

    #[test]
    fn test_api_metrics_merge() {
        let model = sample_model_info();
        let mut metrics1 = ApiMetrics::new();
        metrics1.add_request(1000, 500, None, None, &model);

        let mut metrics2 = ApiMetrics::new();
        metrics2.add_request(2000, 1000, Some(100), None, &model);

        metrics1.merge(&metrics2);

        assert_eq!(metrics1.total_input_tokens, 3000);
        assert_eq!(metrics1.total_output_tokens, 1500);
        assert_eq!(metrics1.total_cache_creation_tokens, 100);
        assert_eq!(metrics1.request_count, 2);
    }

    #[test]
    fn test_combine_api_requests_text_only() {
        let model = sample_model_info();
        let chunks = vec![
            ApiStreamChunk::Text {
                text: "Hello ".to_string(),
            },
            ApiStreamChunk::Text {
                text: "World".to_string(),
            },
        ];

        let result = combine_api_requests(&chunks, &model);
        assert_eq!(result.text, "Hello World");
        assert_eq!(result.metrics.request_count, 0);
    }

    #[test]
    fn test_combine_api_requests_with_usage() {
        let model = sample_model_info();
        let chunks = vec![
            ApiStreamChunk::Text {
                text: "Hello".to_string(),
            },
            ApiStreamChunk::Usage {
                input_tokens: 1000,
                output_tokens: 500,
                cache_write_tokens: Some(100),
                cache_read_tokens: Some(200),
                reasoning_tokens: None,
                total_cost: None,
            },
        ];

        let result = combine_api_requests(&chunks, &model);
        assert_eq!(result.text, "Hello");
        assert_eq!(result.metrics.total_input_tokens, 1000);
        assert_eq!(result.metrics.total_output_tokens, 500);
        assert_eq!(result.metrics.total_cache_creation_tokens, 100);
        assert_eq!(result.metrics.total_cache_read_tokens, 200);
        assert_eq!(result.metrics.request_count, 1);
    }

    #[test]
    fn test_combine_api_requests_with_preset_cost() {
        let model = sample_model_info();
        let chunks = vec![ApiStreamChunk::Usage {
            input_tokens: 1000,
            output_tokens: 500,
            cache_write_tokens: None,
            cache_read_tokens: None,
            reasoning_tokens: None,
            total_cost: Some(0.05),
        }];

        let result = combine_api_requests(&chunks, &model);
        // Should use the preset cost, not calculate
        assert!((result.metrics.total_cost - 0.05).abs() < 1e-10);
    }

    #[test]
    fn test_get_api_metrics() {
        let model = sample_model_info();
        let chunks = vec![
            ApiStreamChunk::Usage {
                input_tokens: 1000,
                output_tokens: 500,
                cache_write_tokens: None,
                cache_read_tokens: None,
                reasoning_tokens: None,
                total_cost: None,
            },
            ApiStreamChunk::Usage {
                input_tokens: 2000,
                output_tokens: 1000,
                cache_write_tokens: Some(100),
                cache_read_tokens: None,
                reasoning_tokens: None,
                total_cost: None,
            },
        ];

        let metrics = get_api_metrics(&chunks, &model);
        assert_eq!(metrics.total_input_tokens, 3000);
        assert_eq!(metrics.total_output_tokens, 1500);
        assert_eq!(metrics.total_cache_creation_tokens, 100);
        assert_eq!(metrics.request_count, 2);
    }

    #[test]
    fn test_combine_ignores_non_text_usage_chunks() {
        let model = sample_model_info();
        let chunks = vec![
            ApiStreamChunk::Reasoning {
                text: "thinking...".to_string(),
                signature: None,
            },
            ApiStreamChunk::ToolCall {
                id: "call_1".to_string(),
                name: "test_tool".to_string(),
                arguments: "{}".to_string(),
            },
        ];

        let result = combine_api_requests(&chunks, &model);
        assert_eq!(result.text, "");
        assert_eq!(result.metrics.request_count, 0);
    }
}

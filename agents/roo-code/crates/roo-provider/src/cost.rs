//! API cost calculation utilities.
//!
//! Derived from `src/shared/cost.ts`.
//! Provides functions for calculating API call costs based on token usage
//! and model pricing, supporting both Anthropic and OpenAI billing models.

use roo_types::model::{ModelInfo, ServiceTier};

// ---------------------------------------------------------------------------
// ApiCostResult
// ---------------------------------------------------------------------------

/// Result of an API cost calculation.
///
/// Source: `src/shared/cost.ts` — `ApiCostResult`
#[derive(Debug, Clone, PartialEq)]
pub struct ApiCostResult {
    /// Total input tokens (including cached tokens where applicable).
    pub total_input_tokens: u64,
    /// Total output tokens.
    pub total_output_tokens: u64,
    /// Total cost in USD.
    pub total_cost: f64,
}

// ---------------------------------------------------------------------------
// Long context pricing
// ---------------------------------------------------------------------------

/// Applies long context pricing multipliers when the input token count
/// exceeds the model's threshold.
///
/// Source: `src/shared/cost.ts` — `applyLongContextPricing`
///
/// Returns a modified `ModelInfo` with adjusted prices, or the original
/// if long context pricing does not apply.
fn apply_long_context_pricing(
    model_info: &ModelInfo,
    total_input_tokens: u64,
    service_tier: Option<ServiceTier>,
) -> ModelInfo {
    let pricing = match &model_info.long_context_pricing {
        Some(p) => p,
        None => return model_info.clone(),
    };

    if total_input_tokens <= pricing.threshold_tokens {
        return model_info.clone();
    }

    // Check if the pricing applies to the given service tier
    if let Some(ref tiers) = pricing.applies_to_service_tiers {
        let effective_tier = service_tier.unwrap_or(ServiceTier::Default);
        if !tiers.contains(&effective_tier) {
            return model_info.clone();
        }
    }

    // Apply multipliers
    let mut adjusted = model_info.clone();
    if let (Some(price), Some(multiplier)) = (model_info.input_price, pricing.input_price_multiplier) {
        adjusted.input_price = Some(price * multiplier);
    }
    if let (Some(price), Some(multiplier)) = (model_info.output_price, pricing.output_price_multiplier) {
        adjusted.output_price = Some(price * multiplier);
    }
    if let (Some(price), Some(multiplier)) = (model_info.cache_writes_price, pricing.cache_writes_price_multiplier) {
        adjusted.cache_writes_price = Some(price * multiplier);
    }
    if let (Some(price), Some(multiplier)) = (model_info.cache_reads_price, pricing.cache_reads_price_multiplier) {
        adjusted.cache_reads_price = Some(price * multiplier);
    }

    adjusted
}

// ---------------------------------------------------------------------------
// Internal cost calculation
// ---------------------------------------------------------------------------

/// Internal cost calculation shared by both Anthropic and OpenAI paths.
///
/// All prices are per-million tokens. Returns the total cost.
fn calculate_cost_internal(
    model_info: &ModelInfo,
    input_tokens: u64,
    output_tokens: u64,
    cache_creation_tokens: u64,
    cache_read_tokens: u64,
) -> f64 {
    let cache_writes_cost = model_info.cache_writes_price.unwrap_or(0.0) * cache_creation_tokens as f64 / 1_000_000.0;
    let cache_reads_cost = model_info.cache_reads_price.unwrap_or(0.0) * cache_read_tokens as f64 / 1_000_000.0;
    let base_input_cost = model_info.input_price.unwrap_or(0.0) * input_tokens as f64 / 1_000_000.0;
    let output_cost = model_info.output_price.unwrap_or(0.0) * output_tokens as f64 / 1_000_000.0;

    cache_writes_cost + cache_reads_cost + base_input_cost + output_cost
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Calculates API cost using the Anthropic billing model.
///
/// For Anthropic-compliant usage, the `input_tokens` count does **NOT** include
/// cached tokens. The total input is computed as:
/// `input_tokens + cache_creation_tokens + cache_read_tokens`.
///
/// Source: `src/shared/cost.ts` — `calculateApiCostAnthropic`
pub fn calculate_api_cost_anthropic(
    model_info: &ModelInfo,
    input_tokens: u64,
    output_tokens: u64,
    cache_creation_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
) -> ApiCostResult {
    let cache_creation = cache_creation_tokens.unwrap_or(0);
    let cache_read = cache_read_tokens.unwrap_or(0);

    // For Anthropic: inputTokens does NOT include cached tokens
    let total_input_tokens = input_tokens + cache_creation + cache_read;

    let total_cost = calculate_cost_internal(
        model_info,
        input_tokens,
        output_tokens,
        cache_creation,
        cache_read,
    );

    ApiCostResult {
        total_input_tokens,
        total_output_tokens: output_tokens,
        total_cost,
    }
}

/// Calculates API cost using the OpenAI billing model.
///
/// For OpenAI-compliant usage, the `input_tokens` count **INCLUDES** cached tokens.
/// Non-cached input tokens are computed as:
/// `max(0, input_tokens - cache_creation_tokens - cache_read_tokens)`.
///
/// Supports long context pricing when the model defines it.
///
/// Source: `src/shared/cost.ts` — `calculateApiCostOpenAI`
pub fn calculate_api_cost_openai(
    model_info: &ModelInfo,
    input_tokens: u64,
    output_tokens: u64,
    cache_creation_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
    service_tier: Option<ServiceTier>,
) -> ApiCostResult {
    let cache_creation = cache_creation_tokens.unwrap_or(0);
    let cache_read = cache_read_tokens.unwrap_or(0);

    // For OpenAI: input_tokens ALREADY includes all tokens (cached + non-cached)
    let non_cached_input = input_tokens.saturating_sub(cache_creation).saturating_sub(cache_read);

    let effective_model_info = apply_long_context_pricing(model_info, input_tokens, service_tier);

    let total_cost = calculate_cost_internal(
        &effective_model_info,
        non_cached_input,
        output_tokens,
        cache_creation,
        cache_read,
    );

    ApiCostResult {
        total_input_tokens: input_tokens,
        total_output_tokens: output_tokens,
        total_cost,
    }
}

/// Calculates API cost with a simplified interface.
///
/// This is a convenience wrapper that delegates to [`calculate_api_cost_openai`]
/// without a service tier.
///
/// # Arguments
/// * `model_info` — Model pricing information
/// * `input_tokens` — Input token count (may or may not include cached tokens depending on provider)
/// * `output_tokens` — Output token count
/// * `cache_creation_tokens` — Tokens written to cache (optional)
/// * `cache_read_tokens` — Tokens read from cache (optional)
pub fn calculate_api_cost(
    model_info: &ModelInfo,
    input_tokens: u64,
    output_tokens: u64,
    cache_creation_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
) -> f64 {
    calculate_cost_internal(
        model_info,
        input_tokens,
        output_tokens,
        cache_creation_tokens.unwrap_or(0),
        cache_read_tokens.unwrap_or(0),
    )
}

/// Parses a per-token price into a per-million-tokens price.
///
/// Source: `src/shared/cost.ts` — `parseApiPrice`
///
/// # Arguments
/// * `price` — The per-token price (may be `None` or a JSON value)
///
/// # Returns
/// `Some(price_per_million)` if the price is valid and non-zero, `None` otherwise.
pub fn parse_api_price(price: Option<serde_json::Value>) -> Option<f64> {
    price.and_then(|v| {
        let parsed = match v {
            serde_json::Value::Number(n) => n.as_f64(),
            serde_json::Value::String(s) => s.parse::<f64>().ok(),
            _ => None,
        };
        parsed.map(|p| p * 1_000_000.0)
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use roo_types::model::LongContextPricing;

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
    fn test_calculate_api_cost_basic() {
        let model = sample_model_info();
        let cost = calculate_api_cost(&model, 1000, 500, None, None);
        // input: 3.0 * 1000 / 1_000_000 = 0.003
        // output: 15.0 * 500 / 1_000_000 = 0.0075
        assert!((cost - 0.0105).abs() < 1e-10);
    }

    #[test]
    fn test_calculate_api_cost_with_cache() {
        let model = sample_model_info();
        let cost = calculate_api_cost(&model, 1000, 500, Some(200), Some(300));
        // input: 3.0 * 1000 / 1_000_000 = 0.003
        // output: 15.0 * 500 / 1_000_000 = 0.0075
        // cache_writes: 3.75 * 200 / 1_000_000 = 0.00075
        // cache_reads: 0.3 * 300 / 1_000_000 = 0.00009
        assert!((cost - 0.01134).abs() < 1e-10);
    }

    #[test]
    fn test_calculate_api_cost_no_pricing() {
        let model = ModelInfo::default();
        let cost = calculate_api_cost(&model, 1000, 500, Some(200), Some(300));
        assert_eq!(cost, 0.0);
    }

    #[test]
    fn test_calculate_api_cost_anthropic() {
        let model = sample_model_info();
        let result = calculate_api_cost_anthropic(&model, 1000, 500, Some(200), Some(300));
        // total_input = 1000 + 200 + 300 = 1500 (Anthropic: input does NOT include cache)
        assert_eq!(result.total_input_tokens, 1500);
        assert_eq!(result.total_output_tokens, 500);
        // cost: 3.0*1000/1M + 15.0*500/1M + 3.75*200/1M + 0.3*300/1M
        //     = 0.003 + 0.0075 + 0.00075 + 0.00009 = 0.01134
        assert!((result.total_cost - 0.01134).abs() < 1e-10);
    }

    #[test]
    fn test_calculate_api_cost_anthropic_no_cache() {
        let model = sample_model_info();
        let result = calculate_api_cost_anthropic(&model, 1000, 500, None, None);
        assert_eq!(result.total_input_tokens, 1000);
        assert_eq!(result.total_output_tokens, 500);
        assert!((result.total_cost - 0.0105).abs() < 1e-10);
    }

    #[test]
    fn test_calculate_api_cost_openai() {
        let model = sample_model_info();
        let result = calculate_api_cost_openai(&model, 1500, 500, Some(200), Some(300), None);
        // OpenAI: input_tokens ALREADY includes cached tokens
        // non_cached = max(0, 1500 - 200 - 300) = 1000
        assert_eq!(result.total_input_tokens, 1500);
        assert_eq!(result.total_output_tokens, 500);
        // cost: 3.0*1000/1M + 15.0*500/1M + 3.75*200/1M + 0.3*300/1M = 0.01134
        assert!((result.total_cost - 0.01134).abs() < 1e-10);
    }

    #[test]
    fn test_calculate_api_cost_openai_no_cache() {
        let model = sample_model_info();
        let result = calculate_api_cost_openai(&model, 1000, 500, None, None, None);
        assert_eq!(result.total_input_tokens, 1000);
        assert_eq!(result.total_output_tokens, 500);
        assert!((result.total_cost - 0.0105).abs() < 1e-10);
    }

    #[test]
    fn test_apply_long_context_pricing_no_pricing() {
        let model = sample_model_info();
        let result = apply_long_context_pricing(&model, 100_000, None);
        assert_eq!(result.input_price, Some(3.0));
    }

    #[test]
    fn test_apply_long_context_pricing_below_threshold() {
        let mut model = sample_model_info();
        model.long_context_pricing = Some(LongContextPricing {
            threshold_tokens: 200_000,
            input_price_multiplier: Some(2.0),
            output_price_multiplier: None,
            cache_writes_price_multiplier: None,
            cache_reads_price_multiplier: None,
            applies_to_service_tiers: None,
        });
        let result = apply_long_context_pricing(&model, 100_000, None);
        // Below threshold, no change
        assert_eq!(result.input_price, Some(3.0));
    }

    #[test]
    fn test_apply_long_context_pricing_above_threshold() {
        let mut model = sample_model_info();
        model.long_context_pricing = Some(LongContextPricing {
            threshold_tokens: 100_000,
            input_price_multiplier: Some(2.0),
            output_price_multiplier: Some(1.5),
            cache_writes_price_multiplier: Some(2.0),
            cache_reads_price_multiplier: Some(2.0),
            applies_to_service_tiers: None,
        });
        let result = apply_long_context_pricing(&model, 150_000, None);
        assert_eq!(result.input_price, Some(6.0)); // 3.0 * 2.0
        assert_eq!(result.output_price, Some(22.5)); // 15.0 * 1.5
        assert_eq!(result.cache_writes_price, Some(7.5)); // 3.75 * 2.0
        assert_eq!(result.cache_reads_price, Some(0.6)); // 0.3 * 2.0
    }

    #[test]
    fn test_apply_long_context_pricing_service_tier_filter() {
        let mut model = sample_model_info();
        model.long_context_pricing = Some(LongContextPricing {
            threshold_tokens: 100_000,
            input_price_multiplier: Some(2.0),
            output_price_multiplier: None,
            cache_writes_price_multiplier: None,
            cache_reads_price_multiplier: None,
            applies_to_service_tiers: Some(vec![ServiceTier::Flex]),
        });
        // Default tier is not in the list, so no change
        let result = apply_long_context_pricing(&model, 150_000, Some(ServiceTier::Default));
        assert_eq!(result.input_price, Some(3.0));
        // Flex tier is in the list, so multiplier applies
        let result = apply_long_context_pricing(&model, 150_000, Some(ServiceTier::Flex));
        assert_eq!(result.input_price, Some(6.0));
    }

    #[test]
    fn test_parse_api_price_valid_number() {
        let val = serde_json::json!(0.000003);
        let result = parse_api_price(Some(val));
        assert_eq!(result, Some(3.0));
    }

    #[test]
    fn test_parse_api_price_valid_string() {
        let val = serde_json::json!("0.000003");
        let result = parse_api_price(Some(val));
        assert_eq!(result, Some(3.0));
    }

    #[test]
    fn test_parse_api_price_none() {
        let result = parse_api_price(None);
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_api_price_invalid_string() {
        let val = serde_json::json!("not_a_number");
        let result = parse_api_price(Some(val));
        assert_eq!(result, None);
    }

    #[test]
    fn test_openai_saturating_sub_no_negative() {
        // When cache tokens > input tokens, non_cached should be 0
        let model = sample_model_info();
        let result = calculate_api_cost_openai(&model, 100, 50, Some(200), Some(300), None);
        // non_cached = max(0, 100 - 200 - 300) = 0
        assert_eq!(result.total_input_tokens, 100);
        // cost: 3.0*0/1M + 15.0*50/1M + 3.75*200/1M + 0.3*300/1M
        //     = 0 + 0.00075 + 0.00075 + 0.00009 = 0.00159
        assert!((result.total_cost - 0.00159).abs() < 1e-10);
    }
}

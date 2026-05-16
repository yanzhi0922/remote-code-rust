//! MiniMax model definitions.

use roo_types::model::ModelInfo;
use std::collections::HashMap;

/// Default MiniMax model ID.
pub const DEFAULT_MODEL_ID: &str = "MiniMax-M2.7";

fn apply_minimax_model_metadata(models: &mut HashMap<String, ModelInfo>) {
    for (model_id, info) in models.iter_mut() {
        info.included_tools = Some(vec!["search_and_replace".to_string()]);
        info.excluded_tools = Some(vec!["apply_diff".to_string()]);
        info.preserve_reasoning = Some(true);

        let Some(description) = info.description.as_mut() else {
            continue;
        };

        if !description.contains("pricing-paygo") {
            description
                .push_str(" See pricing at https://platform.minimax.io/docs/guides/pricing-paygo.");
        }
        if model_id.ends_with("highspeed")
            && !description.contains("Requires TokenPlan High-Speed subscription")
        {
            description.push_str(
                " Requires TokenPlan High-Speed subscription for use with TokenPlan keys.",
            );
        }
        if !description.contains("usage is billed per request, not per token") {
            description.push_str(
                " Note: When using TokenPlan, usage is billed per request, not per token.",
            );
        }
    }
}

/// Returns the supported MiniMax models.
pub fn models() -> HashMap<String, ModelInfo> {
    let mut m = HashMap::new();

    m.insert(
        "MiniMax-M2.5".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 204_800,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.3),
            output_price: Some(1.2),
            cache_writes_price: Some(0.375),
            cache_reads_price: Some(0.03),
            description: Some(
                "MiniMax M2.5, the latest MiniMax model with enhanced coding and agentic \
                 capabilities, building on the strengths of the M2 series."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "MiniMax-M2.5-highspeed".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 204_800,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.6),
            output_price: Some(2.4),
            cache_writes_price: Some(0.375),
            cache_reads_price: Some(0.03),
            description: Some(
                "MiniMax M2.5 highspeed: same performance as M2.5 but with faster response \
                 (approximately 100 tps vs 60 tps)."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "MiniMax-M2.7".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 204_800,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.3),
            output_price: Some(1.2),
            cache_writes_price: Some(0.375),
            cache_reads_price: Some(0.06),
            description: Some(
                "MiniMax M2.7, the latest MiniMax model with recursive self-improvement capabilities."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "MiniMax-M2.7-highspeed".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 204_800,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.6),
            output_price: Some(2.4),
            cache_writes_price: Some(0.375),
            cache_reads_price: Some(0.06),
            description: Some(
                "MiniMax M2.7 highspeed: same performance as M2.7 but with faster response \
                 (approximately 100 tps vs 60 tps)."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "MiniMax-M2".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 204_800,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.3),
            output_price: Some(1.2),
            cache_writes_price: Some(0.375),
            cache_reads_price: Some(0.03),
            description: Some(
                "MiniMax M2, a model born for Agents and code, featuring Top-tier Coding \
                 Capabilities, Powerful Agentic Performance, and Ultimate Cost-Effectiveness & Speed."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "MiniMax-M2-Stable".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 204_800,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.3),
            output_price: Some(1.2),
            cache_writes_price: Some(0.375),
            cache_reads_price: Some(0.03),
            description: Some(
                "MiniMax M2 Stable (High Concurrency, Commercial Use), a model born for \
                 Agents and code."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "MiniMax-M2.1".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 204_800,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.3),
            output_price: Some(1.2),
            cache_writes_price: Some(0.375),
            cache_reads_price: Some(0.03),
            description: Some(
                "MiniMax M2.1 builds on M2 with improved overall performance for agentic \
                 coding tasks and significantly faster response times."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    m.insert(
        "MiniMax-M2.1-highspeed".to_string(),
        ModelInfo {
            max_tokens: Some(16_384),
            context_window: 204_800,
            supports_images: Some(false),
            supports_prompt_cache: true,
            input_price: Some(0.6),
            output_price: Some(2.4),
            cache_writes_price: Some(0.375),
            cache_reads_price: Some(0.03),
            description: Some(
                "MiniMax M2.1 highspeed: same performance as M2.1 but with faster response \
                 (approximately 100 tps vs 60 tps)."
                    .to_string(),
            ),
            ..Default::default()
        },
    );

    apply_minimax_model_metadata(&mut m);
    m
}

/// Returns the default model ID.
pub fn default_model_id() -> String {
    DEFAULT_MODEL_ID.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimax_models_match_roo_code_tool_and_reasoning_metadata() {
        for (model_id, info) in models() {
            assert_eq!(
                info.included_tools.as_deref(),
                Some(&["search_and_replace".to_string()][..]),
                "{model_id} included tools"
            );
            assert_eq!(
                info.excluded_tools.as_deref(),
                Some(&["apply_diff".to_string()][..]),
                "{model_id} excluded tools"
            );
            assert_eq!(info.preserve_reasoning, Some(true), "{model_id}");
            let description = info.description.as_deref().unwrap_or_default();
            assert!(description.contains("pricing-paygo"), "{model_id}");
            assert!(
                description.contains("usage is billed per request, not per token"),
                "{model_id}"
            );
            if model_id.ends_with("highspeed") {
                assert!(
                    description.contains("Requires TokenPlan High-Speed subscription"),
                    "{model_id}"
                );
            }
        }
    }
}

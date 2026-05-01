//! Unified model parameter computation.
//!
//! Derived from `src/api/transform/model-params.ts` and `src/shared/api.ts`.
//! Computes all model parameters (max tokens, temperature, reasoning config)
//! for a given provider format, model, and settings combination.

use serde_json::Value;

use roo_types::model::{ModelInfo, ReasoningEffortExtended, ReasoningEffortSetting, VerbosityLevel};
use roo_types::provider_settings::ProviderSettings;

use super::reasoning::{
    effort_extended_to_setting, effort_setting_to_extended,
    get_anthropic_reasoning, get_gemini_reasoning, get_openai_reasoning, get_openrouter_reasoning,
    should_use_reasoning_budget, should_use_reasoning_effort, GetModelReasoningOptions,
    ANTHROPIC_DEFAULT_MAX_TOKENS, DEFAULT_HYBRID_REASONING_MODEL_MAX_TOKENS,
    DEFAULT_HYBRID_REASONING_MODEL_THINKING_TOKENS, GEMINI_25_PRO_MIN_THINKING_TOKENS,
};

// ---------------------------------------------------------------------------
// Format enum
// ---------------------------------------------------------------------------

/// API message format for parameter computation.
///
/// Each format corresponds to a specific provider family and determines
/// which reasoning parameter shape is returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Anthropic Messages API format.
    Anthropic,
    /// OpenAI Chat Completions API format.
    OpenAi,
    /// Google Gemini GenerateContent API format.
    Gemini,
    /// OpenRouter proxy API format.
    OpenRouter,
}

// ---------------------------------------------------------------------------
// ModelParams struct
// ---------------------------------------------------------------------------

/// Computed model parameters for an API request.
///
/// This struct contains all the parameters needed to configure an API call
/// to any supported provider. The `reasoning` and `thinking` fields hold
/// provider-specific reasoning configurations as JSON values.
///
/// - `reasoning`: Provider-specific reasoning config (OpenAI, Gemini, OpenRouter)
/// - `thinking`: Anthropic-specific thinking config
#[derive(Debug, Clone, Default)]
pub struct ModelParams {
    /// Maximum output tokens the model should generate.
    pub max_tokens: Option<u64>,
    /// Sampling temperature. `None` means the provider default is used
    /// (or the parameter is unsupported for the given model).
    pub temperature: Option<f64>,
    /// Provider-specific reasoning configuration (non-Anthropic providers).
    ///
    /// - OpenAI: `{ "reasoning_effort": "low"|"medium"|"high" }`
    /// - Gemini: `{ "thinkingBudget": N, "includeThoughts": true }` or
    ///   `{ "thinkingLevel": "low", "includeThoughts": true }`
    /// - OpenRouter: `{ "max_tokens": N }` or `{ "effort": "low" }`
    pub reasoning: Option<Value>,
    /// Anthropic-specific thinking configuration.
    ///
    /// `{ "type": "enabled", "budget_tokens": N }`
    pub thinking: Option<Value>,
    /// Resolved reasoning effort level.
    pub reasoning_effort: Option<ReasoningEffortExtended>,
    /// Resolved reasoning budget (token count).
    pub reasoning_budget: Option<u64>,
    /// Output verbosity level.
    pub verbosity: Option<VerbosityLevel>,
}

// ---------------------------------------------------------------------------
// Options struct
// ---------------------------------------------------------------------------

/// Options for computing model parameters.
///
/// Mirrors the TypeScript `GetModelParamsOptions<T>` type.
#[derive(Debug, Clone)]
pub struct GetModelParamsOptions<'a> {
    /// The API format to compute parameters for.
    pub format: Format,
    /// The model identifier string (e.g. `"claude-3-7-sonnet-20250219"`).
    pub model_id: &'a str,
    /// The model information describing capabilities and defaults.
    pub model: &'a ModelInfo,
    /// Provider settings including user preferences.
    pub settings: &'a ProviderSettings,
    /// Default temperature to use when neither settings nor model specify one.
    pub default_temperature: f64,
}

// ---------------------------------------------------------------------------
// Max output tokens computation
// ---------------------------------------------------------------------------

/// Compute the maximum output tokens for a given model and settings.
///
/// This function implements the centralized logic for determining the
/// `max_tokens` / `maxTokens` parameter:
///
/// 1. **Reasoning-budget models** use `settings.model_max_tokens` or
///    [`DEFAULT_HYBRID_REASONING_MODEL_MAX_TOKENS`].
/// 2. **Anthropic context** with `supports_reasoning_budget` uses
///    [`ANTHROPIC_DEFAULT_MAX_TOKENS`].
/// 3. **Anthropic context** with no explicit `max_tokens` uses
///    [`ANTHROPIC_DEFAULT_MAX_TOKENS`].
/// 4. **Explicit model `max_tokens`** is clamped to 20% of the context
///    window, except for GPT-5 models which use their full value.
/// 5. **Fallback**: `None` for non-Anthropic formats, or
///    [`ANTHROPIC_DEFAULT_MAX_TOKENS`] when no format is specified.
pub fn get_model_max_output_tokens(
    model_id: &str,
    model: &ModelInfo,
    settings: &ProviderSettings,
    format: Option<Format>,
) -> Option<u64> {
    if should_use_reasoning_budget(model, settings) {
        return Some(
            settings
                .model_max_tokens
                .unwrap_or(DEFAULT_HYBRID_REASONING_MODEL_MAX_TOKENS),
        );
    }

    let is_anthropic_context = model_id.contains("claude")
        || format == Some(Format::Anthropic)
        || (format == Some(Format::OpenRouter) && model_id.starts_with("anthropic/"));

    // For "Hybrid" reasoning models, discard the model's actual maxTokens for Anthropic contexts
    if model.supports_reasoning_budget.unwrap_or(false) && is_anthropic_context {
        return Some(ANTHROPIC_DEFAULT_MAX_TOKENS);
    }

    // For Anthropic contexts, always ensure a maxTokens value is set
    if is_anthropic_context && model.max_tokens.map_or(true, |t| t == 0) {
        return Some(ANTHROPIC_DEFAULT_MAX_TOKENS);
    }

    // If model has explicit maxTokens, clamp it to 20% of the context window
    // Exception: GPT-5 models should use their exact configured max output tokens
    if let Some(max_tokens) = model.max_tokens {
        // Check if this is a GPT-5 model (case-insensitive)
        let is_gpt5_model = model_id.to_lowercase().contains("gpt-5");

        // GPT-5 models bypass the 20% cap and use their full configured max tokens
        if is_gpt5_model {
            return Some(max_tokens);
        }

        // All other models are clamped to 20% of context window
        return Some(std::cmp::min(
            max_tokens,
            ((model.context_window as f64) * 0.2).ceil() as u64,
        ));
    }

    // For non-Anthropic formats without explicit maxTokens, return undefined
    if format.is_some() {
        return None;
    }

    // Default fallback
    Some(ANTHROPIC_DEFAULT_MAX_TOKENS)
}

// ---------------------------------------------------------------------------
// Main calculation function
// ---------------------------------------------------------------------------

/// Compute all model parameters for a given provider format.
///
/// This is the main entry point for parameter computation. It resolves
/// temperature, max tokens, reasoning budget/effort, and provider-specific
/// reasoning configuration based on the model capabilities and user settings.
///
/// # Arguments
///
/// * `opts` - The computation options including format, model, settings, etc.
///
/// # Returns
///
/// A [`ModelParams`] struct with all computed parameters.
///
/// # Examples
///
/// ```
/// # use roo_provider::transform::model_params::*;
/// # use roo_provider::transform::reasoning;
/// # use roo_types::model::ModelInfo;
/// # use roo_types::provider_settings::ProviderSettings;
/// let model = ModelInfo {
///     max_tokens: Some(4096),
///     context_window: 128_000,
///     ..Default::default()
/// };
/// let settings = ProviderSettings::default();
/// let opts = GetModelParamsOptions {
///     format: Format::OpenAi,
///     model_id: "gpt-4",
///     model: &model,
///     settings: &settings,
///     default_temperature: 0.0,
/// };
/// let params = calculate_model_params(opts);
/// assert!(params.max_tokens.is_some());
/// assert!(params.temperature.is_some());
/// ```
pub fn calculate_model_params(opts: GetModelParamsOptions) -> ModelParams {
    let GetModelParamsOptions {
        format,
        model_id,
        model,
        settings,
        default_temperature,
    } = opts;

    // Use the centralized logic for computing maxTokens
    let max_tokens = get_model_max_output_tokens(model_id, model, settings, Some(format));

    // Resolve temperature: settings → model default → caller default
    let mut temperature = settings
        .model_temperature
        .flatten()
        .or(model.default_temperature)
        .unwrap_or(default_temperature);

    let mut reasoning_budget: Option<u64> = None;
    let mut reasoning_effort: Option<ReasoningEffortExtended> = None;
    let verbosity = settings.verbosity;

    if should_use_reasoning_budget(model, settings) {
        // Check if this is a Gemini 2.5 Pro model
        let is_gemini_25_pro = model_id.contains("gemini-2.5-pro");

        // If `model_max_thinking_tokens` is not specified use the default.
        // For Gemini 2.5 Pro, default to 128 instead of 8192
        let default_thinking_tokens = if is_gemini_25_pro {
            GEMINI_25_PRO_MIN_THINKING_TOKENS
        } else {
            DEFAULT_HYBRID_REASONING_MODEL_THINKING_TOKENS
        };
        reasoning_budget = Some(
            settings
                .model_max_thinking_tokens
                .unwrap_or(default_thinking_tokens),
        );

        // Reasoning cannot exceed 80% of the `maxTokens` value.
        if let Some(mt) = max_tokens {
            let cap = ((mt as f64) * 0.8).floor() as u64;
            if reasoning_budget.unwrap() > cap {
                reasoning_budget = Some(cap);
            }
        }

        // Reasoning cannot be less than minimum tokens.
        // For Gemini 2.5 Pro models, the minimum is 128 tokens
        // For other models, the minimum is 1024 tokens
        let min_thinking_tokens = if is_gemini_25_pro {
            GEMINI_25_PRO_MIN_THINKING_TOKENS
        } else {
            1024
        };
        if reasoning_budget.unwrap() < min_thinking_tokens {
            reasoning_budget = Some(min_thinking_tokens);
        }

        // "Hybrid" reasoning models require a temperature of 1.0
        temperature = 1.0;
    } else if should_use_reasoning_effort(model, settings) {
        // "Traditional" reasoning models use the `reasoningEffort` parameter.
        // Only fallback to model default if user hasn't explicitly set a value.
        // If customReasoningEffort is "disable", don't fallback to model default.
        let effort = settings
            .reasoning_effort
            .or_else(|| model.reasoning_effort.map(effort_extended_to_setting));

        // Capability and settings checks are handled by should_use_reasoning_effort.
        // Here we simply propagate the resolved effort, while still treating
        // "disable" as an omission.
        if let Some(e) = effort {
            if e != ReasoningEffortSetting::Disable {
                reasoning_effort = effort_setting_to_extended(e);
            }
        }
    }

    // Build the reasoning options for provider-specific computation
    let reasoning_opts = GetModelReasoningOptions {
        model,
        reasoning_budget,
        reasoning_effort: reasoning_effort.map(|e| effort_extended_to_setting(e)),
        settings,
    };

    // Compute provider-specific reasoning config
    let (reasoning, thinking) = match format {
        Format::Anthropic => {
            let thinking = get_anthropic_reasoning(&reasoning_opts);
            (None, thinking)
        }
        Format::OpenAi => {
            // Special case for o1 and o3-mini, which don't support temperature.
            if model_id.starts_with("o1") || model_id.starts_with("o3-mini") {
                temperature = 0.0; // will be set to None below
            }
            let reasoning = get_openai_reasoning(&reasoning_opts);
            (reasoning, None)
        }
        Format::Gemini => {
            let reasoning = get_gemini_reasoning(&reasoning_opts);
            (reasoning, None)
        }
        Format::OpenRouter => {
            // Special case for o1-pro, which doesn't support temperature.
            if model_id == "openai/o1-pro" {
                temperature = 0.0; // will be set to None below
            }
            let reasoning = get_openrouter_reasoning(&reasoning_opts);
            (reasoning, None)
        }
    };

    // Handle temperature suppression for specific models
    let temperature = if matches!(format, Format::OpenAi)
        && (model_id.starts_with("o1") || model_id.starts_with("o3-mini"))
    {
        None
    } else if matches!(format, Format::OpenRouter) && model_id == "openai/o1-pro" {
        None
    } else {
        Some(temperature)
    };

    ModelParams {
        max_tokens,
        temperature,
        reasoning,
        thinking,
        reasoning_effort,
        reasoning_budget,
        verbosity,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Helper: basic model with no special capabilities.
    fn basic_model() -> ModelInfo {
        ModelInfo {
            max_tokens: Some(4096),
            context_window: 128_000,
            ..Default::default()
        }
    }

    /// Helper: hybrid reasoning model (budget-based).
    fn hybrid_model() -> ModelInfo {
        ModelInfo {
            required_reasoning_budget: Some(true),
            supports_reasoning_budget: Some(true),
            max_tokens: Some(16_384),
            max_thinking_tokens: Some(8192),
            context_window: 200_000,
            ..Default::default()
        }
    }

    /// Helper: effort-based reasoning model.
    fn effort_model() -> ModelInfo {
        ModelInfo {
            supports_reasoning_effort: Some(Value::Bool(true)),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            max_tokens: Some(4096),
            context_window: 128_000,
            ..Default::default()
        }
    }

    fn default_settings() -> ProviderSettings {
        ProviderSettings::default()
    }

    // ---- get_model_max_output_tokens tests ----

    #[test]
    fn test_max_tokens_reasoning_budget_model() {
        let model = hybrid_model();
        let settings = default_settings();
        let result = get_model_max_output_tokens("claude-3-7", &model, &settings, Some(Format::Anthropic));
        assert_eq!(result, Some(DEFAULT_HYBRID_REASONING_MODEL_MAX_TOKENS));
    }

    #[test]
    fn test_max_tokens_reasoning_budget_custom() {
        let model = hybrid_model();
        let settings = ProviderSettings {
            model_max_tokens: Some(32_768),
            ..Default::default()
        };
        let result = get_model_max_output_tokens("claude-3-7", &model, &settings, Some(Format::Anthropic));
        assert_eq!(result, Some(32_768));
    }

    #[test]
    fn test_max_tokens_anthropic_context_no_max() {
        let model = ModelInfo {
            context_window: 200_000,
            ..Default::default()
        };
        let settings = default_settings();
        let result = get_model_max_output_tokens("claude-3-5-sonnet", &model, &settings, None);
        assert_eq!(result, Some(ANTHROPIC_DEFAULT_MAX_TOKENS));
    }

    #[test]
    fn test_max_tokens_clamped_to_20_percent() {
        let model = ModelInfo {
            max_tokens: Some(100_000),
            context_window: 200_000,
            ..Default::default()
        };
        let settings = default_settings();
        let result = get_model_max_output_tokens("gpt-4", &model, &settings, Some(Format::OpenAi));
        // 20% of 200_000 = 40_000, which is less than 100_000
        assert_eq!(result, Some(40_000));
    }

    #[test]
    fn test_max_tokens_gpt5_bypasses_cap() {
        let model = ModelInfo {
            max_tokens: Some(100_000),
            context_window: 200_000,
            ..Default::default()
        };
        let settings = default_settings();
        let result = get_model_max_output_tokens("gpt-5-turbo", &model, &settings, Some(Format::OpenAi));
        // GPT-5 models bypass the 20% cap
        assert_eq!(result, Some(100_000));
    }

    #[test]
    fn test_max_tokens_no_format_no_model_tokens() {
        let model = ModelInfo {
            context_window: 128_000,
            ..Default::default()
        };
        let settings = default_settings();
        let result = get_model_max_output_tokens("some-model", &model, &settings, None);
        assert_eq!(result, Some(ANTHROPIC_DEFAULT_MAX_TOKENS));
    }

    #[test]
    fn test_max_tokens_non_anthropic_no_model_tokens() {
        let model = ModelInfo {
            context_window: 128_000,
            ..Default::default()
        };
        let settings = default_settings();
        let result = get_model_max_output_tokens("some-model", &model, &settings, Some(Format::OpenAi));
        assert!(result.is_none());
    }

    // ---- calculate_model_params tests ----

    #[test]
    fn test_calculate_params_openai_basic() {
        let model = basic_model();
        let settings = default_settings();
        let opts = GetModelParamsOptions {
            format: Format::OpenAi,
            model_id: "gpt-4",
            model: &model,
            settings: &settings,
            default_temperature: 0.7,
        };
        let params = calculate_model_params(opts);
        assert_eq!(params.max_tokens, Some(4096));
        assert_eq!(params.temperature, Some(0.7));
        assert!(params.reasoning.is_none());
        assert!(params.thinking.is_none());
    }

    #[test]
    fn test_calculate_params_anthropic_hybrid() {
        let model = hybrid_model();
        let settings = default_settings();
        let opts = GetModelParamsOptions {
            format: Format::Anthropic,
            model_id: "claude-3-7-sonnet-20250219",
            model: &model,
            settings: &settings,
            default_temperature: 0.5,
        };
        let params = calculate_model_params(opts);
        // Temperature should be forced to 1.0 for hybrid reasoning models
        assert_eq!(params.temperature, Some(1.0));
        // Thinking should be enabled with budget
        assert!(params.thinking.is_some());
        let thinking = params.thinking.unwrap();
        assert_eq!(thinking["type"], "enabled");
        assert!(thinking.get("budget_tokens").is_some());
        // Reasoning budget should be set
        assert!(params.reasoning_budget.is_some());
    }

    #[test]
    fn test_calculate_params_gemini_25_pro_min_tokens() {
        let model = ModelInfo {
            required_reasoning_budget: Some(true),
            supports_reasoning_budget: Some(true),
            max_tokens: Some(65_536),
            context_window: 1_048_576,
            ..Default::default()
        };
        let settings = default_settings();
        let opts = GetModelParamsOptions {
            format: Format::Gemini,
            model_id: "gemini-2.5-pro",
            model: &model,
            settings: &settings,
            default_temperature: 0.5,
        };
        let params = calculate_model_params(opts);
        // Gemini 2.5 Pro should use 128 as minimum thinking tokens
        assert!(params.reasoning_budget.unwrap() >= GEMINI_25_PRO_MIN_THINKING_TOKENS);
        // Reasoning should have thinkingBudget
        assert!(params.reasoning.is_some());
        let reasoning = params.reasoning.unwrap();
        assert!(reasoning.get("thinkingBudget").is_some());
    }

    #[test]
    fn test_calculate_params_openai_o1_no_temperature() {
        let model = ModelInfo {
            supports_reasoning_effort: Some(Value::Bool(true)),
            reasoning_effort: Some(ReasoningEffortExtended::High),
            max_tokens: Some(100_000),
            context_window: 200_000,
            ..Default::default()
        };
        let settings = ProviderSettings {
            reasoning_effort: Some(ReasoningEffortSetting::High),
            ..Default::default()
        };
        let opts = GetModelParamsOptions {
            format: Format::OpenAi,
            model_id: "o1-preview",
            model: &model,
            settings: &settings,
            default_temperature: 0.7,
        };
        let params = calculate_model_params(opts);
        // o1 models should not have temperature
        assert!(params.temperature.is_none());
    }

    #[test]
    fn test_calculate_params_openrouter_o1_pro_no_temperature() {
        let model = ModelInfo {
            supports_reasoning_effort: Some(Value::Bool(true)),
            reasoning_effort: Some(ReasoningEffortExtended::High),
            max_tokens: Some(100_000),
            context_window: 200_000,
            ..Default::default()
        };
        let settings = ProviderSettings {
            reasoning_effort: Some(ReasoningEffortSetting::High),
            ..Default::default()
        };
        let opts = GetModelParamsOptions {
            format: Format::OpenRouter,
            model_id: "openai/o1-pro",
            model: &model,
            settings: &settings,
            default_temperature: 0.7,
        };
        let params = calculate_model_params(opts);
        // o1-pro via OpenRouter should not have temperature
        assert!(params.temperature.is_none());
    }

    #[test]
    fn test_calculate_params_effort_based_reasoning() {
        let model = effort_model();
        let settings = ProviderSettings {
            reasoning_effort: Some(ReasoningEffortSetting::High),
            ..Default::default()
        };
        let opts = GetModelParamsOptions {
            format: Format::OpenAi,
            model_id: "o3-mini",
            model: &model,
            settings: &settings,
            default_temperature: 0.7,
        };
        let params = calculate_model_params(opts);
        assert_eq!(params.reasoning_effort, Some(ReasoningEffortExtended::High));
        // o3-mini should not have temperature
        assert!(params.temperature.is_none());
    }

    #[test]
    fn test_calculate_params_budget_80_percent_cap() {
        let model = ModelInfo {
            required_reasoning_budget: Some(true),
            supports_reasoning_budget: Some(true),
            max_tokens: Some(16_384),
            context_window: 200_000,
            ..Default::default()
        };
        // Set thinking tokens to something larger than 80% of max_tokens
        let settings = ProviderSettings {
            model_max_thinking_tokens: Some(16_384),
            ..Default::default()
        };
        let opts = GetModelParamsOptions {
            format: Format::Anthropic,
            model_id: "claude-3-7",
            model: &model,
            settings: &settings,
            default_temperature: 0.5,
        };
        let params = calculate_model_params(opts);
        // 80% of 16384 = 13107
        assert_eq!(params.reasoning_budget, Some(13_107));
    }

    #[test]
    fn test_calculate_params_budget_min_tokens() {
        let model = ModelInfo {
            required_reasoning_budget: Some(true),
            supports_reasoning_budget: Some(true),
            max_tokens: Some(16_384),
            context_window: 200_000,
            ..Default::default()
        };
        // Set thinking tokens below minimum (1024 for non-Gemini)
        let settings = ProviderSettings {
            model_max_thinking_tokens: Some(512),
            ..Default::default()
        };
        let opts = GetModelParamsOptions {
            format: Format::Anthropic,
            model_id: "claude-3-7",
            model: &model,
            settings: &settings,
            default_temperature: 0.5,
        };
        let params = calculate_model_params(opts);
        // Should be clamped to minimum 1024
        assert_eq!(params.reasoning_budget, Some(1024));
    }

    #[test]
    fn test_calculate_params_custom_temperature() {
        let model = basic_model();
        let settings = ProviderSettings {
            model_temperature: Some(Some(0.3)),
            ..Default::default()
        };
        let opts = GetModelParamsOptions {
            format: Format::OpenAi,
            model_id: "gpt-4",
            model: &model,
            settings: &settings,
            default_temperature: 0.7,
        };
        let params = calculate_model_params(opts);
        assert_eq!(params.temperature, Some(0.3));
    }

    #[test]
    fn test_calculate_params_verbosity() {
        let model = basic_model();
        let settings = ProviderSettings {
            verbosity: Some(VerbosityLevel::Low),
            ..Default::default()
        };
        let opts = GetModelParamsOptions {
            format: Format::OpenAi,
            model_id: "gpt-4",
            model: &model,
            settings: &settings,
            default_temperature: 0.7,
        };
        let params = calculate_model_params(opts);
        assert_eq!(params.verbosity, Some(VerbosityLevel::Low));
    }

    #[test]
    fn test_calculate_params_gemini_effort_based() {
        let model = ModelInfo {
            supports_reasoning_effort: Some(json!(["low", "medium", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            max_tokens: Some(65_536),
            context_window: 1_048_576,
            ..Default::default()
        };
        let settings = ProviderSettings {
            reasoning_effort: Some(ReasoningEffortSetting::High),
            ..Default::default()
        };
        let opts = GetModelParamsOptions {
            format: Format::Gemini,
            model_id: "gemini-3-pro-preview",
            model: &model,
            settings: &settings,
            default_temperature: 0.5,
        };
        let params = calculate_model_params(opts);
        assert!(params.reasoning.is_some());
        let reasoning = params.reasoning.unwrap();
        assert_eq!(reasoning["thinkingLevel"], "high");
    }

    #[test]
    fn test_calculate_params_disable_effort() {
        let model = effort_model();
        let settings = ProviderSettings {
            reasoning_effort: Some(ReasoningEffortSetting::Disable),
            ..Default::default()
        };
        let opts = GetModelParamsOptions {
            format: Format::OpenAi,
            model_id: "some-model",
            model: &model,
            settings: &settings,
            default_temperature: 0.7,
        };
        let params = calculate_model_params(opts);
        // "disable" should result in no reasoning effort
        assert!(params.reasoning_effort.is_none());
        assert!(params.reasoning.is_none());
    }
}

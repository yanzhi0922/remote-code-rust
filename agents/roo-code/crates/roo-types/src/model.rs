//! Model type definitions.
//!
//! Derived from `packages/types/src/model.ts`.
//! Defines model information schema and related types.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// ReasoningEffort
// ---------------------------------------------------------------------------

/// Reasoning effort levels.
///
/// Source: `packages/types/src/model.ts` — `reasoningEfforts`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffort {
    Low,
    Medium,
    High,
}

/// Extended reasoning effort (includes "none" and "minimal").
///
/// Source: `packages/types/src/model.ts` — `reasoningEffortsExtended`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffortExtended {
    None,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
}

/// Reasoning effort user setting (includes "disable").
///
/// Source: `packages/types/src/model.ts` — `reasoningEffortSettingValues`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReasoningEffortSetting {
    Disable,
    None,
    Minimal,
    Low,
    Medium,
    High,
    Xhigh,
}

// ---------------------------------------------------------------------------
// VerbosityLevel
// ---------------------------------------------------------------------------

/// Verbosity levels for model output.
///
/// Source: `packages/types/src/model.ts` — `verbosityLevels`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VerbosityLevel {
    Low,
    Medium,
    High,
}

// ---------------------------------------------------------------------------
// ServiceTier
// ---------------------------------------------------------------------------

/// Service tiers (OpenAI Responses API).
///
/// Source: `packages/types/src/model.ts` — `serviceTiers`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServiceTier {
    Default,
    Flex,
    Priority,
}

// ---------------------------------------------------------------------------
// ModelParameter
// ---------------------------------------------------------------------------

/// Model parameters that can be configured.
///
/// Source: `packages/types/src/model.ts` — `modelParameters`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelParameter {
    MaxTokens,
    Temperature,
    Reasoning,
    IncludeReasoning,
}

// ---------------------------------------------------------------------------
// LongContextPricing
// ---------------------------------------------------------------------------

/// Long context pricing configuration for a model.
///
/// Source: `packages/types/src/model.ts` — `longContextPricing` in `modelInfoSchema`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LongContextPricing {
    pub threshold_tokens: u64,
    pub input_price_multiplier: Option<f64>,
    pub output_price_multiplier: Option<f64>,
    pub cache_writes_price_multiplier: Option<f64>,
    pub cache_reads_price_multiplier: Option<f64>,
    pub applies_to_service_tiers: Option<Vec<ServiceTier>>,
}

// ---------------------------------------------------------------------------
// ModelTier
// ---------------------------------------------------------------------------

/// Service tier with pricing information.
///
/// Source: `packages/types/src/model.ts` — `tiers` in `modelInfoSchema`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelTier {
    pub name: Option<ServiceTier>,
    pub context_window: u64,
    pub input_price: Option<f64>,
    pub output_price: Option<f64>,
    pub cache_writes_price: Option<f64>,
    pub cache_reads_price: Option<f64>,
}

// ---------------------------------------------------------------------------
// ModelInfo
// ---------------------------------------------------------------------------

/// Information about an AI model.
///
/// Source: `packages/types/src/model.ts` — `modelInfoSchema`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    /// Maximum tokens the model can output in a single response.
    pub max_tokens: Option<u64>,
    /// Maximum thinking/reasoning tokens.
    pub max_thinking_tokens: Option<u64>,
    /// Context window size in tokens.
    pub context_window: u64,
    /// Whether the model supports image inputs.
    #[serde(default)]
    pub supports_images: Option<bool>,
    /// Whether the model supports prompt caching.
    #[serde(default)]
    pub supports_prompt_cache: bool,
    /// Default prompt cache retention policy.
    pub prompt_cache_retention: Option<String>,
    /// Whether the model supports an output verbosity parameter.
    pub supports_verbosity: Option<bool>,
    /// Whether the model supports a reasoning budget.
    pub supports_reasoning_budget: Option<bool>,
    /// Whether the model supports simple on/off binary reasoning.
    pub supports_reasoning_binary: Option<bool>,
    /// Whether the model supports the temperature parameter.
    pub supports_temperature: Option<bool>,
    /// Default temperature for the model.
    pub default_temperature: Option<f64>,
    /// Whether a reasoning budget is required.
    pub required_reasoning_budget: Option<bool>,
    /// Whether the model supports reasoning effort.
    /// Can be a boolean or an array of allowed effort values.
    pub supports_reasoning_effort: Option<serde_json::Value>,
    /// Whether reasoning effort is required.
    pub required_reasoning_effort: Option<bool>,
    /// Whether to preserve reasoning in the conversation.
    pub preserve_reasoning: Option<bool>,
    /// List of supported parameters.
    pub supported_parameters: Option<Vec<ModelParameter>>,
    /// Cost per million input tokens (in USD).
    pub input_price: Option<f64>,
    /// Cost per million output tokens (in USD).
    pub output_price: Option<f64>,
    /// Cost per million cache write tokens (in USD).
    pub cache_writes_price: Option<f64>,
    /// Cost per million cache read tokens (in USD).
    pub cache_reads_price: Option<f64>,
    /// Long context pricing configuration.
    pub long_context_pricing: Option<LongContextPricing>,
    /// Description of the model.
    pub description: Option<String>,
    /// Default reasoning effort value.
    pub reasoning_effort: Option<ReasoningEffortExtended>,
    /// Minimum tokens per cache point.
    pub min_tokens_per_cache_point: Option<u64>,
    /// Maximum cache points.
    pub max_cache_points: Option<u64>,
    /// Cacheable fields.
    pub cachable_fields: Option<Vec<String>>,
    /// Whether the model is deprecated.
    pub deprecated: Option<bool>,
    /// Whether the model should hide vendor/company identity.
    pub is_stealth_model: Option<bool>,
    /// Whether the model is free (no cost).
    pub is_free: Option<bool>,
    /// Tools excluded from native protocol.
    pub excluded_tools: Option<Vec<String>>,
    /// Tools included for native protocol.
    pub included_tools: Option<Vec<String>>,
    /// Service tiers with pricing information.
    pub tiers: Option<Vec<ModelTier>>,
}

impl Default for ModelInfo {
    fn default() -> Self {
        Self {
            max_tokens: None,
            max_thinking_tokens: None,
            context_window: 0,
            supports_images: None,
            supports_prompt_cache: false,
            prompt_cache_retention: None,
            supports_verbosity: None,
            supports_reasoning_budget: None,
            supports_reasoning_binary: None,
            supports_temperature: None,
            default_temperature: None,
            required_reasoning_budget: None,
            supports_reasoning_effort: None,
            required_reasoning_effort: None,
            preserve_reasoning: None,
            supported_parameters: None,
            input_price: None,
            output_price: None,
            cache_writes_price: None,
            cache_reads_price: None,
            long_context_pricing: None,
            description: None,
            reasoning_effort: None,
            min_tokens_per_cache_point: None,
            max_cache_points: None,
            cachable_fields: None,
            deprecated: None,
            is_stealth_model: None,
            is_free: None,
            excluded_tools: None,
            included_tools: None,
            tiers: None,
        }
    }
}

// ---------------------------------------------------------------------------
// ModelRecord
// ---------------------------------------------------------------------------

/// A map of model ID to model info.
///
/// Source: `packages/types/src/model.ts`
pub type ModelRecord = std::collections::HashMap<String, ModelInfo>;

/// Router models — models fetched dynamically from providers.
///
/// Source: `packages/types/src/model.ts` — `RouterModels`
pub type RouterModels = std::collections::HashMap<String, ModelRecord>;

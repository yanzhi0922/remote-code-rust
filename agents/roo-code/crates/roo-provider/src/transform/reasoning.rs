//! Reasoning parameter computation for various API providers.
//!
//! Derived from `src/api/transform/reasoning.ts` and `src/shared/api.ts`.
//! Computes provider-specific reasoning/thinking parameters based on model
//! capabilities and user settings.

use serde_json::{json, Value};

use roo_types::model::{ModelInfo, ReasoningEffortExtended, ReasoningEffortSetting};
use roo_types::provider_settings::ProviderSettings;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Valid Gemini thinking levels for effort-based reasoning.
pub const GEMINI_THINKING_LEVELS: &[&str] = &["minimal", "low", "medium", "high"];

/// Default max output tokens for hybrid reasoning models.
pub const DEFAULT_HYBRID_REASONING_MODEL_MAX_TOKENS: u64 = 16_384;

/// Default thinking tokens for hybrid reasoning models.
pub const DEFAULT_HYBRID_REASONING_MODEL_THINKING_TOKENS: u64 = 8_192;

/// Minimum thinking tokens for Gemini 2.5 Pro models.
pub const GEMINI_25_PRO_MIN_THINKING_TOKENS: u64 = 128;

/// Default max output tokens for Anthropic models.
pub const ANTHROPIC_DEFAULT_MAX_TOKENS: u64 = 8192;

// ---------------------------------------------------------------------------
// Conversion helpers
// ---------------------------------------------------------------------------

/// Convert a [`ReasoningEffortExtended`] to its lowercase string representation.
fn effort_extended_to_str(e: ReasoningEffortExtended) -> &'static str {
    match e {
        ReasoningEffortExtended::None => "none",
        ReasoningEffortExtended::Minimal => "minimal",
        ReasoningEffortExtended::Low => "low",
        ReasoningEffortExtended::Medium => "medium",
        ReasoningEffortExtended::High => "high",
        ReasoningEffortExtended::Xhigh => "xhigh",
    }
}

/// Convert a [`ReasoningEffortSetting`] to its lowercase string representation.
fn effort_setting_to_str(e: ReasoningEffortSetting) -> &'static str {
    match e {
        ReasoningEffortSetting::Disable => "disable",
        ReasoningEffortSetting::None => "none",
        ReasoningEffortSetting::Minimal => "minimal",
        ReasoningEffortSetting::Low => "low",
        ReasoningEffortSetting::Medium => "medium",
        ReasoningEffortSetting::High => "high",
        ReasoningEffortSetting::Xhigh => "xhigh",
    }
}

/// Convert a [`ReasoningEffortExtended`] to a [`ReasoningEffortSetting`].
///
/// This is needed because model defaults use [`ReasoningEffortExtended`] while
/// settings use [`ReasoningEffortSetting`] (which adds the `Disable` variant).
pub fn effort_extended_to_setting(e: ReasoningEffortExtended) -> ReasoningEffortSetting {
    match e {
        ReasoningEffortExtended::None => ReasoningEffortSetting::None,
        ReasoningEffortExtended::Minimal => ReasoningEffortSetting::Minimal,
        ReasoningEffortExtended::Low => ReasoningEffortSetting::Low,
        ReasoningEffortExtended::Medium => ReasoningEffortSetting::Medium,
        ReasoningEffortExtended::High => ReasoningEffortSetting::High,
        ReasoningEffortExtended::Xhigh => ReasoningEffortSetting::Xhigh,
    }
}

/// Convert a [`ReasoningEffortSetting`] to a [`ReasoningEffortExtended`].
///
/// Returns `None` for the `Disable` variant since it has no extended equivalent.
pub fn effort_setting_to_extended(e: ReasoningEffortSetting) -> Option<ReasoningEffortExtended> {
    match e {
        ReasoningEffortSetting::Disable => None,
        ReasoningEffortSetting::None => Some(ReasoningEffortExtended::None),
        ReasoningEffortSetting::Minimal => Some(ReasoningEffortExtended::Minimal),
        ReasoningEffortSetting::Low => Some(ReasoningEffortExtended::Low),
        ReasoningEffortSetting::Medium => Some(ReasoningEffortExtended::Medium),
        ReasoningEffortSetting::High => Some(ReasoningEffortExtended::High),
        ReasoningEffortSetting::Xhigh => Some(ReasoningEffortExtended::Xhigh),
    }
}

// ---------------------------------------------------------------------------
// Helpers: should_use_reasoning_*
// ---------------------------------------------------------------------------

/// Determine whether a reasoning *budget* (token count) should be used.
///
/// Returns `true` when the model requires a reasoning budget, or when it
/// supports one and the user has enabled reasoning effort in settings.
///
/// # Examples
///
/// ```
/// # use roo_provider::transform::reasoning::should_use_reasoning_budget;
/// # use roo_types::model::ModelInfo;
/// # use roo_types::provider_settings::ProviderSettings;
/// let model = ModelInfo { required_reasoning_budget: Some(true), ..Default::default() };
/// let settings = ProviderSettings::default();
/// assert!(should_use_reasoning_budget(&model, &settings));
/// ```
pub fn should_use_reasoning_budget(model: &ModelInfo, settings: &ProviderSettings) -> bool {
    model.required_reasoning_budget.unwrap_or(false)
        || (model.supports_reasoning_budget.unwrap_or(false)
            && settings.enable_reasoning_effort.unwrap_or(false))
}

/// Determine whether a reasoning *effort* level should be used.
///
/// The logic mirrors the TypeScript implementation:
/// 1. Explicit off switch (`enable_reasoning_effort == Some(false)`) → `false`
/// 2. "disable" effort → `false`
/// 3. Array capability → `true` only if selected effort is in the array
/// 4. Boolean `true` capability → require a selected effort
/// 5. No explicit capability → only allow when model has a default effort
pub fn should_use_reasoning_effort(model: &ModelInfo, settings: &ProviderSettings) -> bool {
    // Explicit off switch
    if settings.enable_reasoning_effort == Some(false) {
        return false;
    }

    // Resolve the selected effort from settings or model default
    let selected_effort = settings
        .reasoning_effort
        .or_else(|| model.reasoning_effort.map(effort_extended_to_setting));

    // "disable" explicitly omits reasoning
    if selected_effort == Some(ReasoningEffortSetting::Disable) {
        return false;
    }

    let cap = &model.supports_reasoning_effort;

    // Capability array: use only if selected is included
    if let Some(Value::Array(arr)) = cap {
        if let Some(effort) = &selected_effort {
            let effort_str = effort_setting_to_str(*effort);
            return arr.iter().any(|v| v.as_str() == Some(effort_str));
        }
        return false;
    }

    // Boolean capability: true → require a selected effort
    if let Some(Value::Bool(true)) = cap {
        return selected_effort.is_some();
    }

    // Not explicitly supported: only allow when model has a default effort
    model.reasoning_effort.is_some()
}

// ---------------------------------------------------------------------------
// Gemini thinking level validation
// ---------------------------------------------------------------------------

/// Check whether a string is a valid Gemini thinking level.
///
/// Valid levels are: `"minimal"`, `"low"`, `"medium"`, `"high"`.
pub fn is_gemini_thinking_level(value: &str) -> bool {
    GEMINI_THINKING_LEVELS.contains(&value)
}

// ---------------------------------------------------------------------------
// Options struct
// ---------------------------------------------------------------------------

/// Options for computing provider-specific reasoning parameters.
///
/// This struct is passed to each provider-specific reasoning function and
/// contains all the information needed to compute the correct parameters.
#[derive(Debug, Clone)]
pub struct GetModelReasoningOptions<'a> {
    /// The model information describing capabilities and defaults.
    pub model: &'a ModelInfo,
    /// User-specified reasoning budget (token count).
    pub reasoning_budget: Option<u64>,
    /// User-specified reasoning effort level (may include `Disable`).
    pub reasoning_effort: Option<ReasoningEffortSetting>,
    /// Provider settings including user preferences.
    pub settings: &'a ProviderSettings,
}

// ---------------------------------------------------------------------------
// Provider-specific reasoning functions
// ---------------------------------------------------------------------------

/// Compute Anthropic reasoning (thinking) parameters.
///
/// Returns `Some(Value)` with `{ "type": "enabled", "budget_tokens": N }`
/// when the model supports reasoning budgets, otherwise `None`.
///
/// # Examples
///
/// ```
/// # use roo_provider::transform::reasoning::*;
/// # use roo_types::model::ModelInfo;
/// # use roo_types::provider_settings::ProviderSettings;
/// let model = ModelInfo {
///     required_reasoning_budget: Some(true),
///     ..Default::default()
/// };
/// let settings = ProviderSettings::default();
/// let opts = GetModelReasoningOptions {
///     model: &model,
///     reasoning_budget: Some(4096),
///     reasoning_effort: None,
///     settings: &settings,
/// };
/// let result = get_anthropic_reasoning(&opts).unwrap();
/// assert_eq!(result["type"], "enabled");
/// assert_eq!(result["budget_tokens"], 4096);
/// ```
pub fn get_anthropic_reasoning(opts: &GetModelReasoningOptions) -> Option<Value> {
    if should_use_reasoning_budget(opts.model, opts.settings) {
        opts.reasoning_budget.map(|budget| {
            json!({
                "type": "enabled",
                "budget_tokens": budget
            })
        })
    } else {
        None
    }
}

/// Compute OpenAI reasoning parameters.
///
/// Returns `Some(Value)` with `{ "reasoning_effort": "low"|"medium"|"high" }`
/// when the model supports reasoning effort and a valid effort is selected.
/// Returns `None` if the effort is `"disable"` or not set.
pub fn get_openai_reasoning(opts: &GetModelReasoningOptions) -> Option<Value> {
    if !should_use_reasoning_effort(opts.model, opts.settings) {
        return None;
    }

    match opts.reasoning_effort {
        Some(ReasoningEffortSetting::Disable) | None => None,
        Some(effort) => {
            let effort_str = effort_setting_to_str(effort);
            Some(json!({ "reasoning_effort": effort_str }))
        }
    }
}

/// Compute Gemini reasoning (thinkingConfig) parameters.
///
/// Budget-based models return `{ "thinkingBudget": N, "includeThoughts": true }`.
/// Effort-based models return `{ "thinkingLevel": "low", "includeThoughts": true }`.
///
/// For effort-based models, the selected effort is validated against the model's
/// supported efforts. If the selected effort is not supported, the model's default
/// effort is used as a fallback.
pub fn get_gemini_reasoning(opts: &GetModelReasoningOptions) -> Option<Value> {
    // Budget-based (2.5) models: use thinkingBudget, not thinkingLevel.
    if should_use_reasoning_budget(opts.model, opts.settings) {
        return opts.reasoning_budget.map(|budget| {
            json!({
                "thinkingBudget": budget,
                "includeThoughts": true
            })
        });
    }

    // For effort-based Gemini models, rely directly on the selected effort value.
    // We intentionally ignore enableReasoningEffort here so that explicitly chosen
    // efforts in the UI always translate into a thinkingConfig.
    let selected_effort = opts
        .settings
        .reasoning_effort
        .or_else(|| opts.model.reasoning_effort.map(effort_extended_to_setting));

    // Respect "off" / unset semantics from the effort selector itself.
    let selected = match &selected_effort {
        None => return None,
        Some(ReasoningEffortSetting::Disable) => return None,
        Some(e) => *e,
    };

    let effort_str = effort_setting_to_str(selected);

    // Validate that the selected effort is supported by this specific model.
    // e.g. gemini-3-pro-preview only supports ["low", "high"] — sending
    // "medium" (carried over from a different model's settings) causes errors.
    let effort_to_use = if let Some(Value::Array(arr)) = &opts.model.supports_reasoning_effort {
        if is_gemini_thinking_level(effort_str)
            && !arr.iter().any(|v| v.as_str() == Some(effort_str))
        {
            // Fall back to model default
            opts.model
                .reasoning_effort
                .map(effort_extended_to_str)
                .unwrap_or("")
                .to_string()
        } else {
            effort_str.to_string()
        }
    } else {
        effort_str.to_string()
    };

    // Effort-based models on Google GenAI support minimal/low/medium/high levels.
    if effort_to_use.is_empty() || !is_gemini_thinking_level(&effort_to_use) {
        return None;
    }

    Some(json!({
        "thinkingLevel": effort_to_use,
        "includeThoughts": true
    }))
}

/// Compute OpenRouter reasoning parameters.
///
/// Budget-based models return `{ "max_tokens": N }`.
/// Effort-based models return `{ "effort": "low"|"medium"|"high" }`.
/// Returns `None` if reasoning is not applicable or the effort is `"disable"`.
pub fn get_openrouter_reasoning(opts: &GetModelReasoningOptions) -> Option<Value> {
    if should_use_reasoning_budget(opts.model, opts.settings) {
        opts.reasoning_budget.map(|budget| json!({ "max_tokens": budget }))
    } else if should_use_reasoning_effort(opts.model, opts.settings) {
        match opts.reasoning_effort {
            Some(ReasoningEffortSetting::Disable) | None => None,
            Some(effort) => {
                let effort_str = effort_setting_to_str(effort);
                Some(json!({ "effort": effort_str }))
            }
        }
    } else {
        None
    }
}

/// Compute Roo-specific reasoning parameters.
///
/// Returns `{ "enabled": bool, "effort": "..." }` for Roo models that
/// support reasoning effort. The logic handles:
/// - Models with required reasoning effort
/// - Explicit off switch from settings
/// - Default "off" when no effort is selected
/// - "disable" and "minimal" sentinels that omit the reasoning field
pub fn get_roo_reasoning(opts: &GetModelReasoningOptions) -> Option<Value> {
    // Check if model supports reasoning effort
    let cap = &opts.model.supports_reasoning_effort;
    let supports = match cap {
        None | Some(Value::Bool(false)) | Some(Value::Null) => false,
        Some(Value::Bool(true)) => true,
        Some(Value::Array(_)) => true,
        _ => false,
    };

    if !supports {
        return None;
    }

    // Required reasoning effort: honor the provided effort if valid
    if opts.model.required_reasoning_effort.unwrap_or(false) {
        if let Some(effort) = opts.reasoning_effort {
            if effort != ReasoningEffortSetting::Disable
                && effort != ReasoningEffortSetting::Minimal
            {
                let effort_str = effort_setting_to_str(effort);
                return Some(json!({ "enabled": true, "effort": effort_str }));
            }
        }
        return Some(json!({ "enabled": true }));
    }

    // Explicit off switch from settings
    if opts.settings.enable_reasoning_effort == Some(false) {
        return Some(json!({ "enabled": false }));
    }

    // No effort selected → treat as explicit "off"
    if opts.reasoning_effort.is_none() {
        return Some(json!({ "enabled": false }));
    }

    // "disable" → omit the reasoning field entirely
    if opts.reasoning_effort == Some(ReasoningEffortSetting::Disable) {
        return None;
    }

    // "minimal" → omit the reasoning field entirely
    if opts.reasoning_effort == Some(ReasoningEffortSetting::Minimal) {
        return None;
    }

    // Enable with the selected effort
    if let Some(effort) = opts.reasoning_effort {
        let effort_str = effort_setting_to_str(effort);
        return Some(json!({ "enabled": true, "effort": effort_str }));
    }

    None
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a basic model with reasoning budget support.
    fn budget_model() -> ModelInfo {
        ModelInfo {
            required_reasoning_budget: Some(true),
            supports_reasoning_budget: Some(true),
            context_window: 200_000,
            ..Default::default()
        }
    }

    /// Helper to create a basic model with reasoning effort support (boolean).
    fn effort_model_bool() -> ModelInfo {
        ModelInfo {
            supports_reasoning_effort: Some(Value::Bool(true)),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            context_window: 128_000,
            ..Default::default()
        }
    }

    /// Helper to create a model with reasoning effort support (array).
    fn effort_model_array() -> ModelInfo {
        ModelInfo {
            supports_reasoning_effort: Some(json!(["low", "medium", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            context_window: 128_000,
            ..Default::default()
        }
    }

    fn default_settings() -> ProviderSettings {
        ProviderSettings::default()
    }

    // ---- should_use_reasoning_budget tests ----

    #[test]
    fn test_should_use_reasoning_budget_required() {
        let model = budget_model();
        let settings = default_settings();
        assert!(should_use_reasoning_budget(&model, &settings));
    }

    #[test]
    fn test_should_use_reasoning_budget_supported_and_enabled() {
        let model = ModelInfo {
            supports_reasoning_budget: Some(true),
            context_window: 128_000,
            ..Default::default()
        };
        let settings = ProviderSettings {
            enable_reasoning_effort: Some(true),
            ..Default::default()
        };
        assert!(should_use_reasoning_budget(&model, &settings));
    }

    #[test]
    fn test_should_use_reasoning_budget_supported_but_disabled() {
        let model = ModelInfo {
            supports_reasoning_budget: Some(true),
            context_window: 128_000,
            ..Default::default()
        };
        let settings = ProviderSettings {
            enable_reasoning_effort: Some(false),
            ..Default::default()
        };
        assert!(!should_use_reasoning_budget(&model, &settings));
    }

    #[test]
    fn test_should_use_reasoning_budget_not_supported() {
        let model = ModelInfo {
            context_window: 128_000,
            ..Default::default()
        };
        let settings = default_settings();
        assert!(!should_use_reasoning_budget(&model, &settings));
    }

    // ---- should_use_reasoning_effort tests ----

    #[test]
    fn test_should_use_reasoning_effort_explicit_off() {
        let model = effort_model_bool();
        let settings = ProviderSettings {
            enable_reasoning_effort: Some(false),
            ..Default::default()
        };
        assert!(!should_use_reasoning_effort(&model, &settings));
    }

    #[test]
    fn test_should_use_reasoning_effort_disable_setting() {
        let model = effort_model_bool();
        let settings = ProviderSettings {
            reasoning_effort: Some(ReasoningEffortSetting::Disable),
            ..Default::default()
        };
        assert!(!should_use_reasoning_effort(&model, &settings));
    }

    #[test]
    fn test_should_use_reasoning_effort_array_includes_selected() {
        let model = effort_model_array();
        let settings = ProviderSettings {
            reasoning_effort: Some(ReasoningEffortSetting::Low),
            ..Default::default()
        };
        assert!(should_use_reasoning_effort(&model, &settings));
    }

    #[test]
    fn test_should_use_reasoning_effort_array_excludes_selected() {
        let model = ModelInfo {
            supports_reasoning_effort: Some(json!(["low", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            context_window: 128_000,
            ..Default::default()
        };
        let settings = ProviderSettings {
            reasoning_effort: Some(ReasoningEffortSetting::Medium),
            ..Default::default()
        };
        // "medium" is not in ["low", "high"], so should return false
        assert!(!should_use_reasoning_effort(&model, &settings));
    }

    #[test]
    fn test_should_use_reasoning_effort_bool_requires_selected() {
        let model = ModelInfo {
            supports_reasoning_effort: Some(Value::Bool(true)),
            context_window: 128_000,
            ..Default::default() // no reasoning_effort default
        };
        let settings = default_settings();
        // No effort selected → false
        assert!(!should_use_reasoning_effort(&model, &settings));
    }

    #[test]
    fn test_should_use_reasoning_effort_model_default() {
        let model = ModelInfo {
            reasoning_effort: Some(ReasoningEffortExtended::High),
            context_window: 128_000,
            ..Default::default()
        };
        let settings = default_settings();
        // Model has default effort → true
        assert!(should_use_reasoning_effort(&model, &settings));
    }

    // ---- get_anthropic_reasoning tests ----

    #[test]
    fn test_get_anthropic_reasoning_with_budget() {
        let model = budget_model();
        let settings = default_settings();
        let opts = GetModelReasoningOptions {
            model: &model,
            reasoning_budget: Some(4096),
            reasoning_effort: None,
            settings: &settings,
        };
        let result = get_anthropic_reasoning(&opts).unwrap();
        assert_eq!(result["type"], "enabled");
        assert_eq!(result["budget_tokens"], 4096);
    }

    #[test]
    fn test_get_anthropic_reasoning_no_budget() {
        let model = ModelInfo {
            context_window: 128_000,
            ..Default::default()
        };
        let settings = default_settings();
        let opts = GetModelReasoningOptions {
            model: &model,
            reasoning_budget: Some(4096),
            reasoning_effort: None,
            settings: &settings,
        };
        assert!(get_anthropic_reasoning(&opts).is_none());
    }

    #[test]
    fn test_get_anthropic_reasoning_no_budget_value() {
        let model = budget_model();
        let settings = default_settings();
        let opts = GetModelReasoningOptions {
            model: &model,
            reasoning_budget: None,
            reasoning_effort: None,
            settings: &settings,
        };
        assert!(get_anthropic_reasoning(&opts).is_none());
    }

    // ---- get_openai_reasoning tests ----

    #[test]
    fn test_get_openai_reasoning_with_effort() {
        let model = effort_model_bool();
        let settings = ProviderSettings {
            reasoning_effort: Some(ReasoningEffortSetting::High),
            ..Default::default()
        };
        let opts = GetModelReasoningOptions {
            model: &model,
            reasoning_budget: None,
            reasoning_effort: Some(ReasoningEffortSetting::High),
            settings: &settings,
        };
        let result = get_openai_reasoning(&opts).unwrap();
        assert_eq!(result["reasoning_effort"], "high");
    }

    #[test]
    fn test_get_openai_reasoning_disable() {
        let model = effort_model_bool();
        let settings = default_settings();
        let opts = GetModelReasoningOptions {
            model: &model,
            reasoning_budget: None,
            reasoning_effort: Some(ReasoningEffortSetting::Disable),
            settings: &settings,
        };
        assert!(get_openai_reasoning(&opts).is_none());
    }

    #[test]
    fn test_get_openai_reasoning_no_effort() {
        let model = effort_model_bool();
        let settings = default_settings();
        let opts = GetModelReasoningOptions {
            model: &model,
            reasoning_budget: None,
            reasoning_effort: None,
            settings: &settings,
        };
        assert!(get_openai_reasoning(&opts).is_none());
    }

    // ---- get_gemini_reasoning tests ----

    #[test]
    fn test_get_gemini_reasoning_budget_based() {
        let model = budget_model();
        let settings = default_settings();
        let opts = GetModelReasoningOptions {
            model: &model,
            reasoning_budget: Some(8192),
            reasoning_effort: None,
            settings: &settings,
        };
        let result = get_gemini_reasoning(&opts).unwrap();
        assert_eq!(result["thinkingBudget"], 8192);
        assert_eq!(result["includeThoughts"], true);
    }

    #[test]
    fn test_get_gemini_reasoning_effort_based() {
        let model = ModelInfo {
            supports_reasoning_effort: Some(json!(["low", "medium", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Medium),
            context_window: 128_000,
            ..Default::default()
        };
        let settings = ProviderSettings {
            reasoning_effort: Some(ReasoningEffortSetting::High),
            ..Default::default()
        };
        let opts = GetModelReasoningOptions {
            model: &model,
            reasoning_budget: None,
            reasoning_effort: Some(ReasoningEffortSetting::High),
            settings: &settings,
        };
        let result = get_gemini_reasoning(&opts).unwrap();
        assert_eq!(result["thinkingLevel"], "high");
        assert_eq!(result["includeThoughts"], true);
    }

    #[test]
    fn test_get_gemini_reasoning_unsupported_effort_fallback() {
        let model = ModelInfo {
            supports_reasoning_effort: Some(json!(["low", "high"])),
            reasoning_effort: Some(ReasoningEffortExtended::Low),
            context_window: 128_000,
            ..Default::default()
        };
        let settings = ProviderSettings {
            reasoning_effort: Some(ReasoningEffortSetting::Medium),
            ..Default::default()
        };
        let opts = GetModelReasoningOptions {
            model: &model,
            reasoning_budget: None,
            reasoning_effort: Some(ReasoningEffortSetting::Medium),
            settings: &settings,
        };
        let result = get_gemini_reasoning(&opts).unwrap();
        // "medium" is not in ["low", "high"], so falls back to model default "low"
        assert_eq!(result["thinkingLevel"], "low");
    }

    #[test]
    fn test_get_gemini_reasoning_disable() {
        let model = effort_model_array();
        let settings = ProviderSettings {
            reasoning_effort: Some(ReasoningEffortSetting::Disable),
            ..Default::default()
        };
        let opts = GetModelReasoningOptions {
            model: &model,
            reasoning_budget: None,
            reasoning_effort: Some(ReasoningEffortSetting::Disable),
            settings: &settings,
        };
        assert!(get_gemini_reasoning(&opts).is_none());
    }

    // ---- get_openrouter_reasoning tests ----

    #[test]
    fn test_get_openrouter_reasoning_budget() {
        let model = budget_model();
        let settings = default_settings();
        let opts = GetModelReasoningOptions {
            model: &model,
            reasoning_budget: Some(4096),
            reasoning_effort: None,
            settings: &settings,
        };
        let result = get_openrouter_reasoning(&opts).unwrap();
        assert_eq!(result["max_tokens"], 4096);
    }

    #[test]
    fn test_get_openrouter_reasoning_effort() {
        let model = effort_model_bool();
        let settings = ProviderSettings {
            reasoning_effort: Some(ReasoningEffortSetting::Medium),
            ..Default::default()
        };
        let opts = GetModelReasoningOptions {
            model: &model,
            reasoning_budget: None,
            reasoning_effort: Some(ReasoningEffortSetting::Medium),
            settings: &settings,
        };
        let result = get_openrouter_reasoning(&opts).unwrap();
        assert_eq!(result["effort"], "medium");
    }

    #[test]
    fn test_get_openrouter_reasoning_disable() {
        let model = effort_model_bool();
        let settings = default_settings();
        let opts = GetModelReasoningOptions {
            model: &model,
            reasoning_budget: None,
            reasoning_effort: Some(ReasoningEffortSetting::Disable),
            settings: &settings,
        };
        assert!(get_openrouter_reasoning(&opts).is_none());
    }

    // ---- get_roo_reasoning tests ----

    #[test]
    fn test_get_roo_reasoning_required_with_valid_effort() {
        let model = ModelInfo {
            supports_reasoning_effort: Some(Value::Bool(true)),
            required_reasoning_effort: Some(true),
            context_window: 128_000,
            ..Default::default()
        };
        let settings = default_settings();
        let opts = GetModelReasoningOptions {
            model: &model,
            reasoning_budget: None,
            reasoning_effort: Some(ReasoningEffortSetting::High),
            settings: &settings,
        };
        let result = get_roo_reasoning(&opts).unwrap();
        assert_eq!(result["enabled"], true);
        assert_eq!(result["effort"], "high");
    }

    #[test]
    fn test_get_roo_reasoning_required_with_disable() {
        let model = ModelInfo {
            supports_reasoning_effort: Some(Value::Bool(true)),
            required_reasoning_effort: Some(true),
            context_window: 128_000,
            ..Default::default()
        };
        let settings = default_settings();
        let opts = GetModelReasoningOptions {
            model: &model,
            reasoning_budget: None,
            reasoning_effort: Some(ReasoningEffortSetting::Disable),
            settings: &settings,
        };
        let result = get_roo_reasoning(&opts).unwrap();
        assert_eq!(result["enabled"], true);
        assert!(result.get("effort").is_none());
    }

    #[test]
    fn test_get_roo_reasoning_explicit_off() {
        let model = effort_model_bool();
        let settings = ProviderSettings {
            enable_reasoning_effort: Some(false),
            ..Default::default()
        };
        let opts = GetModelReasoningOptions {
            model: &model,
            reasoning_budget: None,
            reasoning_effort: Some(ReasoningEffortSetting::High),
            settings: &settings,
        };
        let result = get_roo_reasoning(&opts).unwrap();
        assert_eq!(result["enabled"], false);
    }

    #[test]
    fn test_get_roo_reasoning_no_effort_is_off() {
        let model = effort_model_bool();
        let settings = default_settings();
        let opts = GetModelReasoningOptions {
            model: &model,
            reasoning_budget: None,
            reasoning_effort: None,
            settings: &settings,
        };
        let result = get_roo_reasoning(&opts).unwrap();
        assert_eq!(result["enabled"], false);
    }

    #[test]
    fn test_get_roo_reasoning_disable_returns_none() {
        let model = effort_model_bool();
        let settings = default_settings();
        let opts = GetModelReasoningOptions {
            model: &model,
            reasoning_budget: None,
            reasoning_effort: Some(ReasoningEffortSetting::Disable),
            settings: &settings,
        };
        assert!(get_roo_reasoning(&opts).is_none());
    }

    #[test]
    fn test_get_roo_reasoning_minimal_returns_none() {
        let model = effort_model_bool();
        let settings = default_settings();
        let opts = GetModelReasoningOptions {
            model: &model,
            reasoning_budget: None,
            reasoning_effort: Some(ReasoningEffortSetting::Minimal),
            settings: &settings,
        };
        assert!(get_roo_reasoning(&opts).is_none());
    }

    #[test]
    fn test_get_roo_reasoning_with_effort() {
        let model = effort_model_bool();
        let settings = default_settings();
        let opts = GetModelReasoningOptions {
            model: &model,
            reasoning_budget: None,
            reasoning_effort: Some(ReasoningEffortSetting::Medium),
            settings: &settings,
        };
        let result = get_roo_reasoning(&opts).unwrap();
        assert_eq!(result["enabled"], true);
        assert_eq!(result["effort"], "medium");
    }

    // ---- is_gemini_thinking_level tests ----

    #[test]
    fn test_is_gemini_thinking_level_valid() {
        assert!(is_gemini_thinking_level("minimal"));
        assert!(is_gemini_thinking_level("low"));
        assert!(is_gemini_thinking_level("medium"));
        assert!(is_gemini_thinking_level("high"));
    }

    #[test]
    fn test_is_gemini_thinking_level_invalid() {
        assert!(!is_gemini_thinking_level("none"));
        assert!(!is_gemini_thinking_level("xhigh"));
        assert!(!is_gemini_thinking_level("disable"));
        assert!(!is_gemini_thinking_level(""));
    }

    // ---- conversion helper tests ----

    #[test]
    fn test_effort_extended_roundtrip() {
        let variants = [
            ReasoningEffortExtended::None,
            ReasoningEffortExtended::Minimal,
            ReasoningEffortExtended::Low,
            ReasoningEffortExtended::Medium,
            ReasoningEffortExtended::High,
            ReasoningEffortExtended::Xhigh,
        ];
        for v in variants {
            let setting = effort_extended_to_setting(v);
            let back = effort_setting_to_extended(setting).unwrap();
            assert_eq!(v, back);
        }
    }

    #[test]
    fn test_effort_setting_disable_to_extended_is_none() {
        assert!(effort_setting_to_extended(ReasoningEffortSetting::Disable).is_none());
    }
}

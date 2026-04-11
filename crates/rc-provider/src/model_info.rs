//! Model context-window information database.
//!
//! Provides [`ModelInfo`] and [`get_model_info`] for looking up the maximum
//! context window and output token limits of supported LLM models.  The lookup
//! uses fuzzy, case-insensitive matching so that versioned model names (e.g.
//! `gpt-4o-2024-05-13`) are resolved correctly.

// ---------------------------------------------------------------------------
// ModelInfo
// ---------------------------------------------------------------------------

/// Context-window metadata for a single model family variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelInfo {
    /// Maximum input context length in tokens.
    pub max_context: u64,
    /// Maximum output tokens the model can generate in a single response.
    pub max_output: u64,
    /// Model family identifier (e.g. `"glm"`, `"openai"`, `"anthropic"`).
    pub family: &'static str,
}

// ---------------------------------------------------------------------------
// Public lookup
// ---------------------------------------------------------------------------

/// Return the [`ModelInfo`] for the given model name.
///
/// Matching is **fuzzy** and case-insensitive: the function checks for known
/// substrings in the lowercased model name.  More specific patterns are tested
/// first so that, for example, `"glm-4-airx"` is not accidentally matched by
/// the broader `"glm-4"` rule.
///
/// # Fallback
///
/// If no known pattern matches, a conservative default of **128 K** context
/// and **4 K** output is returned with `family = "unknown"`.
#[must_use]
pub fn get_model_info(model: &str) -> ModelInfo {
    let lower = model.to_lowercase();

    // --- GLM series (order matters: more specific first) -------------------

    if lower.contains("glm-4-long") {
        return ModelInfo { max_context: 1_000_000, max_output: 4_096, family: "glm" };
    }
    if lower.contains("glm-4-airx") {
        return ModelInfo { max_context: 8_192, max_output: 4_096, family: "glm" };
    }
    // glm-4v / glm-4v-plus both share the same limits
    if lower.contains("glm-4v") {
        return ModelInfo { max_context: 8_192, max_output: 4_096, family: "glm" };
    }
    // Catch-all for glm-4, glm-4-plus, glm-4-air, glm-4-flash, glm-4-flashx
    if lower.contains("glm-4") || lower.contains("glm4") {
        return ModelInfo { max_context: 128_000, max_output: 4_096, family: "glm" };
    }

    // --- OpenAI series -----------------------------------------------------

    // o3-mini, o1, o1-preview all share 200 K / 100 K
    if lower.contains("o3-mini") || lower.contains("o1-preview") || lower == "o1" {
        return ModelInfo { max_context: 200_000, max_output: 100_000, family: "openai" };
    }
    if lower.contains("o1-mini") {
        return ModelInfo { max_context: 128_000, max_output: 65_536, family: "openai" };
    }
    // gpt-4o (including dated snapshots like gpt-4o-2024-05-13) and gpt-4-turbo
    if lower.contains("gpt-4o") || lower.contains("gpt-4-turbo") {
        return ModelInfo { max_context: 128_000, max_output: 16_384, family: "openai" };
    }
    if lower.contains("gpt-3.5") {
        return ModelInfo { max_context: 16_385, max_output: 4_096, family: "openai" };
    }

    // --- Anthropic series --------------------------------------------------

    // claude-3.5-sonnet, claude-3-5-sonnet (API naming), claude-3.5-haiku → 8 192 output
    if lower.contains("claude-3.5") || lower.contains("claude-3-5-sonnet") || lower.contains("claude-3-5-haiku") {
        return ModelInfo { max_context: 200_000, max_output: 8_192, family: "anthropic" };
    }
    // claude-3-opus, claude-3-sonnet, claude-3-haiku → 4 096 output
    if lower.contains("claude-3") {
        return ModelInfo { max_context: 200_000, max_output: 4_096, family: "anthropic" };
    }

    // --- DeepSeek series ---------------------------------------------------

    if lower.contains("deepseek") {
        return ModelInfo { max_context: 128_000, max_output: 8_192, family: "deepseek" };
    }

    // --- Qwen series -------------------------------------------------------

    if lower.contains("qwen-max") {
        return ModelInfo { max_context: 32_768, max_output: 8_192, family: "qwen" };
    }
    if lower.contains("qwen") {
        return ModelInfo { max_context: 131_072, max_output: 8_192, family: "qwen" };
    }

    // --- Default fallback --------------------------------------------------

    ModelInfo { max_context: 128_000, max_output: 4_096, family: "unknown" }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- GLM series ---------------------------------------------------------

    #[test]
    fn test_glm_models() {
        let info = get_model_info("glm-4-plus");
        assert_eq!(info.max_context, 128_000);
        assert_eq!(info.max_output, 4_096);
        assert_eq!(info.family, "glm");

        let info = get_model_info("glm-4-long");
        assert_eq!(info.max_context, 1_000_000);
        assert_eq!(info.max_output, 4_096);

        let info = get_model_info("glm-4-air");
        assert_eq!(info.max_context, 128_000);

        let info = get_model_info("glm-4-airx");
        assert_eq!(info.max_context, 8_192);

        let info = get_model_info("glm-4-flash");
        assert_eq!(info.max_context, 128_000);

        let info = get_model_info("glm-4-flashx");
        assert_eq!(info.max_context, 128_000);

        let info = get_model_info("glm-4");
        assert_eq!(info.max_context, 128_000);

        let info = get_model_info("glm-4v");
        assert_eq!(info.max_context, 8_192);

        let info = get_model_info("glm-4v-plus");
        assert_eq!(info.max_context, 8_192);

        // Case-insensitive
        let info = get_model_info("GLM-4-Plus");
        assert_eq!(info.max_context, 128_000);
        assert_eq!(info.family, "glm");
    }

    // -- OpenAI series ------------------------------------------------------

    #[test]
    fn test_openai_models() {
        let info = get_model_info("gpt-4o");
        assert_eq!(info.max_context, 128_000);
        assert_eq!(info.max_output, 16_384);
        assert_eq!(info.family, "openai");

        let info = get_model_info("gpt-4o-2024-05-13");
        assert_eq!(info.max_context, 128_000);
        assert_eq!(info.max_output, 16_384);

        let info = get_model_info("gpt-4o-mini");
        assert_eq!(info.max_context, 128_000);
        assert_eq!(info.max_output, 16_384);

        let info = get_model_info("gpt-4-turbo");
        assert_eq!(info.max_context, 128_000);
        assert_eq!(info.max_output, 16_384);

        let info = get_model_info("gpt-3.5-turbo");
        assert_eq!(info.max_context, 16_385);
        assert_eq!(info.max_output, 4_096);

        let info = get_model_info("o1");
        assert_eq!(info.max_context, 200_000);
        assert_eq!(info.max_output, 100_000);

        let info = get_model_info("o1-mini");
        assert_eq!(info.max_context, 128_000);
        assert_eq!(info.max_output, 65_536);

        let info = get_model_info("o1-preview");
        assert_eq!(info.max_context, 200_000);
        assert_eq!(info.max_output, 100_000);

        let info = get_model_info("o3-mini");
        assert_eq!(info.max_context, 200_000);
        assert_eq!(info.max_output, 100_000);
    }

    // -- Anthropic series ---------------------------------------------------

    #[test]
    fn test_anthropic_models() {
        let info = get_model_info("claude-3-5-sonnet-20241022");
        assert_eq!(info.max_context, 200_000);
        assert_eq!(info.max_output, 8_192);
        assert_eq!(info.family, "anthropic");

        let info = get_model_info("claude-3.5-haiku");
        assert_eq!(info.max_context, 200_000);
        assert_eq!(info.max_output, 8_192);

        let info = get_model_info("claude-3-opus");
        assert_eq!(info.max_context, 200_000);
        assert_eq!(info.max_output, 4_096);

        let info = get_model_info("claude-3-sonnet");
        assert_eq!(info.max_context, 200_000);
        assert_eq!(info.max_output, 4_096);

        let info = get_model_info("claude-3-haiku");
        assert_eq!(info.max_context, 200_000);
        assert_eq!(info.max_output, 4_096);
    }

    // -- DeepSeek series ----------------------------------------------------

    #[test]
    fn test_deepseek_models() {
        let info = get_model_info("deepseek-v3");
        assert_eq!(info.max_context, 128_000);
        assert_eq!(info.max_output, 8_192);
        assert_eq!(info.family, "deepseek");

        let info = get_model_info("deepseek-r1");
        assert_eq!(info.max_context, 128_000);
        assert_eq!(info.max_output, 8_192);
    }

    // -- Qwen series --------------------------------------------------------

    #[test]
    fn test_qwen_models() {
        let info = get_model_info("qwen-max");
        assert_eq!(info.max_context, 32_768);
        assert_eq!(info.max_output, 8_192);
        assert_eq!(info.family, "qwen");

        let info = get_model_info("qwen-plus");
        assert_eq!(info.max_context, 131_072);
        assert_eq!(info.max_output, 8_192);

        let info = get_model_info("qwen-turbo");
        assert_eq!(info.max_context, 131_072);
        assert_eq!(info.max_output, 8_192);
    }

    // -- Unknown model fallback ---------------------------------------------

    #[test]
    fn test_unknown_model_fallback() {
        let info = get_model_info("some-unknown-model");
        assert_eq!(info.max_context, 128_000);
        assert_eq!(info.max_output, 4_096);
        assert_eq!(info.family, "unknown");

        let info = get_model_info("");
        assert_eq!(info.family, "unknown");
    }
}

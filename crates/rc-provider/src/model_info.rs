//! Model context-window information database.
//!
//! Provides [`ModelInfo`], [`ModelCapability`], and [`get_model_info`] for
//! looking up the maximum context window, output token limits, multimodal
//! support, and capability tags of supported LLM models.  The lookup uses
//! fuzzy, case-insensitive matching so that versioned model names (e.g.
//! `gpt-4o-2024-05-13`) are resolved correctly.
//!
//! # Information sources
//!
//! - 智谱 AI: <https://open.bigmodel.cn> (2025-01, 2026-04)
//! - MiniMax:  <https://www.minimaxi.com> (2025-06, 2026-04)
//! - OpenAI:   <https://platform.openai.com> (2025-01, 2026-04)
//! - Anthropic: <https://docs.anthropic.com> (2025-01, 2026-04)
//! - DeepSeek: <https://platform.deepseek.com> (2025-01)
//! - Qwen:     <https://help.aliyun.com> (2025-01)
//! - Google:   <https://ai.google.dev> (2025-01, 2026-04)
//! - Moonshot: <https://platform.moonshot.cn> (2025-01)
//! - 百度 ERNIE: <https://cloud.baidu.com> (2025-01)

// ---------------------------------------------------------------------------
// ModelCapability
// ---------------------------------------------------------------------------

/// A single capability tag that a model may advertise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelCapability {
    /// Plain text generation / chat completion.
    Text,
    /// Image understanding (vision).
    Vision,
    /// Video understanding.
    Video,
    /// Audio understanding (speech-to-text or audio analysis).
    Audio,
    /// Function / tool calling.
    ToolUse,
    /// Extended reasoning (o1 / o3 / R1 style chain-of-thought).
    Reasoning,
    /// Code generation optimised for programming tasks.
    Code,
    /// Image generation (DALL·E, CogView, etc.).
    ImageGeneration,
}

// ---------------------------------------------------------------------------
// ModelInfo
// ---------------------------------------------------------------------------

/// Context-window metadata and capability flags for a single model variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelInfo {
    /// Maximum input context length in tokens.
    pub max_context: u64,
    /// Maximum output tokens the model can generate in a single response.
    pub max_output: u64,
    /// Model family identifier (e.g. `"glm"`, `"openai"`, `"anthropic"`).
    pub family: &'static str,
    /// Whether the model accepts multimodal input (images / video / audio).
    pub multimodal: bool,
    /// Fine-grained capability tags.
    pub capabilities: &'static [ModelCapability],
}

// ---------------------------------------------------------------------------
// Convenience constructors (keep call-sites concise)
// ---------------------------------------------------------------------------

impl ModelInfo {
    /// Text-only model shorthand.
    const fn text(cx: u64, out: u64, fam: &'static str) -> Self {
        Self {
            max_context: cx,
            max_output: out,
            family: fam,
            multimodal: false,
            capabilities: &[ModelCapability::Text, ModelCapability::ToolUse],
        }
    }

    /// Multimodal model shorthand (implies Vision + ToolUse).
    const fn multi(cx: u64, out: u64, fam: &'static str) -> Self {
        Self {
            max_context: cx,
            max_output: out,
            family: fam,
            multimodal: true,
            capabilities: &[
                ModelCapability::Text,
                ModelCapability::Vision,
                ModelCapability::ToolUse,
            ],
        }
    }

    /// Reasoning model shorthand (o1 / o3 / R1 style).
    const fn reasoning(cx: u64, out: u64, fam: &'static str) -> Self {
        Self {
            max_context: cx,
            max_output: out,
            family: fam,
            multimodal: false,
            capabilities: &[
                ModelCapability::Text,
                ModelCapability::Reasoning,
                ModelCapability::Code,
            ],
        }
    }

    /// Multimodal + reasoning model shorthand.
    const fn multi_reasoning(cx: u64, out: u64, fam: &'static str) -> Self {
        Self {
            max_context: cx,
            max_output: out,
            family: fam,
            multimodal: true,
            capabilities: &[
                ModelCapability::Text,
                ModelCapability::Vision,
                ModelCapability::Reasoning,
                ModelCapability::Code,
                ModelCapability::ToolUse,
            ],
        }
    }
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
#[allow(clippy::too_many_lines)]
pub fn get_model_info(model: &str) -> ModelInfo {
    let lower = model.to_lowercase();

    // --- GLM-5 series (智谱 AI, 2025+) — most specific first ----------------

    // GLM-5v-turbo: multimodal vision model
    // Source: https://open.bigmodel.cn — 2026-04
    if lower.contains("glm-5v") || lower.contains("glm5v") {
        return ModelInfo::multi(128_000, 4_096, "glm");
    }

    // GLM-5.1: flagship text model
    // Source: https://open.bigmodel.cn — 2026-04
    if lower.contains("glm-5") || lower.contains("glm5") {
        return ModelInfo {
            max_context: 128_000,
            max_output: 8_192,
            family: "glm",
            multimodal: false,
            capabilities: &[
                ModelCapability::Text,
                ModelCapability::ToolUse,
                ModelCapability::Code,
            ],
        };
    }

    // --- GLM-4 series (智谱 AI) — most specific first -----------------------

    if lower.contains("glm-4-long") {
        return ModelInfo::text(1_000_000, 4_096, "glm");
    }
    if lower.contains("glm-4-airx") {
        return ModelInfo::text(8_192, 4_096, "glm");
    }
    // glm-4v / glm-4v-plus — multimodal vision models
    if lower.contains("glm-4v") {
        return ModelInfo::multi(8_192, 4_096, "glm");
    }
    // Catch-all for glm-4, glm-4-plus, glm-4-air, glm-4-flash, glm-4-flashx
    if lower.contains("glm-4") || lower.contains("glm4") {
        return ModelInfo::text(128_000, 4_096, "glm");
    }

    // --- MiniMax series — https://www.minimaxi.com --------------------------

    // MiniMax-M1: flagship text model, 1M context
    // Source: https://www.minimaxi.com — 2025-06
    if lower.contains("minimax-m1") || lower.contains("m1") && lower.contains("minimax") {
        return ModelInfo::text(1_000_000, 8_192, "minimax");
    }

    // MiniMax-M2.7: latest text model, 1M context
    // Source: https://www.minimaxi.com — 2026-04
    if lower.contains("minimax-m2") || lower.contains("m2") && lower.contains("minimax") {
        return ModelInfo {
            max_context: 1_000_000,
            max_output: 8_192,
            family: "minimax",
            multimodal: false,
            capabilities: &[
                ModelCapability::Text,
                ModelCapability::ToolUse,
                ModelCapability::Code,
            ],
        };
    }

    // abab-7: conversation model
    if lower.contains("abab-7") || lower.contains("abab7") {
        return ModelInfo::text(128_000, 4_096, "minimax");
    }

    // abab-6.5s: lightweight conversation model
    if lower.contains("abab-6") || lower.contains("abab6") {
        return ModelInfo::text(128_000, 4_096, "minimax");
    }

    // --- OpenAI series -------------------------------------------------------

    // o3: reasoning model
    // Source: https://platform.openai.com — 2025-04
    if lower == "o3" || lower.contains("o3-") && !lower.contains("o3-mini") {
        return ModelInfo::multi_reasoning(200_000, 100_000, "openai");
    }

    // o4-mini: lightweight reasoning model
    // Source: https://platform.openai.com — 2025-04
    if lower.contains("o4-mini") {
        return ModelInfo::reasoning(200_000, 100_000, "openai");
    }

    // o3-mini, o1, o1-preview all share 200K / 100K
    if lower.contains("o3-mini") || lower.contains("o1-preview") || lower == "o1" {
        return ModelInfo::reasoning(200_000, 100_000, "openai");
    }
    if lower.contains("o1-mini") {
        return ModelInfo::reasoning(128_000, 65_536, "openai");
    }

    // GPT-4.5: latest GPT model (if released)
    // Source: https://platform.openai.com — 2025-04, conservative estimates
    if lower.contains("gpt-4.5") || lower.contains("gpt-45") {
        return ModelInfo::multi(128_000, 16_384, "openai");
    }

    // gpt-4o (including dated snapshots like gpt-4o-2024-05-13) and gpt-4-turbo
    if lower.contains("gpt-4o") || lower.contains("gpt-4-turbo") {
        return ModelInfo::multi(128_000, 16_384, "openai");
    }
    if lower.contains("gpt-3.5") {
        return ModelInfo::text(16_385, 4_096, "openai");
    }

    // --- Anthropic series ----------------------------------------------------

    // Claude 4 / Claude 3.7 Sonnet — latest generation
    // Source: https://docs.anthropic.com — 2026-04, conservative estimates
    if lower.contains("claude-4") || lower.contains("claude4") {
        return ModelInfo::multi(200_000, 16_384, "anthropic");
    }
    if lower.contains("claude-3.7") || lower.contains("claude-3-7") {
        return ModelInfo::multi(200_000, 16_384, "anthropic");
    }

    // claude-3.5-sonnet, claude-3-5-sonnet (API naming), claude-3.5-haiku → 8 192 output
    if lower.contains("claude-3.5") || lower.contains("claude-3-5-sonnet") || lower.contains("claude-3-5-haiku") {
        return ModelInfo::multi(200_000, 8_192, "anthropic");
    }
    // claude-3-opus, claude-3-sonnet, claude-3-haiku → 4 096 output
    if lower.contains("claude-3") {
        return ModelInfo::multi(200_000, 4_096, "anthropic");
    }

    // --- DeepSeek series -----------------------------------------------------

    // DeepSeek-R1: reasoning model
    if lower.contains("deepseek-r1") {
        return ModelInfo::reasoning(128_000, 8_192, "deepseek");
    }

    if lower.contains("deepseek") {
        return ModelInfo {
            max_context: 128_000,
            max_output: 8_192,
            family: "deepseek",
            multimodal: false,
            capabilities: &[
                ModelCapability::Text,
                ModelCapability::ToolUse,
                ModelCapability::Code,
            ],
        };
    }

    // --- Qwen series (通义千问) -----------------------------------------------

    // Qwen-VL-Max: multimodal vision-language model
    if lower.contains("qwen-vl") {
        return ModelInfo::multi(32_768, 8_192, "qwen");
    }

    // Qwen-Long: long-context text model
    // Source: https://help.aliyun.com — 1M context
    if lower.contains("qwen-long") {
        return ModelInfo::text(1_000_000, 8_192, "qwen");
    }

    if lower.contains("qwen-max") {
        return ModelInfo::multi(32_768, 8_192, "qwen");
    }
    if lower.contains("qwen") {
        return ModelInfo::multi(131_072, 8_192, "qwen");
    }

    // --- Google Gemini series — https://ai.google.dev -----------------------

    // Gemini 2.5 Pro: latest flagship, 1M context
    // Source: https://ai.google.dev — 2025-03
    if lower.contains("gemini-2.5") || lower.contains("gemini-25") {
        return ModelInfo::multi_reasoning(1_000_000, 8_192, "gemini");
    }

    // Gemini 2.0 Pro: high-capability multimodal, 2M context
    // Source: https://ai.google.dev — conservative estimates
    // NOTE: must be checked before the generic gemini-2.0 catch-all.
    if lower.contains("gemini-2.0-pro") || lower.contains("gemini-20-pro") {
        return ModelInfo::multi(2_000_000, 8_192, "gemini");
    }

    // Gemini 2.0 Flash: fast multimodal, 1M context
    // Source: https://ai.google.dev — 2024-12
    if lower.contains("gemini-2.0") || lower.contains("gemini-20") {
        return ModelInfo::multi(1_000_000, 8_192, "gemini");
    }

    // Gemini 1.5 Pro: 2M context multimodal
    if lower.contains("gemini-1.5-pro") || lower.contains("gemini-15-pro") {
        return ModelInfo::multi(2_000_000, 8_192, "gemini");
    }

    // Gemini 1.5 Flash: 1M context multimodal
    if lower.contains("gemini-1.5") || lower.contains("gemini-15") {
        return ModelInfo::multi(1_000_000, 8_192, "gemini");
    }

    // Catch-all for any other Gemini variants
    if lower.contains("gemini") {
        return ModelInfo::multi(1_000_000, 8_192, "gemini");
    }

    // --- Moonshot / Kimi (月之暗面) — https://platform.moonshot.cn -----------

    // Source: https://platform.moonshot.cn — 2025-01
    if lower.contains("moonshot-v1-128k") || lower.contains("moonshot-128k") {
        return ModelInfo::text(128_000, 4_096, "moonshot");
    }
    if lower.contains("moonshot-v1-32k") || lower.contains("moonshot-32k") {
        return ModelInfo::text(32_768, 4_096, "moonshot");
    }
    if lower.contains("moonshot-v1-8k") || lower.contains("moonshot-8k") {
        return ModelInfo::text(8_192, 4_096, "moonshot");
    }
    // Catch-all for moonshot / kimi
    if lower.contains("moonshot") || lower.contains("kimi") {
        return ModelInfo::text(128_000, 4_096, "moonshot");
    }

    // --- ERNIE / 百度文心 — https://cloud.baidu.com --------------------------

    // Source: https://cloud.baidu.com — 2025-01
    if lower.contains("ernie-4.0-turbo") || lower.contains("ernie-4-turbo") || lower.contains("ernie-4.0-128k") {
        return ModelInfo::text(128_000, 4_096, "ernie");
    }
    if lower.contains("ernie-4.0") || lower.contains("ernie-4") || lower.contains("ernie4") {
        return ModelInfo::text(8_192, 4_096, "ernie");
    }
    if lower.contains("ernie-3.5") || lower.contains("ernie-3") {
        return ModelInfo::text(8_192, 4_096, "ernie");
    }
    // Catch-all for ERNIE
    if lower.contains("ernie") {
        return ModelInfo::text(8_192, 4_096, "ernie");
    }

    // --- Default fallback ----------------------------------------------------

    ModelInfo {
        max_context: 128_000,
        max_output: 4_096,
        family: "unknown",
        multimodal: false,
        capabilities: &[ModelCapability::Text],
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- GLM-5 series --------------------------------------------------------

    #[test]
    fn test_glm5_models() {
        let info = get_model_info("glm-5.1");
        assert_eq!(info.max_context, 128_000);
        assert_eq!(info.max_output, 8_192);
        assert_eq!(info.family, "glm");
        assert!(!info.multimodal);
        assert!(info.capabilities.contains(&ModelCapability::Text));
        assert!(info.capabilities.contains(&ModelCapability::Code));

        let info = get_model_info("GLM-5.1");
        assert_eq!(info.family, "glm");

        let info = get_model_info("glm-5v-turbo");
        assert_eq!(info.max_context, 128_000);
        assert_eq!(info.family, "glm");
        assert!(info.multimodal);
        assert!(info.capabilities.contains(&ModelCapability::Vision));

        let info = get_model_info("glm-5v-plus");
        assert!(info.multimodal);
    }

    // -- GLM-4 series --------------------------------------------------------

    #[test]
    fn test_glm_models() {
        let info = get_model_info("glm-4-plus");
        assert_eq!(info.max_context, 128_000);
        assert_eq!(info.max_output, 4_096);
        assert_eq!(info.family, "glm");
        assert!(!info.multimodal);

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
        assert!(info.multimodal);

        let info = get_model_info("glm-4v-plus");
        assert_eq!(info.max_context, 8_192);
        assert!(info.multimodal);

        // Case-insensitive
        let info = get_model_info("GLM-4-Plus");
        assert_eq!(info.max_context, 128_000);
        assert_eq!(info.family, "glm");
    }

    // -- MiniMax series ------------------------------------------------------

    #[test]
    fn test_minimax_models() {
        let info = get_model_info("MiniMax-M1");
        assert_eq!(info.max_context, 1_000_000);
        assert_eq!(info.max_output, 8_192);
        assert_eq!(info.family, "minimax");
        assert!(!info.multimodal);
        assert!(info.capabilities.contains(&ModelCapability::Text));

        let info = get_model_info("minimax-m2.7");
        assert_eq!(info.max_context, 1_000_000);
        assert_eq!(info.max_output, 8_192);
        assert_eq!(info.family, "minimax");
        assert!(info.capabilities.contains(&ModelCapability::Code));

        let info = get_model_info("abab-7");
        assert_eq!(info.max_context, 128_000);
        assert_eq!(info.family, "minimax");

        let info = get_model_info("abab-6.5s");
        assert_eq!(info.max_context, 128_000);
        assert_eq!(info.family, "minimax");
    }

    // -- OpenAI series -------------------------------------------------------

    #[test]
    fn test_openai_models() {
        let info = get_model_info("gpt-4o");
        assert_eq!(info.max_context, 128_000);
        assert_eq!(info.max_output, 16_384);
        assert_eq!(info.family, "openai");
        assert!(info.multimodal);

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
        assert!(info.capabilities.contains(&ModelCapability::Reasoning));

        let info = get_model_info("o1-mini");
        assert_eq!(info.max_context, 128_000);
        assert_eq!(info.max_output, 65_536);

        let info = get_model_info("o1-preview");
        assert_eq!(info.max_context, 200_000);
        assert_eq!(info.max_output, 100_000);

        let info = get_model_info("o3-mini");
        assert_eq!(info.max_context, 200_000);
        assert_eq!(info.max_output, 100_000);
        assert!(info.capabilities.contains(&ModelCapability::Reasoning));

        // New models
        let info = get_model_info("o3");
        assert_eq!(info.max_context, 200_000);
        assert_eq!(info.max_output, 100_000);
        assert!(info.multimodal);
        assert!(info.capabilities.contains(&ModelCapability::Reasoning));

        let info = get_model_info("o4-mini");
        assert_eq!(info.max_context, 200_000);
        assert_eq!(info.max_output, 100_000);
        assert!(info.capabilities.contains(&ModelCapability::Reasoning));

        let info = get_model_info("gpt-4.5");
        assert_eq!(info.max_context, 128_000);
        assert_eq!(info.max_output, 16_384);
        assert!(info.multimodal);
    }

    // -- Anthropic series ----------------------------------------------------

    #[test]
    fn test_anthropic_models() {
        let info = get_model_info("claude-3-5-sonnet-20241022");
        assert_eq!(info.max_context, 200_000);
        assert_eq!(info.max_output, 8_192);
        assert_eq!(info.family, "anthropic");
        assert!(info.multimodal);

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

        // Claude 4
        let info = get_model_info("claude-4-opus");
        assert_eq!(info.max_context, 200_000);
        assert_eq!(info.max_output, 16_384);
        assert!(info.multimodal);

        // Claude 3.7 Sonnet
        let info = get_model_info("claude-3.7-sonnet");
        assert_eq!(info.max_context, 200_000);
        assert_eq!(info.max_output, 16_384);
    }

    // -- DeepSeek series -----------------------------------------------------

    #[test]
    fn test_deepseek_models() {
        let info = get_model_info("deepseek-v3");
        assert_eq!(info.max_context, 128_000);
        assert_eq!(info.max_output, 8_192);
        assert_eq!(info.family, "deepseek");
        assert!(!info.multimodal);
        assert!(info.capabilities.contains(&ModelCapability::Code));

        let info = get_model_info("deepseek-r1");
        assert_eq!(info.max_context, 128_000);
        assert_eq!(info.max_output, 8_192);
        assert!(info.capabilities.contains(&ModelCapability::Reasoning));

        let info = get_model_info("deepseek-v2.5");
        assert_eq!(info.max_context, 128_000);
    }

    // -- Qwen series ---------------------------------------------------------

    #[test]
    fn test_qwen_models() {
        let info = get_model_info("qwen-max");
        assert_eq!(info.max_context, 32_768);
        assert_eq!(info.max_output, 8_192);
        assert_eq!(info.family, "qwen");
        assert!(info.multimodal);

        let info = get_model_info("qwen-plus");
        assert_eq!(info.max_context, 131_072);
        assert_eq!(info.max_output, 8_192);

        let info = get_model_info("qwen-turbo");
        assert_eq!(info.max_context, 131_072);
        assert_eq!(info.max_output, 8_192);

        let info = get_model_info("qwen-vl-max");
        assert_eq!(info.max_context, 32_768);
        assert!(info.multimodal);
        assert!(info.capabilities.contains(&ModelCapability::Vision));

        let info = get_model_info("qwen-long");
        assert_eq!(info.max_context, 1_000_000);
        assert!(!info.multimodal);
    }

    // -- Gemini series -------------------------------------------------------

    #[test]
    fn test_gemini_models() {
        // Gemini 2.5 Pro
        let info = get_model_info("gemini-2.5-pro");
        assert_eq!(info.max_context, 1_000_000);
        assert_eq!(info.max_output, 8_192);
        assert_eq!(info.family, "gemini");
        assert!(info.multimodal);
        assert!(info.capabilities.contains(&ModelCapability::Reasoning));

        // Gemini 2.0 Flash
        let info = get_model_info("gemini-2.0-flash");
        assert_eq!(info.max_context, 1_000_000);
        assert!(info.multimodal);

        // Gemini 2.0 Pro
        let info = get_model_info("gemini-2.0-pro");
        assert_eq!(info.max_context, 2_000_000);

        // Gemini 1.5 Pro
        let info = get_model_info("gemini-1.5-pro");
        assert_eq!(info.max_context, 2_000_000);
        assert!(info.multimodal);

        // Gemini 1.5 Flash
        let info = get_model_info("gemini-1.5-flash");
        assert_eq!(info.max_context, 1_000_000);

        // Generic gemini fallback
        let info = get_model_info("gemini-exp");
        assert_eq!(info.family, "gemini");
        assert!(info.multimodal);
    }

    // -- Moonshot series -----------------------------------------------------

    #[test]
    fn test_moonshot_models() {
        let info = get_model_info("moonshot-v1-128k");
        assert_eq!(info.max_context, 128_000);
        assert_eq!(info.max_output, 4_096);
        assert_eq!(info.family, "moonshot");
        assert!(!info.multimodal);

        let info = get_model_info("moonshot-v1-32k");
        assert_eq!(info.max_context, 32_768);

        let info = get_model_info("moonshot-v1-8k");
        assert_eq!(info.max_context, 8_192);

        // Catch-all
        let info = get_model_info("moonshot-v1");
        assert_eq!(info.family, "moonshot");

        // Kimi alias
        let info = get_model_info("kimi-latest");
        assert_eq!(info.family, "moonshot");
    }

    // -- ERNIE series --------------------------------------------------------

    #[test]
    fn test_ernie_models() {
        let info = get_model_info("ernie-4.0-8k");
        assert_eq!(info.max_context, 8_192);
        assert_eq!(info.max_output, 4_096);
        assert_eq!(info.family, "ernie");
        assert!(!info.multimodal);

        let info = get_model_info("ernie-4.0-turbo");
        assert_eq!(info.max_context, 128_000);

        let info = get_model_info("ernie-3.5-8k");
        assert_eq!(info.max_context, 8_192);

        // Catch-all
        let info = get_model_info("ernie-speed");
        assert_eq!(info.family, "ernie");
    }

    // -- Multimodal flag verification ----------------------------------------

    #[test]
    fn test_multimodal_flag() {
        // Models that SHOULD be multimodal
        let multimodal_models = [
            "gpt-4o",
            "gpt-4o-mini",
            "gpt-4-turbo",
            "gpt-4.5",
            "claude-3-5-sonnet-20241022",
            "claude-3-opus",
            "claude-3-haiku",
            "claude-4-sonnet",
            "glm-4v",
            "glm-4v-plus",
            "glm-5v-turbo",
            "qwen-max",
            "qwen-plus",
            "qwen-vl-max",
            "gemini-2.0-flash",
            "gemini-1.5-pro",
            "o3",
        ];
        for model in multimodal_models {
            let info = get_model_info(model);
            assert!(info.multimodal, "{model} should be multimodal");
            assert!(
                info.capabilities.contains(&ModelCapability::Vision),
                "{model} should have Vision capability"
            );
        }

        // Models that should NOT be multimodal
        let text_only_models = [
            "glm-4-plus",
            "glm-4-long",
            "glm-4-flash",
            "glm-5.1",
            "minimax-m1",
            "minimax-m2.7",
            "abab-7",
            "o1",
            "o1-mini",
            "o3-mini",
            "o4-mini",
            "gpt-3.5-turbo",
            "deepseek-v3",
            "deepseek-r1",
            "moonshot-v1-128k",
            "ernie-4.0-8k",
            "qwen-long",
        ];
        for model in text_only_models {
            let info = get_model_info(model);
            assert!(!info.multimodal, "{model} should NOT be multimodal");
        }
    }

    // -- Reasoning capability verification -----------------------------------

    #[test]
    fn test_reasoning_capability() {
        let reasoning_models = ["o1", "o1-mini", "o1-preview", "o3", "o3-mini", "o4-mini", "deepseek-r1"];
        for model in reasoning_models {
            let info = get_model_info(model);
            assert!(
                info.capabilities.contains(&ModelCapability::Reasoning),
                "{model} should have Reasoning capability"
            );
        }

        // Non-reasoning models
        let non_reasoning = ["gpt-4o", "glm-4-plus", "deepseek-v3", "qwen-max"];
        for model in non_reasoning {
            let info = get_model_info(model);
            assert!(
                !info.capabilities.contains(&ModelCapability::Reasoning),
                "{model} should NOT have Reasoning capability"
            );
        }
    }

    // -- Unknown model fallback ----------------------------------------------

    #[test]
    fn test_unknown_model_fallback() {
        let info = get_model_info("some-unknown-model");
        assert_eq!(info.max_context, 128_000);
        assert_eq!(info.max_output, 4_096);
        assert_eq!(info.family, "unknown");
        assert!(!info.multimodal);

        let info = get_model_info("");
        assert_eq!(info.family, "unknown");
    }
}

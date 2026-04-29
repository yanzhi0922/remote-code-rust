//! Transform layer for converting between API formats.

pub mod anthropic_filter;
pub mod ai_sdk;
pub mod bedrock_converse_format;
pub mod cache_strategy;
pub mod caching;
pub mod gemini_format;
pub mod image_cleaning;
pub mod minimax_format;
pub mod mistral_format;
pub mod model_params;
pub mod openai_format;
pub mod r1_zai_format;
pub mod reasoning;
pub mod responses_api;
pub mod responses_api_stream;
pub mod stream;
pub mod vscode_lm_format;

pub use anthropic_filter::filter_non_anthropic_blocks;
pub use gemini_format::{
    build_tool_id_to_name_map, convert_anthropic_content_to_gemini,
    convert_anthropic_message_to_gemini, GeminiContent, GeminiConversionOptions, GeminiPart,
};
pub use image_cleaning::maybe_remove_image_blocks;
pub use mistral_format::{convert_to_mistral_messages, normalize_mistral_tool_call_id};
pub use model_params::{
    calculate_model_params, get_model_max_output_tokens, Format, GetModelParamsOptions,
    ModelParams,
};
pub use openai_format::{
    consolidate_reasoning_details, convert_to_openai_messages, map_reasoning_details,
    sanitize_gemini_messages, ConvertToOpenAiMessagesOptions, ReasoningDetail,
};
pub use r1_zai_format::{convert_to_r1_zai_messages, R1ZaiOptions};
pub use reasoning::{
    effort_extended_to_setting, effort_setting_to_extended, get_anthropic_reasoning,
    get_gemini_reasoning, get_openai_reasoning, get_openrouter_reasoning, get_roo_reasoning,
    is_gemini_thinking_level, should_use_reasoning_budget, should_use_reasoning_effort,
    GetModelReasoningOptions, ANTHROPIC_DEFAULT_MAX_TOKENS,
    DEFAULT_HYBRID_REASONING_MODEL_MAX_TOKENS, DEFAULT_HYBRID_REASONING_MODEL_THINKING_TOKENS,
    GEMINI_25_PRO_MIN_THINKING_TOKENS, GEMINI_THINKING_LEVELS,
};
pub use responses_api::{
    convert_to_responses_api_input, normalize_usage, parse_responses_api_stream, ResponseApiEvent,
};

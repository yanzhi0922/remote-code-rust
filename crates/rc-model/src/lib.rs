//! Model management for the remote-code-rust workspace.
//!
//! This crate provides:
//! - **Model definitions** ([`model::Model`], [`model::ModelSetting`]) and
//!   resolution logic ([`model::resolve_model`]).
//! - **Capability queries** ([`capabilities::get_capabilities`]) for images,
//!   tool use, extended thinking, 1M context, etc.
//! - **Provider detection** ([`providers::detect_provider`]) for Anthropic,
//!   Bedrock, Vertex, and OpenAI-compatible endpoints.
//! - **Alias resolution** ([`aliases::resolve_alias`]) for short names like
//!   `"sonnet"`, `"opus"`, `"haiku"`.
//! - **Validation** ([`validate::validate_model_id`]) for model ID format
//!   and allowlist membership.
//! - **Allowlist** ([`allowlist::is_model_allowed`]) for enterprise model
//!   access control.
//! - **1M context access** ([`check_1m::has_1m_access`]) checks based on
//!   subscription tier.

pub mod aliases;
pub mod allowlist;
pub mod capabilities;
pub mod check_1m;
pub mod model;
pub mod providers;
pub mod validate;

// Re-export the most commonly used types at the crate root.
pub use aliases::{is_model_alias, is_model_family_alias, resolve_alias};
pub use allowlist::{default_allowlist, is_model_allowed};
pub use capabilities::{
    EffortLevel, ModelCapabilities, get_capabilities, model_supports_1m, model_supports_thinking,
};
pub use check_1m::{
    ExtraUsageState, OneMContext, SubscriptionTier, has_1m_access, has_1m_tag, strip_1m_tag,
};
pub use model::{
    Model, ModelSetting, ResolveContext, default_main_loop_model, get_small_fast_model,
    parse_user_specified_model, resolve_model,
};
pub use providers::{
    ModelProvider, ProviderConfig, default_haiku_model, default_opus_model, default_sonnet_model,
    detect_provider, is_first_party_base_url, provider_model_id,
};
pub use validate::{
    ValidationError, get_canonical_name, get_public_model_display_name,
    normalize_model_string_for_api, validate_model_id,
};

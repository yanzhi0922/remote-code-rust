//! Image generation type definitions.
//!
//! Derived from `packages/types/src/image-generation.ts`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// ImageGenerationProvider
// ---------------------------------------------------------------------------

/// Image generation provider.
///
/// Source: `packages/types/src/image-generation.ts` — `ImageGenerationProvider`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageGenerationProvider {
    #[serde(rename = "openrouter")]
    Openrouter,
    #[serde(rename = "roo")]
    Roo,
}

// ---------------------------------------------------------------------------
// ImageGenerationApiMethod
// ---------------------------------------------------------------------------

/// API method used for image generation.
///
/// Source: `packages/types/src/image-generation.ts` — `ImageGenerationApiMethod`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageGenerationApiMethod {
    ChatCompletions,
    ImagesApi,
}

// ---------------------------------------------------------------------------
// ImageGenerationModel
// ---------------------------------------------------------------------------

/// An image generation model.
///
/// Source: `packages/types/src/image-generation.ts` — `ImageGenerationModel`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGenerationModel {
    pub value: String,
    pub label: String,
    pub provider: ImageGenerationProvider,
    pub api_method: Option<ImageGenerationApiMethod>,
}

/// Returns the default image generation models.
pub fn image_generation_models() -> Vec<ImageGenerationModel> {
    vec![
        ImageGenerationModel {
            value: "google/gemini-2.5-flash-image".into(),
            label: "Gemini 2.5 Flash Image".into(),
            provider: ImageGenerationProvider::Openrouter,
            api_method: None,
        },
        ImageGenerationModel {
            value: "google/gemini-3-pro-image-preview".into(),
            label: "Gemini 3 Pro Image Preview".into(),
            provider: ImageGenerationProvider::Openrouter,
            api_method: None,
        },
        ImageGenerationModel {
            value: "openai/gpt-5-image".into(),
            label: "GPT-5 Image".into(),
            provider: ImageGenerationProvider::Openrouter,
            api_method: None,
        },
        ImageGenerationModel {
            value: "openai/gpt-5-image-mini".into(),
            label: "GPT-5 Image Mini".into(),
            provider: ImageGenerationProvider::Openrouter,
            api_method: None,
        },
        ImageGenerationModel {
            value: "black-forest-labs/flux.2-flex".into(),
            label: "Black Forest Labs FLUX.2 Flex".into(),
            provider: ImageGenerationProvider::Openrouter,
            api_method: None,
        },
        ImageGenerationModel {
            value: "black-forest-labs/flux.2-pro".into(),
            label: "Black Forest Labs FLUX.2 Pro".into(),
            provider: ImageGenerationProvider::Openrouter,
            api_method: None,
        },
        ImageGenerationModel {
            value: "google/gemini-2.5-flash-image".into(),
            label: "Gemini 2.5 Flash Image".into(),
            provider: ImageGenerationProvider::Roo,
            api_method: None,
        },
        ImageGenerationModel {
            value: "google/gemini-3-pro-image".into(),
            label: "Gemini 3 Pro Image".into(),
            provider: ImageGenerationProvider::Roo,
            api_method: None,
        },
        ImageGenerationModel {
            value: "bfl/flux-2-pro:free".into(),
            label: "Black Forest Labs FLUX.2 Pro (Free)".into(),
            provider: ImageGenerationProvider::Roo,
            api_method: Some(ImageGenerationApiMethod::ImagesApi),
        },
    ]
}

/// Returns the image generation model IDs.
pub fn image_generation_model_ids() -> Vec<String> {
    image_generation_models().iter().map(|m| m.value.clone()).collect()
}

/// Gets the image generation provider with backwards compatibility.
pub fn get_image_generation_provider(
    explicit_provider: Option<ImageGenerationProvider>,
    has_existing_model: bool,
) -> ImageGenerationProvider {
    match explicit_provider {
        Some(p) => p,
        None if has_existing_model => ImageGenerationProvider::Openrouter,
        None => ImageGenerationProvider::Roo,
    }
}

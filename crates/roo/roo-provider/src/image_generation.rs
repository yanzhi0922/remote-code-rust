//! Image generation utilities for providers that support it.
//!
//! Source: `src/api/providers/utils/image-generation.ts`
//!
//! Two approaches:
//! 1. `generate_image_with_provider` — uses chat completions with `modalities: ["image", "text"]`
//!    (OpenRouter and Roo Cloud default path)
//! 2. `generate_image_with_images_api` — uses the OpenAI Images API `/images/generations`
//!    (Roo Cloud when `apiMethod === "images_api"`)

// ProviderError and Result are not directly used in this module but are re-exported
// from the crate for consumers that need them alongside image generation types.

/// Result of an image generation request.
#[derive(Debug, Clone)]
pub struct ImageGenerationResult {
    pub success: bool,
    pub image_data: Option<String>,
    pub image_format: Option<String>,
    pub error: Option<String>,
}

impl ImageGenerationResult {
    pub fn ok(data_url: String, format: String) -> Self {
        Self {
            success: true,
            image_data: Some(data_url),
            image_format: Some(format),
            error: None,
        }
    }

    pub fn err(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            image_data: None,
            image_format: None,
            error: Some(msg.into()),
        }
    }
}

/// Options for chat-completions-based image generation.
///
/// Source: `src/api/providers/utils/image-generation.ts` — `ImageGenerationOptions`
pub struct ImageGenerationOptions {
    pub base_url: String,
    pub auth_token: String,
    pub model: String,
    pub prompt: String,
    pub input_image: Option<String>,
}

/// Generate an image using the chat completions API with `modalities: ["image", "text"]`.
///
/// Source: `src/api/providers/utils/image-generation.ts` — `generateImageWithProvider`
pub async fn generate_image_with_provider(
    http_client: &reqwest::Client,
    opts: &ImageGenerationOptions,
) -> ImageGenerationResult {
    let url = format!("{}/chat/completions", opts.base_url.trim_end_matches('/'));

    let content = if let Some(ref img) = opts.input_image {
        serde_json::json!([
            { "type": "text", "text": opts.prompt },
            { "type": "image_url", "image_url": { "url": img } }
        ])
    } else {
        serde_json::json!(opts.prompt)
    };

    let body = serde_json::json!({
        "model": opts.model,
        "messages": [{
            "role": "user",
            "content": content
        }],
        "modalities": ["image", "text"]
    });

    let response = match http_client
        .post(&url)
        .header("Authorization", format!("Bearer {}", opts.auth_token))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return ImageGenerationResult::err(format!("HTTP error: {e}")),
    };

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        return ImageGenerationResult::err(format!("API error {status}: {text}"));
    }

    let resp: serde_json::Value = match response.json().await {
        Ok(v) => v,
        Err(e) => return ImageGenerationResult::err(format!("Parse error: {e}")),
    };

    extract_image_from_response(&resp)
}

/// Options for Images API-based generation.
///
/// Source: `src/api/providers/utils/image-generation.ts` — `ImagesApiOptions`
pub struct ImagesApiOptions {
    pub base_url: String,
    pub auth_token: String,
    pub model: String,
    pub prompt: String,
    pub input_image: Option<String>,
    pub size: Option<String>,
    pub quality: Option<String>,
    pub output_format: Option<String>,
}

/// Generate an image using the OpenAI Images API (`/images/generations`).
///
/// Source: `src/api/providers/utils/image-generation.ts` — `generateImageWithImagesApi`
pub async fn generate_image_with_images_api(
    http_client: &reqwest::Client,
    opts: &ImagesApiOptions,
) -> ImageGenerationResult {
    let url = format!("{}/images/generations", opts.base_url.trim_end_matches('/'));

    let mut body = serde_json::json!({
        "model": opts.model,
        "prompt": opts.prompt,
        "response_format": "b64_json"
    });

    if let Some(ref size) = opts.size {
        body["size"] = serde_json::json!(size);
    }
    if let Some(ref quality) = opts.quality {
        body["quality"] = serde_json::json!(quality);
    }
    if let Some(ref fmt) = opts.output_format {
        body["output_format"] = serde_json::json!(fmt);
    }

    let response = match http_client
        .post(&url)
        .header("Authorization", format!("Bearer {}", opts.auth_token))
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => return ImageGenerationResult::err(format!("HTTP error: {e}")),
    };

    if !response.status().is_success() {
        let status = response.status().as_u16();
        let text = response.text().await.unwrap_or_default();
        return ImageGenerationResult::err(format!("API error {status}: {text}"));
    }

    let resp: serde_json::Value = match response.json().await {
        Ok(v) => v,
        Err(e) => return ImageGenerationResult::err(format!("Parse error: {e}")),
    };

    // Handle b64_json response
    if let Some(b64) = resp
        .get("data")
        .and_then(|d| d.get(0))
        .and_then(|d| d.get("b64_json"))
        .and_then(|v| v.as_str())
    {
        let fmt = opts.output_format.as_deref().unwrap_or("png");
        return ImageGenerationResult::ok(
            format!("data:image/{fmt};base64,{b64}"),
            fmt.to_string(),
        );
    }

    // Handle URL response
    if let Some(image_url) = resp
        .get("data")
        .and_then(|d| d.get(0))
        .and_then(|d| d.get("url"))
        .and_then(|v| v.as_str())
    {
        return ImageGenerationResult::ok(image_url.to_string(), "png".to_string());
    }

    ImageGenerationResult::err("No image data found in response")
}

/// Extract image data from a chat completions response with image modality.
fn extract_image_from_response(resp: &serde_json::Value) -> ImageGenerationResult {
    let images = resp
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("images"))
        .and_then(|i| i.as_array());

    let Some(images) = images else {
        // Fall back to content-based extraction
        if let Some(content) = resp
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
        {
            if content.starts_with("data:image/") {
                let fmt = extract_format_from_data_url(content);
                return ImageGenerationResult::ok(content.to_string(), fmt);
            }
        }
        return ImageGenerationResult::err("No image found in response");
    };

    for image in images {
        if let Some(url) = image
            .get("image_url")
            .and_then(|iu| iu.get("url"))
            .and_then(|u| u.as_str())
        {
            let fmt = extract_format_from_data_url(url);
            return ImageGenerationResult::ok(url.to_string(), fmt);
        }
    }

    ImageGenerationResult::err("No image URL found in response images array")
}

/// Extract the image format from a data URL (e.g. "data:image/png;base64,..." -> "png").
fn extract_format_from_data_url(data_url: &str) -> String {
    if let Some(start) = data_url.strip_prefix("data:image/") {
        if let Some(end) = start.find(';') {
            return start[..end].to_string();
        }
    }
    "png".to_string()
}

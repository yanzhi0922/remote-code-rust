//! generate_image tool implementation.
//!
//! Handles image generation via external API providers (OpenRouter, Roo).
//! Corresponds to `src/core/tools/GenerateImageTool.ts` in the TS source.
//!
//! This module provides:
//! - Parameter validation
//! - Input image handling (reading, format validation, base64 encoding)
//! - Result types for image generation
//! - A trait for provider-specific image generation API calls

// ---------------------------------------------------------------------------
// GenerateImageParams
// ---------------------------------------------------------------------------

/// Parameters for the generate_image tool.
///
/// Matches the TS `GenerateImageParams` interface:
/// ```ts
/// interface GenerateImageParams {
///     prompt: string
///     path: string
///     image?: string
/// }
/// ```
#[derive(Debug, Clone)]
pub struct GenerateImageParams {
    /// The text prompt describing what to generate.
    pub prompt: String,
    /// The file path where the generated image should be saved.
    pub path: String,
    /// Optional path to an input image to edit/transform.
    pub image: Option<String>,
}

// ---------------------------------------------------------------------------
// GenerateImageResult
// ---------------------------------------------------------------------------

/// Result of a successful image generation operation.
#[derive(Debug, Clone)]
pub struct GenerateImageResult {
    /// The path where the image was saved.
    pub path: String,
    /// The final file path (may differ from input if extension was added).
    pub final_path: String,
    /// The format of the generated image (e.g., "png", "jpeg").
    pub image_format: String,
    /// Whether an input image was used for editing.
    pub used_input_image: bool,
}

// ---------------------------------------------------------------------------
// ImageGenerationError
// ---------------------------------------------------------------------------

/// Errors specific to the generate_image tool.
#[derive(Debug, Clone)]
pub enum ImageGenerationError {
    /// Image generation is not enabled (experimental feature).
    FeatureDisabled,
    /// Missing required parameter.
    MissingParam(String),
    /// Input image not found.
    InputImageNotFound(String),
    /// Unsupported image format.
    UnsupportedFormat(String),
    /// Failed to read input image.
    InputImageReadFailed(String),
    /// Access denied by .rooignore.
    AccessDenied(String),
    /// API key required but not provided.
    ApiKeyRequired(String),
    /// Provider API call failed.
    ProviderError(String),
    /// No image data received from provider.
    NoImageData,
    /// Invalid image data format received.
    InvalidImageFormat,
    /// Failed to save image.
    SaveFailed(String),
}

impl std::fmt::Display for ImageGenerationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FeatureDisabled => write!(
                f,
                "Image generation is an experimental feature that must be enabled in settings. Please enable 'Image Generation' in the Experimental Settings section."
            ),
            Self::MissingParam(param) => write!(f, "Missing required parameter '{}'", param),
            Self::InputImageNotFound(path) => write!(f, "Input image not found: {}", path),
            Self::UnsupportedFormat(format) => write!(
                f,
                "Unsupported image format: {}. Supported formats: png, jpg, jpeg, gif, webp",
                format
            ),
            Self::InputImageReadFailed(msg) => {
                write!(f, "Failed to read input image: {}", msg)
            }
            Self::AccessDenied(path) => write!(f, "Access denied by .rooignore: {}", path),
            Self::ApiKeyRequired(msg) => write!(f, "{}", msg),
            Self::ProviderError(msg) => write!(f, "Image generation failed: {}", msg),
            Self::NoImageData => write!(f, "No image data received"),
            Self::InvalidImageFormat => write!(f, "Invalid image format received"),
            Self::SaveFailed(msg) => write!(f, "Failed to save image: {}", msg),
        }
    }
}

impl std::error::Error for ImageGenerationError {}

// ---------------------------------------------------------------------------
// ImageProviderResponse
// ---------------------------------------------------------------------------

/// Response from an image generation provider.
///
/// Corresponds to the result returned by `RooHandler.generateImage` or
/// `OpenRouterHandler.generateImage` in the TS source.
#[derive(Debug, Clone)]
pub struct ImageProviderResponse {
    /// Whether the generation was successful.
    pub success: bool,
    /// The generated image data as a base64 data URI.
    pub image_data: Option<String>,
    /// Error message if generation failed.
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// ImageGenerationProvider trait
// ---------------------------------------------------------------------------

/// Trait for image generation providers.
///
/// Implementations call the actual API (OpenRouter, Roo Cloud, etc.).
/// This mirrors the TS pattern where `RooHandler` and `OpenRouterHandler`
/// each have a `generateImage` method.
pub trait ImageGenerationProvider: Send + Sync {
    /// Generate an image from a prompt.
    ///
    /// # Arguments
    /// * `prompt` - The text prompt describing what to generate.
    /// * `model` - The model ID to use for generation.
    /// * `input_image_data` - Optional base64 data URI of an input image.
    /// * `api_key` - Optional API key for the provider.
    fn generate_image(
        &self,
        prompt: &str,
        model: &str,
        input_image_data: Option<&str>,
        api_key: Option<&str>,
    ) -> Result<ImageProviderResponse, ImageGenerationError>;
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Supported input image formats.
pub const SUPPORTED_IMAGE_FORMATS: &[&str] = &["png", "jpg", "jpeg", "gif", "webp"];

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

/// Validate generate_image parameters.
///
/// Matches TS `GenerateImageTool.execute` validation:
/// 1. `prompt` must be non-empty.
/// 2. `path` must be non-empty.
pub fn validate_generate_image_params(params: &GenerateImageParams) -> Result<(), ImageGenerationError> {
    if params.prompt.is_empty() {
        return Err(ImageGenerationError::MissingParam("prompt".to_string()));
    }

    if params.path.is_empty() {
        return Err(ImageGenerationError::MissingParam("path".to_string()));
    }

    Ok(())
}

/// Validate that an image file extension is supported.
///
/// Matches TS:
/// ```ts
/// const supportedFormats = ["png", "jpg", "jpeg", "gif", "webp"]
/// if (!supportedFormats.includes(imageExtension)) { ... }
/// ```
pub fn validate_image_format(extension: &str) -> Result<(), ImageGenerationError> {
    let ext_lower = extension.to_lowercase();
    if SUPPORTED_IMAGE_FORMATS.contains(&ext_lower.as_str()) {
        Ok(())
    } else {
        Err(ImageGenerationError::UnsupportedFormat(ext_lower))
    }
}

// ---------------------------------------------------------------------------
// Input image handling
// ---------------------------------------------------------------------------

/// Read an input image file and convert it to a base64 data URI.
///
/// Matches TS:
/// ```ts
/// const imageBuffer = await fs.readFile(inputImageFullPath)
/// const imageExtension = path.extname(inputImageFullPath).toLowerCase().replace(".", "")
/// const mimeType = imageExtension === "jpg" ? "jpeg" : imageExtension
/// inputImageData = `data:image/${mimeType};base64,${imageBuffer.toString("base64")}`
/// ```
pub fn encode_image_to_data_uri(image_data: &[u8], extension: &str) -> String {
    let ext_lower = extension.to_lowercase();
    let mime_type = if ext_lower == "jpg" {
        "jpeg"
    } else {
        &ext_lower
    };
    let base64 = base64_encode(image_data);
    format!("data:image/{};base64,{}", mime_type, base64)
}

/// Simple base64 encoding without external dependency.
fn base64_encode(data: &[u8]) -> String {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();

    let chunks = data.chunks(3);
    for chunk in chunks {
        let b0 = chunk[0] as u32;
        let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };

        let triple = (b0 << 16) | (b1 << 8) | b2;

        result.push(CHARSET[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARSET[((triple >> 12) & 0x3F) as usize] as char);

        if chunk.len() > 1 {
            result.push(CHARSET[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(CHARSET[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }

    result
}

// ---------------------------------------------------------------------------
// Output image handling
// ---------------------------------------------------------------------------

/// Parse a base64 data URI to extract the image format and raw data.
///
/// Matches TS:
/// ```ts
/// const base64Match = result.imageData.match(/^data:image\/(png|jpeg|jpg);base64,(.+)$/)
/// ```
pub fn parse_image_data_uri(data_uri: &str) -> Option<(String, Vec<u8>)> {
    let re = regex::Regex::new(r"^data:image/(png|jpeg|jpg);base64,(.+)$").ok()?;
    let caps = re.captures(data_uri)?;
    let format = caps.get(1)?.as_str().to_string();
    let base64_data = caps.get(2)?.as_str();
    let decoded = base64_decode(base64_data)?;
    Some((format, decoded))
}

/// Simple base64 decoding.
fn base64_decode(input: &str) -> Option<Vec<u8>> {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = Vec::new();
    let input = input.trim_end_matches('=');
    let bytes: Vec<u8> = input
        .bytes()
        .filter_map(|b| {
            CHARSET.iter().position(|&c| c == b).map(|pos| pos as u8)
        })
        .collect();

    for chunk in bytes.chunks(4) {
        if chunk.len() < 2 {
            break;
        }
        let b0 = chunk[0] as u32;
        let b1 = chunk[1] as u32;
        let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
        let b3 = if chunk.len() > 3 { chunk[3] as u32 } else { 0 };

        let triple = (b0 << 18) | (b1 << 12) | (b2 << 6) | b3;

        result.push(((triple >> 16) & 0xFF) as u8);
        if chunk.len() > 2 {
            result.push(((triple >> 8) & 0xFF) as u8);
        }
        if chunk.len() > 3 {
            result.push((triple & 0xFF) as u8);
        }
    }

    Some(result)
}

/// Determine the final output path, adding an extension if needed.
///
/// Matches TS:
/// ```ts
/// let finalPath = relPath
/// if (!finalPath.match(/\.(png|jpg|jpeg)$/i)) {
///     finalPath = `${finalPath}.${imageFormat === "jpeg" ? "jpg" : imageFormat}`
/// }
/// ```
pub fn determine_final_path(rel_path: &str, image_format: &str) -> String {
    let re = regex::Regex::new(r"(?i)\.(png|jpg|jpeg)$");
    match re {
        Ok(regex) if regex.is_match(rel_path) => rel_path.to_string(),
        _ => {
            let ext = if image_format == "jpeg" {
                "jpg"
            } else {
                image_format
            };
            format!("{}.{}", rel_path, ext)
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- validate_generate_image_params tests ----

    #[test]
    fn test_validate_valid_params() {
        let params = GenerateImageParams {
            prompt: "A sunset".to_string(),
            path: "images/sunset.png".to_string(),
            image: None,
        };
        assert!(validate_generate_image_params(&params).is_ok());
    }

    #[test]
    fn test_validate_missing_prompt() {
        let params = GenerateImageParams {
            prompt: String::new(),
            path: "images/sunset.png".to_string(),
            image: None,
        };
        let err = validate_generate_image_params(&params).unwrap_err();
        assert!(matches!(err, ImageGenerationError::MissingParam(p) if p == "prompt"));
    }

    #[test]
    fn test_validate_missing_path() {
        let params = GenerateImageParams {
            prompt: "A sunset".to_string(),
            path: String::new(),
            image: None,
        };
        let err = validate_generate_image_params(&params).unwrap_err();
        assert!(matches!(err, ImageGenerationError::MissingParam(p) if p == "path"));
    }

    #[test]
    fn test_validate_with_input_image() {
        let params = GenerateImageParams {
            prompt: "Edit this".to_string(),
            path: "images/output.png".to_string(),
            image: Some("images/input.jpg".to_string()),
        };
        assert!(validate_generate_image_params(&params).is_ok());
    }

    // ---- validate_image_format tests ----

    #[test]
    fn test_supported_format_png() {
        assert!(validate_image_format("png").is_ok());
    }

    #[test]
    fn test_supported_format_jpg() {
        assert!(validate_image_format("jpg").is_ok());
    }

    #[test]
    fn test_supported_format_jpeg() {
        assert!(validate_image_format("jpeg").is_ok());
    }

    #[test]
    fn test_supported_format_gif() {
        assert!(validate_image_format("gif").is_ok());
    }

    #[test]
    fn test_supported_format_webp() {
        assert!(validate_image_format("webp").is_ok());
    }

    #[test]
    fn test_unsupported_format_bmp() {
        let err = validate_image_format("bmp").unwrap_err();
        assert!(matches!(err, ImageGenerationError::UnsupportedFormat(f) if f == "bmp"));
    }

    #[test]
    fn test_unsupported_format_tiff() {
        let err = validate_image_format("tiff").unwrap_err();
        assert!(matches!(err, ImageGenerationError::UnsupportedFormat(f) if f == "tiff"));
    }

    // ---- encode_image_to_data_uri tests ----

    #[test]
    fn test_encode_png() {
        let data = b"fake png data";
        let uri = encode_image_to_data_uri(data, "png");
        assert!(uri.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn test_encode_jpg_becomes_jpeg() {
        let data = b"fake jpg data";
        let uri = encode_image_to_data_uri(data, "jpg");
        assert!(uri.starts_with("data:image/jpeg;base64,"));
    }

    #[test]
    fn test_encode_jpeg_stays_jpeg() {
        let data = b"fake jpeg data";
        let uri = encode_image_to_data_uri(data, "jpeg");
        assert!(uri.starts_with("data:image/jpeg;base64,"));
    }

    #[test]
    fn test_encode_webp() {
        let data = b"fake webp data";
        let uri = encode_image_to_data_uri(data, "webp");
        assert!(uri.starts_with("data:image/webp;base64,"));
    }

    // ---- parse_image_data_uri tests ----

    #[test]
    fn test_parse_valid_png_uri() {
        let data = base64_encode(b"test");
        let uri = format!("data:image/png;base64,{}", data);
        let result = parse_image_data_uri(&uri);
        assert!(result.is_some());
        let (format, decoded) = result.unwrap();
        assert_eq!(format, "png");
        assert_eq!(decoded, b"test");
    }

    #[test]
    fn test_parse_valid_jpeg_uri() {
        let data = base64_encode(b"jpeg content");
        let uri = format!("data:image/jpeg;base64,{}", data);
        let result = parse_image_data_uri(&uri);
        assert!(result.is_some());
        let (format, _) = result.unwrap();
        assert_eq!(format, "jpeg");
    }

    #[test]
    fn test_parse_invalid_uri() {
        let result = parse_image_data_uri("not a data uri");
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_invalid_format() {
        let data = base64_encode(b"test");
        let uri = format!("data:image/bmp;base64,{}", data);
        let result = parse_image_data_uri(&uri);
        assert!(result.is_none());
    }

    // ---- determine_final_path tests ----

    #[test]
    fn test_path_with_png_extension() {
        let result = determine_final_path("images/test.png", "png");
        assert_eq!(result, "images/test.png");
    }

    #[test]
    fn test_path_with_jpg_extension() {
        let result = determine_final_path("images/test.jpg", "jpeg");
        assert_eq!(result, "images/test.jpg");
    }

    #[test]
    fn test_path_with_jpeg_extension() {
        let result = determine_final_path("images/test.jpeg", "jpeg");
        assert_eq!(result, "images/test.jpeg");
    }

    #[test]
    fn test_path_without_extension_png() {
        let result = determine_final_path("images/test", "png");
        assert_eq!(result, "images/test.png");
    }

    #[test]
    fn test_path_without_extension_jpeg() {
        let result = determine_final_path("images/test", "jpeg");
        assert_eq!(result, "images/test.jpg");
    }

    #[test]
    fn test_path_with_uppercase_extension() {
        let result = determine_final_path("images/test.PNG", "png");
        assert_eq!(result, "images/test.PNG");
    }

    // ---- base64 encode/decode roundtrip tests ----

    #[test]
    fn test_base64_roundtrip_hello() {
        let original = b"Hello, World!";
        let encoded = base64_encode(original);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_base64_roundtrip_empty() {
        let original: &[u8] = b"";
        let encoded = base64_encode(original);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_base64_roundtrip_binary() {
        let original: &[u8] = &[0, 1, 2, 255, 254, 253];
        let encoded = base64_encode(original);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_base64_roundtrip_single_byte() {
        let original: &[u8] = b"A";
        let encoded = base64_encode(original);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_base64_roundtrip_two_bytes() {
        let original: &[u8] = b"AB";
        let encoded = base64_encode(original);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, original);
    }

    // ---- ImageGenerationError Display tests ----

    #[test]
    fn test_error_display_feature_disabled() {
        let err = ImageGenerationError::FeatureDisabled;
        let msg = format!("{}", err);
        assert!(msg.contains("experimental feature"));
    }

    #[test]
    fn test_error_display_unsupported_format() {
        let err = ImageGenerationError::UnsupportedFormat("bmp".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("bmp"));
        assert!(msg.contains("png"));
    }

    #[test]
    fn test_error_display_provider_error() {
        let err = ImageGenerationError::ProviderError("API timeout".to_string());
        let msg = format!("{}", err);
        assert!(msg.contains("API timeout"));
    }

    // ---- ImageProviderResponse tests ----

    #[test]
    fn test_provider_response_success() {
        let response = ImageProviderResponse {
            success: true,
            image_data: Some("data:image/png;base64,abc".to_string()),
            error: None,
        };
        assert!(response.success);
        assert!(response.image_data.is_some());
        assert!(response.error.is_none());
    }

    #[test]
    fn test_provider_response_failure() {
        let response = ImageProviderResponse {
            success: false,
            image_data: None,
            error: Some("Rate limit exceeded".to_string()),
        };
        assert!(!response.success);
        assert!(response.image_data.is_none());
        assert!(response.error.is_some());
    }
}

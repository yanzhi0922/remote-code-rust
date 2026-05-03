//! Image file processing for the read_file tool.
//!
//! Handles reading image files (png, jpg, jpeg, gif, webp, svg, bmp, ico,
//! tiff, tif, avif) and encoding them as base64 data URLs for multimodal LLM
//! consumption. This mirrors the TS implementation in
//! `src/core/tools/helpers/imageHelpers.ts`.

use std::path::Path;

use crate::types::FsToolError;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default maximum allowed image file size in bytes (5 MB).
///
/// Source: `imageHelpers.ts` — `DEFAULT_MAX_IMAGE_FILE_SIZE_MB`
pub const DEFAULT_MAX_IMAGE_FILE_SIZE_MB: f64 = 5.0;

/// Default maximum total memory usage for all images in a single read
/// operation (20 MB).
///
/// Source: `imageHelpers.ts` — `DEFAULT_MAX_TOTAL_IMAGE_SIZE_MB`
pub const DEFAULT_MAX_TOTAL_IMAGE_SIZE_MB: f64 = 20.0;

// ---------------------------------------------------------------------------
// Supported formats
// ---------------------------------------------------------------------------

/// Supported image file extensions (lowercase, with leading dot).
const SUPPORTED_IMAGE_EXTENSIONS: &[&str] = &[
    ".png",
    ".jpg",
    ".jpeg",
    ".gif",
    ".webp",
    ".svg",
    ".bmp",
    ".ico",
    ".tiff",
    ".tif",
    ".avif",
];

/// Map from file extension to MIME type.
fn mime_type_for_ext(ext: &str) -> &'static str {
    match ext {
        ".png" => "image/png",
        ".jpg" | ".jpeg" => "image/jpeg",
        ".gif" => "image/gif",
        ".webp" => "image/webp",
        ".svg" => "image/svg+xml",
        ".bmp" => "image/bmp",
        ".ico" => "image/x-icon",
        ".tiff" | ".tif" => "image/tiff",
        ".avif" => "image/avif",
        _ => "application/octet-stream",
    }
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// The result of processing an image file.
#[derive(Debug, Clone)]
pub struct ImageContent {
    /// The complete data URL, e.g. `data:image/png;base64,iVBOR...`.
    pub data_url: String,
    /// The MIME type, e.g. `image/png`.
    pub media_type: String,
    /// The original file size in bytes.
    pub size_bytes: u64,
}

/// Why an image was skipped / could not be processed.
#[derive(Debug, Clone)]
pub enum ImageSkipReason {
    /// The file exceeds the per-image size limit.
    SizeLimit {
        size_bytes: u64,
        max_mb: f64,
    },
    /// Adding this image would exceed the cumulative memory budget.
    MemoryLimit {
        size_bytes: u64,
        current_total_mb: f64,
        max_total_mb: f64,
    },
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Check whether the given file path has a supported image extension.
///
/// The check is case-insensitive.
pub fn is_image_file(path: &Path) -> bool {
    let ext = match path.extension().and_then(|e| e.to_str()) {
        Some(e) => e.to_lowercase(),
        None => return false,
    };
    let ext_with_dot = format!(".{}", ext);
    SUPPORTED_IMAGE_EXTENSIONS.contains(&ext_with_dot.as_str())
}

/// Read an image file and encode it as a base64 data URL.
///
/// # Errors
///
/// Returns `FsToolError::Io` if the file cannot be read.
/// Returns `FsToolError::Validation` if the file exceeds the maximum allowed
/// image size or would exceed the cumulative memory budget.
pub fn process_image_file(
    path: &Path,
    max_image_size_mb: f64,
    current_total_mb: f64,
    max_total_mb: f64,
) -> Result<Result<ImageContent, ImageSkipReason>, FsToolError> {
    let metadata = std::fs::metadata(path)?;
    let size_bytes = metadata.len();
    let size_mb = size_bytes as f64 / (1024.0 * 1024.0);

    // Check per-image size limit
    if size_mb > max_image_size_mb {
        return Ok(Err(ImageSkipReason::SizeLimit {
            size_bytes,
            max_mb: max_image_size_mb,
        }));
    }

    // Check cumulative memory limit
    if current_total_mb + size_mb > max_total_mb {
        return Ok(Err(ImageSkipReason::MemoryLimit {
            size_bytes,
            current_total_mb,
            max_total_mb,
        }));
    }

    let raw = std::fs::read(path)?;

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e.to_lowercase()))
        .unwrap_or_default();

    let media_type = mime_type_for_ext(&ext);
    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &raw);
    let data_url = format!("data:{};base64,{}", media_type, b64);

    Ok(Ok(ImageContent {
        data_url,
        media_type: media_type.to_string(),
        size_bytes,
    }))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_is_image_file_png() {
        assert!(is_image_file(Path::new("photo.png")));
        assert!(is_image_file(Path::new("photo.PNG")));
        assert!(is_image_file(Path::new("/some/path/photo.png")));
    }

    #[test]
    fn test_is_image_file_jpg() {
        assert!(is_image_file(Path::new("photo.jpg")));
        assert!(is_image_file(Path::new("photo.jpeg")));
        assert!(is_image_file(Path::new("photo.JPEG")));
    }

    #[test]
    fn test_is_image_file_various() {
        assert!(is_image_file(Path::new("a.gif")));
        assert!(is_image_file(Path::new("a.webp")));
        assert!(is_image_file(Path::new("a.svg")));
        assert!(is_image_file(Path::new("a.bmp")));
        assert!(is_image_file(Path::new("a.ico")));
        assert!(is_image_file(Path::new("a.tiff")));
        assert!(is_image_file(Path::new("a.tif")));
        assert!(is_image_file(Path::new("a.avif")));
    }

    #[test]
    fn test_is_image_file_not_image() {
        assert!(!is_image_file(Path::new("readme.md")));
        assert!(!is_image_file(Path::new("code.rs")));
        assert!(!is_image_file(Path::new("data.bin")));
        assert!(!is_image_file(Path::new("noext")));
    }

    #[test]
    fn test_mime_type_for_ext() {
        assert_eq!(mime_type_for_ext(".png"), "image/png");
        assert_eq!(mime_type_for_ext(".jpg"), "image/jpeg");
        assert_eq!(mime_type_for_ext(".jpeg"), "image/jpeg");
        assert_eq!(mime_type_for_ext(".gif"), "image/gif");
        assert_eq!(mime_type_for_ext(".webp"), "image/webp");
        assert_eq!(mime_type_for_ext(".svg"), "image/svg+xml");
        assert_eq!(mime_type_for_ext(".bmp"), "image/bmp");
        assert_eq!(mime_type_for_ext(".ico"), "image/x-icon");
        assert_eq!(mime_type_for_ext(".tiff"), "image/tiff");
        assert_eq!(mime_type_for_ext(".tif"), "image/tiff");
        assert_eq!(mime_type_for_ext(".avif"), "image/avif");
    }

    #[test]
    fn test_process_image_file_small() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test.png");

        // Minimal valid-ish PNG (1x1 white pixel)
        let png_bytes: &[u8] = &[
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
            0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01,
            0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
            0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41,
            0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00,
            0x00, 0x00, 0x02, 0x00, 0x01, 0xE2, 0x21, 0xBC,
            0x33, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
            0x44, 0xAE, 0x42, 0x60, 0x82,
        ];

        let mut f = std::fs::File::create(&file_path).unwrap();
        f.write_all(png_bytes).unwrap();
        drop(f);

        let result = process_image_file(&file_path, 5.0, 0.0, 20.0)
            .unwrap()
            .unwrap();

        assert!(result.data_url.starts_with("data:image/png;base64,"));
        assert_eq!(result.media_type, "image/png");
        assert_eq!(result.size_bytes, png_bytes.len() as u64);
    }

    #[test]
    fn test_process_image_file_size_limit() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("big.png");

        // Create a 100-byte file with null bytes (looks binary)
        let mut f = std::fs::File::create(&file_path).unwrap();
        f.write_all(b"hello\x00world\x00this\x00is\x00binary\x00data\x00padding\x00padding\x00more\x00pad\x00end")
            .unwrap();
        drop(f);

        // Set max image size to 0.00001 MB (~10 bytes), which the file exceeds
        let result = process_image_file(&file_path, 0.00001, 0.0, 20.0).unwrap();

        assert!(result.is_err());
        if let Err(ImageSkipReason::SizeLimit { size_bytes, max_mb }) = result {
            assert!(size_bytes > 0);
            assert_eq!(max_mb, 0.00001);
        } else {
            panic!("Expected SizeLimit");
        }
    }

    #[test]
    fn test_process_image_file_memory_limit() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("img.png");

        let mut f = std::fs::File::create(&file_path).unwrap();
        f.write_all(b"\x89PNG\r\n\x1a\nhello").unwrap();
        drop(f);

        // Current total already near the max — set max_total so file pushes it over
        let result = process_image_file(&file_path, 5.0, 0.00001, 0.00001).unwrap();

        assert!(result.is_err());
        if let Err(ImageSkipReason::MemoryLimit { .. }) = result {
            // expected
        } else {
            panic!("Expected MemoryLimit");
        }
    }

    #[test]
    fn test_process_image_file_not_found() {
        let result = process_image_file(Path::new("/nonexistent/file.png"), 5.0, 0.0, 20.0);
        assert!(result.is_err());
    }

    #[test]
    fn test_process_image_file_jpg() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("photo.jpg");

        // Minimal JPEG (SOI + EOI markers)
        let jpeg_bytes: &[u8] = &[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x02, 0x00, 0x00, 0xFF, 0xD9];

        let mut f = std::fs::File::create(&file_path).unwrap();
        f.write_all(jpeg_bytes).unwrap();
        drop(f);

        let result = process_image_file(&file_path, 5.0, 0.0, 20.0)
            .unwrap()
            .unwrap();

        assert!(result.data_url.starts_with("data:image/jpeg;base64,"));
        assert_eq!(result.media_type, "image/jpeg");
    }

    #[test]
    fn test_process_image_file_svg() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("icon.svg");

        // SVG is text, but we treat it as an image format
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="1" height="1"><rect fill="red"/></svg>"#;

        let mut f = std::fs::File::create(&file_path).unwrap();
        f.write_all(svg.as_bytes()).unwrap();
        drop(f);

        let result = process_image_file(&file_path, 5.0, 0.0, 20.0)
            .unwrap()
            .unwrap();

        assert!(result.data_url.starts_with("data:image/svg+xml;base64,"));
        assert_eq!(result.media_type, "image/svg+xml");
    }
}

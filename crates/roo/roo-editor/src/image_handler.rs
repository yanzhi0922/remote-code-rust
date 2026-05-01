//! Image Handler
//!
//! Handles opening, saving, and processing images from data URIs and file paths.
//! Mirrors `image-handler.ts`.

use std::path::{Path, PathBuf};

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors during image handling.
#[derive(Debug, thiserror::Error)]
pub enum ImageHandlerError {
    #[error("Invalid data URI")]
    InvalidDataUri,

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Base64 decode error: {0}")]
    Base64Error(#[from] base64::DecodeError),
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Options for image operations.
#[derive(Debug, Clone)]
pub struct ImageOptions {
    pub action: Option<String>,
}

/// Result of parsing a data URI.
#[derive(Debug, Clone)]
pub struct ParsedDataUri {
    pub format: String,
    pub data: Vec<u8>,
}

// ---------------------------------------------------------------------------
// Image handler functions
// ---------------------------------------------------------------------------

/// Check if a string is a file path (vs data URI or URL).
///
/// Source: `image-handler.ts` — `openImage`
pub fn is_file_path(input: &str) -> bool {
    !input.starts_with("data:")
        && !input.starts_with("http:")
        && !input.starts_with("https:")
        && !input.starts_with("vscode-resource:")
        && !input.starts_with("file+.vscode-resource")
}

/// Parse a data URI into format and binary data.
///
/// Source: `image-handler.ts` — data URI parsing
pub fn parse_data_uri(data_uri: &str) -> Result<ParsedDataUri, ImageHandlerError> {
    let re = regex_lite::Regex::new(r"^data:image/([a-zA-Z]+);base64,(.+)$").unwrap();

    if let Some(caps) = re.captures(data_uri) {
        let format = caps.get(1).unwrap().as_str().to_string();
        let base64_data = caps.get(2).unwrap().as_str();
        let data = BASE64.decode(base64_data)?;

        Ok(ParsedDataUri { format, data })
    } else {
        Err(ImageHandlerError::InvalidDataUri)
    }
}

/// Save image data to a temporary file and return the path.
///
/// Source: `image-handler.ts` — temp file creation
pub async fn save_image_to_temp(
    format: &str,
    data: &[u8],
    temp_dir: &Path,
) -> Result<PathBuf, ImageHandlerError> {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    let temp_path = temp_dir.join(format!("temp_image_{}.{}", timestamp, format));
    tokio::fs::write(&temp_path, data).await?;
    Ok(temp_path)
}

/// Save image data to a specified file path.
pub async fn save_image_to_file(
    file_path: &Path,
    data: &[u8],
) -> Result<(), ImageHandlerError> {
    if let Some(parent) = file_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(file_path, data).await?;
    Ok(())
}

/// Convert image data to a data URI string.
pub fn image_to_data_uri(format: &str, data: &[u8]) -> String {
    let base64_data = BASE64.encode(data);
    format!("data:image/{};base64,{}", format, base64_data)
}

/// Resolve a relative file path to an absolute path using a workspace root.
pub fn resolve_image_path(path: &str, workspace_root: Option<&Path>) -> PathBuf {
    if Path::new(path).is_absolute() {
        PathBuf::from(path)
    } else if let Some(root) = workspace_root {
        root.join(path)
    } else {
        PathBuf::from(path)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_file_path_true() {
        assert!(is_file_path("/absolute/path.png"));
        assert!(is_file_path("relative/path.png"));
        assert!(is_file_path("image.png"));
    }

    #[test]
    fn test_is_file_path_false() {
        assert!(!is_file_path("data:image/png;base64,abc"));
        assert!(!is_file_path("http://example.com/image.png"));
        assert!(!is_file_path("https://example.com/image.png"));
    }

    #[test]
    fn test_parse_data_uri_valid() {
        let data_uri = "data:image/png;base64,aGVsbG8=";
        let parsed = parse_data_uri(data_uri).unwrap();
        assert_eq!(parsed.format, "png");
        assert_eq!(parsed.data, b"hello");
    }

    #[test]
    fn test_parse_data_uri_invalid() {
        assert!(parse_data_uri("not-a-data-uri").is_err());
        assert!(parse_data_uri("data:text/plain;base64,abc").is_err());
    }

    #[test]
    fn test_image_to_data_uri() {
        let data = b"hello";
        let uri = image_to_data_uri("png", data);
        assert!(uri.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn test_resolve_image_path_absolute() {
        let path = resolve_image_path("/absolute/path.png", None);
        assert_eq!(path, PathBuf::from("/absolute/path.png"));
    }

    #[test]
    fn test_resolve_image_path_relative() {
        let path = resolve_image_path("relative.png", Some(Path::new("/workspace")));
        assert_eq!(path, PathBuf::from("/workspace/relative.png"));
    }

    #[tokio::test]
    async fn test_save_image_to_file() {
        let tmp = tempfile::tempdir().unwrap();
        let file_path = tmp.path().join("test.png");
        save_image_to_file(&file_path, b"test data").await.unwrap();
        assert!(file_path.exists());
        let content = tokio::fs::read(&file_path).await.unwrap();
        assert_eq!(content, b"test data");
    }
}

/// Text extraction utilities for various file formats.
/// Mirrors src/integrations/misc/extract-text.ts

use std::path::Path;

/// Extract text content from a file based on its extension.
/// Returns the extracted text or an error message.
pub fn extract_text(file_path: &Path, max_bytes: Option<usize>) -> std::io::Result<String> {
    let metadata = std::fs::metadata(file_path)?;

    // Check file size if max_bytes is specified
    if let Some(max) = max_bytes {
        if metadata.len() as usize > max {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("File too large: {} bytes (max: {})", metadata.len(), max),
            ));
        }
    }

    let extension = file_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    match extension.as_str() {
        "txt" | "md" | "markdown" | "rst" | "csv" | "tsv" | "log" | "json" | "jsonl"
        | "yaml" | "yml" | "toml" | "xml" | "html" | "htm" | "css" | "js" | "jsx" | "ts"
        | "tsx" | "py" | "rb" | "rs" | "go" | "java" | "c" | "cpp" | "h" | "hpp" | "cs"
        | "swift" | "kt" | "scala" | "sh" | "bash" | "zsh" | "fish" | "ps1" | "bat" | "cmd"
        | "sql" | "graphql" | "proto" | "dockerfile" | "makefile" | "cmake" | "gradle"
        | "properties" | "ini" | "cfg" | "conf" | "env" | "gitignore" | "dockerignore"
        | "lock" => {
            // Plain text files - just read them
            std::fs::read_to_string(file_path)
        }
        "pdf" => {
            // PDF - return a placeholder (full PDF extraction requires external crate)
            Ok(format!("[PDF file: {} bytes]", metadata.len()))
        }
        "doc" | "docx" => {
            Ok(format!("[Word document: {} bytes]", metadata.len()))
        }
        "xls" | "xlsx" => {
            Ok(format!("[Excel spreadsheet: {} bytes]", metadata.len()))
        }
        "ppt" | "pptx" => {
            Ok(format!("[PowerPoint: {} bytes]", metadata.len()))
        }
        "zip" | "tar" | "gz" | "bz2" | "xz" | "7z" | "rar" => {
            Ok(format!("[Archive: {} bytes]", metadata.len()))
        }
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "svg" | "webp" | "ico" => {
            Ok(format!("[Image: {} bytes]", metadata.len()))
        }
        "mp3" | "wav" | "ogg" | "flac" | "aac" | "m4a" => {
            Ok(format!("[Audio: {} bytes]", metadata.len()))
        }
        "mp4" | "avi" | "mkv" | "mov" | "wmv" | "webm" => {
            Ok(format!("[Video: {} bytes]", metadata.len()))
        }
        _ => {
            // Try to read as text for unknown extensions
            match std::fs::read_to_string(file_path) {
                Ok(text) => Ok(text),
                Err(_) => Ok(format!(
                    "[Binary file: {} bytes, extension: {}]",
                    metadata.len(),
                    extension
                )),
            }
        }
    }
}

/// Check if a file extension is supported for text extraction.
pub fn is_supported_extension(extension: &str) -> bool {
    let text_extensions = [
        "txt", "md", "markdown", "rst", "csv", "tsv", "log", "json", "jsonl", "yaml", "yml",
        "toml", "xml", "html", "htm", "css", "js", "jsx", "ts", "tsx", "py", "rb", "rs", "go",
        "java", "c", "cpp", "h", "hpp", "cs", "swift", "kt", "scala", "sh", "bash", "zsh",
        "fish", "ps1", "bat", "cmd", "sql", "graphql", "proto",
    ];
    text_extensions.contains(&extension.to_lowercase().as_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_file(content: &[u8]) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content).unwrap();
        file
    }

    fn create_test_file_with_ext(extension: &str, content: &[u8]) -> NamedTempFile {
        let suffix = format!(".{}", extension);
        let mut builder = tempfile::Builder::new();
        builder.suffix(&suffix);
        let mut file = builder.tempfile().unwrap();
        file.write_all(content).unwrap();
        file
    }

    #[test]
    fn test_extract_text_file() {
        let file = create_test_file(b"hello world");
        let result = extract_text(file.path(), None).unwrap();
        assert_eq!("hello world", result);
    }

    #[test]
    fn test_extract_text_with_max_bytes() {
        let file = create_test_file(b"hello world");
        let result = extract_text(file.path(), Some(5));
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_text_json() {
        let file = create_test_file_with_ext("json", b"{\"key\": \"value\"}");
        let result = extract_text(file.path(), None).unwrap();
        assert!(result.contains("key"));
    }

    #[test]
    fn test_extract_text_pdf() {
        let file = create_test_file_with_ext("pdf", b"%PDF-1.4 fake content");
        let result = extract_text(file.path(), None).unwrap();
        assert!(result.contains("[PDF file:"));
    }

    #[test]
    fn test_extract_text_image() {
        let file = create_test_file_with_ext("png", &[0x89, 0x50, 0x4E, 0x47]);
        let result = extract_text(file.path(), None).unwrap();
        assert!(result.contains("[Image:"));
    }

    #[test]
    fn test_is_supported_extension() {
        assert!(is_supported_extension("txt"));
        assert!(is_supported_extension("json"));
        assert!(is_supported_extension("rs"));
        assert!(is_supported_extension("py"));
        assert!(!is_supported_extension("exe"));
        assert!(!is_supported_extension("bin"));
    }
}
